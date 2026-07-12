use std::{env, fs, path::PathBuf, process::Command};

#[test]
fn variadic_native_functions_fail_during_downstream_macro_expansion() {
    let crate_dir = fixture_dir("variadic-function");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/unsupported_first_class_call.lisp")
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
    assert!(
        diagnostics.contains("native variadic functions"),
        "{diagnostics}"
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
