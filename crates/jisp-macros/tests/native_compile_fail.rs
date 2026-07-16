use std::{env, fs, path::PathBuf, process::Command};

#[test]
fn ui_html_native_values_fail_during_downstream_macro_expansion() {
    assert_downstream_compile_fails(
        "ui-html",
        "tests/fixtures/unsupported_ui_html.lisp",
        "calls outside native module",
    );
}

#[test]
fn heterogeneous_dynamic_object_access_fails_during_downstream_macro_expansion() {
    assert_downstream_compile_fails(
        "heterogeneous-dynamic-object",
        "../../examples/collection-toolbox/unsupported.lisp",
        "expected static field or homogeneous closed object",
    );
}

#[test]
fn polymorphic_open_row_definition_fails_during_downstream_macro_expansion() {
    assert_downstream_compile_fails(
        "open-row-polymorphism",
        "../../examples/collection-toolbox/open-row.lisp",
        "does not support polymorphic definition `score-of`",
    );
}

fn assert_downstream_compile_fails(name: &str, source: &str, expected: &str) {
    let crate_dir = fixture_dir(name);
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(source)
        .canonicalize()
        .unwrap();
    let macros_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = macros_dir.join("../../target");

    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"jisp-macro-compile-fail\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\njisp-macros = {{ path = {macros_dir:?} }}\n"
        ),
    )
    .unwrap();
    fs::create_dir_all(crate_dir.join("src")).unwrap();
    fs::write(
        crate_dir.join("src/main.rs"),
        format!("jisp_macros::lisp_file!({fixture:?});\n\nfn main() {{}}\n"),
    )
    .unwrap();

    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(&crate_dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .args(["check", "--offline", "--quiet"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "the downstream crate unexpectedly compiled"
    );
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        diagnostics.contains("failed to generate native Rust for Jisp source"),
        "{diagnostics}"
    );
    assert!(diagnostics.contains(expected), "{diagnostics}");
    assert!(
        diagnostics.contains(&fixture.display().to_string()),
        "{diagnostics}"
    );
}

#[test]
fn bigint_native_values_compile_in_downstream_macro_crates() {
    let crate_dir = fixture_dir("bigint");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bigint.lisp")
        .canonicalize()
        .unwrap();
    let macros_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = macros_dir.join("../../target");

    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"jisp-macro-bigint\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\njisp-macros = {{ path = {macros_dir:?} }}\nnum-bigint = \"0.4\"\n"
        ),
    )
    .unwrap();
    fs::create_dir_all(crate_dir.join("src")).unwrap();
    fs::write(
        crate_dir.join("src/main.rs"),
        format!(
            "jisp_macros::lisp_file!({fixture:?});\n\nfn main() {{\n    assert_eq!(entry().to_string(), \"9223372036854775810\");\n}}\n"
        ),
    )
    .unwrap();

    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(&crate_dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .args(["run", "--offline", "--quiet"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-macro-compile-fail")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}
