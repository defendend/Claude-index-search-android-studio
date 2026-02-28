fn main() {
    // Compile tree-sitter-bsl C parser
    cc::Build::new()
        .include("tree-sitter-bsl/src")
        .file("tree-sitter-bsl/src/parser.c")
        .warnings(false)
        .compile("tree-sitter-bsl");
}
