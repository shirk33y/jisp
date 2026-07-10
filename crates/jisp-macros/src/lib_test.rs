use std::{collections::BTreeSet, fs, path::PathBuf};

use crate::import_dependencies;

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

fn dependency_set(paths: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    paths.into_iter().collect()
}

fn canonical(path: &PathBuf) -> PathBuf {
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
