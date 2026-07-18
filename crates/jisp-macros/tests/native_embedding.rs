use std::{env, fs, path::PathBuf, process::Command};

#[test]
fn documented_manifest_matches_the_executable_downstream_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let docs = fs::read_to_string(root.join("docs/RUST_EMBEDDING.md")).unwrap();
    let manifest =
        fs::read_to_string(fixture_dir("downstream-embedding").join("Cargo.toml")).unwrap();
    let dependencies = manifest.split_once("[dependencies]\n").unwrap().1.trim();

    assert!(docs.contains(dependencies));
}

#[test]
fn documented_downstream_embedding_builds_and_runs_offline() {
    let (_, output) = downstream_command("downstream-embedding", ["run", "--offline", "--quiet"]);

    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn documented_downstream_failure_reports_a_ranged_jisp_diagnostic() {
    let (fixture, output) = downstream_command(
        "downstream-embedding-fail",
        ["check", "--offline", "--quiet"],
    );
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !output.status.success(),
        "the downstream crate unexpectedly compiled"
    );
    assert!(
        diagnostics.contains("failed to generate native Rust for Jisp source"),
        "{diagnostics}"
    );
    assert!(diagnostics.contains("JISP-TYPE"), "{diagnostics}");
    assert!(
        diagnostics.contains(&format!(
            "--> {}:7:7",
            fixture.join("src/invalid.lisp").display()
        )),
        "{diagnostics}"
    );
}

fn downstream_command<const N: usize>(
    fixture: &str,
    args: [&str; N],
) -> (PathBuf, std::process::Output) {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let directory = copied_fixture_dir(fixture);
    let output = Command::new(cargo)
        .current_dir(&directory)
        .env(
            "CARGO_TARGET_DIR",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"),
        )
        .args(args)
        .output()
        .unwrap();
    (directory, output)
}

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"))
}

fn copied_fixture_dir(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-downstream-embedding")
        .join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).unwrap();
    }
    copy_dir(&fixture_dir(name), &directory);
    let manifest = directory.join("Cargo.toml");
    let source = fs::read_to_string(&manifest).unwrap();
    let macros_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        manifest,
        source.replace("path = \"../../..\"", &format!("path = {macros_dir:?}")),
    )
    .unwrap();
    directory.canonicalize().unwrap()
}

fn copy_dir(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
