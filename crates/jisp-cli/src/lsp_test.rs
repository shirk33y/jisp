use std::path::Path;

use crate::{lsp_definition, lsp_diagnostics, lsp_hover, remapped_cargo_errors};

#[test]
fn diagnostics_keep_jisp_codes_and_utf16_ranges() {
    let text = "(export main (fn () \"🙂\" \"unterminated";
    let diagnostics = lsp_diagnostics("file:///unicode.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["source"], "jisp");
    assert_eq!(diagnostics[0]["code"], "JISP-L003");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(
        diagnostics[0]["range"]["start"]["character"],
        "(export main (fn () \"🙂\" ".encode_utf16().count()
    );
}

#[test]
fn diagnostics_keep_parser_codes_and_eof_ranges() {
    let text = "\"unterminated";
    let diagnostics = lsp_diagnostics("file:///unterminated.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["source"], "jisp");
    assert_eq!(diagnostics[0]["code"], "JISP-L003");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 0);
    assert_eq!(
        diagnostics[0]["range"]["end"]["character"],
        text.encode_utf16().count()
    );
}

#[test]
fn diagnostics_keep_multiline_ranges_after_unicode_text() {
    let text = "(def label \"🙂\")\n(export main\n  (fn ()\n    (+ 1 true)))\n";
    let diagnostics = lsp_diagnostics("file:///multiline-unicode.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "JISP-TYPE");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 3);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 4);
}

#[test]
fn native_remapping_ignores_errors_outside_generated_rust() {
    let generated = jisp::emit_rust_detailed("main.lisp", "(export main (fn () 1))").unwrap();
    let cargo_json = r#"{
        "reason":"compiler-message",
        "message":{
            "level":"error",
            "message":"foreign compiler error",
            "spans":[{
                "is_primary":true,
                "file_name":"dependency.rs",
                "byte_start":0
            }]
        }
    }"#;

    assert!(remapped_cargo_errors(cargo_json, &generated, Path::new("src/lib.rs")).is_empty());
}

#[test]
fn hover_reports_an_inferred_top_level_type() {
    let text = "(def answer 42)\n(export main (fn () answer))\n";
    let hover = lsp_hover("file:///main.lisp", text, 0, 6).unwrap();

    assert_eq!(hover["contents"]["kind"], "markdown");
    assert_eq!(hover["contents"]["value"], "**answer** — `int`");
}

#[test]
fn definition_resolves_a_local_top_level_name() {
    let text = "(def answer 42)\n(export main (fn () (+ answer 1)))\n";
    let definition = lsp_definition("file:///main.lisp", text, 1, 24).unwrap();

    assert_eq!(definition["uri"], "file:///main.lisp");
    assert_eq!(definition["range"]["start"]["line"], 0);
    assert_eq!(definition["range"]["start"]["character"], 5);
}

#[test]
fn definition_resolves_lambda_and_sequential_let_bindings() {
    let text =
        "(export main (fn (value) (let (offset 1 total (+ value offset)) (+ total value))))\n";

    let parameter = lsp_definition("file:///main.lisp", text, 0, 52).unwrap();
    let offset = lsp_definition("file:///main.lisp", text, 0, 58).unwrap();
    let total = lsp_definition("file:///main.lisp", text, 0, 70).unwrap();

    assert_eq!(parameter["range"]["start"]["character"], 18);
    assert_eq!(offset["range"]["start"]["character"], 31);
    assert_eq!(total["range"]["start"]["character"], 40);
}

#[test]
fn definition_resolves_case_pattern_bindings() {
    let text = "(export main (fn () (case (some 1) ((some value) (+ value 1)))))\n";
    let use_offset = text.rfind("value").unwrap();
    let declaration = lsp_definition("file:///main.lisp", text, 0, use_offset).unwrap();

    assert_eq!(
        declaration["range"]["start"]["character"],
        text.find("value").unwrap()
    );
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

#[test]
fn definition_resolves_an_imported_macro() {
    let directory =
        std::env::temp_dir().join(format!("jisp-lsp-macro-definition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let macros = directory.join("macros.lisp");
    std::fs::write(&macros, "(def wrap (~ (fn (value) `(+ ,value 1))))\n").unwrap();
    let text = "(macro-import m \"macros.lisp\")\n(export main (fn () (m.wrap 41)))\n";
    std::fs::write(&main, text).unwrap();

    let uri = format!("file://{}", main.display());
    let position = text.lines().nth(1).unwrap().find("m.wrap").unwrap() + 2;
    let definition = lsp_definition(&uri, text, 1, position).unwrap();

    assert_eq!(definition["uri"], format!("file://{}", macros.display()));
    assert_eq!(definition["range"]["start"]["line"], 0);
    let _ = std::fs::remove_dir_all(&directory);
}
