use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{generate_file, import_dependencies};

#[test]
fn import_dependencies_include_transitive_source_files() {
    let dir = fixture_dir("macro-import-dependencies");
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
(export main (fn () app.answer))
"#,
    )
    .unwrap();

    let dependencies = dependency_set(import_dependencies(&main).unwrap());

    assert_eq!(
        dependencies,
        BTreeSet::from([canonical(&app), canonical(&math)])
    );
}

#[test]
fn generate_file_emits_native_tokens_without_value_fallback() {
    let dir = fixture_dir("macro-native-codegen");
    let main = dir.join("main.lisp");
    fs::write(
        &main,
        r#"
(def answer (fn () 42))
(export entry (fn () (answer)))
"#,
    )
    .unwrap();

    let generated = generate_file(&main).unwrap();
    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("fn answer () -> i64"));
    assert!(tokens.contains("pub fn entry () -> i64"));
    assert!(tokens.contains("answer ()"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

fn dependency_set(paths: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    paths.into_iter().collect()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap()
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-macro-fixtures")
        .join(format!("{}-{}", name, std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}
