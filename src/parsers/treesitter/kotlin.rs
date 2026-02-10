//! Tree-sitter based Kotlin/Java parser

use anyhow::Result;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

static KT_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_kotlin_ng::LANGUAGE.into());

static KT_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&KT_LANGUAGE, include_str!("queries/kotlin.scm"))
        .expect("Failed to compile Kotlin tree-sitter query")
});

pub static KOTLIN_PARSER: KotlinParser = KotlinParser;

pub struct KotlinParser;

impl LanguageParser for KotlinParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &KT_LANGUAGE)?;
        let mut symbols = Vec::new();
        let query = &*KT_QUERY;
        let mut cursor = QueryCursor::new();

        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names.iter().position(|n| *n == name).map(|i| i as u32)
        };

        let idx_class_name = idx("class_name");
        let idx_class_decl = idx("class_decl");
        let idx_object_name = idx("object_name");
        let idx_object_decl = idx("object_decl");
        let idx_func_name = idx("func_name");
        let idx_property_name = idx("property_name");
        let idx_typealias_name = idx("typealias_name");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // Class or Interface declaration
            if let Some(name_cap) = find_capture(m, idx_class_name) {
                let decl_cap = find_capture(m, idx_class_decl);
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);

                if let Some(decl) = decl_cap {
                    let decl_node = &decl.node;

                    // Determine if this is an interface or class
                    let is_interface = has_keyword(decl_node, content, "interface");

                    // Check modifiers for enum, sealed, data, etc.
                    let has_enum_modifier = has_class_modifier(decl_node, content, "enum");

                    let kind = if is_interface {
                        SymbolKind::Interface
                    } else if has_enum_modifier {
                        // enum class maps to Class (not Enum), matching regex parser
                        SymbolKind::Class
                    } else {
                        SymbolKind::Class
                    };

                    // Parse inheritance from delegation_specifiers
                    let parents = if is_interface {
                        // Interface parents are always "extends"
                        parse_delegation_specifiers_for_interface(decl_node, content)
                    } else {
                        parse_delegation_specifiers(decl_node, content)
                    };

                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents,
                    });
                }
                continue;
            }

            // Object declaration
            if let Some(name_cap) = find_capture(m, idx_object_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);

                let parents = if let Some(decl) = find_capture(m, idx_object_decl) {
                    parse_delegation_specifiers(&decl.node, content)
                } else {
                    vec![]
                };

                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Object,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Function declaration
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

            // Property declaration (val/var)
            if let Some(cap) = find_capture(m, idx_property_name) {
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

            // Type alias
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

/// Check if a class_declaration node contains a specific keyword (e.g., "interface", "class")
/// by looking at its anonymous children (the keyword tokens).
fn has_keyword(node: &tree_sitter::Node, content: &str, keyword: &str) -> bool {
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        // Anonymous children are keywords
        if !child.is_named() && node_text(content, &child) == keyword {
            return true;
        }
    }
    false
}

/// Check if a class_declaration has a specific class_modifier (e.g., "enum", "sealed", "data")
fn has_class_modifier(node: &tree_sitter::Node, content: &str, modifier: &str) -> bool {
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        if child.kind() == "modifiers" {
            let mut mod_walker = child.walk();
            for mod_child in child.children(&mut mod_walker) {
                if mod_child.kind() == "class_modifier"
                    && node_text(content, &mod_child) == modifier
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Parse delegation_specifiers from a class/object declaration node.
/// Returns parent list with (name, inherit_kind) where:
/// - constructor_invocation (has parentheses) -> "extends"
/// - plain type (no parentheses) -> "implements"
fn parse_delegation_specifiers(
    decl_node: &tree_sitter::Node,
    content: &str,
) -> Vec<(String, String)> {
    let mut parents = Vec::new();

    let mut walker = decl_node.walk();
    for child in decl_node.children(&mut walker) {
        if child.kind() == "delegation_specifiers" {
            let mut ds_walker = child.walk();
            for specifier in child.children(&mut ds_walker) {
                if specifier.kind() == "delegation_specifier" {
                    if let Some((name, kind)) = parse_single_delegation_specifier(&specifier, content) {
                        parents.push((name, kind));
                    }
                }
            }
        }
    }

    parents
}

/// Parse delegation_specifiers for an interface.
/// Interface parents are always "extends".
fn parse_delegation_specifiers_for_interface(
    decl_node: &tree_sitter::Node,
    content: &str,
) -> Vec<(String, String)> {
    let mut parents = Vec::new();

    let mut walker = decl_node.walk();
    for child in decl_node.children(&mut walker) {
        if child.kind() == "delegation_specifiers" {
            let mut ds_walker = child.walk();
            for specifier in child.children(&mut ds_walker) {
                if specifier.kind() == "delegation_specifier" {
                    if let Some(name) = extract_type_name_from_specifier(&specifier, content) {
                        parents.push((name, "extends".to_string()));
                    }
                }
            }
        }
    }

    parents
}

/// Parse a single delegation_specifier node.
/// Returns (parent_name, "extends"|"implements").
fn parse_single_delegation_specifier(
    specifier: &tree_sitter::Node,
    content: &str,
) -> Option<(String, String)> {
    let mut walker = specifier.walk();
    for child in specifier.children(&mut walker) {
        match child.kind() {
            "constructor_invocation" => {
                // Has parentheses -> extends
                let name = extract_type_name_from_node(&child, content)?;
                return Some((name, "extends".to_string()));
            }
            // "type" is a supertype that resolves to user_type, nullable_type, etc.
            "user_type" => {
                let name = extract_user_type_name(&child, content)?;
                return Some((name, "implements".to_string()));
            }
            "nullable_type" | "parenthesized_type" | "function_type" | "non_nullable_type" => {
                let name = extract_type_name_from_node(&child, content)?;
                return Some((name, "implements".to_string()));
            }
            _ => {}
        }
    }
    None
}

/// Extract the type name from a delegation_specifier (for interface parents)
fn extract_type_name_from_specifier(
    specifier: &tree_sitter::Node,
    content: &str,
) -> Option<String> {
    let mut walker = specifier.walk();
    for child in specifier.children(&mut walker) {
        match child.kind() {
            "constructor_invocation" => {
                return extract_type_name_from_node(&child, content);
            }
            "user_type" => {
                return extract_user_type_name(&child, content);
            }
            "nullable_type" | "parenthesized_type" | "function_type" | "non_nullable_type" => {
                return extract_type_name_from_node(&child, content);
            }
            _ => {}
        }
    }
    None
}

/// Extract the first identifier (type name) from a node by walking its descendants.
/// Used for constructor_invocation and other compound type nodes.
fn extract_type_name_from_node(node: &tree_sitter::Node, content: &str) -> Option<String> {
    // For constructor_invocation, the structure is:
    //   constructor_invocation -> type -> user_type -> identifier
    // For user_type directly:
    //   user_type -> identifier
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        if child.kind() == "identifier" {
            return Some(node_text(content, &child).to_string());
        }
        // Recurse into type and user_type nodes
        if child.kind() == "user_type" || child.kind() == "type"
            || child.kind() == "nullable_type" || child.kind() == "non_nullable_type"
        {
            if let Some(name) = extract_type_name_from_node(&child, content) {
                return Some(name);
            }
        }
    }
    None
}

/// Extract the name from a user_type node.
/// user_type -> identifier (possibly with type_arguments)
fn extract_user_type_name(node: &tree_sitter::Node, content: &str) -> Option<String> {
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        if child.kind() == "identifier" {
            return Some(node_text(content, &child).to_string());
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
        let content = "class MyService {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "MyService" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_parse_data_class() {
        let content = "data class User(val name: String, val age: Int)\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_parse_object() {
        let content = "object Singleton {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Singleton" && s.kind == SymbolKind::Object));
    }

    #[test]
    fn test_parse_interface() {
        let content = "interface Repository {\n    fun getAll(): List<Item>\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Repository" && s.kind == SymbolKind::Interface));
    }

    #[test]
    fn test_parse_sealed_interface() {
        let content = "sealed interface Result {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Result" && s.kind == SymbolKind::Interface));
    }

    #[test]
    fn test_parse_function() {
        let content = "fun processPayment(amount: Double): Boolean {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "processPayment" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_suspend_function() {
        let content = "    suspend fun fetchData(): Result<Data> {\n    }\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "fetchData" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_property() {
        let content = "    val name: String = \"test\"\n    var count: Int = 0\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "count" && s.kind == SymbolKind::Property));
    }

    #[test]
    fn test_parse_typealias() {
        let content = "typealias StringMap = Map<String, String>\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "StringMap" && s.kind == SymbolKind::TypeAlias));
    }

    #[test]
    fn test_parse_class_with_inheritance() {
        let content = "class MyFragment(arg: String) : Fragment(), Serializable {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "MyFragment" && s.kind == SymbolKind::Class).unwrap();
        assert!(cls.parents.iter().any(|(p, k)| p == "Fragment" && k == "extends"));
        assert!(cls.parents.iter().any(|(p, k)| p == "Serializable" && k == "implements"));
    }

    #[test]
    fn test_parse_class_simple_inheritance() {
        let content = "class Child : Parent {\n}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Child").unwrap();
        assert!(!cls.parents.is_empty());
    }

    #[test]
    fn test_comments_ignored() {
        let content = "// class FakeClass {}\nclass RealClass {}\n/* fun fake() {} */\nfun real() {}\n";
        let symbols = KOTLIN_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "RealClass"));
        assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
        assert!(symbols.iter().any(|s| s.name == "real"));
        assert!(!symbols.iter().any(|s| s.name == "fake"));
    }
}
