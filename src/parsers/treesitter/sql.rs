//! Tree-sitter based SQL parser

// Force linking of tree_sitter_sql crate which provides the compiled C grammar.
// Without this, the linker won't include the static library containing tree_sitter_sql().
extern crate tree_sitter_sql;

use anyhow::Result;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

// Declare the C function from tree-sitter-sql grammar.
// We use our own declaration to get *const TSLanguage (v0.26 type)
// instead of going through the crate's Rust wrapper which returns
// tree-sitter v0.19's Language type.
unsafe extern "C" {
    fn tree_sitter_sql() -> *const tree_sitter::ffi::TSLanguage;
}

fn sql_language() -> Language {
    unsafe { Language::from_raw(tree_sitter_sql()) }
}

static SQL_LANGUAGE: LazyLock<Language> = LazyLock::new(sql_language);

static SQL_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&SQL_LANGUAGE, include_str!("queries/sql.scm"))
        .expect("Failed to compile SQL tree-sitter query")
});

pub static SQL_PARSER: SqlParser = SqlParser;

pub struct SqlParser;

impl LanguageParser for SqlParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &SQL_LANGUAGE)?;
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let query = &*SQL_QUERY;

        // Build capture name -> index map
        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names.iter().position(|n| *n == name).map(|i| i as u32)
        };

        let idx_table_name = idx("table_name");
        let idx_func_name = idx("func_name");
        let idx_index_name = idx("index_name");
        let idx_type_name = idx("type_name");
        let idx_domain_name = idx("domain_name");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // CREATE TABLE
            if let Some(cap) = find_capture(m, idx_table_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // CREATE FUNCTION / PROCEDURE
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

            // CREATE INDEX
            if let Some(cap) = find_capture(m, idx_index_name) {
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

            // CREATE TYPE
            if let Some(cap) = find_capture(m, idx_type_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // CREATE DOMAIN
            if let Some(cap) = find_capture(m, idx_domain_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
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
    fn test_create_table() {
        let content = "CREATE TABLE users (\n    id INT PRIMARY KEY,\n    name VARCHAR(255)\n);\n";
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "users" && s.kind == SymbolKind::Class),
            "Expected table 'users' as Class, got: {:?}", symbols
        );
    }

    #[test]
    fn test_create_function() {
        let content = r#"CREATE FUNCTION get_user_count() RETURNS INTEGER AS $$
BEGIN
    RETURN (SELECT COUNT(*) FROM users);
END;
$$ LANGUAGE plpgsql;
"#;
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "get_user_count" && s.kind == SymbolKind::Function),
            "Expected function 'get_user_count' as Function, got: {:?}", symbols
        );
    }

    #[test]
    fn test_create_index() {
        let content = "CREATE INDEX idx_users_name ON users (name);\n";
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "idx_users_name" && s.kind == SymbolKind::Property),
            "Expected index 'idx_users_name' as Property, got: {:?}", symbols
        );
    }

    #[test]
    fn test_create_type() {
        let content = "CREATE TYPE address AS (street VARCHAR, city VARCHAR);\n";
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "address" && s.kind == SymbolKind::Class),
            "Expected type 'address' as Class, got: {:?}", symbols
        );
    }

    #[test]
    fn test_create_domain() {
        let content = "CREATE DOMAIN positive_int AS INT CHECK (VALUE > 0);\n";
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "positive_int" && s.kind == SymbolKind::Class),
            "Expected domain 'positive_int' as Class, got: {:?}", symbols
        );
    }

    #[test]
    fn test_comments_ignored() {
        let content = r#"-- CREATE TABLE fake_table (id INT);
/* CREATE TABLE fake_table2 (id INT); */
CREATE TABLE real_table (id INT);
"#;
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "real_table"),
            "Expected 'real_table', got: {:?}", symbols
        );
        assert!(
            !symbols.iter().any(|s| s.name == "fake_table"),
            "Should not find 'fake_table' in comments, got: {:?}", symbols
        );
        assert!(
            !symbols.iter().any(|s| s.name == "fake_table2"),
            "Should not find 'fake_table2' in comments, got: {:?}", symbols
        );
    }

    #[test]
    fn test_multiple_statements() {
        let content = r#"CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INT
);

CREATE INDEX idx_orders_user ON orders (user_id);

CREATE FUNCTION count_orders() RETURNS INTEGER AS $$
BEGIN
    RETURN (SELECT COUNT(*) FROM orders);
END;
$$ LANGUAGE plpgsql;
"#;
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "orders" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "idx_orders_user" && s.kind == SymbolKind::Property));
        assert!(symbols.iter().any(|s| s.name == "count_orders" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_unique_index() {
        let content = "CREATE UNIQUE INDEX idx_email ON users (email);\n";
        let symbols = SQL_PARSER.parse_symbols(content).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "idx_email" && s.kind == SymbolKind::Property),
            "Expected unique index 'idx_email' as Property, got: {:?}", symbols
        );
    }
}
