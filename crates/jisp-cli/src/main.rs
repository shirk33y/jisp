use std::{fs, path::PathBuf, process};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

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
    EmitRust {
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
            let value = match jisp::run_main(&path, &text) {
                Ok(value) => value,
                Err(error) => report_jisp_error(&path, &text, &error),
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
        Command::EmitRust { path } => {
            let _ = path;
            anyhow::bail!("native Rust code generation is listed in TODO.md");
        }
    }
    Ok(())
}

fn read(path: &PathBuf) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn report_jisp_error(path: &PathBuf, text: &str, error: &jisp::Error) -> ! {
    if let Some(rendered) = render_diagnostics(path, text, error) {
        eprintln!("{rendered}");
    } else {
        eprintln!("{error}");
    }
    process::exit(1);
}

fn report_jisp_module_error(error: &jisp::ModuleError) -> ! {
    if let Some(rendered) = error.render_diagnostics() {
        eprintln!("{rendered}");
    } else {
        eprintln!("{}", error.error);
    }
    process::exit(1);
}

fn render_diagnostics(path: &PathBuf, text: &str, error: &jisp::Error) -> Option<String> {
    let diagnostics = match error {
        jisp::Error::Parse(error) => &error.diagnostics,
        jisp::Error::Lower(error) => &error.diagnostics,
        _ => return None,
    };

    let mut sources = jisp::jisp_core::SourceMap::default();
    sources.add(path.display().to_string(), text.to_owned());
    Some(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(&sources))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
