//! Tree-sitter based Swift parser

use anyhow::Result;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

static SWIFT_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_swift::LANGUAGE.into());

static SWIFT_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&SWIFT_LANGUAGE, include_str!("queries/swift.scm"))
        .expect("Failed to compile Swift tree-sitter query")
});

pub static SWIFT_PARSER: SwiftParser = SwiftParser;

pub struct SwiftParser;

impl LanguageParser for SwiftParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &SWIFT_LANGUAGE)?;
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let query = &*SWIFT_QUERY;

        // Build capture name -> index map
        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names.iter().position(|n| *n == name).map(|i| i as u32)
        };

        let idx_decl_kind = idx("decl_kind");
        let idx_class_name = idx("class_name");
        let idx_enum_name = idx("enum_name");
        let idx_ext_type = idx("ext_type");
        let idx_protocol_name = idx("protocol_name");
        let idx_func_name = idx("func_name");
        let idx_init_name = idx("init_name");
        let idx_prop_name = idx("prop_name");
        let idx_typealias_name = idx("typealias_name");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // Class / Struct / Actor
            if let Some(name_cap) = find_capture(m, idx_class_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);

                // Determine kind from declaration_kind
                let kind = if let Some(dk_cap) = find_capture(m, idx_decl_kind) {
                    let dk = node_text(content, &dk_cap.node);
                    match dk {
                        "class" | "actor" => SymbolKind::Class,
                        "struct" => SymbolKind::Class,
                        _ => SymbolKind::Class,
                    }
                } else {
                    SymbolKind::Class
                };

                // Walk the class_declaration node for inheritance_specifier children
                let parents = if let Some(decl_node) = name_cap.node.parent() {
                    collect_parents_from_node(&decl_node, content)
                } else {
                    vec![]
                };

                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Enum
            if let Some(name_cap) = find_capture(m, idx_enum_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                let parents = if let Some(decl_node) = name_cap.node.parent() {
                    collect_parents_from_node(&decl_node, content)
                } else {
                    vec![]
                };

                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Enum,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Extension
            if let Some(ext_cap) = find_capture(m, idx_ext_type) {
                let type_name = node_text(content, &ext_cap.node);
                // Strip generic parameters if present
                let base_name = type_name.split('<').next().unwrap_or(type_name).trim();
                let extended_name = format!("{}+Extension", base_name);
                let line = node_line(&ext_cap.node);

                symbols.push(ParsedSymbol {
                    name: extended_name,
                    kind: SymbolKind::Object,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![(base_name.to_string(), "extends".to_string())],
                });
                continue;
            }

            // Protocol
            if let Some(name_cap) = find_capture(m, idx_protocol_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                let parents = if let Some(decl_node) = name_cap.node.parent() {
                    collect_parents_from_node(&decl_node, content)
                } else {
                    vec![]
                };

                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Function
            if let Some(cap) = find_capture(m, idx_func_name) {
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

            // Init
            if let Some(cap) = find_capture(m, idx_init_name) {
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: "init".to_string(),
                    kind: SymbolKind::Function,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Property
            if let Some(cap) = find_capture(m, idx_prop_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Property,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // TypeAlias
            if let Some(cap) = find_capture(m, idx_typealias_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::TypeAlias,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }
        }

        Ok(symbols)
    }
}

/// Collect parent types by walking a declaration node's inheritance_specifier children.
/// First parent is "extends", the rest are "implements".
fn collect_parents_from_node(node: &tree_sitter::Node, content: &str) -> Vec<(String, String)> {
    let mut parents = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "inheritance_specifier" {
            // Find the type_identifier inside user_type
            if let Some(type_name) = find_type_identifier_in(&child, content) {
                let kind = if parents.is_empty() { "extends" } else { "implements" };
                parents.push((type_name, kind.to_string()));
            }
        }
    }
    parents
}

/// Find the first type_identifier in a node's descendants
fn find_type_identifier_in(node: &tree_sitter::Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            let name = node_text(content, &child);
            let name = name.split('<').next().unwrap_or(name).trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        if let Some(found) = find_type_identifier_in(&child, content) {
            return Some(found);
        }
    }
    None
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
        let content = "class ViewController: UIViewController {\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "ViewController").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
        assert!(cls.parents.iter().any(|(p, _)| p == "UIViewController"));
    }

    #[test]
    fn test_parse_public_final_class() {
        let content = "public final class AppDelegate: UIResponder, UIApplicationDelegate {\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "AppDelegate").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
        assert!(cls.parents.iter().any(|(p, k)| p == "UIResponder" && k == "extends"));
        assert!(cls.parents.iter().any(|(p, k)| p == "UIApplicationDelegate" && k == "implements"));
    }

    #[test]
    fn test_parse_struct() {
        let content = "struct User: Codable, Equatable {\n    let id: Int\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let s = symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(s.kind, SymbolKind::Class); // struct treated as class
        assert!(s.parents.iter().any(|(p, _)| p == "Codable"));
    }

    #[test]
    fn test_parse_enum() {
        let content = "enum Direction: String, CaseIterable {\n    case north, south\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let e = symbols.iter().find(|s| s.name == "Direction").unwrap();
        assert_eq!(e.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_parse_protocol() {
        let content = "protocol Fetchable: AnyObject {\n    func fetch() async\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let p = symbols.iter().find(|s| s.name == "Fetchable").unwrap();
        assert_eq!(p.kind, SymbolKind::Interface);
        assert!(p.parents.iter().any(|(p, _)| p == "AnyObject"));
    }

    #[test]
    fn test_parse_actor() {
        let content = "actor DataStore {\n    func save() {}\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let a = symbols.iter().find(|s| s.name == "DataStore").unwrap();
        assert_eq!(a.kind, SymbolKind::Class); // actor treated as class
    }

    #[test]
    fn test_parse_extension() {
        let content = "extension String: CustomProtocol {\n    func trimmed() -> String { self }\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let ext = symbols.iter().find(|s| s.name == "String+Extension").unwrap();
        assert_eq!(ext.kind, SymbolKind::Object);
        assert!(ext.parents.iter().any(|(p, k)| p == "String" && k == "extends"));
    }

    #[test]
    fn test_parse_function() {
        let content = "func loadData(id: Int) async throws -> Data {\n    fatalError()\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let f = symbols.iter().find(|s| s.name == "loadData").unwrap();
        assert_eq!(f.kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_init() {
        let content = "class Foo {\n    public init(name: String) {\n        self.name = name\n    }\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "init" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_property() {
        let content = "class Foo {\n    var name: String = \"\"\n    let count: Int = 0\n    static var shared: Foo = Foo()\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "count" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "shared" && s.kind == SymbolKind::Property));
    }

    #[test]
    fn test_parse_typealias() {
        let content = "public typealias Completion = (Result<Data, Error>) -> Void\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let ta = symbols.iter().find(|s| s.name == "Completion").unwrap();
        assert_eq!(ta.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_parse_nested_function() {
        let content = "class ViewController {\n    func loadData() async throws -> Data {\n        fatalError()\n    }\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "loadData" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_generic_class() {
        let content = "class Container<T>: Sequence {\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Container").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
        assert!(cls.parents.iter().any(|(p, _)| p == "Sequence"));
    }

    #[test]
    fn test_comments_ignored() {
        let content = "// class FakeClass {}\nclass RealClass {\n}\n/* func fakeFunc() {} */\nfunc realFunc() {}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "RealClass"));
        assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
        assert!(symbols.iter().any(|s| s.name == "realFunc"));
        assert!(!symbols.iter().any(|s| s.name == "fakeFunc"));
    }

    #[test]
    fn test_parse_class_no_parents() {
        let content = "class Empty {\n}\n";
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Empty").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
        assert!(cls.parents.is_empty());
    }

    #[test]
    fn test_parse_multiple_declarations() {
        let content = r#"
class ViewController: UIViewController, UITableViewDelegate {
    var name: String = ""
    let count: Int = 0
    func loadData() async throws -> Data { fatalError() }
    init(name: String) { self.name = name }
}
struct User: Codable { let id: Int }
enum Direction: String { case north }
protocol Fetchable: AnyObject { func fetch() async }
actor DataStore { func save() {} }
extension String { func trimmed() -> String { self } }
typealias Completion = (Result<Data, Error>) -> Void
"#;
        let symbols = SWIFT_PARSER.parse_symbols(content).unwrap();

        // Check that all major declarations are found
        assert!(symbols.iter().any(|s| s.name == "ViewController" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "Direction" && s.kind == SymbolKind::Enum));
        assert!(symbols.iter().any(|s| s.name == "Fetchable" && s.kind == SymbolKind::Interface));
        assert!(symbols.iter().any(|s| s.name == "DataStore" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "String+Extension" && s.kind == SymbolKind::Object));
        assert!(symbols.iter().any(|s| s.name == "Completion" && s.kind == SymbolKind::TypeAlias));
        assert!(symbols.iter().any(|s| s.name == "loadData" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "init" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "count" && s.kind == SymbolKind::Property));
    }
}
