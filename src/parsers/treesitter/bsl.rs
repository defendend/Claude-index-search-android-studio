//! Tree-sitter based BSL (1C:Enterprise) parser

use anyhow::Result;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};
use std::sync::LazyLock;

use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;
use super::{LanguageParser, parse_tree, node_text, node_line, line_text};

// Link the tree-sitter-bsl C library (compiled via build.rs)
unsafe extern "C" {
    fn tree_sitter_bsl() -> *const tree_sitter::ffi::TSLanguage;
}

fn bsl_language() -> Language {
    unsafe { Language::from_raw(tree_sitter_bsl()) }
}

static BSL_LANGUAGE: LazyLock<Language> = LazyLock::new(bsl_language);

static BSL_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&BSL_LANGUAGE, include_str!("queries/bsl.scm"))
        .expect("Failed to compile BSL tree-sitter query")
});

pub static BSL_PARSER: BslParser = BslParser;

pub struct BslParser;

impl LanguageParser for BslParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &BSL_LANGUAGE)?;
        let mut symbols = Vec::new();
        let query = &*BSL_QUERY;
        let mut cursor = QueryCursor::new();

        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names.iter().position(|n| *n == name).map(|i| i as u32)
        };

        let idx_proc_name = idx("proc_name");
        let idx_func_name = idx("func_name");
        let idx_var_name = idx("var_name");
        let idx_region_name = idx("region_name");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // Procedure
            if let Some(cap) = find_capture(m, idx_proc_name) {
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

            // Variable
            if let Some(cap) = find_capture(m, idx_var_name) {
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

            // Region (as package/namespace grouping)
            if let Some(cap) = find_capture(m, idx_region_name) {
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
    fn test_parse_procedure_ru() {
        let content = "Процедура МояПроцедура()\nКонецПроцедуры\n";
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "МояПроцедура" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_function_en() {
        let content = "Function GetData() Export\n    Return 42;\nEndFunction\n";
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "GetData" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_procedure_en() {
        let content = "Procedure DoWork(Param1)\n    // work\nEndProcedure\n";
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "DoWork" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_parse_variable() {
        let content = "Перем МояПеременная;\n";
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "МояПеременная" && s.kind == SymbolKind::Property));
    }

    #[test]
    fn test_parse_region() {
        let content = "#Область ОбработчикиСобытий\n#КонецОбласти\n";
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        assert!(symbols.iter().any(|s| s.name == "ОбработчикиСобытий" && s.kind == SymbolKind::Package));
    }

    #[test]
    fn test_parse_complex_module() {
        let content = r#"
Перем МодульнаяПеременная;

#Область ОбработчикиСобытий

Процедура ПриСозданииНаСервере(Отказ, СтандартнаяОбработка)
КонецПроцедуры

Функция ПолучитьДанные() Экспорт
    Возврат 42;
КонецФункции

#КонецОбласти
"#;
        let symbols = BSL_PARSER.parse_symbols(content).unwrap();
        let funcs: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Function).collect();
        let props: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Property).collect();
        let pkgs: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Package).collect();
        assert_eq!(funcs.len(), 2);
        assert!(props.len() >= 1);
        assert_eq!(pkgs.len(), 1);
    }
}
