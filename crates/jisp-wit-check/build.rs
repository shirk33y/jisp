use std::fs;
use std::path::Path;
use std::process::Command;

use wit_bindgen_core::{wit_parser::Resolve, Files, WorldGenerator};

const WORLD: &str = "jisp-ui-host";

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wit = root.join("../../wit");
    let output = std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR");
    let output = Path::new(&output);
    println!("cargo:rerun-if-changed={}", wit.display());
    println!("cargo:rerun-if-env-changed=JISP_VERIFY_C_BINDING");
    generate(
        Box::new(wit_bindgen_rust::Opts::default().build()),
        &wit,
        &output.join("rust"),
    );
    generate(
        wit_bindgen_c::Opts::default().build(),
        &wit,
        &output.join("c"),
    );
    if std::env::var_os("JISP_VERIFY_C_BINDING").is_some() {
        compile_c_binding(&output.join("c"));
    }
}

fn generate(mut generator: Box<dyn WorldGenerator>, wit: &Path, output: &Path) {
    let mut resolve = Resolve::default();
    let (package, _) = resolve
        .push_path(wit)
        .unwrap_or_else(|error| panic!("could not parse {}: {error}", wit.display()));
    let world = resolve
        .select_world(&[package], Some(WORLD))
        .unwrap_or_else(|error| panic!("could not select WIT world `{WORLD}`: {error}"));
    let mut files = Files::default();
    generator
        .generate(&mut resolve, world, &mut files)
        .unwrap_or_else(|error| panic!("could not generate bindings for `{WORLD}`: {error}"));
    if files.iter().next().is_none() {
        panic!("WIT generator emitted no bindings for `{WORLD}`");
    }
    for (name, contents) in files.iter() {
        let path = output.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("could not create generated binding directory");
        }
        fs::write(path, contents).expect("could not write generated binding");
    }
}

fn compile_c_binding(output: &Path) {
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let source = output.join("jisp_ui_host.c");
    let status = Command::new(&compiler)
        .args(["-std=c11", "-fsyntax-only", "-Wno-attributes", "-I"])
        .arg(output)
        .arg(&source)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "could not run C compiler `{}` for generated binding: {error}",
                Path::new(&compiler).display()
            )
        });
    assert!(
        status.success(),
        "C compiler rejected generated binding {}",
        source.display()
    );
}
