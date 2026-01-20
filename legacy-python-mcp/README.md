# Legacy Python MCP Implementation

This folder contains the original Python implementation of kotlin-index (v1.0.0 - v2.5.2).

**Status**: Archived. Use Rust CLI (v3.0.0+) instead.

## Why Archived?

The Rust implementation provides:
- **26 of 33 commands faster** than Python
- **Top speedups**: imports (260x), dependents (100x), deps/class (90x)
- **Smaller binary**: ~4.4 MB vs ~273 MB (Python venv)
- **More complete index**: 900K+ references vs 438K

## Structure

```
src/kotlin_index/
├── cli.py              # CLI (typer)
├── server.py           # MCP server (FastMCP)
├── db/
│   ├── database.py     # SQLite
│   └── schema.py       # DB schema
└── indexer/
    ├── file_indexer.py    # File indexing
    ├── module_indexer.py  # Gradle parsing
    └── symbol_indexer.py  # Kotlin/Java AST (tree-sitter)
```

## Running (if needed)

```bash
cd legacy-python-mcp
pip install -r requirements.txt
python -m kotlin_index --help
```

## Changelog (v1.0.0 - v2.5.2)

### v2.5.2
- Project-specific databases: Each project now has its own index database

### v2.5.1
- Use ripgrep for 10-15x faster grep-based searches

### v2.5.0
- Add `composables`, `previews`, `suspend`, `flows` commands

### v2.4.1
- Fix `callers`, `outline`, `api` commands

### v2.4.0
- Add `todo`, `deprecated`, `suppress`, `extensions`, `api`, `deeplinks` commands

### v2.3.0
- Add `callers`, `imports`, `provides`, `inject` commands

### v2.2.0
- Add `hierarchy`, `annotations`, `changed` commands

### v2.1.0
- Fix `class` command, add `update` command

### v2.0.0
- pip package, CLI with typer + rich, Skill for Claude Code

### v1.2.0
- Java support (tree-sitter-java), Find Usages, Find Implementations

### v1.1.0
- Incremental indexing, better module detection

### v1.0.0
- Initial release: File/symbol/module search, MCP server
