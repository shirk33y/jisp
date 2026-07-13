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

#[test]
fn definition_resolves_a_qualified_import() {
    let directory =
        std::env::temp_dir().join(format!("jisp-lsp-definition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let math = directory.join("math.lisp");
    std::fs::write(&math, "(export increment (fn (value) (+ value 1)))\n").unwrap();
    let text = "(import math \"math\")\n(export main (fn () (math.increment 41)))\n";
    std::fs::write(&main, text).unwrap();

    let uri = format!("file://{}", main.display());
    let definition = lsp_definition(&uri, text, 1, 29).unwrap();

    assert_eq!(definition["uri"], format!("file://{}", math.display()));
    assert_eq!(definition["range"]["start"]["line"], 0);
    let _ = std::fs::remove_dir_all(&directory);
}
