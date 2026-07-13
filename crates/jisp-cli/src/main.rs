use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jisp_core::{Diagnostic, Node, NodeKind, SourceId, SyntaxParser};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_yaml::YamlParser;

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
    Fmt {
        path: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        write: bool,
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
        Command::Fmt { path, check, write } => format_file(&path, check, write)?,
    }
    Ok(())
}

fn format_file(path: &Path, check: bool, write: bool) -> Result<()> {
    if check && write {
        anyhow::bail!("jisp fmt accepts either --check or --write, not both");
    }
    let original = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let extension = path.extension().and_then(|extension| extension.to_str());
    let formatted = match extension {
        Some("lisp" | "jisp") => {
            let nodes = LispParser
                .parse_module(SourceId(0), &original)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            format_lisp_module(&nodes)
        }
        Some("json") => {
            let nodes = JsonParser
                .parse_module(SourceId(0), &original)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            format_json_module(&nodes)?
        }
        Some("yaml" | "yml") => {
            let nodes = YamlParser
                .parse_module(SourceId(0), &original)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            format_yaml_module(&nodes)
        }
        _ => {
            anyhow::bail!("jisp fmt currently supports .lisp, .jisp, .json, .yaml, and .yml files")
        }
    };
    if check {
        if formatted != original {
            anyhow::bail!("{} is not formatted", path.display());
        }
        println!("ok: {}", path.display());
    } else if write {
        fs::write(path, formatted).with_context(|| format!("write {}", path.display()))?;
    } else {
        print!("{formatted}");
    }
    Ok(())
}

fn format_json_module(nodes: &[Node]) -> Result<String> {
    let root = serde_json::Value::Array(nodes.iter().map(json_node).collect());
    Ok(format!("{}\n", serde_json::to_string_pretty(&root)?))
}

fn format_yaml_module(nodes: &[Node]) -> String {
    format!(
        "[{}]\n",
        nodes
            .iter()
            .map(format_yaml_node)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn format_yaml_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => {
            serde_json::to_string(value.as_ref()).expect("string serialization")
        }
        NodeKind::Form(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_yaml_node)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn json_node(node: &Node) -> serde_json::Value {
    match &node.kind {
        NodeKind::Null => serde_json::Value::Null,
        NodeKind::Bool(value) => serde_json::Value::Bool(*value),
        NodeKind::Int(value) => serde_json::json!(value),
        NodeKind::Float(value) => serde_json::json!(value),
        NodeKind::Symbol(value) => serde_json::json!(value.as_str()),
        NodeKind::String(value) => serde_json::json!(["str", value]),
        NodeKind::Form(items) => {
            let string_template = matches!(
                items.first().and_then(Node::as_symbol),
                Some("str" | "str.lines")
            );
            serde_json::Value::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        if string_template && index > 0 {
                            if let NodeKind::String(value) = &item.kind {
                                return serde_json::json!(value);
                            }
                        }
                        json_node(item)
                    })
                    .collect(),
            )
        }
    }
}

fn format_lisp_module(nodes: &[Node]) -> String {
    let mut output = nodes
        .iter()
        .map(format_lisp_node)
        .collect::<Vec<_>>()
        .join("\n");
    output.push('\n');
    output
}

fn format_lisp_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => {
            serde_json::to_string(value.as_ref()).expect("string serialization")
        }
        NodeKind::Form(items)
            if matches!(
                items.first().and_then(Node::as_symbol),
                Some("`" | "," | ",@")
            ) && items.len() == 2 =>
        {
            format!(
                "{}{}",
                items[0].as_symbol().unwrap(),
                format_lisp_node(&items[1])
            )
        }
        NodeKind::Form(items) => format!(
            "({})",
            items
                .iter()
                .map(format_lisp_node)
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        format_json_module, format_lisp_module, format_yaml_module, remapped_cargo_errors,
        JsonParser, LispParser, SourceId, SyntaxParser, YamlParser,
    };

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

    #[test]
    fn lisp_formatter_round_trips_the_normalized_ast() {
        let original = "(export main (fn () (str \"x\" ,\"y\")))";
        let parser = LispParser;
        let nodes = parser.parse_module(SourceId(0), original).unwrap();
        let formatted = format_lisp_module(&nodes);
        let reparsed = parser.parse_module(SourceId(0), &formatted).unwrap();

        assert!(nodes
            .iter()
            .zip(&reparsed)
            .all(|(left, right)| same_kind(left, right)));
        assert_eq!(formatted, format_lisp_module(&reparsed));
    }

    #[test]
    fn json_formatter_preserves_string_template_literals() {
        let original =
            r#"[["export", "main", ["fn", [], ["str", "hello", [",", ["str", " world"]]]]] ]"#;
        let parser = JsonParser;
        let nodes = parser.parse_module(SourceId(0), original).unwrap();
        let formatted = format_json_module(&nodes).unwrap();
        let reparsed = parser.parse_module(SourceId(0), &formatted).unwrap();

        assert!(nodes
            .iter()
            .zip(&reparsed)
            .all(|(left, right)| same_kind(left, right)));
        assert!(formatted.contains("\"hello\""));
        assert!(formatted.contains("\" world\""));
    }

    #[test]
    fn yaml_formatter_preserves_symbols_and_strings() {
        let original = r#"[[export, main, [fn, [], [str, "hello"]]]]"#;
        let parser = YamlParser;
        let nodes = parser.parse_module(SourceId(0), original).unwrap();
        let formatted = format_yaml_module(&nodes);
        let reparsed = parser.parse_module(SourceId(0), &formatted).unwrap();

        assert!(nodes
            .iter()
            .zip(&reparsed)
            .all(|(left, right)| same_kind(left, right)));
        assert!(formatted.contains("export"));
        assert!(formatted.contains("\"hello\""));
    }

    fn same_kind(left: &jisp_core::Node, right: &jisp_core::Node) -> bool {
        match (&left.kind, &right.kind) {
            (jisp_core::NodeKind::Null, jisp_core::NodeKind::Null) => true,
            (jisp_core::NodeKind::Bool(left), jisp_core::NodeKind::Bool(right)) => left == right,
            (jisp_core::NodeKind::Int(left), jisp_core::NodeKind::Int(right)) => left == right,
            (jisp_core::NodeKind::Float(left), jisp_core::NodeKind::Float(right)) => left == right,
            (jisp_core::NodeKind::Symbol(left), jisp_core::NodeKind::Symbol(right)) => {
                left == right
            }
            (jisp_core::NodeKind::String(left), jisp_core::NodeKind::String(right)) => {
                left == right
            }
            (jisp_core::NodeKind::Form(left), jisp_core::NodeKind::Form(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| same_kind(left, right))
            }
            _ => false,
        }
    }
}
