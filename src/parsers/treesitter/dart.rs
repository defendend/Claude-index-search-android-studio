//! Tree-sitter based Dart parser

use anyhow::Result;
use tree_sitter::{Language, Node};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

static DART_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_dart::language());

pub static DART_PARSER: DartParser = DartParser;

pub struct DartParser;

impl LanguageParser for DartParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &DART_LANGUAGE)?;
        let mut symbols = Vec::new();

        // Walk the tree manually since tree-sitter-dart 0.0.4 has limited query support
        walk_node(&tree.root_node(), content, &mut symbols);

        Ok(symbols)
    }
}

/// Recursively walk the AST and extract symbols
fn walk_node(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    match node.kind() {
        "import_or_export" => {
            extract_import(node, content, symbols);
        }
        "class_definition" => {
            extract_class(node, content, symbols);
            // Continue walking for inner declarations (methods, constructors, etc.)
            walk_class_body(node, content, symbols);
            return; // Don't recurse further, we handled it
        }
        "mixin_declaration" => {
            extract_mixin(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "extension_declaration" => {
            extract_extension(node, content, symbols);
            walk_extension_body(node, content, symbols);
            return;
        }
        "extension_type_declaration" => {
            extract_extension_type(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "enum_declaration" => {
            extract_enum(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "type_alias" => {
            extract_typedef(node, content, symbols);
            return;
        }
        // Top-level functions: tree-sitter-dart 0.0.4 wraps them in lambda_expression
        "lambda_expression" => {
            if is_top_level(node) {
                extract_lambda_function(node, content, symbols);
            }
            return;
        }
        "function_signature" => {
            // Only handle top-level function signatures (without body)
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_function_signature(node, content, symbols);
            }
            return;
        }
        "getter_signature" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_getter(node, content, symbols);
            }
            return;
        }
        "setter_signature" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_setter(node, content, symbols);
            }
            return;
        }
        // Top-level variable declarations (tree-sitter-dart 0.0.4 uses local_variable_declaration)
        "local_variable_declaration" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_local_var_as_property(node, content, symbols);
            }
            return;
        }
        // Top-level variable declarations
        "initialized_identifier_list" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_top_level_vars(node, content, symbols);
            }
            return;
        }
        "static_final_declaration_list" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_top_level_consts(node, content, symbols);
            }
            return;
        }
        // ERROR recovery: tree-sitter-dart 0.0.4 doesn't know Dart 3 modifiers
        // sealed/base/final class, extension type, mixin class
        "ERROR" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                try_recover_from_error(node, content, symbols);
            }
            return;
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(&child, content, symbols);
    }
}

/// Check if a node is a direct child of the program (top-level)
fn is_top_level(node: &Node) -> bool {
    node.parent()
        .map(|p| p.kind() == "program")
        .unwrap_or(false)
}

/// Check if a node is within a top-level unnamed wrapper (program > anonymous_node > this)
fn is_in_top_level_wrapper(node: &Node) -> bool {
    if let Some(parent) = node.parent() {
        if parent.kind() == "program" {
            return true;
        }
        // Some constructs are wrapped in an unnamed sequence node at top level
        if let Some(grandparent) = parent.parent() {
            if grandparent.kind() == "program" {
                return true;
            }
        }
    }
    false
}

/// Extract import/export declaration
fn extract_import(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();
    let full_text = node_text(content, node);

    // Find URI string in the import
    if let Some(uri_node) = find_descendant_by_kind(node, "uri") {
        let uri_text = node_text(content, &uri_node);
        // Strip quotes from the URI
        let path = uri_text.trim_matches('\'').trim_matches('"');
        // Extract short name: last segment without .dart
        let short_name = path.rsplit('/').next().unwrap_or(path)
            .trim_end_matches(".dart");

        // Check if it's an export
        let _is_export = full_text.trim_start().starts_with("export");

        symbols.push(ParsedSymbol {
            name: short_name.to_string(),
            kind: SymbolKind::Import,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract class definition
fn extract_class(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Determine kind: check for "interface" modifier
    let full_text = node_text(content, node);
    let decl_prefix = full_text.split('{').next().unwrap_or("");

    let kind = if decl_prefix.contains("interface class") || decl_prefix.contains("interface  class") {
        SymbolKind::Interface
    } else {
        SymbolKind::Class
    };

    // Extract parents
    let mut parents = Vec::new();

    // superclass field
    if let Some(superclass_node) = node.child_by_field_name("superclass") {
        extract_superclass_parents(&superclass_node, content, &mut parents);
    }

    // interfaces field
    if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
        extract_interfaces_parents(&interfaces_node, content, &mut parents);
    }

    symbols.push(ParsedSymbol {
        name,
        kind,
        line,
        signature: sig,
        parents,
    });
}

/// Extract parents from a superclass node
fn extract_superclass_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                let name = node_text(content, &child).to_string();
                let base = name.split('<').next().unwrap_or(&name).trim().to_string();
                if !base.is_empty() {
                    parents.push((base, "extends".to_string()));
                }
            }
            "mixins" => {
                extract_mixins_parents(&child, content, parents);
            }
            _ => {
                if child.kind() != "extends" && child.named_child_count() > 0 {
                    extract_type_names_from_node(&child, content, parents, "extends");
                }
            }
        }
    }
}

/// Extract parent types from a mixins node ("with" clause)
fn extract_mixins_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    extract_type_names_from_node(node, content, parents, "with");
}

/// Extract parent types from an interfaces node ("implements" clause)
fn extract_interfaces_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    extract_type_names_from_node(node, content, parents, "implements");
}

/// Recursively extract type_identifier names from a node, for a given relationship kind
fn extract_type_names_from_node(node: &Node, content: &str, parents: &mut Vec<(String, String)>, kind: &str) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            let name = node_text(content, &child).to_string();
            if !name.is_empty() {
                parents.push((name, kind.to_string()));
            }
        } else if child.named_child_count() > 0 && child.kind() != "type_arguments" {
            extract_type_names_from_node(&child, content, parents, kind);
        }
    }
}

/// Extract mixin declaration
fn extract_mixin(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Check if this is "mixin class" (Dart 3) — tree-sitter-dart 0.0.4 parses it
    // as mixin_declaration with an ERROR child "class"
    let has_class_keyword = {
        let mut cursor = node.walk();
        let result = node.children(&mut cursor).any(|c| c.kind() == "ERROR" && node_text(content, &c).trim() == "class");
        result
    };

    if has_class_keyword {
        // "mixin class" → treat as Class
        let name = find_mixin_name(node, content);
        if name.is_empty() { return; }

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Class,
            line,
            signature: sig,
            parents: vec![],
        });
        return;
    }

    // Regular mixin
    let name = find_mixin_name(node, content);
    if name.is_empty() { return; }

    let mut parents = Vec::new();

    let node_text_full = node_text(content, node);
    let mut cursor = node.walk();
    let mut found_on = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "on" {
            found_on = true;
        }
        if child.kind() == "type_identifier" && found_on {
            let type_name = node_text(content, &child).to_string();
            if !type_name.is_empty() {
                parents.push((type_name, "extends".to_string()));
            }
        }
        if child.kind() == "interfaces" {
            extract_interfaces_parents(&child, content, &mut parents);
        }
    }

    // Fallback: parse from text if no parents found via tree
    if parents.is_empty() && node_text_full.contains(" on ") {
        let on_part = node_text_full.split(" on ").nth(1).unwrap_or("");
        let on_types = on_part.split("implements").next().unwrap_or(on_part);
        let on_types = on_types.split('{').next().unwrap_or(on_types);
        for t in on_types.split(',') {
            let type_name = t.trim().split('<').next().unwrap_or("").trim();
            if !type_name.is_empty() {
                parents.push((type_name.to_string(), "extends".to_string()));
            }
        }
        if let Some(impl_part) = node_text_full.split("implements").nth(1) {
            let impl_part = impl_part.split('{').next().unwrap_or(impl_part);
            for t in impl_part.split(',') {
                let type_name = t.trim().split('<').next().unwrap_or("").trim();
                if !type_name.is_empty() {
                    parents.push((type_name.to_string(), "implements".to_string()));
                }
            }
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Interface,
        line,
        signature: sig,
        parents,
    });
}

/// Find the mixin name from a mixin_declaration node
fn find_mixin_name(node: &Node, content: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return node_text(content, &child).to_string();
        }
    }
    String::new()
}

/// Extract extension declaration
fn extract_extension(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return, // Anonymous extension, skip
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // "on" type is the "class" field in extension_declaration
    if let Some(class_node) = node.child_by_field_name("class") {
        let on_type = if class_node.kind() == "type_identifier" {
            node_text(content, &class_node).to_string()
        } else {
            find_first_type_identifier(&class_node, content)
                .unwrap_or_default()
        };
        let base = on_type.split('<').next().unwrap_or(&on_type).trim().to_string();
        if !base.is_empty() {
            parents.push((base, "extends".to_string()));
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Object,
        line,
        signature: sig,
        parents,
    });
}

/// Extract extension type declaration
fn extract_extension_type(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // interfaces
    if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
        extract_interfaces_parents(&interfaces_node, content, &mut parents);
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Class,
        line,
        signature: sig,
        parents,
    });
}

/// Extract enum declaration
fn extract_enum(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // Standard tree: mixins and interfaces as children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "mixins" => extract_mixins_parents(&child, content, &mut parents),
            "interfaces" => extract_interfaces_parents(&child, content, &mut parents),
            // tree-sitter-dart 0.0.4: "with"/"implements" end up in ERROR node
            "ERROR" => {
                extract_parents_from_error_text(&child, content, &mut parents);
            }
            _ => {}
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Enum,
        line,
        signature: sig,
        parents,
    });
}

/// Extract typedef/type_alias
fn extract_typedef(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = find_first_type_identifier(node, content)
        .or_else(|| {
            let text = node_text(content, node);
            let after_typedef = text.strip_prefix("typedef")?.trim();
            let name_part = after_typedef.split(|c: char| c == '=' || c == '(' || c == '<').next()?;
            let tokens: Vec<&str> = name_part.split_whitespace().collect();
            if tokens.len() >= 2 {
                Some(tokens[tokens.len() - 1].to_string())
            } else if tokens.len() == 1 {
                Some(tokens[0].to_string())
            } else {
                None
            }
        });

    if let Some(name) = name {
        if !name.is_empty() {
            symbols.push(ParsedSymbol {
                name,
                kind: SymbolKind::TypeAlias,
                line,
                signature: sig,
                parents: vec![],
            });
        }
    }
}

/// Extract a function from lambda_expression at top level.
/// tree-sitter-dart 0.0.4 wraps "void main() {}" as lambda_expression > function_signature + function_body
fn extract_lambda_function(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    // Find function_signature child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_signature" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(content, &name_node).to_string();
                let line = node_line(&child);
                let sig = line_text(content, line).trim().to_string();

                symbols.push(ParsedSymbol {
                    name,
                    kind: SymbolKind::Function,
                    line,
                    signature: sig,
                    parents: vec![],
                });
                return;
            }
            // Fallback: find identifier child
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    let name = node_text(content, &inner).to_string();
                    let line = node_line(&child);
                    let sig = line_text(content, line).trim().to_string();

                    symbols.push(ParsedSymbol {
                        name,
                        kind: SymbolKind::Function,
                        line,
                        signature: sig,
                        parents: vec![],
                    });
                    return;
                }
            }
        }
    }
}

/// Extract a function_signature at top level
fn extract_function_signature(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract getter
fn extract_getter(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Property,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract setter
fn extract_setter(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Property,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract top-level variable from local_variable_declaration.
/// tree-sitter-dart 0.0.4 uses local_variable_declaration for top-level vars.
fn extract_local_var_as_property(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    // local_variable_declaration > initialized_variable_definition > identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "initialized_variable_definition" {
            if let Some(id) = find_first_identifier(&child, content) {
                let line = node_line(&child);
                symbols.push(ParsedSymbol {
                    name: id,
                    kind: SymbolKind::Property,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
            }
        }
    }
}

/// Walk class body for methods, constructors, getters, setters
fn walk_class_body(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let body = find_descendant_by_kind(node, "class_body")
        .or_else(|| find_descendant_by_kind(node, "enum_body"));

    if let Some(body) = body {
        walk_body_declarations(&body, content, symbols);
    }
}

/// Walk extension body for methods
fn walk_extension_body(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(body) = node.child_by_field_name("body") {
        walk_body_declarations(&body, content, symbols);
    }
}

/// Walk body for declarations (methods, constructors, getters, setters, properties)
fn walk_body_declarations(body: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        walk_body_member(&child, content, symbols);
    }
}

/// Process a single member in a class/extension body
fn walk_body_member(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    match node.kind() {
        "declaration" => {
            extract_declaration(node, content, symbols);
        }
        "method_signature" => {
            extract_method_signature(node, content, symbols);
        }
        "function_signature" => {
            extract_function_signature(node, content, symbols);
        }
        "getter_signature" => {
            extract_getter(node, content, symbols);
        }
        "setter_signature" => {
            extract_setter(node, content, symbols);
        }
        "constructor_signature" => {
            extract_constructor(node, content, symbols);
        }
        "factory_constructor_signature" => {
            extract_factory_constructor(node, content, symbols);
        }
        "constant_constructor_signature" => {
            extract_const_constructor(node, content, symbols);
        }
        _ => {
            // Recurse one level to find declarations
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_body_member(&child, content, symbols);
            }
        }
    }
}

/// Extract declaration (wraps method_signature, variable decls, etc.)
fn extract_declaration(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_signature" => {
                extract_function_signature(&child, content, symbols);
            }
            "getter_signature" => {
                extract_getter(&child, content, symbols);
            }
            "setter_signature" => {
                extract_setter(&child, content, symbols);
            }
            "constructor_signature" => {
                extract_constructor(&child, content, symbols);
            }
            "factory_constructor_signature" => {
                extract_factory_constructor(&child, content, symbols);
            }
            "constant_constructor_signature" => {
                extract_const_constructor(&child, content, symbols);
            }
            _ => {}
        }
    }
}

/// Extract method_signature (wraps constructor_signature, function_signature, etc.)
fn extract_method_signature(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_signature" => {
                extract_function_signature(&child, content, symbols);
            }
            "getter_signature" => {
                extract_getter(&child, content, symbols);
            }
            "setter_signature" => {
                extract_setter(&child, content, symbols);
            }
            "constructor_signature" => {
                extract_constructor(&child, content, symbols);
            }
            "factory_constructor_signature" => {
                extract_factory_constructor(&child, content, symbols);
            }
            "constant_constructor_signature" => {
                extract_const_constructor(&child, content, symbols);
            }
            _ => {}
        }
    }
}

/// Extract constructor: ClassName(...) or ClassName.named(...)
fn extract_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Always use collect_constructor_name to get the full name (ClassName.namedPart)
    // because child_by_field_name("name") only returns the class part
    let name_text = collect_constructor_name(node, content);

    if !name_text.is_empty() {
        symbols.push(ParsedSymbol {
            name: name_text,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Collect constructor name from node children (identifiers and dots joined)
fn collect_constructor_name(node: &Node, content: &str) -> String {
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            parts.push(node_text(content, &child));
        }
        // Stop at formal_parameter_list (constructor args)
        if child.kind() == "formal_parameter_list" {
            break;
        }
    }
    parts.join(".")
}

/// Extract factory constructor
fn extract_factory_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = collect_constructor_name(node, content);

    if !name.is_empty() {
        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract const constructor
fn extract_const_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = collect_constructor_name(node, content);

    if !name.is_empty() {
        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract top-level variable declarations (final/var/type)
fn extract_top_level_vars(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "initialized_identifier" {
            if let Some(id) = find_first_identifier(&child, content) {
                let line = node_line(&child);
                symbols.push(ParsedSymbol {
                    name: id,
                    kind: SymbolKind::Property,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
            }
        }
    }
}

/// Extract top-level constant declarations
fn extract_top_level_consts(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "static_final_declaration" {
            if let Some(id) = find_first_identifier(&child, content) {
                let line = node_line(&child);
                symbols.push(ParsedSymbol {
                    name: id,
                    kind: SymbolKind::Property,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
            }
        }
    }
}

/// Try to recover declarations from ERROR nodes.
/// tree-sitter-dart 0.0.4 doesn't understand Dart 3 modifiers:
/// - sealed class, base class, final class → ERROR + block sibling
/// - extension type → ERROR + block sibling
fn try_recover_from_error(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let text = node_text(content, node).trim().to_string();
    let line = node_line(node);

    // Check for "sealed class X", "base class X", "final class X"
    if let Some(class_info) = try_parse_modified_class(&text) {
        let sig_line = line_text(content, line).trim().to_string();
        // Try to find block sibling for body methods
        let mut parents = Vec::new();
        // Parse parents from the text after the class name
        parse_parents_from_class_text(&text, &mut parents);

        symbols.push(ParsedSymbol {
            name: class_info.name,
            kind: class_info.kind,
            line,
            signature: sig_line,
            parents,
        });

        // Walk the next sibling (block node) for body declarations
        if let Some(next) = node.next_sibling() {
            if next.kind() == "block" {
                walk_body_declarations(&next, content, symbols);
            }
        }
    }

    // Check for "extension type X(...) implements Y"
    if text.starts_with("extension type ") || text.starts_with("extension  type ") {
        if let Some(ext_type_info) = try_parse_extension_type(&text) {
            let sig_line = line_text(content, line).trim().to_string();
            symbols.push(ParsedSymbol {
                name: ext_type_info.name,
                kind: SymbolKind::Class,
                line,
                signature: sig_line,
                parents: ext_type_info.parents,
            });
        }
    }
}

struct ClassInfo {
    name: String,
    kind: SymbolKind,
}

struct ExtTypeInfo {
    name: String,
    parents: Vec<(String, String)>,
}

/// Try to parse "sealed/base/final class ClassName" from ERROR text
fn try_parse_modified_class(text: &str) -> Option<ClassInfo> {
    // Patterns: "sealed class X", "base class X", "final class X",
    //           "abstract sealed class X", etc.
    let words: Vec<&str> = text.split_whitespace().collect();

    // Find "class" keyword
    let class_idx = words.iter().position(|w| *w == "class")?;
    if class_idx + 1 >= words.len() { return None; }

    let name = words[class_idx + 1].to_string();
    // Strip generic parameters
    let name = name.split('<').next().unwrap_or(&name).trim().to_string();

    if name.is_empty() { return None; }

    // Check for modifiers before "class"
    let modifiers: Vec<&str> = words[..class_idx].to_vec();
    let kind = if modifiers.contains(&"interface") {
        SymbolKind::Interface
    } else {
        SymbolKind::Class
    };

    Some(ClassInfo { name, kind })
}

/// Parse parents from class declaration text (after class name)
fn parse_parents_from_class_text(text: &str, parents: &mut Vec<(String, String)>) {
    // Find "extends", "with", "implements" in the text
    let parts = text.split_whitespace().collect::<Vec<_>>();

    let mut mode = "";
    for &word in &parts {
        match word {
            "extends" => { mode = "extends"; continue; }
            "with" => { mode = "with"; continue; }
            "implements" => { mode = "implements"; continue; }
            "class" | "sealed" | "base" | "final" | "abstract" | "interface" => continue,
            _ => {}
        }
        if !mode.is_empty() {
            // This word is a type name
            let name = word.trim_end_matches(',').split('<').next().unwrap_or("").trim();
            if !name.is_empty() && name != "{" && name != "}" {
                parents.push((name.to_string(), mode.to_string()));
            }
        }
    }
}

/// Try to parse "extension type X(...) implements Y" from ERROR text
fn try_parse_extension_type(text: &str) -> Option<ExtTypeInfo> {
    let words: Vec<&str> = text.split_whitespace().collect();

    // Find "type" keyword after "extension"
    let type_idx = words.iter().position(|w| *w == "type")?;
    if type_idx + 1 >= words.len() { return None; }

    let name_raw = words[type_idx + 1];
    let name = name_raw.split('(').next().unwrap_or(name_raw).trim().to_string();

    if name.is_empty() { return None; }

    let mut parents = Vec::new();
    if let Some(impl_idx) = words.iter().position(|w| *w == "implements") {
        for &word in &words[impl_idx + 1..] {
            let type_name = word.trim_end_matches(',').split('<').next().unwrap_or("").trim();
            if !type_name.is_empty() && type_name != "{" && type_name != "}" {
                parents.push((type_name.to_string(), "implements".to_string()));
            }
        }
    }

    Some(ExtTypeInfo { name, parents })
}

/// Extract parents from ERROR node text (for enum with/implements in tree-sitter-dart 0.0.4)
fn extract_parents_from_error_text(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    let text = node_text(content, node);
    let words: Vec<&str> = text.split_whitespace().collect();

    let mut mode = "";
    for &word in &words {
        match word {
            "with" => { mode = "with"; continue; }
            "implements" => { mode = "implements"; continue; }
            _ => {}
        }
        if !mode.is_empty() {
            let name = word.trim_end_matches(',').split('<').next().unwrap_or("").trim();
            if !name.is_empty() {
                parents.push((name.to_string(), mode.to_string()));
            }
        }
    }
}

/// Find first identifier child node and return its text
fn find_first_identifier(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(node_text(content, &child).to_string());
        }
    }
    None
}

/// Find first type_identifier in descendants
fn find_first_type_identifier(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            return Some(node_text(content, &child).to_string());
        }
        if let Some(found) = find_first_type_identifier(&child, content) {
            return Some(found);
        }
    }
    None
}

/// Find a descendant node by kind
fn find_descendant_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
        if let Some(found) = find_descendant_by_kind(&child, kind) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_class() {
        let content = "class MyWidget extends StatefulWidget {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "MyWidget").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
        assert!(cls.parents.iter().any(|(p, k)| p == "StatefulWidget" && k == "extends"),
            "Expected extends StatefulWidget, got: {:?}", cls.parents);
    }

    #[test]
    fn test_parse_abstract_class() {
        let content = "abstract class BaseService {\n  Future<void> init();\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "BaseService").unwrap();
        assert_eq!(cls.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_sealed_class() {
        let content = "sealed class Result {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Result").expect(
            &format!("Should find sealed class Result, got: {:?}", symbols));
        assert_eq!(cls.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_abstract_interface_class() {
        let content = "abstract interface class AppScope {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "AppScope").unwrap();
        assert_eq!(cls.kind, SymbolKind::Interface,
            "abstract interface class should be Interface, got: {:?}", cls.kind);
    }

    #[test]
    fn test_parse_class_with_parents() {
        let content = "class ApiService extends BaseService with LoggerMixin implements Disposable {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "ApiService" && s.kind == SymbolKind::Class).unwrap();
        assert!(cls.parents.iter().any(|(p, k)| p == "BaseService" && k == "extends"),
            "Expected extends BaseService, got: {:?}", cls.parents);
        assert!(cls.parents.iter().any(|(p, k)| p == "LoggerMixin" && k == "with"),
            "Expected with LoggerMixin, got: {:?}", cls.parents);
        assert!(cls.parents.iter().any(|(p, k)| p == "Disposable" && k == "implements"),
            "Expected implements Disposable, got: {:?}", cls.parents);
    }

    #[test]
    fn test_parse_mixin() {
        let content = "mixin LoggerMixin on Object {\n  void log(String msg) {}\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let m = symbols.iter().find(|s| s.name == "LoggerMixin").unwrap();
        assert_eq!(m.kind, SymbolKind::Interface);
        assert!(m.parents.iter().any(|(p, k)| p == "Object" && k == "extends"),
            "Expected extends Object, got: {:?}", m.parents);
    }

    #[test]
    fn test_parse_mixin_with_implements() {
        let content = "mixin _PublicAppScopeImpl on _AppScopeDeps implements AppScope {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let m = symbols.iter().find(|s| s.name == "_PublicAppScopeImpl").unwrap();
        assert_eq!(m.kind, SymbolKind::Interface);
        assert!(m.parents.iter().any(|(p, k)| p == "_AppScopeDeps" && k == "extends"),
            "should have _AppScopeDeps as extends parent, got: {:?}", m.parents);
        assert!(m.parents.iter().any(|(p, k)| p == "AppScope" && k == "implements"),
            "should have AppScope as implements parent, got: {:?}", m.parents);
    }

    #[test]
    fn test_parse_extension() {
        let content = "extension DateTimeX on DateTime {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let ext = symbols.iter().find(|s| s.name == "DateTimeX").unwrap();
        assert_eq!(ext.kind, SymbolKind::Object);
        assert!(ext.parents.iter().any(|(p, k)| p == "DateTime" && k == "extends"),
            "Expected extends DateTime, got: {:?}", ext.parents);
    }

    #[test]
    fn test_parse_extension_type() {
        let content = "extension type UserId(int id) implements int {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let et = symbols.iter().find(|s| s.name == "UserId").expect(
            &format!("Should find extension type UserId, got: {:?}", symbols));
        assert_eq!(et.kind, SymbolKind::Class);
        assert!(et.parents.iter().any(|(p, _)| p == "int"),
            "Expected implements int, got: {:?}", et.parents);
    }

    #[test]
    fn test_parse_enum() {
        let content = "enum Status {\n  loading,\n  success,\n  error,\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let e = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(e.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_parse_enum_with_parents() {
        let content = "enum EnhancedEnum with Mixin implements Interface {\n  value1,\n  value2;\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let e = symbols.iter().find(|s| s.name == "EnhancedEnum").unwrap();
        assert_eq!(e.kind, SymbolKind::Enum);
        assert!(e.parents.iter().any(|(p, k)| p == "Mixin" && k == "with"),
            "Expected with Mixin, got: {:?}", e.parents);
        assert!(e.parents.iter().any(|(p, k)| p == "Interface" && k == "implements"),
            "Expected implements Interface, got: {:?}", e.parents);
    }

    #[test]
    fn test_parse_typedef() {
        let content = "typedef JsonMap = Map<String, dynamic>;\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let td = symbols.iter().find(|s| s.name == "JsonMap").unwrap();
        assert_eq!(td.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_parse_typedef_callback() {
        let content = "typedef VoidCallback = void Function();\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let td = symbols.iter().find(|s| s.name == "VoidCallback").unwrap();
        assert_eq!(td.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_parse_function() {
        let content = "void main() {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "main" && s.kind == SymbolKind::Function),
            "Should find main function, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_async_function() {
        let content = "Future<int> fetchData() async {\n  return 0;\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "fetchData" && s.kind == SymbolKind::Function),
            "Should find fetchData function, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_arrow_function() {
        let content = "String formatName(String first, String last) => '$first $last';\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "formatName" && s.kind == SymbolKind::Function),
            "Should find formatName function, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_getter_setter() {
        let content = r#"class Foo {
  int get count => _count;
  set count(int value) {
    _count = value;
  }
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let getters: Vec<_> = symbols.iter()
            .filter(|s| s.name == "count" && s.kind == SymbolKind::Property)
            .collect();
        assert!(getters.len() >= 1, "should find getter 'count', got: {:?}", symbols);
        let setters: Vec<_> = symbols.iter()
            .filter(|s| s.name == "count" && s.kind == SymbolKind::Property && s.signature.contains("set "))
            .collect();
        assert!(setters.len() >= 1, "should find setter 'count', got: {:?}", symbols);
    }

    #[test]
    fn test_parse_constructor() {
        let content = r#"class MyService {
  MyService(this._dep);
  MyService.fromJson(Map<String, dynamic> json) {}
  factory MyService.create() => MyService(Dep());
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "MyService" && s.kind == SymbolKind::Class),
            "Should find class MyService, got: {:?}", symbols);
        // Named constructors
        assert!(symbols.iter().any(|s| s.name == "MyService.fromJson" && s.kind == SymbolKind::Function),
            "Should find MyService.fromJson constructor, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "MyService.create" && s.kind == SymbolKind::Function),
            "Should find MyService.create factory constructor, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_import() {
        let content = "import 'package:flutter/material.dart';\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "material" && s.kind == SymbolKind::Import),
            "Should find import 'material', got: {:?}", symbols);
    }

    #[test]
    fn test_parse_export() {
        let content = "export 'src/my_widget.dart';\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "my_widget" && s.kind == SymbolKind::Import),
            "Should find export 'my_widget', got: {:?}", symbols);
    }

    #[test]
    fn test_parse_dart_async_import() {
        let content = "import 'dart:async';\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "dart:async" && s.kind == SymbolKind::Import),
            "Should find import 'dart:async', got: {:?}", symbols);
    }

    #[test]
    fn test_parse_property() {
        let content = "final String appName = 'MyApp';\nconst int maxRetries = 3;\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "appName" && s.kind == SymbolKind::Property),
            "Should find property appName, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "maxRetries" && s.kind == SymbolKind::Property),
            "Should find property maxRetries, got: {:?}", symbols);
    }

    #[test]
    fn test_comments_ignored() {
        let content = r#"
// class FakeClass {
/* class AnotherFake { */
class RealClass {
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(!symbols.iter().any(|s| s.name == "FakeClass"),
            "Should not find FakeClass in comments");
        assert!(!symbols.iter().any(|s| s.name == "AnotherFake"),
            "Should not find AnotherFake in comments");
        assert!(symbols.iter().any(|s| s.name == "RealClass" && s.kind == SymbolKind::Class),
            "Should find RealClass, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_method_inside_class() {
        let content = r#"class ApiService {
  Future<void> init() async {}
  void doSomething() {}
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "init" && s.kind == SymbolKind::Function),
            "Should find method init, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "doSomething" && s.kind == SymbolKind::Function),
            "Should find method doSomething, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_method_inside_extension() {
        let content = r#"extension ApiServiceX on ApiService {
  void ping() {}
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "ApiServiceX" && s.kind == SymbolKind::Object),
            "Should find extension ApiServiceX, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "ping" && s.kind == SymbolKind::Function),
            "Should find method ping, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_method_inside_mixin() {
        let content = r#"mixin LoggerMixin on Object {
  void log(String msg) {}
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "LoggerMixin" && s.kind == SymbolKind::Interface),
            "Should find mixin LoggerMixin, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "log" && s.kind == SymbolKind::Function),
            "Should find method log, got: {:?}", symbols);
    }

    #[test]
    fn test_full_dart_file() {
        let content = r#"
import 'package:flutter/material.dart';
import 'dart:async';

typedef JsonMap = Map<String, dynamic>;

const String appVersion = '1.0.0';

mixin LoggerMixin on Object {
  void log(String msg) {}
}

abstract class BaseService {
  Future<void> init();
}

class ApiService extends BaseService with LoggerMixin implements Disposable {
  final String baseUrl;

  ApiService(this.baseUrl);

  ApiService.withDefault() : baseUrl = 'https://api.example.com';

  factory ApiService.create() => ApiService.withDefault();

  Future<void> init() async {}

  String get endpoint => '$baseUrl/v1';

  set timeout(int value) {}
}

extension ApiServiceX on ApiService {
  void ping() {}
}

enum Status {
  loading,
  success,
  error,
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();

        // Imports
        assert!(symbols.iter().any(|s| s.name == "material" && s.kind == SymbolKind::Import),
            "Should find import 'material', got: {:?}", symbols);

        // Typedef
        assert!(symbols.iter().any(|s| s.name == "JsonMap" && s.kind == SymbolKind::TypeAlias),
            "Should find typedef JsonMap, got: {:?}", symbols);

        // Property
        assert!(symbols.iter().any(|s| s.name == "appVersion" && s.kind == SymbolKind::Property),
            "Should find property appVersion, got: {:?}", symbols);

        // Mixin
        let mixin = symbols.iter().find(|s| s.name == "LoggerMixin").unwrap();
        assert_eq!(mixin.kind, SymbolKind::Interface);

        // Abstract class
        let base = symbols.iter().find(|s| s.name == "BaseService").unwrap();
        assert_eq!(base.kind, SymbolKind::Class);

        // Class with full inheritance
        let api = symbols.iter().find(|s| s.name == "ApiService" && s.kind == SymbolKind::Class).unwrap();
        assert!(api.parents.iter().any(|(p, k)| p == "BaseService" && k == "extends"),
            "Expected extends BaseService, got: {:?}", api.parents);
        assert!(api.parents.iter().any(|(p, k)| p == "LoggerMixin" && k == "with"),
            "Expected with LoggerMixin, got: {:?}", api.parents);
        assert!(api.parents.iter().any(|(p, k)| p == "Disposable" && k == "implements"),
            "Expected implements Disposable, got: {:?}", api.parents);

        // Constructors
        assert!(symbols.iter().any(|s| s.name == "ApiService.withDefault" && s.kind == SymbolKind::Function),
            "Should find constructor ApiService.withDefault, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "ApiService.create" && s.kind == SymbolKind::Function),
            "Should find factory ApiService.create, got: {:?}", symbols);

        // Getter/Setter
        assert!(symbols.iter().any(|s| s.name == "endpoint" && s.kind == SymbolKind::Property),
            "Should find getter endpoint, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "timeout" && s.kind == SymbolKind::Property),
            "Should find setter timeout, got: {:?}", symbols);

        // Extension
        let ext = symbols.iter().find(|s| s.name == "ApiServiceX").unwrap();
        assert_eq!(ext.kind, SymbolKind::Object);
        assert!(ext.parents.iter().any(|(p, k)| p == "ApiService" && k == "extends"),
            "Expected extends ApiService, got: {:?}", ext.parents);

        // Enum
        assert!(symbols.iter().any(|s| s.name == "Status" && s.kind == SymbolKind::Enum),
            "Should find enum Status, got: {:?}", symbols);

        // Function inside class
        assert!(symbols.iter().any(|s| s.name == "init" && s.kind == SymbolKind::Function),
            "Should find method init, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_class_with_generics() {
        let content = "class Repository<T extends Model> implements BaseRepo<T> {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "Repository" && s.kind == SymbolKind::Class).unwrap();
        assert!(cls.parents.iter().any(|(p, k)| p == "BaseRepo" && k == "implements"),
            "Expected implements BaseRepo, got: {:?}", cls.parents);
    }

    #[test]
    fn test_parse_base_class() {
        let content = "base class BaseModel {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "BaseModel").expect(
            &format!("Should find base class BaseModel, got: {:?}", symbols));
        assert_eq!(cls.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_final_class() {
        let content = "final class FinalModel {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "FinalModel").expect(
            &format!("Should find final class FinalModel, got: {:?}", symbols));
        assert_eq!(cls.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_mixin_class() {
        let content = "mixin class MixinClass {\n}\n";
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "MixinClass").expect(
            &format!("Should find mixin class MixinClass, got: {:?}", symbols));
        assert_eq!(cls.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_multiple_imports() {
        let content = r#"
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
export 'src/utils.dart';
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "material" && s.kind == SymbolKind::Import),
            "Should find material import, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "provider" && s.kind == SymbolKind::Import),
            "Should find provider import, got: {:?}", symbols);
        assert!(symbols.iter().any(|s| s.name == "utils" && s.kind == SymbolKind::Import),
            "Should find utils export, got: {:?}", symbols);
    }

    #[test]
    fn test_parse_class_multiline() {
        let content = r#"class _AppScopeContainer extends AppScopeContainer
    with _AppScopeDeps, _AppScopeInitializeQueue, _PublicAppScopeImpl {
}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        let cls = symbols.iter().find(|s| s.name == "_AppScopeContainer" && s.kind == SymbolKind::Class).unwrap();
        assert!(cls.parents.iter().any(|(p, k)| p == "AppScopeContainer" && k == "extends"),
            "should have AppScopeContainer as extends, got: {:?}", cls.parents);
        assert!(cls.parents.iter().any(|(p, k)| p == "_AppScopeDeps" && k == "with"),
            "should have _AppScopeDeps as with, got: {:?}", cls.parents);
        assert!(cls.parents.iter().any(|(p, k)| p == "_AppScopeInitializeQueue" && k == "with"),
            "should have _AppScopeInitializeQueue as with, got: {:?}", cls.parents);
        assert!(cls.parents.iter().any(|(p, k)| p == "_PublicAppScopeImpl" && k == "with"),
            "should have _PublicAppScopeImpl as with, got: {:?}", cls.parents);
    }

    #[test]
    fn test_parse_top_level_getter_setter() {
        let content = r#"
String get appName => 'MyApp';
set appName(String value) {}
"#;
        let symbols = DART_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "appName" && s.kind == SymbolKind::Property),
            "Should find top-level getter appName, got: {:?}", symbols);
    }
}
