use crate::lsp_definition;

#[test]
fn definition_resolves_a_local_top_level_name() {
    let text = "(def answer 42)\n(export main (fn () (+ answer 1)))\n";
    let definition = lsp_definition("file:///main.lisp", text, 1, 24).unwrap();

    assert_eq!(definition["uri"], "file:///main.lisp");
    assert_eq!(definition["range"]["start"]["line"], 0);
    assert_eq!(definition["range"]["start"]["character"], 0);
}

#[test]
fn definition_ignores_unknown_and_non_name_symbols() {
    let text = "(export main (fn () 42))\n";

    assert!(lsp_definition("file:///main.lisp", text, 0, 1).is_none());
    assert!(lsp_definition("file:///main.lisp", text, 0, 19).is_none());
}
