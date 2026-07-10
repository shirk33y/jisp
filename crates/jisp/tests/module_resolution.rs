use std::fs;
use std::path::PathBuf;

#[test]
fn check_types_resolves_file_imports() {
    let dir = fixture_dir("file-imports");
    let main = dir.join("main.lisp");
    fs::write(
        dir.join("math.lisp"),
        "(export inc (fn (value) (+ value 1)))",
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math.lisp")
(export main (fn () (math.inc 41)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();

    jisp::check(&main, &text).unwrap();
}

#[test]
fn check_types_resolves_directory_imports_with_mixed_syntax() {
    let dir = fixture_dir("directory-imports");
    let module_dir = dir.join("math");
    fs::create_dir_all(&module_dir).unwrap();
    let main = dir.join("main.lisp");
    fs::write(
        module_dir.join("inc.lisp"),
        "(export inc (fn (value) (+ value 1)))",
    )
    .unwrap();
    fs::write(
        module_dir.join("dec.json"),
        r#"[["export","dec",["fn",["value"],["-","value",1]]]]"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.dec (math.inc 41))))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();

    jisp::check(&main, &text).unwrap();
}

#[test]
fn check_types_rejects_import_cycles() {
    let dir = fixture_dir("import-cycles");
    let main = dir.join("main.lisp");
    fs::write(
        dir.join("a.lisp"),
        r#"
(import b "b.lisp")
(export value b.value)
"#,
    )
    .unwrap();
    fs::write(
        dir.join("b.lisp"),
        r#"
(import a "a.lisp")
(export value a.value)
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import a "a.lisp")
(export main a.value)
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let err = match jisp::check(&main, &text) {
        Ok(_) => panic!("expected import cycle"),
        Err(err) => err,
    };

    assert!(matches!(err, jisp::Error::ImportCycle(_)), "{err}");
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-import-fixtures")
        .join(format!("{}-{}", name, std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}
