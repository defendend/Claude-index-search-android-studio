//! Tree-sitter based Ruby parser

use anyhow::Result;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

static RUBY_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_ruby::LANGUAGE.into());

static RUBY_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&RUBY_LANGUAGE, include_str!("queries/ruby.scm"))
        .expect("Failed to compile Ruby tree-sitter query")
});

pub static RUBY_PARSER: RubyParser = RubyParser;

pub struct RubyParser;

impl LanguageParser for RubyParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &RUBY_LANGUAGE)?;
        let mut symbols = Vec::new();
        let query = &*RUBY_QUERY;
        let mut cursor = QueryCursor::new();

        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names.iter().position(|n| *n == name).map(|i| i as u32)
        };

        let idx_class_name = idx("class_name");
        let idx_class_parent = idx("class_parent");
        let idx_module_name = idx("module_name");
        let idx_method_name = idx("method_name");
        let idx_singleton_object = idx("singleton_object");
        let idx_singleton_method_name = idx("singleton_method_name");
        let idx_assign_const_name = idx("assign_const_name");
        let idx_call_method = idx("call_method");
        let idx_call_first_arg = idx("call_first_arg");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // Class definition
            if let Some(name_cap) = find_capture(m, idx_class_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                let parents = find_capture(m, idx_class_parent)
                    .map(|p| vec![(node_text(content, &p.node).to_string(), "extends".to_string())])
                    .unwrap_or_default();
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Module definition
            if let Some(cap) = find_capture(m, idx_module_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Package,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Singleton method: def self.method_name
            if let Some(obj_cap) = find_capture(m, idx_singleton_object) {
                if let Some(name_cap) = find_capture(m, idx_singleton_method_name) {
                    let obj = node_text(content, &obj_cap.node);
                    let method_name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    symbols.push(ParsedSymbol {
                        name: format!("{}.{}", obj, method_name),
                        kind: SymbolKind::Function,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // Instance method: def method_name
            if let Some(cap) = find_capture(m, idx_method_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Constant assignment: CONST_NAME = value
            if let Some(cap) = find_capture(m, idx_assign_const_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                if is_constant_name(name) {
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Constant,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // Call expressions (DSL patterns)
            if let Some(method_cap) = find_capture(m, idx_call_method) {
                let method = node_text(content, &method_cap.node);
                let line = node_line(&method_cap.node);
                let first_arg = find_capture(m, idx_call_first_arg)
                    .map(|c| node_text(content, &c.node));

                // Skip calls with a receiver (e.g., Foo.bar, obj.method)
                // We only want bare calls like `require 'json'`, `include Mod`, etc.
                let call_node = method_cap.node.parent();
                let has_receiver = call_node
                    .map(|n| n.child_by_field_name("receiver").is_some())
                    .unwrap_or(false);

                match method {
                    // require / require_relative
                    "require" | "require_relative" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let path = arg.trim_matches(|c| c == '\'' || c == '"');
                            symbols.push(ParsedSymbol {
                                name: path.to_string(),
                                kind: SymbolKind::Import,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // include / extend / prepend
                    "include" | "extend" | "prepend" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            symbols.push(ParsedSymbol {
                                name: format!("{} {}", method, arg),
                                kind: SymbolKind::Import,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // attr_reader / attr_writer / attr_accessor
                    "attr_reader" | "attr_writer" | "attr_accessor" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!(":{}", sym_name),
                                kind: SymbolKind::Property,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // Rails associations: has_many, has_one, belongs_to, has_and_belongs_to_many
                    "has_many" | "has_one" | "belongs_to" | "has_and_belongs_to_many"
                        if !has_receiver =>
                    {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!("{} :{}", method, sym_name),
                                kind: SymbolKind::Property,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // Rails validates
                    "validates" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!("validates :{}", sym_name),
                                kind: SymbolKind::Annotation,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // Rails callbacks
                    "before_action" | "after_action" | "around_action"
                    | "before_create" | "after_create"
                    | "before_save" | "after_save"
                    | "before_destroy" | "after_destroy"
                    | "before_validation" | "after_validation"
                        if !has_receiver =>
                    {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!("{} :{}", method, sym_name),
                                kind: SymbolKind::Annotation,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // Rails scope
                    "scope" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!("scope :{}", sym_name),
                                kind: SymbolKind::Function,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // RSpec describe / context
                    "describe" | "context" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let desc = arg.trim_matches(|c| c == '\'' || c == '"');
                            symbols.push(ParsedSymbol {
                                name: format!("{} \"{}\"", method, desc),
                                kind: SymbolKind::Class,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // RSpec it / specify
                    "it" | "specify" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let desc = arg.trim_matches(|c| c == '\'' || c == '"');
                            symbols.push(ParsedSymbol {
                                name: format!("{} \"{}\"", method, desc),
                                kind: SymbolKind::Function,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    // RSpec let / let! / subject
                    "let" | "let!" | "subject" if !has_receiver => {
                        if let Some(arg) = first_arg {
                            let sym_name = normalize_symbol(arg);
                            symbols.push(ParsedSymbol {
                                name: format!("{}(:{})", method, sym_name),
                                kind: SymbolKind::Property,
                                line,
                                signature: line_text(content, line).trim().to_string(),
                                parents: vec![],
                            });
                        }
                    }

                    _ => {}
                }
                continue;
            }
        }

        Ok(symbols)
    }
}

/// Check if a name is an ALL_CAPS constant
fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
        && name.chars().any(|c| c.is_uppercase())
}

/// Normalize a Ruby symbol argument: strip leading `:` from `:name`
fn normalize_symbol(s: &str) -> &str {
    s.strip_prefix(':').unwrap_or(s)
}

/// Find a capture by index in a match
fn find_capture<'a>(
    m: &'a tree_sitter::QueryMatch<'a, 'a>,
    idx: Option<u32>,
) -> Option<&'a tree_sitter::QueryCapture<'a>> {
    let idx = idx?;
    m.captures.iter().find(|c| c.index == idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_class() {
        let content = "class User < ApplicationRecord\n  def initialize\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s|
            s.name == "User"
            && s.parents.iter().any(|(p, k)| p == "ApplicationRecord" && k == "extends")
        ));
    }

    #[test]
    fn test_parse_class_no_parent() {
        let content = "class Service\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Service" && s.kind == SymbolKind::Class);
        assert!(cls.is_some());
        assert!(cls.unwrap().parents.is_empty());
    }

    #[test]
    fn test_parse_namespaced_class() {
        let content = "class Admin::Dashboard\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s|
            s.name == "Admin::Dashboard" && s.kind == SymbolKind::Class
        ));
    }

    #[test]
    fn test_parse_module() {
        let content = "module Authenticatable\n  def authenticate\n    true\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Authenticatable" && s.kind == SymbolKind::Package));
    }

    #[test]
    fn test_parse_namespaced_module() {
        let content = "module Admin::Helpers\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s|
            s.name == "Admin::Helpers" && s.kind == SymbolKind::Package
        ));
    }

    #[test]
    fn test_parse_nested_module_class() {
        let content = "module Admin\n  class Dashboard\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Admin" && s.kind == SymbolKind::Package));
        assert!(symbols.iter().any(|s| s.name == "Dashboard" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_parse_instance_method() {
        let content = "class Foo\n  def bar\n    42\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "bar" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_method_with_question_mark() {
        let content = "class Foo\n  def valid?\n    true\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "valid?" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_method_with_bang() {
        let content = "class Foo\n  def save!\n    persist\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "save!" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_class_method() {
        let content = "class Service\n  def self.call(params)\n    new(params).call\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "self.call" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_require() {
        let content = "require 'json'\nrequire 'net/http'\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "json" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "net/http" && s.kind == SymbolKind::Import));
    }

    #[test]
    fn test_parse_require_relative() {
        let content = "require_relative './helpers'\nrequire_relative '../models/user'\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "./helpers" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "../models/user" && s.kind == SymbolKind::Import));
    }

    #[test]
    fn test_parse_include_extend_prepend() {
        let content = "class User\n  include Authenticatable\n  extend ClassMethods\n  prepend Trackable\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "include Authenticatable" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "extend ClassMethods" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "prepend Trackable" && s.kind == SymbolKind::Import));
    }

    #[test]
    fn test_parse_attr_accessor() {
        let content = "class User\n  attr_reader :name\n  attr_writer :email\n  attr_accessor :age\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == ":name" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == ":email" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == ":age" && s.kind == SymbolKind::Property));
    }

    #[test]
    fn test_parse_constants() {
        let content = "class Config\n  LIMIT = 100\n  DEFAULT_ROLE = \"user\"\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "LIMIT" && s.kind == SymbolKind::Constant));
        assert!(symbols.iter().any(|s| s.name == "DEFAULT_ROLE" && s.kind == SymbolKind::Constant));
    }

    #[test]
    fn test_parse_rails_associations() {
        let content = r#"class Post < ApplicationRecord
  belongs_to :author
  has_many :comments
  has_one :featured_image
  has_and_belongs_to_many :tags
end
"#;
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "belongs_to :author" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "has_many :comments" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "has_one :featured_image" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "has_and_belongs_to_many :tags" && s.kind == SymbolKind::Property));
    }

    #[test]
    fn test_parse_rails_validates() {
        let content = "class User < ApplicationRecord\n  validates :name\n  validates :email\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "validates :name" && s.kind == SymbolKind::Annotation));
        assert!(symbols.iter().any(|s| s.name == "validates :email" && s.kind == SymbolKind::Annotation));
    }

    #[test]
    fn test_parse_rails_callbacks() {
        let content = "class Post < ApplicationRecord\n  before_save :normalize_title\n  after_create :notify_subscribers\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "before_save :normalize_title" && s.kind == SymbolKind::Annotation));
        assert!(symbols.iter().any(|s| s.name == "after_create :notify_subscribers" && s.kind == SymbolKind::Annotation));
    }

    #[test]
    fn test_parse_rails_scope() {
        let content = "class Post < ApplicationRecord\n  scope :published, -> { where(published: true) }\n  scope :recent, -> { order(created_at: :desc) }\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "scope :published" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "scope :recent" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_rspec_describe_context() {
        let content = r#"describe "User" do
  context "when valid" do
    it "returns true" do
    end
  end
end
"#;
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s|
            s.name.contains("describe") && s.name.contains("User") && s.kind == SymbolKind::Class
        ));
        assert!(symbols.iter().any(|s|
            s.name.contains("context") && s.name.contains("when valid") && s.kind == SymbolKind::Class
        ));
    }

    #[test]
    fn test_parse_rspec_it_specify() {
        let content = r#"describe "User" do
  it "returns true for valid user" do
    expect(true).to be_truthy
  end

  specify "returns false for invalid" do
    expect(false).to be_falsy
  end
end
"#;
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s|
            s.name.contains("it") && s.name.contains("returns true") && s.kind == SymbolKind::Function
        ));
        assert!(symbols.iter().any(|s|
            s.name.contains("specify") && s.name.contains("returns false") && s.kind == SymbolKind::Function
        ));
    }

    #[test]
    fn test_parse_rspec_let() {
        let content = "describe User do\n  let(:user) { build(:user) }\n  let!(:admin) { create(:admin) }\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s|
            s.name.contains("let") && s.name.contains("user") && s.kind == SymbolKind::Property
        ));
        assert!(symbols.iter().any(|s|
            s.name.contains("let!") && s.name.contains("admin") && s.kind == SymbolKind::Property
        ));
    }

    #[test]
    fn test_parse_full_rails_model() {
        let content = r#"require 'json'

class Post < ApplicationRecord
  include Publishable
  extend Searchable

  attr_accessor :draft_content

  CATEGORIES = %w[tech science art].freeze

  has_many :comments
  belongs_to :author

  validates :title
  validates :content

  scope :published, -> { where(published: true) }

  before_save :normalize_title
  after_create :notify_subscribers

  def initialize(attrs = {})
    super
  end

  def self.find_by_slug(slug)
    where(slug: slug).first
  end

  def publish!
    update(published: true)
  end

  private

  def normalize_title
    self.title = title.strip
  end
end
"#;
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();

        // Imports
        assert!(symbols.iter().any(|s| s.name == "json" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "include Publishable" && s.kind == SymbolKind::Import));
        assert!(symbols.iter().any(|s| s.name == "extend Searchable" && s.kind == SymbolKind::Import));

        // Class
        assert!(symbols.iter().any(|s| s.name == "Post" && s.kind == SymbolKind::Class));

        // Properties
        assert!(symbols.iter().any(|s| s.name == ":draft_content" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "has_many :comments" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "belongs_to :author" && s.kind == SymbolKind::Property));

        // Constants
        assert!(symbols.iter().any(|s| s.name == "CATEGORIES" && s.kind == SymbolKind::Constant));

        // Annotations
        assert!(symbols.iter().any(|s| s.name == "validates :title" && s.kind == SymbolKind::Annotation));
        assert!(symbols.iter().any(|s| s.name == "before_save :normalize_title" && s.kind == SymbolKind::Annotation));

        // Functions
        assert!(symbols.iter().any(|s| s.name == "scope :published" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "initialize" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "self.find_by_slug" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "publish!" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "normalize_title" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_comments_ignored() {
        let content = "# class FakeClass\n# def fake_method\nclass RealClass\n  def real_method\n  end\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "RealClass"));
        assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
        assert!(symbols.iter().any(|s| s.name == "real_method"));
        assert!(!symbols.iter().any(|s| s.name == "fake_method"));
    }

    #[test]
    fn test_parse_method_with_params() {
        let content = "def process(input, output = nil)\n  input\nend\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "process" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_constant_not_class() {
        // Constants should not be confused with class names
        let content = "VERSION = \"1.0\"\nMAX_RETRIES = 3\n";
        let symbols = RUBY_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "VERSION" && s.kind == SymbolKind::Constant));
        assert!(symbols.iter().any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Constant));
        assert!(!symbols.iter().any(|s| s.kind == SymbolKind::Class && s.name == "VERSION"));
    }
}
