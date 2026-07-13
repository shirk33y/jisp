use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jisp_core::{Diagnostic, Node, NodeKind, SourceId, Span, SyntaxParser};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_yaml::YamlParser;
use sha2::{Digest, Sha256};

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
        path: Option<PathBuf>,
    },
    Schema {
        output: Option<PathBuf>,
    },
    ExportSchema {
        path: PathBuf,
        export: String,
        #[arg(long = "type")]
        type_: Option<String>,
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
    Repl {
        #[arg(long)]
        state: Option<PathBuf>,
    },
    Lsp,
    Init {
        path: Option<PathBuf>,
    },
    Lock {
        path: Option<PathBuf>,
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
            let path = path.unwrap_or(package_entry(Path::new("."))?);
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
            type_,
            output,
        } => {
            let text = read(&path)?;
            let schema = jisp::export_schema_with_type(&path, &text, &export, type_.as_deref())
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
        Command::Repl { state } => repl(state.as_deref())?,
        Command::Lsp => lsp()?,
        Command::Init { path } => init_project(path.as_deref().unwrap_or_else(|| Path::new(".")))?,
        Command::Lock { path } => lock_project(path.as_deref().unwrap_or_else(|| Path::new(".")))?,
    }
    Ok(())
}

fn package_entry(directory: &Path) -> Result<PathBuf> {
    let manifest = directory.join("jisp.toml");
    let text =
        fs::read_to_string(&manifest).with_context(|| format!("read {}", manifest.display()))?;
    let entry = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("entry ="))
        .map(str::trim)
        .and_then(|value| {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .context("jisp.toml must contain a quoted `entry` field")?;
    Ok(directory.join(entry))
}

fn init_project(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    let manifest = path.join("jisp.toml");
    let entry = path.join("main.lisp");
    if manifest.exists() || entry.exists() {
        anyhow::bail!(
            "refusing to initialize {} because jisp.toml or main.lisp already exists",
            path.display()
        );
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != ".")
        .unwrap_or("jisp-project");
    fs::write(
        &manifest,
        format!("[package]\nname = {name:?}\nversion = \"0.1.0\"\nentry = \"main.lisp\"\n"),
    )?;
    fs::write(
        &entry,
        "(export main\n  (fn ()\n    (str \"Hello from Jisp\")))\n",
    )?;
    println!("initialized Jisp package: {}", path.display());
    Ok(())
}

fn lock_project(path: &Path) -> Result<()> {
    let entry = package_entry(path)?;
    let manifest_path = path.join("jisp.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let registry_dependencies = manifest_registry_dependencies(&manifest);
    let mut registry_entries = registry_lock_entries_for_manifest(path, &registry_dependencies)?;
    registry_entries.sort_by(|left, right| left.name.cmp(&right.name));
    if !registry_entries.is_empty() {
        let provisional_lock = render_lockfile(&entry, &[], &registry_entries);
        let lock_path = path.join("jisp.lock");
        fs::write(&lock_path, provisional_lock)
            .with_context(|| format!("write {}", lock_path.display()))?;
    }
    let text = read(&entry)?;
    let parsed = jisp::check_detailed(&entry, &text).map_err(|error| {
        anyhow::anyhow!(
            "{}",
            error
                .render_diagnostics()
                .unwrap_or_else(|| error.error.to_string())
        )
    })?;
    let mut registry_entries =
        used_registry_lock_entries(path, &registry_dependencies, &parsed.dependencies)?;
    registry_entries.sort_by(|left, right| left.name.cmp(&right.name));
    let registry_sources = registry_entries
        .iter()
        .filter_map(|entry| path.join(&entry.source).canonicalize().ok())
        .collect::<HashSet<_>>();
    let mut dependencies = parsed
        .dependencies
        .into_iter()
        .filter(|dependency| !registry_sources.contains(dependency))
        .map(|dependency| dependency.display().to_string())
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();
    let lock = render_lockfile(&entry, &dependencies, &registry_entries);
    let lock_path = path.join("jisp.lock");
    fs::write(&lock_path, lock).with_context(|| format!("write {}", lock_path.display()))?;
    println!("locked Jisp package: {}", lock_path.display());
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryLockEntry {
    name: String,
    registry: Option<String>,
    package: Option<String>,
    version: String,
    source: String,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestRegistryDependency {
    name: String,
    registry: Option<String>,
    package: String,
    version: String,
    checksum: Option<String>,
}

fn registry_lock_entries_for_manifest(
    project: &Path,
    dependencies: &[ManifestRegistryDependency],
) -> Result<Vec<RegistryLockEntry>> {
    if dependencies.is_empty() {
        return Ok(vec![]);
    }
    let existing = fs::read_to_string(project.join("jisp.lock"))
        .ok()
        .map(|lockfile| parse_registry_lock_entries(&lockfile))
        .unwrap_or_default();
    let mut entries = vec![];
    for dependency in dependencies {
        if let Some(entry) = existing.get(&dependency.name).filter(|entry| {
            entry.version == dependency.version
                && dependency
                    .checksum
                    .as_ref()
                    .is_none_or(|checksum| checksum.eq_ignore_ascii_case(&entry.checksum))
        }) {
            entries.push(entry.clone());
            continue;
        }
        if let Some(entry) = registry_lock_entry_from_local_index(project, dependency)? {
            entries.push(entry);
            continue;
        }
        if dependency
            .registry
            .as_deref()
            .is_some_and(registry_is_remote_url)
        {
            anyhow::bail!(
                "registry dependency `{}` uses remote registry `{}`; remote registry lookup and downloads are not implemented yet",
                dependency.name,
                dependency.registry.as_deref().unwrap()
            );
        }
    }
    Ok(entries)
}

fn registry_is_remote_url(registry: &str) -> bool {
    registry.starts_with("https://") || registry.starts_with("http://")
}

fn registry_lock_entry_from_local_index(
    project: &Path,
    dependency: &ManifestRegistryDependency,
) -> Result<Option<RegistryLockEntry>> {
    let Some(registry) = &dependency.registry else {
        return Ok(None);
    };
    let registry_root = project.join(registry);
    if !registry_root.is_dir() {
        return Ok(None);
    }
    let index_path = registry_root
        .join(&dependency.package)
        .join(format!("{}.toml", dependency.version));
    if !index_path.exists() {
        return Ok(None);
    }
    let index = fs::read_to_string(&index_path)
        .with_context(|| format!("read {}", index_path.display()))?;
    let source = quoted_assignment(&index, "source")
        .with_context(|| format!("{} must contain `source`", index_path.display()))?;
    let checksum = quoted_assignment(&index, "checksum")
        .or_else(|| dependency.checksum.clone())
        .with_context(|| format!("{} must contain `checksum`", index_path.display()))?;
    if let Some(manifest_checksum) = &dependency.checksum {
        anyhow::ensure!(
            manifest_checksum.eq_ignore_ascii_case(&checksum),
            "registry dependency `{}` checksum {} does not match index checksum {}",
            dependency.name,
            manifest_checksum,
            checksum
        );
    }
    let source_path = registry_root.join(&source);
    let bytes =
        fs::read(&source_path).with_context(|| format!("read {}", source_path.display()))?;
    let actual = sha256_checksum(&bytes);
    anyhow::ensure!(
        actual == checksum.to_ascii_lowercase(),
        "registry dependency `{}` checksum mismatch for {}: expected {}, got {}",
        dependency.name,
        source_path.display(),
        checksum,
        actual
    );
    let cache_dir = project.join(".jisp/cache");
    fs::create_dir_all(&cache_dir).with_context(|| format!("create {}", cache_dir.display()))?;
    let cache_name =
        registry_cache_file_name(&dependency.package, &dependency.version, &source_path);
    let cache_path = cache_dir.join(cache_name);
    fs::write(&cache_path, bytes).with_context(|| format!("write {}", cache_path.display()))?;
    Ok(Some(RegistryLockEntry {
        name: dependency.name.clone(),
        registry: dependency.registry.clone(),
        package: Some(dependency.package.clone()),
        version: dependency.version.clone(),
        source: format!(
            ".jisp/cache/{}",
            cache_path.file_name().unwrap().to_string_lossy()
        ),
        checksum: checksum.to_ascii_lowercase(),
    }))
}

fn registry_cache_file_name(package: &str, version: &str, source: &Path) -> String {
    let mut name = format!("{package}-{version}")
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            _ => '_',
        })
        .collect::<String>();
    if let Some(extension) = source.extension().and_then(|extension| extension.to_str()) {
        name.push('.');
        name.push_str(extension);
    }
    name
}

fn quoted_assignment(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.split('#').next()?.trim();
        let (name, value) = line.split_once('=')?;
        (name.trim() == key)
            .then_some(value.trim())?
            .trim_end_matches(',')
            .trim()
            .strip_prefix('"')?
            .strip_suffix('"')
            .map(str::to_owned)
    })
}

fn sha256_checksum(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn used_registry_lock_entries(
    project: &Path,
    registry_dependencies: &[ManifestRegistryDependency],
    dependencies: &[PathBuf],
) -> Result<Vec<RegistryLockEntry>> {
    if registry_dependencies.is_empty() {
        return Ok(vec![]);
    }
    let lock_path = project.join("jisp.lock");
    let Ok(lockfile) = fs::read_to_string(&lock_path) else {
        return Ok(vec![]);
    };
    let entries = parse_registry_lock_entries(&lockfile);
    let dependency_paths = dependencies.iter().collect::<HashSet<_>>();
    let mut used = vec![];
    for dependency in registry_dependencies {
        let Some(entry) = entries.get(&dependency.name) else {
            continue;
        };
        let source = project
            .join(&entry.source)
            .canonicalize()
            .with_context(|| format!("canonicalize locked registry source {}", entry.source))?;
        if dependency_paths.contains(&source) {
            used.push(entry.clone());
        }
    }
    Ok(used)
}

fn manifest_registry_dependencies(manifest: &str) -> Vec<ManifestRegistryDependency> {
    let mut dependencies = manifest_dependency_tables(manifest)
        .into_iter()
        .filter_map(|(name, table)| {
            if manifest_inline_value(&table, "path").is_some() {
                return None;
            }
            let version = manifest_inline_value(&table, "version")?;
            Some(ManifestRegistryDependency {
                package: manifest_inline_value(&table, "package").unwrap_or_else(|| name.clone()),
                registry: manifest_inline_value(&table, "registry"),
                checksum: manifest_inline_value(&table, "checksum"),
                name,
                version,
            })
        })
        .collect::<Vec<_>>();
    dependencies.sort_by(|left, right| left.name.cmp(&right.name));
    dependencies.dedup_by(|left, right| left.name == right.name);
    dependencies
}

fn manifest_dependency_tables(manifest: &str) -> Vec<(String, String)> {
    let mut dependencies = false;
    let mut tables = vec![];
    let mut current_inline_table: Option<(String, String)> = None;
    for line in manifest.lines() {
        let Some(line) = line.split('#').next().map(str::trim) else {
            continue;
        };
        if line.starts_with('[') {
            flush_manifest_dependency_table(&mut tables, current_inline_table.take());
            dependencies = line == "[dependencies]";
            continue;
        }
        if !dependencies {
            continue;
        }
        if let Some((name, mut table)) = current_inline_table.take() {
            if !line.is_empty() {
                table.push('\n');
                table.push_str(line);
            }
            if line.starts_with('}') {
                flush_manifest_dependency_table(&mut tables, Some((name, table)));
            } else {
                current_inline_table = Some((name, table));
            }
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.starts_with('{') {
            let name = name.trim().to_owned();
            if value.contains('}') {
                tables.push((name, value.to_owned()));
            } else {
                current_inline_table = Some((name, value.to_owned()));
            }
        }
    }
    flush_manifest_dependency_table(&mut tables, current_inline_table);
    tables
}

fn flush_manifest_dependency_table(
    tables: &mut Vec<(String, String)>,
    dependency: Option<(String, String)>,
) {
    if let Some((name, table)) = dependency {
        tables.push((name, table));
    }
}

fn manifest_inline_value(inline: &str, key: &str) -> Option<String> {
    inline
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split([',', '\n'])
        .find_map(|item| {
            let (name, value) = item.split_once('=')?;
            let value = value.trim().trim_end_matches(',').trim();
            (name.trim() == key)
                .then_some(value)?
                .strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_owned)
        })
}

fn parse_registry_lock_entries(lockfile: &str) -> HashMap<String, RegistryLockEntry> {
    let mut entries = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut current_fields = HashMap::<String, String>::new();
    for line in lockfile.lines() {
        let Some(line) = line.split('#').next().map(str::trim) else {
            continue;
        };
        if line.starts_with('[') {
            flush_registry_lock_entry(&mut entries, current_name.take(), &mut current_fields);
            current_name = line
                .strip_prefix("[registry.")
                .and_then(|name| name.strip_suffix(']'))
                .map(str::to_owned);
            continue;
        }
        let Some(name) = current_name.as_ref() else {
            continue;
        };
        if line.is_empty() || name.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if let Some(value) = quoted_toml_string(value.trim()) {
            current_fields.insert(key.trim().to_owned(), value.to_owned());
        }
    }
    flush_registry_lock_entry(&mut entries, current_name, &mut current_fields);
    entries
}

fn flush_registry_lock_entry(
    entries: &mut HashMap<String, RegistryLockEntry>,
    name: Option<String>,
    fields: &mut HashMap<String, String>,
) {
    let Some(name) = name else {
        fields.clear();
        return;
    };
    let Some(version) = fields.remove("version") else {
        fields.clear();
        return;
    };
    let Some(source) = fields.remove("source") else {
        fields.clear();
        return;
    };
    let Some(checksum) = fields.remove("checksum") else {
        fields.clear();
        return;
    };
    entries.insert(
        name.clone(),
        RegistryLockEntry {
            name,
            registry: fields.remove("registry"),
            package: fields.remove("package"),
            version,
            source,
            checksum,
        },
    );
    fields.clear();
}

fn quoted_toml_string(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn render_lockfile(
    entry: &Path,
    dependencies: &[String],
    registry_entries: &[RegistryLockEntry],
) -> String {
    let mut output = format!(
        "# This file is generated by `jisp lock`.\nversion = 1\nentry = {:?}\n\n",
        entry.display().to_string()
    );
    output.push_str("[dependencies]\n");
    for dependency in dependencies {
        output.push_str(&format!("source = {dependency:?}\n"));
    }
    for entry in registry_entries {
        output.push_str(&format!("\n[registry.{}]\n", entry.name));
        if let Some(registry) = &entry.registry {
            output.push_str(&format!("registry = {registry:?}\n"));
        }
        if let Some(package) = &entry.package {
            output.push_str(&format!("package = {package:?}\n"));
        }
        output.push_str(&format!("version = {:?}\n", entry.version));
        output.push_str(&format!("source = {:?}\n", entry.source));
        output.push_str(&format!("checksum = {:?}\n", entry.checksum));
    }
    output
}

fn lsp() -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut documents = HashMap::new();
    while let Some(message) = read_lsp_message(&mut input)? {
        let method = message["method"].as_str();
        match method {
            Some("initialize") => write_lsp_message(
                &mut output,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": { "capabilities": {
                        "textDocumentSync": 1,
                        "completionProvider": { "triggerCharacters": ["(", "."] },
                        "hoverProvider": true,
                        "definitionProvider": true
                    } }
                }),
            )?,
            Some("textDocument/completion") => write_lsp_message(
                &mut output,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": lsp_completion_items()
                }),
            )?,
            Some("textDocument/didOpen") | Some("textDocument/didChange") => {
                let document = &message["params"]["textDocument"];
                let uri = document["uri"].as_str().unwrap_or("untitled.lisp");
                let text = if method == Some("textDocument/didOpen") {
                    document["text"].as_str().unwrap_or("")
                } else {
                    message["params"]["contentChanges"]
                        .as_array()
                        .and_then(|changes| changes.last())
                        .and_then(|change| change["text"].as_str())
                        .unwrap_or("")
                };
                documents.insert(uri.to_owned(), text.to_owned());
                write_lsp_message(
                    &mut output,
                    &serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": lsp_diagnostics(uri, text) }
                    }),
                )?;
            }
            Some("textDocument/didClose") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("untitled.lisp");
                documents.remove(uri);
                write_lsp_message(
                    &mut output,
                    &serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": [] }
                    }),
                )?;
            }
            Some("textDocument/hover") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default();
                let position = &message["params"]["position"];
                let result = documents.get(uri).and_then(|text| {
                    lsp_hover(
                        text,
                        position["line"].as_u64()? as usize,
                        position["character"].as_u64()? as usize,
                    )
                });
                write_lsp_message(
                    &mut output,
                    &serde_json::json!({ "jsonrpc": "2.0", "id": message["id"], "result": result }),
                )?;
            }
            Some("textDocument/definition") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default();
                let position = &message["params"]["position"];
                let result = documents.get(uri).and_then(|text| {
                    lsp_definition(
                        uri,
                        text,
                        position["line"].as_u64()? as usize,
                        position["character"].as_u64()? as usize,
                    )
                });
                write_lsp_message(
                    &mut output,
                    &serde_json::json!({ "jsonrpc": "2.0", "id": message["id"], "result": result }),
                )?;
            }
            Some("shutdown") => write_lsp_message(
                &mut output,
                &serde_json::json!({ "jsonrpc": "2.0", "id": message["id"], "result": null }),
            )?,
            Some("exit") => break,
            _ => {}
        }
    }
    Ok(())
}

fn lsp_hover(text: &str, line: usize, character: usize) -> Option<serde_json::Value> {
    let offset = lsp_byte_offset(text, line, character)?;
    let symbol = lsp_symbol_at(text, offset)?;
    let (name, summary) = jisp_core::special_form(symbol)
        .map(|form| (form.name, form.summary))
        .or_else(|| jisp_core::ui_element(symbol).map(|element| (element.name, element.summary)))
        .or_else(|| {
            jisp_core::ui_directive(symbol).map(|directive| (directive.name, directive.summary))
        })?;
    Some(serde_json::json!({
        "contents": { "kind": "markdown", "value": format!("**{}** — {}", name, summary) }
    }))
}

fn lsp_definition(
    uri: &str,
    text: &str,
    line: usize,
    character: usize,
) -> Option<serde_json::Value> {
    let offset = lsp_byte_offset(text, line, character)?;
    let symbol = lsp_symbol_at(text, offset)?;
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let parsed = match jisp::check_detailed(path, text) {
        Ok(parsed) => parsed,
        Err(_) => jisp::parse_detailed(path, text).ok()?,
    };
    let span = lsp_local_binding_span(&parsed.nodes, offset, symbol)
        .or_else(|| {
            parsed
                .module
                .definitions
                .iter()
                .find(|definition| definition.name == symbol)
                .map(|definition| definition.span)
        })
        .or_else(|| lsp_imported_definition_span(&parsed, symbol))?;
    let file = parsed.sources.get(span.source)?;
    Some(serde_json::json!({
        "uri": lsp_source_uri(uri, file.name()),
        "range": { "start": lsp_position(file, span.start), "end": lsp_position(file, span.end) }
    }))
}

fn lsp_local_binding_span(nodes: &[Node], offset: usize, symbol: &str) -> Option<Span> {
    let top_level = nodes
        .iter()
        .filter_map(lsp_top_level_binding)
        .collect::<Vec<_>>();
    nodes
        .iter()
        .find_map(|node| lsp_binding_in_node(node, offset, symbol, &top_level))
}

fn lsp_top_level_binding(node: &Node) -> Option<(&str, Span)> {
    let [head, name, ..] = node.as_form()? else {
        return None;
    };
    if matches!(
        head.as_symbol(),
        Some("def" | "defn" | "export" | "component")
    ) {
        let value = name.as_symbol()?;
        Some((value, name.span))
    } else {
        None
    }
}

fn lsp_binding_in_node(
    node: &Node,
    offset: usize,
    symbol: &str,
    scope: &[(&str, Span)],
) -> Option<Span> {
    if !span_contains(node.span, offset) {
        return None;
    }
    let items = node.as_form()?;
    let head = items.first().and_then(Node::as_symbol);
    match head {
        Some("fn") if items.len() >= 3 => {
            let mut scope = scope.to_vec();
            for parameter in items[1].as_form()? {
                if let Some(name) = parameter.as_symbol().filter(|name| *name != "...") {
                    if span_contains(parameter.span, offset) && name == symbol {
                        return Some(parameter.span);
                    }
                    scope.push((name, parameter.span));
                }
            }
            items[2..]
                .iter()
                .find_map(|body| lsp_binding_in_node(body, offset, symbol, &scope))
        }
        Some("defn" | "component") if items.len() >= 4 => {
            let name = items[1].as_symbol()?;
            if span_contains(items[1].span, offset) && name == symbol {
                return Some(items[1].span);
            }
            let mut scope = scope.to_vec();
            for parameter in items[2].as_form()? {
                if let Some(name) = parameter.as_symbol().filter(|name| *name != "...") {
                    if span_contains(parameter.span, offset) && name == symbol {
                        return Some(parameter.span);
                    }
                    scope.push((name, parameter.span));
                }
            }
            items[3..]
                .iter()
                .find_map(|body| lsp_binding_in_node(body, offset, symbol, &scope))
        }
        Some("let") if items.len() == 3 => {
            let mut scope = scope.to_vec();
            let bindings = items[1].as_form()?;
            for pair in bindings.chunks_exact(2) {
                let name = pair[0].as_symbol()?;
                if span_contains(pair[0].span, offset) && name == symbol {
                    return Some(pair[0].span);
                }
                if let Some(binding) = lsp_binding_in_node(&pair[1], offset, symbol, &scope) {
                    return Some(binding);
                }
                scope.push((name, pair[0].span));
            }
            lsp_binding_in_node(&items[2], offset, symbol, &scope)
        }
        Some("case") if items.len() >= 3 => {
            if let Some(binding) = lsp_binding_in_node(&items[1], offset, symbol, scope) {
                return Some(binding);
            }
            for branch in &items[2..] {
                let branch_items = branch.as_form()?;
                let pattern = branch_items.first()?;
                let mut bindings = Vec::new();
                lsp_pattern_bindings(pattern, &mut bindings);
                if let Some(binding) = bindings.iter().find_map(|(name, span)| {
                    (span_contains(*span, offset) && *name == symbol).then_some(*span)
                }) {
                    return Some(binding);
                }
                let mut branch_scope = scope.to_vec();
                branch_scope.extend(bindings);
                let (guard, body) = match pattern.as_form() {
                    Some(when) if when.first().and_then(Node::as_symbol) == Some("when") => {
                        (when.get(2), &branch_items[1..])
                    }
                    _ => (None, &branch_items[1..]),
                };
                if let Some(guard) = guard {
                    if let Some(binding) = lsp_binding_in_node(guard, offset, symbol, &branch_scope)
                    {
                        return Some(binding);
                    }
                }
                for expression in body {
                    if let Some(binding) =
                        lsp_binding_in_node(expression, offset, symbol, &branch_scope)
                    {
                        return Some(binding);
                    }
                }
            }
            None
        }
        Some("def" | "export") if items.len() == 3 => {
            let name = items[1].as_symbol()?;
            if span_contains(items[1].span, offset) && name == symbol {
                return Some(items[1].span);
            }
            lsp_binding_in_node(&items[2], offset, symbol, scope)
        }
        _ => {
            for item in items {
                if let Some(binding) = lsp_binding_in_node(item, offset, symbol, scope) {
                    return Some(binding);
                }
            }
            scope
                .iter()
                .rev()
                .find_map(|(name, span)| (*name == symbol).then_some(*span))
        }
    }
}

fn lsp_pattern_bindings<'a>(node: &'a Node, output: &mut Vec<(&'a str, Span)>) {
    match &node.kind {
        NodeKind::Symbol(symbol) => {
            if symbol.as_str() != "_" {
                let name = symbol.as_str();
                output.push((name, node.span));
            }
        }
        NodeKind::Form(items) => match items.first().and_then(Node::as_symbol) {
            Some("list") => {
                let mut index = 1;
                while index < items.len() {
                    if items[index].as_symbol() == Some("...") {
                        if let Some(binding) = items.get(index + 1) {
                            lsp_pattern_bindings(binding, output);
                        }
                        break;
                    }
                    lsp_pattern_bindings(&items[index], output);
                    index += 1;
                }
            }
            Some("obj") => {
                for value in items.iter().skip(2).step_by(2) {
                    lsp_pattern_bindings(value, output);
                }
            }
            Some("as") => {
                if let Some(pattern) = items.get(1) {
                    lsp_pattern_bindings(pattern, output);
                }
                if let Some(binding) = items.get(2) {
                    lsp_pattern_bindings(binding, output);
                }
            }
            Some("or") => {
                if let Some(pattern) = items.get(1) {
                    lsp_pattern_bindings(pattern, output);
                }
            }
            Some("when") => {
                if let Some(pattern) = items.get(1) {
                    lsp_pattern_bindings(pattern, output);
                }
            }
            Some(_) | None => {
                for field in items.iter().skip(1) {
                    lsp_pattern_bindings(field, output);
                }
            }
        },
        NodeKind::Null
        | NodeKind::Bool(_)
        | NodeKind::Int(_)
        | NodeKind::Float(_)
        | NodeKind::String(_) => {}
    }
}

fn span_contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn lsp_imported_definition_span(parsed: &jisp::ParsedModule, symbol: &str) -> Option<Span> {
    let (alias, name) = symbol.split_once('.')?;
    let import = parsed
        .module
        .imports
        .iter()
        .find(|import| import.alias == alias)?;
    let import_name = Path::new(&import.path)
        .file_stem()
        .or_else(|| Path::new(&import.path).file_name())?
        .to_str()?;
    let module = parsed.resolved_modules.iter().find_map(|(path, module)| {
        (path.file_stem().and_then(|stem| stem.to_str()) == Some(import_name)).then_some(module)
    })?;
    module
        .definitions
        .iter()
        .find(|definition| definition.name == name)
        .map(|definition| definition.span)
}

fn lsp_source_uri(current_uri: &str, source_name: &str) -> String {
    if current_uri.strip_prefix("file://") == Some(source_name) {
        current_uri.to_owned()
    } else {
        format!("file://{source_name}")
    }
}

fn lsp_byte_offset(text: &str, line: usize, character: usize) -> Option<usize> {
    let line_start = if line == 0 {
        0
    } else {
        text.match_indices('\n').nth(line - 1)?.0 + 1
    };
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |index| line_start + index);
    let line_text = &text[line_start..line_end];
    let mut utf16_units = 0;
    for (byte_offset, ch) in line_text.char_indices() {
        if utf16_units == character {
            return Some(line_start + byte_offset);
        }
        utf16_units += ch.len_utf16();
        if utf16_units > character {
            return None;
        }
    }
    (utf16_units == character).then_some(line_end)
}

fn lsp_symbol_at(text: &str, offset: usize) -> Option<&str> {
    let offset = offset.min(text.len());
    let is_symbol =
        |ch: char| !ch.is_whitespace() && !matches!(ch, '(' | ')' | '[' | ']' | ',' | '`' | '"');
    let start = text[..offset]
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_symbol(*ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let end = text[offset..]
        .char_indices()
        .find(|(_, ch)| !is_symbol(*ch))
        .map_or(text.len(), |(index, _)| offset + index);
    (start < end).then(|| &text[start..end])
}

fn lsp_completion_items() -> Vec<serde_json::Value> {
    jisp_core::SPECIAL_FORMS
        .iter()
        .flat_map(|form| {
            std::iter::once((form.name, form.summary))
                .chain(form.aliases.iter().map(|alias| (*alias, form.summary)))
        })
        .map(|(label, detail)| {
            serde_json::json!({
                "label": label,
                "kind": 3,
                "detail": detail,
            })
        })
        .chain(jisp_core::UI_ELEMENTS.iter().map(|element| {
            serde_json::json!({
                "label": element.name,
                "kind": 7,
                "detail": element.summary,
            })
        }))
        .chain(jisp_core::UI_DIRECTIVES.iter().map(|directive| {
            serde_json::json!({
                "label": directive.name,
                "kind": 3,
                "detail": directive.summary,
            })
        }))
        .collect()
}

fn read_lsp_message(input: &mut impl BufRead) -> Result<Option<serde_json::Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("parse LSP Content-Length")?,
            );
        }
    }
    let length = length.context("missing LSP Content-Length")?;
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    Ok(Some(
        serde_json::from_slice(&bytes).context("parse LSP JSON")?,
    ))
}

fn write_lsp_message(output: &mut impl Write, message: &serde_json::Value) -> Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}

fn lsp_diagnostics(uri: &str, text: &str) -> Vec<serde_json::Value> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let Err(error) = jisp::check_detailed(path, text) else {
        return vec![];
    };
    let Some(diagnostics) = error.diagnostics() else {
        return vec![];
    };
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let file = error.sources.get(diagnostic.primary.span.source)?;
            let start = lsp_position(file, diagnostic.primary.span.start);
            let end = lsp_position(file, diagnostic.primary.span.end);
            Some(serde_json::json!({
                "range": { "start": start, "end": end },
                "severity": 1,
                "code": diagnostic.code,
                "source": "jisp",
                "message": diagnostic.message,
            }))
        })
        .collect()
}

fn lsp_position(file: &jisp_core::SourceFile, offset: usize) -> serde_json::Value {
    let offset = offset.min(file.text().len());
    let before = &file.text()[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .encode_utf16()
        .count();
    serde_json::json!({ "line": line, "character": character })
}

fn repl(state_path: Option<&Path>) -> Result<()> {
    let stdin = io::stdin();
    let mut state = state_path
        .filter(|path| path.exists())
        .map(fs::read_to_string)
        .transpose()
        .with_context(|| "read REPL state")?
        .unwrap_or_default();
    let mut stdout = io::stdout();
    eprintln!("Jisp REPL — :help for commands");
    loop {
        write!(stdout, "jisp> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        match line {
            "" => continue,
            ":quit" | ":q" => break,
            ":reset" => {
                state.clear();
                if let Some(path) = state_path {
                    fs::write(path, "")
                        .with_context(|| format!("write REPL state {}", path.display()))?;
                }
                println!("session reset");
            }
            ":help" => println!(":help, :reset, :quit; definitions persist, other forms evaluate"),
            form => match repl_step(&state, form) {
                Ok((next_state, value)) => {
                    let changed = next_state != state;
                    state = next_state;
                    if changed {
                        if let Some(path) = state_path {
                            fs::write(path, &state)
                                .with_context(|| format!("write REPL state {}", path.display()))?;
                        }
                    }
                    if let Some(value) = value {
                        println!("{value}");
                    }
                }
                Err(error) => eprintln!("{error}"),
            },
        }
    }
    Ok(())
}

fn repl_step(state: &str, form: &str) -> Result<(String, Option<String>)> {
    let candidate = format!("{state}\n{form}\n");
    if is_repl_definition(form) {
        jisp::check("repl.lisp", &candidate).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        return Ok((candidate, None));
    }
    let program = format!("{state}\n(export main (fn () {form}))\n");
    let value = jisp::run_main("repl.lisp", &program)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok((state.to_owned(), Some(value.display_string())))
}

fn is_repl_definition(form: &str) -> bool {
    matches!(
        form.split_whitespace().next(),
        Some("(def" | "(defn" | "(component" | "(type" | "(import")
    )
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
        "[package]\nname = \"jisp_native_check\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nindexmap = \"2\"\nnum-bigint = \"0.4\"\n",
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
                span["is_primary"] == true && is_generated_cargo_span(span, generated_path)
            })?;
            let offset = span["byte_start"].as_u64()? as usize;
            let item = generated.source_map.item_at(offset)?;
            let mut remapped = Diagnostic::error(
                item.source_span,
                diagnostic["message"].as_str().unwrap_or("rustc error"),
            )
            .with_code("JISP-RUST");
            for origin in generated.expansion_map.origin_chain(item.source_span) {
                remapped = remapped.with_secondary(origin, "expanded from here");
            }
            for secondary in diagnostic["spans"].as_array()?.iter().filter(|span| {
                span["is_primary"] != true && is_generated_cargo_span(span, generated_path)
            }) {
                let Some(offset) = secondary["byte_start"].as_u64() else {
                    continue;
                };
                let Some(item) = generated.source_map.item_at(offset as usize) else {
                    continue;
                };
                let label = secondary["label"]
                    .as_str()
                    .filter(|label| !label.is_empty())
                    .unwrap_or("related generated Rust expression");
                remapped = remapped.with_secondary(item.source_span, label);
            }
            Some(remapped.render(&generated.sources))
        })
        .collect()
}

fn is_generated_cargo_span(span: &serde_json::Value, generated_path: &Path) -> bool {
    let path = Path::new(span["file_name"].as_str().unwrap_or_default());
    path == generated_path || path == Path::new("src/lib.rs")
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
        format_json_module, format_lisp_module, format_yaml_module, init_project, lock_project,
        lsp_completion_items, lsp_definition, lsp_diagnostics, lsp_hover, package_entry,
        remapped_cargo_errors, repl_step, sha256_checksum, JsonParser, LispParser, SourceId,
        SyntaxParser, YamlParser,
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
    fn remaps_secondary_cargo_spans_to_jisp_labels() {
        let generated = jisp::emit_rust_detailed("main.lisp", "(export main (fn () 42))").unwrap();
        let primary = generated
            .source_map
            .items
            .iter()
            .find(|item| item.kind == jisp::RustItemKind::Expression)
            .unwrap();
        let secondary = generated
            .source_map
            .item(jisp::RustItemKind::Function, "main")
            .unwrap();
        let primary_offset = primary.generated_range.as_ref().unwrap().start;
        let secondary_offset = secondary.generated_range.as_ref().unwrap().start;
        let json = format!(
            r#"{{"reason":"compiler-message","message":{{"level":"error","message":"synthetic rust error","spans":[{{"is_primary":true,"file_name":"src/lib.rs","byte_start":{primary_offset}}},{{"is_primary":false,"file_name":"src/lib.rs","byte_start":{secondary_offset},"label":"required by this generated function"}}]}}}}"#
        );

        let rendered = remapped_cargo_errors(&json, &generated, Path::new("/tmp/src/lib.rs"));

        assert_eq!(rendered.len(), 1);
        assert!(
            rendered[0].contains("required by this generated function"),
            "{}",
            rendered[0]
        );
    }

    #[test]
    fn remaps_macro_expansion_origins_for_native_diagnostics() {
        let generated = jisp::emit_rust_detailed(
            "main.lisp",
            r#"
(def add-one
  (~ (fn (value)
       `(+ ,value 1))))

(export main (fn () (add-one 41)))
"#,
        )
        .unwrap();
        let item = generated
            .source_map
            .items
            .iter()
            .find(|item| {
                item.kind == jisp::RustItemKind::Expression
                    && !generated
                        .expansion_map
                        .origin_chain(item.source_span)
                        .is_empty()
            })
            .unwrap();
        let offset = item.generated_range.as_ref().unwrap().start;
        let json = format!(
            r#"{{"reason":"compiler-message","message":{{"level":"error","message":"synthetic rust error","spans":[{{"is_primary":true,"file_name":"src/lib.rs","byte_start":{offset}}}]}}}}"#
        );

        let rendered = remapped_cargo_errors(&json, &generated, Path::new("/tmp/src/lib.rs"));

        assert_eq!(rendered.len(), 1);
        assert!(
            rendered[0].contains("expanded from here"),
            "{}",
            rendered[0]
        );
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

    #[test]
    fn repl_keeps_definitions_between_expression_steps() {
        let (state, value) = repl_step("", "(def answer 41)").unwrap();
        assert!(value.is_none());

        let (next_state, value) = repl_step(&state, "(+ answer 1)").unwrap();
        assert_eq!(next_state, state);
        assert_eq!(value.as_deref(), Some("42"));
    }

    #[test]
    fn repl_keeps_defn_definitions_between_expression_steps() {
        let (state, value) = repl_step("", "(defn add-one (value) (+ value 1))").unwrap();
        assert!(value.is_none());

        let (next_state, value) = repl_step(&state, "(add-one 41)").unwrap();
        assert_eq!(next_state, state);
        assert_eq!(value.as_deref(), Some("42"));
    }

    #[test]
    fn lsp_definition_resolves_defn_bindings() {
        let source = "(defn add (left right) (+ left right))\n(export main (fn () (add 1 2)))";
        let definition = lsp_definition("file:///main.lisp", source, 1, 21).unwrap();

        assert_eq!(definition["range"]["start"]["line"], 0);
        assert_eq!(definition["range"]["start"]["character"], 6);
    }

    #[test]
    fn lsp_publishes_frontend_diagnostics_with_lsp_positions() {
        let diagnostics = lsp_diagnostics("file:///main.lisp", "(export main (fn () (+ 1 true)))");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["source"], "jisp");
        assert_eq!(diagnostics[0]["severity"], 1);
        assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
        assert!(diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("no overload"));
    }

    #[test]
    fn lsp_completion_includes_core_and_ui_registries() {
        let items = lsp_completion_items();
        let labels = items
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"case"));
        assert!(labels.contains(&"macro"));
        assert!(labels.contains(&"macro-import"));
        assert!(labels.contains(&"~"));
        assert!(labels.contains(&"`"));
        assert!(labels.contains(&"div"));
        assert!(labels.contains(&"class-if"));
    }

    #[test]
    fn lsp_hover_resolves_core_and_ui_forms_at_utf16_positions() {
        let hover = lsp_hover("\u{1f642} (case value)", 0, 5).unwrap();

        assert_eq!(hover["contents"]["kind"], "markdown");
        assert!(hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("**case**"));
        let ui_hover = lsp_hover("(div (class \"rounded\"))", 0, 2).unwrap();
        assert!(ui_hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("HTML generic container"));
        let directive_hover = lsp_hover("(div (class \"rounded\"))", 0, 7).unwrap();
        assert!(directive_hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("utility classes"));
        assert!(lsp_hover("(unknown value)", 0, 2).is_none());
    }

    #[test]
    fn init_creates_a_manifest_and_runnable_entry_point() {
        let directory = std::env::temp_dir().join(format!("jisp-init-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);

        init_project(&directory).unwrap();

        assert!(directory.join("jisp.toml").exists());
        let entry = std::fs::read_to_string(directory.join("main.lisp")).unwrap();
        assert_eq!(
            jisp::run_main(directory.join("main.lisp"), &entry)
                .unwrap()
                .display_string(),
            "Hello from Jisp"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn package_entry_reads_the_init_manifest() {
        let directory =
            std::env::temp_dir().join(format!("jisp-entry-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        init_project(&directory).unwrap();

        assert_eq!(
            package_entry(&directory).unwrap(),
            directory.join("main.lisp")
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn lock_project_writes_resolved_local_dependencies() {
        let directory = std::env::temp_dir().join(format!("jisp-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let app = directory.join("app");
        let math = directory.join("math");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&math).unwrap();
        std::fs::write(
            app.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { path = \"../math\" }\n",
        )
        .unwrap();
        std::fs::write(
            app.join("main.lisp"),
            "(import math \"math\")\n(export main (fn () (math.inc 41)))",
        )
        .unwrap();
        std::fs::write(
            math.join("main.lisp"),
            "(export inc (fn (value) (+ value 1)))",
        )
        .unwrap();

        lock_project(&app).unwrap();

        let lock = std::fs::read_to_string(app.join("jisp.lock")).unwrap();
        assert!(lock.contains("version = 1"));
        assert!(lock.contains("entry = "));
        assert!(lock.contains("math/main.lisp"), "{lock}");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn lock_project_preserves_used_registry_cache_entries() {
        let directory =
            std::env::temp_dir().join(format!("jisp-lock-registry-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("cache")).unwrap();
        std::fs::write(
            directory.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {\n  registry = \"jisp\",\n  package = \"math\",\n  version = \"1.2.3\",\n  checksum = \"sha256:04d7a7c591eb34cfc76a5446b45ccb8edfe1d6f13da96e841c93f823afad524d\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("main.lisp"),
            "(import math \"math\")\n(export main (fn () (math.inc 41)))",
        )
        .unwrap();
        std::fs::write(
            directory.join("cache/math.lisp"),
            "(export inc (fn (value) (+ value 1)))\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("jisp.lock"),
            "version = 1\n\n[dependencies]\nsource = \"stale/local.lisp\"\n\n[registry.math]\nregistry = \"jisp\"\npackage = \"math\"\nversion = \"1.2.3\"\nsource = \"cache/math.lisp\"\nchecksum = \"sha256:04d7a7c591eb34cfc76a5446b45ccb8edfe1d6f13da96e841c93f823afad524d\"\n",
        )
        .unwrap();

        lock_project(&directory).unwrap();

        let lock = std::fs::read_to_string(directory.join("jisp.lock")).unwrap();
        assert!(lock.contains("[registry.math]"), "{lock}");
        assert!(lock.contains("registry = \"jisp\""), "{lock}");
        assert!(lock.contains("package = \"math\""), "{lock}");
        assert!(lock.contains("version = \"1.2.3\""), "{lock}");
        assert!(lock.contains("source = \"cache/math.lisp\""), "{lock}");
        assert!(lock.contains("checksum = \"sha256:04d7a7c591eb34cfc76a5446b45ccb8edfe1d6f13da96e841c93f823afad524d\""), "{lock}");
        assert!(!lock.contains("stale/local.lisp"), "{lock}");
        assert_eq!(lock.matches("source = \"cache/math.lisp\"").count(), 1);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn lock_project_populates_registry_cache_from_local_index() {
        let directory = std::env::temp_dir().join(format!(
            "jisp-lock-registry-index-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        let project = directory.join("app");
        let registry = directory.join("registry");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(registry.join("math")).unwrap();
        let source = "(export inc (fn (value) (+ value 1)))\n";
        let checksum = sha256_checksum(source.as_bytes());
        std::fs::write(registry.join("math-1.2.3.lisp"), source).unwrap();
        std::fs::write(
            registry.join("math/1.2.3.toml"),
            format!("source = \"math-1.2.3.lisp\"\nchecksum = \"{checksum}\"\n"),
        )
        .unwrap();
        std::fs::write(
            project.join("jisp.toml"),
            format!(
                "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{\n  registry = \"../registry\",\n  package = \"math\",\n  version = \"1.2.3\",\n  checksum = \"{checksum}\"\n}}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            project.join("main.lisp"),
            "(import math \"math\")\n(export main (fn () (math.inc 41)))",
        )
        .unwrap();

        lock_project(&project).unwrap();

        let lock = std::fs::read_to_string(project.join("jisp.lock")).unwrap();
        assert!(project.join(".jisp/cache/math-1.2.3.lisp").exists());
        assert!(lock.contains("[registry.math]"), "{lock}");
        assert!(lock.contains("registry = \"../registry\""), "{lock}");
        assert!(lock.contains("package = \"math\""), "{lock}");
        assert!(lock.contains("version = \"1.2.3\""), "{lock}");
        assert!(
            lock.contains("source = \".jisp/cache/math-1.2.3.lisp\""),
            "{lock}"
        );
        assert!(
            lock.contains(&format!("checksum = \"{checksum}\"")),
            "{lock}"
        );
        let entry = std::fs::read_to_string(project.join("main.lisp")).unwrap();
        assert!(jisp::check_detailed(project.join("main.lisp"), &entry).is_ok());
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn lock_project_rejects_remote_registry_dependencies() {
        let directory = std::env::temp_dir().join(format!(
            "jisp-lock-remote-registry-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {\n  registry = \"https://packages.example.test/jisp\",\n  package = \"math\",\n  version = \"1.2.3\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("main.lisp"),
            "(import math \"math\")\n(export main (fn () (math.inc 41)))",
        )
        .unwrap();

        let error = lock_project(&directory).unwrap_err().to_string();

        assert!(error.contains("remote registry lookup and downloads are not implemented yet"));
        assert!(error.contains("https://packages.example.test/jisp"));
        let _ = std::fs::remove_dir_all(&directory);
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

#[cfg(test)]
mod lsp_test;
