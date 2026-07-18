use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{lock_project, registry_cache_file_name, sha256_checksum};

#[test]
fn local_registry_lock_is_stable_sorted_and_runs_from_cache() {
    let root = fixture_dir("stable-offline");
    let project = root.join("app");
    let registry = root.join("registry");
    fs::create_dir_all(&project).unwrap();
    write_registry_package(
        &registry,
        "math",
        "1.2.3",
        "math.lisp",
        "(export inc (fn (value) (+ value 1)))\n",
    );
    write_registry_package(
        &registry,
        "alpha",
        "1.0.0",
        "alpha.lisp",
        "(export answer (fn () 40))\n",
    );
    let math_checksum = checksum_for(&registry.join("math.lisp"));
    let alpha_checksum = checksum_for(&registry.join("alpha.lisp"));
    fs::write(
        project.join("jisp.toml"),
        format!(
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{ registry = \"../registry\", version = \"1.2.3\", checksum = \"{math_checksum}\" }}\nalpha = {{ registry = \"../registry\", version = \"1.0.0\", checksum = \"{alpha_checksum}\" }}\n"
        ),
    )
    .unwrap();
    let main = project.join("main.lisp");
    fs::write(
        &main,
        "(import math \"math\")\n(import alpha \"alpha\")\n(export main (fn () (+ (math.inc (alpha.answer)) 1)))\n",
    )
    .unwrap();

    lock_project(&project).unwrap();
    let first_lock = fs::read(project.join("jisp.lock")).unwrap();
    lock_project(&project).unwrap();
    assert_eq!(fs::read(project.join("jisp.lock")).unwrap(), first_lock);
    assert!(
        first_lock
            .windows("[registry.alpha]".len())
            .position(|window| window == b"[registry.alpha]")
            < first_lock
                .windows("[registry.math]".len())
                .position(|window| window == b"[registry.math]")
    );

    fs::remove_dir_all(&registry).unwrap();
    let source = fs::read_to_string(&main).unwrap();
    let value = jisp::run_main(&main, &source).unwrap();
    assert_eq!(value.display_string(), "42");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn failed_lock_restores_the_previous_lock_and_cache() {
    let root = fixture_dir("transaction-rollback");
    let project = root.join("app");
    let registry = root.join("registry");
    fs::create_dir_all(&project).unwrap();
    write_registry_package(
        &registry,
        "math",
        "1.2.3",
        "math.lisp",
        "(export inc (fn (value) (+ value 1)))\n",
    );
    let checksum = checksum_for(&registry.join("math.lisp"));
    fs::write(
        project.join("jisp.toml"),
        format!(
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{ registry = \"../registry\", version = \"1.2.3\", checksum = \"{checksum}\" }}\n"
        ),
    )
    .unwrap();
    fs::write(
        project.join("main.lisp"),
        "(import math \"math\")\n(export main (fn () (+ (math.inc 41) true)))\n",
    )
    .unwrap();
    fs::write(project.join("jisp.lock"), "old lock\n").unwrap();

    assert!(lock_project(&project).is_err());
    assert_eq!(
        fs::read_to_string(project.join("jisp.lock")).unwrap(),
        "old lock\n"
    );
    assert!(!project.join(".jisp").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn invalid_index_checksum_does_not_write_lock_or_cache() {
    let root = fixture_dir("invalid-index");
    let project = root.join("app");
    let registry = root.join("registry");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(registry.join("math")).unwrap();
    fs::write(registry.join("math.lisp"), "(export value 1)\n").unwrap();
    fs::write(
        registry.join("math/1.2.3.toml"),
        "source = \"math.lisp\"\nchecksum = \"sha256:deadbeef\"\n",
    )
    .unwrap();
    fs::write(
        project.join("jisp.toml"),
        "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"../registry\", version = \"1.2.3\" }\n",
    )
    .unwrap();
    fs::write(project.join("main.lisp"), "(export main (fn () 42))\n").unwrap();

    let error = lock_project(&project).unwrap_err().to_string();
    assert!(error.contains("checksum mismatch"), "{error}");
    assert!(!project.join("jisp.lock").exists());
    assert!(!project.join(".jisp").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_local_index_fields_do_not_write_lock_or_cache() {
    for (name, index, expected) in [
        (
            "missing-source",
            "checksum = \"sha256:deadbeef\"\n",
            "must contain `source`",
        ),
        (
            "missing-checksum",
            "source = \"math.lisp\"\n",
            "must contain `checksum`",
        ),
    ] {
        let root = fixture_dir(name);
        let project = root.join("app");
        let registry = root.join("registry");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(registry.join("math")).unwrap();
        fs::write(registry.join("math.lisp"), "(export value 1)\n").unwrap();
        fs::write(registry.join("math/1.2.3.toml"), index).unwrap();
        fs::write(
            project.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"../registry\", version = \"1.2.3\" }\n",
        )
        .unwrap();
        fs::write(project.join("main.lisp"), "(export main (fn () 42))\n").unwrap();

        let error = lock_project(&project).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
        assert!(!project.join("jisp.lock").exists());
        assert!(!project.join(".jisp").exists());

        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn manifest_and_index_checksum_mismatch_does_not_write_lock_or_cache() {
    let root = fixture_dir("manifest-index-mismatch");
    let project = root.join("app");
    let registry = root.join("registry");
    fs::create_dir_all(&project).unwrap();
    write_registry_package(
        &registry,
        "math",
        "1.2.3",
        "math.lisp",
        "(export value 1)\n",
    );
    fs::write(
        project.join("jisp.toml"),
        "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"../registry\", version = \"1.2.3\", checksum = \"sha256:deadbeef\" }\n",
    )
    .unwrap();
    fs::write(project.join("main.lisp"), "(export main (fn () 42))\n").unwrap();

    let error = lock_project(&project).unwrap_err().to_string();
    assert!(error.contains("does not match index checksum"), "{error}");
    assert!(!project.join("jisp.lock").exists());
    assert!(!project.join(".jisp").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remote_registry_urls_fail_without_writing_lock_or_cache() {
    for registry in [
        "http://packages.example.test/jisp",
        "https://packages.example.test/jisp",
    ] {
        let root = fixture_dir(if registry.starts_with("https") {
            "https-registry"
        } else {
            "http-registry"
        });
        let project = root.join("app");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("jisp.toml"),
            format!(
                "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{ registry = \"{registry}\", version = \"1.2.3\" }}\n"
            ),
        )
        .unwrap();
        fs::write(
            project.join("main.lisp"),
            "(import math \"math\")\n(export main (fn () (math.inc 41)))\n",
        )
        .unwrap();

        let error = lock_project(&project).unwrap_err().to_string();
        assert!(
            error.contains("remote registry lookup and downloads are not implemented yet"),
            "{error}"
        );
        assert!(!project.join("jisp.lock").exists());
        assert!(!project.join(".jisp").exists());

        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn cache_names_keep_unsafe_package_and_version_spellings_distinct() {
    let first = registry_cache_file_name("a/b", "1?2", Path::new("module.lisp"));
    let second = registry_cache_file_name("a?b", "1/2", Path::new("module.lisp"));
    let third = registry_cache_file_name("a/b", "1?2", Path::new("module.ws"));

    assert_ne!(first, second);
    assert_ne!(first, third);
    assert!(first.ends_with(".lisp"));
    assert!(second.ends_with(".lisp"));
    assert!(third.ends_with(".ws"));
}

fn write_registry_package(registry: &Path, package: &str, version: &str, source: &str, text: &str) {
    fs::create_dir_all(registry.join(package)).unwrap();
    fs::write(registry.join(source), text).unwrap();
    let checksum = checksum_for(&registry.join(source));
    fs::write(
        registry.join(package).join(format!("{version}.toml")),
        format!("source = {source:?}\nchecksum = {checksum:?}\n"),
    )
    .unwrap();
}

fn checksum_for(path: &Path) -> String {
    sha256_checksum(&fs::read(path).unwrap())
}

fn fixture_dir(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("jisp-package-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}
