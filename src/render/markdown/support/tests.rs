use super::*;

#[test]
fn quote_arg_wraps_whitespace_and_escapes_quotes() {
    assert_eq!(quote_arg("BRAF"), "BRAF");
    assert_eq!(quote_arg("BRAF V600E"), "\"BRAF V600E\"");
    assert_eq!(quote_arg("BRAF \"V600E\""), "\"BRAF \\\"V600E\\\"\"");
}

#[test]
fn discover_try_line_quotes_shell_sensitive_queries() {
    assert_eq!(
        discover_try_line("ERBB1\"alias", "resolve abbreviations and synonyms"),
        "Try: biomcp discover \"ERBB1\\\"alias\"   - resolve abbreviations and synonyms"
    );
    assert_eq!(
        discover_try_line("BRAF $(touch marker)", "resolve abbreviations and synonyms"),
        "Try: biomcp discover \"BRAF \\$(touch marker)\"   - resolve abbreviations and synonyms"
    );
    assert_eq!(
        discover_try_line("BRAF V600E", "resolve abbreviations and synonyms"),
        "Try: biomcp discover \"BRAF V600E\"   - resolve abbreviations and synonyms"
    );
}
