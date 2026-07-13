use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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

    let checked = jisp::check(&main, &text).unwrap();
    assert!(checked
        .resolved_modules
        .contains_key(&canonical(&dir.join("math.lisp"))));
}

#[test]
fn run_main_resolves_file_imports() {
    let dir = fixture_dir("runtime-file-imports");
    let main = dir.join("main.lisp");
    fs::write(
        dir.join("math.lisp"),
        "(export inc (fn (value) (+ value 1)))",
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.inc 41)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let value = jisp::run_main(&main, &text).unwrap();

    assert_int(value, 42);
}

#[test]
fn run_main_resolves_local_package_dependencies() {
    let dir = fixture_dir("package-dependencies");
    let app = dir.join("app");
    let math = dir.join("math");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&math).unwrap();
    let main = app.join("main.lisp");
    fs::write(
        app.join("jisp.toml"),
        "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { path = \"../math\" }\n",
    )
    .unwrap();
    fs::write(
        math.join("main.lisp"),
        "(export inc (fn (value) (+ value 1)))",
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.inc 41)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let value = jisp::run_main(&main, &text).unwrap();

    assert_int(value, 42);
}

#[test]
fn sibling_module_wins_over_package_dependency_with_the_same_name() {
    let dir = fixture_dir("package-dependency-shadowing");
    let app = dir.join("app");
    let dependency = dir.join("math");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&dependency).unwrap();
    let main = app.join("main.lisp");
    fs::write(
        app.join("jisp.toml"),
        "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { path = \"../math\" }\n",
    )
    .unwrap();
    fs::write(dependency.join("main.lisp"), "(export value (fn () 1000))").unwrap();
    fs::write(app.join("math.lisp"), "(export value (fn () 42))").unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.value)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let checked = jisp::check(&main, &text).unwrap();
    let value = jisp::run_main(&main, &text).unwrap();

    assert_int(value, 42);
    assert!(checked
        .resolved_modules
        .contains_key(&canonical(&app.join("math.lisp"))));
    assert!(!checked
        .resolved_modules
        .contains_key(&canonical(&dependency.join("main.lisp"))));
}

#[test]
fn type_errors_in_imports_render_the_imported_source() {
    let dir = fixture_dir("imported-type-diagnostics");
    let main = dir.join("main.lisp");
    let broken = dir.join("broken.lisp");
    fs::write(&broken, "(export value (+ 1 true))").unwrap();
    fs::write(
        &main,
        r#"
(import broken "broken.lisp")
(export main (fn () broken.value))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let error = match jisp::check_detailed(&main, &text) {
        Ok(_) => panic!("expected an imported type error"),
        Err(error) => error,
    };

    let rendered = error.render_diagnostics().unwrap();
    assert!(
        rendered.contains(&format!("--> {}:1:15", canonical(&broken).display())),
        "{rendered}"
    );
    assert!(rendered.contains("no overload of `+`"), "{rendered}");
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
        module_dir.join("double.yaml"),
        r#"[[export, double, [fn, [value], [*, value, 2]]]]"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.dec (math.double (math.inc 41)))))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();

    jisp::check(&main, &text).unwrap();
}

#[test]
fn run_main_resolves_directory_imports_with_mixed_syntax() {
    let dir = fixture_dir("runtime-directory-imports");
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
        module_dir.join("double.yaml"),
        r#"[[export, double, [fn, [value], [*, value, 2]]]]"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.dec (math.double (math.inc 41)))))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let value = jisp::run_main(&main, &text).unwrap();

    assert_int(value, 83);
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

    let err = match jisp::run_main(&main, &text) {
        Ok(_) => panic!("expected import cycle"),
        Err(err) => err,
    };

    assert!(matches!(err, jisp::Error::ImportCycle(_)), "{err}");
}

#[test]
fn imports_expose_only_exported_names() {
    let dir = fixture_dir("private-imports");
    let main = dir.join("main.lisp");
    fs::write(
        dir.join("math.lisp"),
        r#"
(def hidden 41)
(export visible 1)
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math.lisp")
(export main (fn () math.hidden))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let err = match jisp::check(&main, &text) {
        Ok(_) => panic!("expected private import type error"),
        Err(err) => err,
    };
    assert!(
        matches!(
            err,
            jisp::Error::Type(jisp::jisp_types::InferError::Located { ref error, .. })
                if matches!(error.as_ref(), jisp::jisp_types::InferError::UnknownName(name) if name == "math.hidden")
        ),
        "{err}"
    );

    let err = match jisp::run_main(&main, &text) {
        Ok(_) => panic!("expected private import type error"),
        Err(err) => err,
    };
    assert!(
        matches!(
            err,
            jisp::Error::Type(jisp::jisp_types::InferError::Located { ref error, .. })
                if matches!(error.as_ref(), jisp::jisp_types::InferError::UnknownName(name) if name == "math.hidden")
        ),
        "{err}"
    );
}

#[test]
fn import_dependencies_include_extensionless_file_imports() {
    let dir = fixture_dir("file-import-dependencies");
    let main = dir.join("main.lisp");
    let math = dir.join("math.lisp");
    fs::write(&math, "(export inc (fn (value) (+ value 1)))").unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.inc 41)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let dependencies = dependency_set(jisp::import_dependencies(&main, &text).unwrap());

    assert_eq!(dependencies, BTreeSet::from([canonical(&math)]));
}

#[test]
fn import_dependencies_include_directory_module_source_files() {
    let dir = fixture_dir("directory-import-dependencies");
    let module_dir = dir.join("math");
    fs::create_dir_all(&module_dir).unwrap();
    let main = dir.join("main.lisp");
    let inc = module_dir.join("inc.lisp");
    let dec = module_dir.join("dec.json");
    let double = module_dir.join("double.yaml");
    fs::write(&inc, "(export inc (fn (value) (+ value 1)))").unwrap();
    fs::write(
        &dec,
        r#"[["export","dec",["fn",["value"],["-","value",1]]]]"#,
    )
    .unwrap();
    fs::write(
        &double,
        r#"[[export, double, [fn, [value], [*, value, 2]]]]"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.dec (math.double (math.inc 41)))))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let dependencies = dependency_set(jisp::import_dependencies(&main, &text).unwrap());

    assert_eq!(
        dependencies,
        BTreeSet::from([canonical(&dec), canonical(&double), canonical(&inc)])
    );
}

#[test]
fn import_dependencies_include_transitive_imports() {
    let dir = fixture_dir("transitive-import-dependencies");
    let main = dir.join("main.lisp");
    let app = dir.join("app.lisp");
    let math = dir.join("math.lisp");
    fs::write(&math, "(export inc (fn (value) (+ value 1)))").unwrap();
    fs::write(
        &app,
        r#"
(import math "math")
(export answer (math.inc 41))
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import app "app")
(export main app.answer)
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let dependencies = dependency_set(jisp::import_dependencies(&main, &text).unwrap());

    assert_eq!(
        dependencies,
        BTreeSet::from([canonical(&app), canonical(&math)])
    );
}

fn assert_int(value: jisp::jisp_eval::Value, expected: i64) {
    match value {
        jisp::jisp_eval::Value::Int(actual) => assert_eq!(actual, expected),
        other => panic!("expected int {expected}, got {}", other.display_string()),
    }
}

fn dependency_set(paths: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    paths.into_iter().collect()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap()
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
