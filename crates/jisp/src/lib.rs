//! Public facade for parsing, lowering, and interpreting Jisp modules.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use jisp_core::{detect_syntax, Node, SourceMap, Syntax, SyntaxParser};
use jisp_eval::{Evaluator, LoadedModule, RuntimeError, Value};
use jisp_ir::{LowerError, Lowerer, Module};
use jisp_types::{ImportTypeEnvironments, Inferencer, Scheme};
use thiserror::Error;

pub use jisp_core;
pub use jisp_eval;
pub use jisp_ir;
pub use jisp_macros::{file, json_file, lisp_file, yaml_file};
pub use jisp_types;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown Jisp syntax for `{0}`")]
    UnknownSyntax(String),
    #[error(transparent)]
    Parse(#[from] jisp_core::ParseError),
    #[error(transparent)]
    Lower(#[from] LowerError),
    #[error(transparent)]
    Type(#[from] jisp_types::InferError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("import `{import}` from `{from}` did not resolve to a Jisp module")]
    ImportNotFound { import: String, from: String },
    #[error("import cycle: {0}")]
    ImportCycle(String),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOptions {
    pub infer_types: bool,
}

pub struct ParsedModule {
    pub sources: SourceMap,
    pub module: Module,
    pub types: Option<BTreeMap<String, Scheme>>,
}

pub fn parse(path: impl AsRef<Path>, text: &str) -> Result<ParsedModule, Error> {
    parse_with_options(path, text, ParseOptions::default())
}

pub fn parse_with_options(
    path: impl AsRef<Path>,
    text: &str,
    options: ParseOptions,
) -> Result<ParsedModule, Error> {
    let path = path.as_ref();
    let syntax =
        detect_syntax(path).ok_or_else(|| Error::UnknownSyntax(path.display().to_string()))?;
    parse_as_with_options(path.display().to_string(), syntax, text, options)
}

pub fn parse_as(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
) -> Result<ParsedModule, Error> {
    parse_as_with_options(name, syntax, text, ParseOptions::default())
}

pub fn parse_as_with_options(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
    options: ParseOptions,
) -> Result<ParsedModule, Error> {
    let name = name.into();
    let mut sources = SourceMap::default();
    let source = sources.add(name.clone(), text.to_owned());
    let nodes = match syntax {
        Syntax::Json => jisp_syntax_json::JsonParser.parse_module(source, text)?,
        Syntax::Yaml => jisp_syntax_yaml::YamlParser.parse_module(source, text)?,
        Syntax::Lisp => jisp_syntax_lisp::LispParser.parse_module(source, text)?,
    };
    let module = Lowerer.lower_module(&nodes)?;
    let types = if options.infer_types {
        let mut resolver = TypeResolver::new(&mut sources);
        let path = Path::new(&name);
        let imports = resolver.import_environments(path, &module)?;
        Some(Inferencer::with_prelude().infer_module_with_imports(&module, &imports)?)
    } else {
        None
    };
    Ok(ParsedModule {
        sources,
        module,
        types,
    })
}

pub fn check(path: impl AsRef<Path>, text: &str) -> Result<ParsedModule, Error> {
    parse_with_options(path, text, ParseOptions { infer_types: true })
}

pub fn evaluate(path: impl AsRef<Path>, text: &str) -> Result<LoadedModule, Error> {
    let parsed = parse(path, text)?;
    Ok(Evaluator::new().load_module(&parsed.module)?)
}

pub fn run_main(path: impl AsRef<Path>, text: &str) -> Result<Value, Error> {
    let parsed = parse(path, text)?;
    Ok(Evaluator::new().run_main(&parsed.module)?)
}

struct TypeResolver<'a> {
    sources: &'a mut SourceMap,
    cache: BTreeMap<PathBuf, BTreeMap<String, Scheme>>,
    stack: Vec<PathBuf>,
}

impl<'a> TypeResolver<'a> {
    fn new(sources: &'a mut SourceMap) -> Self {
        Self {
            sources,
            cache: BTreeMap::new(),
            stack: vec![],
        }
    }

    fn import_environments(
        &mut self,
        importer: &Path,
        module: &Module,
    ) -> Result<ImportTypeEnvironments, Error> {
        let mut environments = BTreeMap::new();
        for import in &module.imports {
            let path = self.resolve_import(importer, &import.path)?;
            let exports = self.infer_exported(&path)?;
            environments.insert(import.path.clone(), exports);
        }
        Ok(environments)
    }

    fn infer_exported(&mut self, path: &Path) -> Result<BTreeMap<String, Scheme>, Error> {
        let key = canonicalize(path)?;
        if let Some(exports) = self.cache.get(&key) {
            return Ok(exports.clone());
        }
        if self.stack.contains(&key) {
            return Err(Error::ImportCycle(format_cycle(&self.stack, &key)));
        }

        self.stack.push(key.clone());
        let module = self.load_module(&key)?;
        let imports = self.import_environments(&key, &module)?;
        let schemes = Inferencer::with_prelude().infer_module_with_imports(&module, &imports)?;
        let exports = exported_schemes(&module, &schemes);
        self.stack.pop();

        self.cache.insert(key, exports.clone());
        Ok(exports)
    }

    fn load_module(&mut self, path: &Path) -> Result<Module, Error> {
        let mut nodes = vec![];
        if path.is_dir() {
            for file in module_files(path)? {
                nodes.extend(self.parse_file(&file)?);
            }
        } else {
            nodes.extend(self.parse_file(path)?);
        }
        Ok(Lowerer.lower_module(&nodes)?)
    }

    fn parse_file(&mut self, path: &Path) -> Result<Vec<Node>, Error> {
        let syntax =
            detect_syntax(path).ok_or_else(|| Error::UnknownSyntax(path.display().to_string()))?;
        let text = fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.display().to_string(),
            source,
        })?;
        let source = self.sources.add(path.display().to_string(), text.clone());
        parse_nodes(source, syntax, &text)
    }

    fn resolve_import(&self, importer: &Path, import: &str) -> Result<PathBuf, Error> {
        let base = if importer.is_dir() {
            importer
        } else {
            importer.parent().unwrap_or_else(|| Path::new("."))
        };
        let raw = Path::new(import);
        let candidate = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            base.join(raw)
        };

        for path in import_candidates(&candidate) {
            if path.is_dir() || path.is_file() && detect_syntax(&path).is_some() {
                return canonicalize(&path);
            }
        }

        Err(Error::ImportNotFound {
            import: import.to_owned(),
            from: importer.display().to_string(),
        })
    }
}

fn parse_nodes(
    source: jisp_core::SourceId,
    syntax: Syntax,
    text: &str,
) -> Result<Vec<Node>, Error> {
    Ok(match syntax {
        Syntax::Json => jisp_syntax_json::JsonParser.parse_module(source, text)?,
        Syntax::Yaml => jisp_syntax_yaml::YamlParser.parse_module(source, text)?,
        Syntax::Lisp => jisp_syntax_lisp::LispParser.parse_module(source, text)?,
    })
}

fn module_files(path: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = vec![];
    for entry in fs::read_dir(path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Read {
            path: path.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_file() && detect_syntax(&path).is_some() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn import_candidates(path: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![path.to_path_buf()];
    if path.extension().is_none() {
        for extension in ["lisp", "jisp", "json", "yaml", "yml"] {
            candidates.push(path.with_extension(extension));
        }
    }
    candidates
}

fn exported_schemes(
    module: &Module,
    schemes: &BTreeMap<String, Scheme>,
) -> BTreeMap<String, Scheme> {
    module
        .exports
        .iter()
        .filter_map(|name| {
            schemes
                .get(name)
                .cloned()
                .map(|scheme| (name.clone(), scheme))
        })
        .collect()
}

fn canonicalize(path: &Path) -> Result<PathBuf, Error> {
    path.canonicalize().map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })
}

fn format_cycle(stack: &[PathBuf], repeated: &Path) -> String {
    let mut seen = false;
    let mut paths = vec![];
    for path in stack {
        if path == repeated {
            seen = true;
        }
        if seen {
            paths.push(path.display().to_string());
        }
    }
    paths.push(repeated.display().to_string());
    paths.join(" -> ")
}
