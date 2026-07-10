use std::{fs, path::PathBuf};

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
    Check { path: PathBuf },
    Run { path: PathBuf },
    Schema { output: Option<PathBuf> },
    EmitRust { path: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { path } => {
            let text = read(&path)?;
            jisp::parse(&path, &text)?;
            println!("ok: {}", path.display());
        }
        Command::Run { path } => {
            let text = read(&path)?;
            let value = jisp::run_main(&path, &text)?;
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
