use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jisp_core::Diagnostic;

#[derive(Parser)]
#[command(name = "jisp", version, about = "Jisp language toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        path: PathBuf,
        #[arg(long)]
        types: bool,
        #[arg(long)]
        deps: bool,
    },
    Run {
        path: PathBuf,
    },
    Schema {
        output: Option<PathBuf>,
    },
    ExportSchema {
        path: PathBuf,
        export: String,
        output: Option<PathBuf>,
    },
    EmitRust {
        path: PathBuf,
    },
    NativeCheck {
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { path, types, deps } => {
            let text = read(&path)?;
            if types || deps {
                let checked = jisp::check_detailed(&path, &text);
                let parsed = match checked {
                    Ok(parsed) => parsed,
                    Err(error) => report_jisp_module_error(&error),
                };
                if deps {
                    for dependency in parsed.dependencies {
                        println!("{}", dependency.display());
                    }
                } else {
                    println!("ok: {}", path.display());
                }
            } else {
                if let Err(error) = jisp::parse_detailed(&path, &text) {
                    report_jisp_module_error(&error);
                }
                println!("ok: {}", path.display());
            }
        }
        Command::Run { path } => {
            let text = read(&path)?;
            let value = match jisp::run_main_detailed(&path, &text) {
                Ok(value) => value,
                Err(error) => report_jisp_module_error(&error),
            };
            println!("{}", value.display_string());
        }
        Command::Schema { output } => {
            let json = serde_json::to_string_pretty(&jisp_core::core_schema())?;
            if let Some(path) = output {
                fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
            } else {
                println!("{json}");
            }
        }
        Command::ExportSchema {
            path,
            export,
            output,
        } => {
            let text = read(&path)?;
            let schema = jisp::export_schema(&path, &text, &export)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let json = serde_json::to_string_pretty(&schema)?;
            if let Some(path) = output {
                fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
            } else {
                println!("{json}");
            }
        }
        Command::EmitRust { path } => {
            let text = read(&path)?;
            let generated = match jisp::emit_rust_detailed(&path, &text) {
                Ok(generated) => generated,
                Err(error) => report_jisp_module_error(&error),
            };
            println!("{}", generated.tokens);
        }
        Command::NativeCheck { path } => native_check(&path)?,
    }
    Ok(())
}

fn native_check(path: &Path) -> Result<()> {
    let text = read(&path.to_path_buf())?;
    let generated = jisp::emit_rust_detailed(path, &text).map_err(|error| {
        anyhow::anyhow!(error
            .render_diagnostics()
            .unwrap_or_else(|| error.error.to_string()))
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("jisp-native-check-{}-{nonce}", process::id()));
    let source_dir = directory.join("src");
    fs::create_dir_all(&source_dir).with_context(|| format!("create {}", source_dir.display()))?;
    let generated_path = source_dir.join("lib.rs");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname = \"jisp_native_check\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nnum-bigint = \"0.4\"\n",
    )?;
    fs::write(&generated_path, generated.tokens.to_string())?;

    let output = process::Command::new("cargo")
        .args(["check", "--offline", "--message-format=json"])
        .current_dir(&directory)
        .output()
        .context("run Cargo native check")?;
    let rendered = remapped_cargo_errors(
        &String::from_utf8_lossy(&output.stdout),
        &generated,
        &generated_path,
    );
    let _ = fs::remove_dir_all(&directory);
    if output.status.success() {
        println!("ok: {}", path.display());
        return Ok(());
    }
    if rendered.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    } else {
        for diagnostic in rendered {
            eprintln!("{diagnostic}");
        }
    }
    anyhow::bail!("native Rust check failed")
}

fn remapped_cargo_errors(
    json_lines: &str,
    generated: &jisp::GeneratedRustModule,
    generated_path: &Path,
) -> Vec<String> {
    json_lines
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-message" && message["message"]["level"] == "error"
        })
        .filter_map(|message| {
            let diagnostic = &message["message"];
            let span = diagnostic["spans"].as_array()?.iter().find(|span| {
                span["is_primary"] == true
                    && (Path::new(span["file_name"].as_str().unwrap_or_default()) == generated_path
                        || Path::new(span["file_name"].as_str().unwrap_or_default())
                            == Path::new("src/lib.rs"))
            })?;
            let offset = span["byte_start"].as_u64()? as usize;
            let item = generated.source_map.item_at(offset)?;
            Some(
                Diagnostic::error(
                    item.source_span,
                    diagnostic["message"].as_str().unwrap_or("rustc error"),
                )
                .with_code("JISP-RUST")
                .render(&generated.sources),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::remapped_cargo_errors;

    #[test]
    fn remaps_a_primary_cargo_span_to_the_containing_jisp_item() {
        let generated = jisp::emit_rust_detailed("main.lisp", "(export main (fn () 42))").unwrap();
        let item = generated
            .source_map
            .item(jisp::RustItemKind::Function, "main")
            .unwrap();
        let offset = item.generated_range.as_ref().unwrap().start;
        let json = format!(
            r#"{{"reason":"compiler-message","message":{{"level":"error","message":"synthetic rust error","spans":[{{"is_primary":true,"file_name":"src/lib.rs","byte_start":{offset}}}]}}}}"#
        );

        let rendered = remapped_cargo_errors(&json, &generated, Path::new("/tmp/src/lib.rs"));

        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("error[JISP-RUST]"), "{}", rendered[0]);
        assert!(
            rendered[0].contains("synthetic rust error"),
            "{}",
            rendered[0]
        );
        assert!(rendered[0].contains("main.lisp:1:1"), "{}", rendered[0]);
    }
}

fn read(path: &PathBuf) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn report_jisp_module_error(error: &jisp::ModuleError) -> ! {
    if let Some(rendered) = error.render_diagnostics() {
        eprintln!("{rendered}");
    } else {
        eprintln!("{}", error.error);
    }
    process::exit(1);
}
