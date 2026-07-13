//! Public facade for parsing, lowering, and interpreting Jisp modules.

#![allow(
    clippy::result_large_err,
    reason = "detailed errors deliberately retain their source map and expansion provenance"
)]

mod native_imports;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs, io,
    path::{Path, PathBuf},
};

use jisp_core::{detect_syntax, Diagnostic, Node, SourceMap, Span, Syntax, SyntaxParser};
use jisp_eval::{Evaluator, ImportValues, LoadedModule, RuntimeError, Value};
use jisp_expand::ExpansionMap;
use jisp_ir::{LowerError, Lowerer, Module};
use jisp_types::{ImportTypeEnvironments, Inferencer, Scheme, Type};
use proc_macro2::TokenStream;
use serde_json::{json, Value as JsonValue};
use thiserror::Error;

pub use jisp_codegen_rust::{RustItemKind, RustSourceItem, RustSourceMap};
pub use jisp_core;
pub use jisp_eval;
pub use jisp_expand;
pub use jisp_ir;
pub use jisp_types;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown Jisp syntax for `{0}`")]
    UnknownSyntax(String),
    #[error(transparent)]
    Parse(#[from] jisp_core::ParseError),
    #[error(transparent)]
    Expand(#[from] jisp_expand::ExpandError),
    #[error(transparent)]
    Lower(#[from] LowerError),
    #[error(transparent)]
    Type(#[from] jisp_types::InferError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Codegen(#[from] jisp_codegen_rust::CodegenError),
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
    #[error("module does not export `main`")]
    MainNotExported,
    #[error("exported `main` does not name a top-level definition")]
    MainNotDefined,
    #[error("exported `main` must be a function with no parameters, got {0}")]
    InvalidMainType(Type),
    #[error("checked module cache is missing import `{0}`")]
    ResolvedImportMissing(String),
    #[error("cannot generate export schema: {0}")]
    Schema(String),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOptions {
    pub infer_types: bool,
}

pub struct ParsedModule {
    pub sources: SourceMap,
    pub module: Module,
    pub expansion_map: ExpansionMap,
    pub types: Option<BTreeMap<String, Scheme>>,
    pub dependencies: Vec<PathBuf>,
    pub resolved_modules: BTreeMap<PathBuf, Module>,
}

type InferredModule = (
    BTreeMap<String, Scheme>,
    Vec<PathBuf>,
    BTreeMap<PathBuf, Module>,
);

pub struct GeneratedRustModule {
    pub sources: SourceMap,
    pub expansion_map: ExpansionMap,
    pub tokens: TokenStream,
    pub source_map: RustSourceMap,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct ModuleError {
    pub sources: SourceMap,
    pub expansion_map: ExpansionMap,
    pub error: Error,
    extra_diagnostics: Vec<Diagnostic>,
}

impl ModuleError {
    fn new(sources: SourceMap, expansion_map: ExpansionMap, error: Error) -> Self {
        Self {
            sources,
            expansion_map,
            error,
            extra_diagnostics: vec![],
        }
    }

    fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.extra_diagnostics.push(diagnostic);
        self
    }

    fn runtime(sources: SourceMap, expansion_map: ExpansionMap, error: RuntimeError) -> Self {
        let mut module_error = Self::new(sources, expansion_map, Error::Runtime(error.clone()));
        if let Some(span) = error.span {
            let mut diagnostic = Diagnostic::error(span, &error.message).with_code("JISP-RUNTIME");
            let mut previous = Some(span);
            for frame in error.stack.iter().copied().take(8) {
                if previous == Some(frame) {
                    continue;
                }
                diagnostic = diagnostic.with_secondary(frame, "while evaluating this expression");
                previous = Some(frame);
            }
            module_error.extra_diagnostics.push(diagnostic);
        }
        module_error
    }

    fn type_failure(sources: SourceMap, expansion_map: ExpansionMap, failure: TypeFailure) -> Self {
        let mut module_error = Self::new(sources, expansion_map, failure.error);
        if let Some(span) = failure.span {
            let message = module_error.error.to_string();
            module_error = module_error
                .with_diagnostic(Diagnostic::error(span, message).with_code("JISP-TYPE"));
        }
        module_error
    }

    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        if !self.extra_diagnostics.is_empty() {
            return Some(&self.extra_diagnostics);
        }
        match &self.error {
            Error::Parse(error) => Some(&error.diagnostics),
            Error::Expand(error) => Some(&error.diagnostics),
            Error::Lower(error) => Some(&error.diagnostics),
            _ => None,
        }
    }

    pub fn render_diagnostics(&self) -> Option<String> {
        let diagnostics = self.diagnostics()?;
        Some(
            diagnostics
                .iter()
                .map(|diagnostic| {
                    let mut diagnostic = diagnostic.clone();
                    for origin in self.expansion_map.origin_chain(diagnostic.primary.span) {
                        diagnostic = diagnostic.with_secondary(origin, "expanded from here");
                    }
                    diagnostic.render(&self.sources)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
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
    parse_as_detailed(name, syntax, text, options).map_err(|error| error.error)
}

pub fn parse_detailed(path: impl AsRef<Path>, text: &str) -> Result<ParsedModule, ModuleError> {
    parse_with_options_detailed(path, text, ParseOptions::default())
}

pub fn parse_with_options_detailed(
    path: impl AsRef<Path>,
    text: &str,
    options: ParseOptions,
) -> Result<ParsedModule, ModuleError> {
    let path = path.as_ref();
    let syntax = detect_syntax(path).ok_or_else(|| {
        ModuleError::new(
            SourceMap::default(),
            ExpansionMap::default(),
            Error::UnknownSyntax(path.display().to_string()),
        )
    })?;
    parse_as_detailed(path.display().to_string(), syntax, text, options)
}

pub fn parse_as_detailed(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
    options: ParseOptions,
) -> Result<ParsedModule, ModuleError> {
    let name = name.into();
    let LoweredModule {
        mut sources,
        module,
        expansion_map,
    } = lower_as_detailed(name.clone(), syntax, text)?;
    let mut dependencies = vec![];
    let mut resolved_modules = BTreeMap::new();
    let types = if options.infer_types {
        let path = Path::new(&name);
        let type_result = infer_module_types(&mut sources, path, &module);
        let (inferred, resolved_dependencies, imported_modules) = match type_result {
            Ok(result) => result,
            Err(failure) => {
                return Err(ModuleError::type_failure(
                    sources,
                    expansion_map.clone(),
                    failure,
                ));
            }
        };
        dependencies = resolved_dependencies;
        resolved_modules = imported_modules;
        Some(inferred)
    } else {
        None
    };
    Ok(ParsedModule {
        sources,
        module,
        expansion_map,
        types,
        dependencies,
        resolved_modules,
    })
}

pub fn emit_rust(path: impl AsRef<Path>, text: &str) -> Result<TokenStream, Error> {
    emit_rust_detailed(path, text)
        .map(|generated| generated.tokens)
        .map_err(|error| error.error)
}

pub fn emit_rust_detailed(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<GeneratedRustModule, ModuleError> {
    let path = path.as_ref();
    let syntax = detect_syntax(path).ok_or_else(|| {
        ModuleError::new(
            SourceMap::default(),
            ExpansionMap::default(),
            Error::UnknownSyntax(path.display().to_string()),
        )
    })?;
    emit_rust_as_detailed(path.display().to_string(), syntax, text)
}

pub fn emit_rust_as_detailed(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
) -> Result<GeneratedRustModule, ModuleError> {
    let name = name.into();
    let LoweredModule {
        mut sources,
        module,
        expansion_map,
    } = lower_as_detailed(name.clone(), syntax, text)?;
    let path = Path::new(&name);
    let codegen_result = generate_rust_module(&mut sources, path, module);
    let (generated, dependencies) = match codegen_result {
        Ok(result) => result,
        Err(failure) => {
            return Err(ModuleError::type_failure(
                sources,
                expansion_map.clone(),
                failure,
            ))
        }
    };
    Ok(GeneratedRustModule {
        sources,
        expansion_map,
        tokens: generated.tokens,
        source_map: generated.source_map,
        dependencies,
    })
}

struct LoweredModule {
    sources: SourceMap,
    module: Module,
    expansion_map: ExpansionMap,
}

fn lower_as_detailed(
    name: String,
    syntax: Syntax,
    text: &str,
) -> Result<LoweredModule, ModuleError> {
    let mut sources = SourceMap::default();
    let source = sources.add(name.clone(), text.to_owned());
    let nodes = match match syntax {
        Syntax::Json => jisp_syntax_json::JsonParser.parse_module(source, text),
        Syntax::Yaml => jisp_syntax_yaml::YamlParser.parse_module(source, text),
        Syntax::Lisp => jisp_syntax_lisp::LispParser.parse_module(source, text),
    } {
        Ok(nodes) => nodes,
        Err(error) => {
            return Err(ModuleError::new(
                sources,
                ExpansionMap::default(),
                error.into(),
            ))
        }
    };
    let expanded = match jisp_expand::expand_module(&nodes) {
        Ok(expanded) => expanded,
        Err(error) => {
            return Err(ModuleError::new(
                sources,
                ExpansionMap::default(),
                error.into(),
            ))
        }
    };
    let module = match Lowerer.lower_module(&expanded.nodes) {
        Ok(module) => module,
        Err(error) => {
            return Err(ModuleError::new(
                sources,
                expanded.expansion_map,
                error.into(),
            ))
        }
    };
    Ok(LoweredModule {
        sources,
        module,
        expansion_map: expanded.expansion_map,
    })
}

fn infer_module_types(
    sources: &mut SourceMap,
    path: &Path,
    module: &Module,
) -> Result<InferredModule, TypeFailure> {
    let mut resolver = TypeResolver::new(sources);
    let imports = match resolver.import_environments(path, module) {
        Ok(imports) => imports,
        Err(error) => {
            return Err(TypeFailure {
                span: type_error_span(&error),
                error,
            })
        }
    };
    let mut inferencer = Inferencer::with_prelude();
    let types = inferencer
        .infer_module_with_imports(module, &imports)
        .map_err(|error| TypeFailure {
            span: error.span().or_else(|| module_span(module)),
            error: error.into(),
        })?;
    let (dependencies, resolved_modules) = resolver.into_parts();
    Ok((types, dependencies, resolved_modules))
}

pub(crate) struct TypeFailure {
    pub(crate) error: Error,
    pub(crate) span: Option<Span>,
}

impl From<Error> for TypeFailure {
    fn from(error: Error) -> Self {
        Self { error, span: None }
    }
}

pub(crate) fn module_span(module: &Module) -> Option<Span> {
    module
        .definitions
        .first()
        .map(|definition| definition.value.span)
        .or_else(|| module.types.first().map(|declaration| declaration.span))
        .or_else(|| module.imports.first().map(|import| import.span))
}

pub(crate) fn type_error_span(error: &Error) -> Option<Span> {
    match error {
        Error::Type(error) => error.span(),
        _ => None,
    }
}

fn generate_rust_module(
    sources: &mut SourceMap,
    path: &Path,
    module: Module,
) -> Result<(jisp_codegen_rust::GeneratedRust, Vec<PathBuf>), TypeFailure> {
    let (typed, dependencies) =
        native_imports::infer_module_with_native_imports(sources, path, module)?;
    let generated = jisp_codegen_rust::generate_detailed(&typed).map_err(Error::from)?;
    Ok((generated, dependencies))
}

pub fn check(path: impl AsRef<Path>, text: &str) -> Result<ParsedModule, Error> {
    parse_with_options(path, text, ParseOptions { infer_types: true })
}

pub fn check_detailed(path: impl AsRef<Path>, text: &str) -> Result<ParsedModule, ModuleError> {
    parse_with_options_detailed(path, text, ParseOptions { infer_types: true })
}

pub fn import_dependencies(path: impl AsRef<Path>, text: &str) -> Result<Vec<PathBuf>, Error> {
    Ok(check(path, text)?.dependencies)
}

pub fn export_schema(path: impl AsRef<Path>, text: &str, export: &str) -> Result<JsonValue, Error> {
    let parsed = check(path, text)?;
    if !parsed.module.exports.iter().any(|name| name == export) {
        return Err(Error::Schema(format!("`{export}` is not a public export")));
    }
    let schemes = parsed
        .types
        .ok_or_else(|| Error::Schema("type information is unavailable".to_owned()))?;
    let scheme = schemes
        .get(export)
        .ok_or_else(|| Error::Schema(format!("export `{export}` has no value definition")))?;
    if !scheme.variables.is_empty() {
        return Err(Error::Schema(format!(
            "export `{export}` is polymorphic and needs an explicit instantiation"
        )));
    }
    Ok(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("Jisp export `{export}`"),
        "schema": json_schema_for_type(&scheme.body)?,
        "dependencies": parsed.dependencies,
    }))
}

fn json_schema_for_type(ty: &Type) -> Result<JsonValue, Error> {
    match ty {
        Type::Null => Ok(json!({ "type": "null" })),
        Type::Bool => Ok(json!({ "type": "boolean" })),
        Type::Int => Ok(json!({ "type": "integer" })),
        Type::BigInt => Ok(json!({ "type": "string", "pattern": "^-?[0-9]+$" })),
        Type::Float => Ok(json!({ "type": "number" })),
        Type::Str => Ok(json!({ "type": "string" })),
        Type::List(item) => Ok(json!({ "type": "array", "items": json_schema_for_type(item)? })),
        Type::Object(row) if row.rest.is_none() => {
            let mut properties = serde_json::Map::new();
            for (name, field) in &row.fields {
                properties.insert(name.clone(), json_schema_for_type(field)?);
            }
            Ok(json!({
                "type": "object",
                "properties": properties,
                "required": row.fields.keys().collect::<Vec<_>>(),
                "additionalProperties": false,
            }))
        }
        Type::Object(_) => Err(Error::Schema(
            "open object rows are not JSON-schemaable".to_owned(),
        )),
        Type::Var(_) => Err(Error::Schema("unresolved type variable".to_owned())),
        Type::Never => Err(Error::Schema(
            "never values have no JSON representation".to_owned(),
        )),
        Type::Function { .. } => Err(Error::Schema(
            "functions have no JSON representation".to_owned(),
        )),
        Type::Named { name, .. } => Err(Error::Schema(format!(
            "named type `{name}` needs an explicit JSON representation"
        ))),
    }
}

pub fn evaluate(path: impl AsRef<Path>, text: &str) -> Result<LoadedModule, Error> {
    let path = path.as_ref();
    let parsed = check(path, text)?;
    let mut evaluator = Evaluator::new();
    let imports = {
        let mut resolver = ValueResolver::new(&mut evaluator, &parsed.resolved_modules);
        resolver.import_values(path, &parsed.module)?
    };
    Ok(evaluator.load_module_with_imports(&parsed.module, &imports)?)
}

pub fn run_main(path: impl AsRef<Path>, text: &str) -> Result<Value, Error> {
    run_main_detailed(path, text).map_err(|error| error.error)
}

pub fn run_main_detailed(path: impl AsRef<Path>, text: &str) -> Result<Value, ModuleError> {
    let path = path.as_ref();
    let parsed = check_detailed(path, text)?;
    if let Err(error) = validate_main(&parsed) {
        return Err(ModuleError::new(
            parsed.sources,
            parsed.expansion_map,
            error,
        ));
    }
    let mut evaluator = Evaluator::new();
    let imports_result = {
        let mut resolver = ValueResolver::new(&mut evaluator, &parsed.resolved_modules);
        resolver.import_values(path, &parsed.module)
    };
    let imports = match imports_result {
        Ok(imports) => imports,
        Err(Error::Runtime(error)) => {
            return Err(ModuleError::runtime(
                parsed.sources,
                parsed.expansion_map,
                error,
            ))
        }
        Err(error) => {
            return Err(ModuleError::new(
                parsed.sources,
                parsed.expansion_map,
                error,
            ))
        }
    };
    match evaluator.run_main_with_imports(&parsed.module, &imports) {
        Ok(value) => Ok(value),
        Err(error) => Err(ModuleError::runtime(
            parsed.sources,
            parsed.expansion_map,
            error,
        )),
    }
}

fn validate_main(parsed: &ParsedModule) -> Result<(), Error> {
    if !parsed.module.exports.iter().any(|export| export == "main") {
        return Err(Error::MainNotExported);
    }
    if !parsed
        .module
        .definitions
        .iter()
        .any(|definition| definition.name == "main")
    {
        return Err(Error::MainNotDefined);
    }

    let main = parsed
        .types
        .as_ref()
        .and_then(|types| types.get("main"))
        .expect("checked modules include schemes for exported definitions");
    match &main.body {
        Type::Function {
            parameters, rest, ..
        } if parameters.is_empty() && rest.is_none() => Ok(()),
        other => Err(Error::InvalidMainType(other.clone())),
    }
}

struct TypeResolver<'a> {
    sources: &'a mut SourceMap,
    cache: BTreeMap<PathBuf, BTreeMap<String, Scheme>>,
    modules: BTreeMap<PathBuf, Module>,
    stack: Vec<PathBuf>,
    dependencies: BTreeSet<PathBuf>,
}

impl<'a> TypeResolver<'a> {
    fn new(sources: &'a mut SourceMap) -> Self {
        Self {
            sources,
            cache: BTreeMap::new(),
            modules: BTreeMap::new(),
            stack: vec![],
            dependencies: BTreeSet::new(),
        }
    }

    fn into_parts(self) -> (Vec<PathBuf>, BTreeMap<PathBuf, Module>) {
        (self.dependencies.into_iter().collect(), self.modules)
    }

    fn import_environments(
        &mut self,
        importer: &Path,
        module: &Module,
    ) -> Result<ImportTypeEnvironments, Error> {
        let mut environments = BTreeMap::new();
        for import in &module.imports {
            let path = resolve_import(importer, &import.path)?;
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
        for file in module_source_files(&key)? {
            self.dependencies.insert(file);
        }
        let module = self.cached_module(&key)?;
        let imports = self.import_environments(&key, &module)?;
        let mut inferencer = Inferencer::with_prelude();
        let schemes = inferencer.infer_module_with_imports(&module, &imports)?;
        let exports = exported_schemes(&module, &schemes);
        self.stack.pop();

        self.cache.insert(key, exports.clone());
        Ok(exports)
    }

    fn cached_module(&mut self, path: &Path) -> Result<Module, Error> {
        if let Some(module) = self.modules.get(path) {
            return Ok(module.clone());
        }
        let module = load_module(self.sources, path)?;
        self.modules.insert(path.to_path_buf(), module.clone());
        Ok(module)
    }
}

struct ValueResolver<'a, 'b> {
    evaluator: &'a mut Evaluator,
    resolved_modules: &'b BTreeMap<PathBuf, Module>,
    cache: BTreeMap<PathBuf, HashMap<String, Value>>,
    stack: Vec<PathBuf>,
}

impl<'a, 'b> ValueResolver<'a, 'b> {
    fn new(evaluator: &'a mut Evaluator, resolved_modules: &'b BTreeMap<PathBuf, Module>) -> Self {
        Self {
            evaluator,
            resolved_modules,
            cache: BTreeMap::new(),
            stack: vec![],
        }
    }

    fn import_values(&mut self, importer: &Path, module: &Module) -> Result<ImportValues, Error> {
        let mut values = HashMap::new();
        for import in &module.imports {
            let path = resolve_import(importer, &import.path)?;
            let exports = self.evaluate_exported(&path)?;
            values.insert(import.path.clone(), exports);
        }
        Ok(values)
    }

    fn evaluate_exported(&mut self, path: &Path) -> Result<HashMap<String, Value>, Error> {
        let key = canonicalize(path)?;
        if let Some(exports) = self.cache.get(&key) {
            return Ok(exports.clone());
        }
        if self.stack.contains(&key) {
            return Err(Error::ImportCycle(format_cycle(&self.stack, &key)));
        }

        self.stack.push(key.clone());
        let module = self
            .resolved_modules
            .get(&key)
            .cloned()
            .ok_or_else(|| Error::ResolvedImportMissing(key.display().to_string()))?;
        let imports = self.import_values(&key, &module)?;
        let loaded = self.evaluator.load_module_with_imports(&module, &imports)?;
        let exports = loaded.exports;
        self.stack.pop();

        self.cache.insert(key, exports.clone());
        Ok(exports)
    }
}

fn resolve_import(importer: &Path, import: &str) -> Result<PathBuf, Error> {
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

fn load_module(sources: &mut SourceMap, path: &Path) -> Result<Module, Error> {
    let mut nodes = vec![];
    for file in module_source_files(path)? {
        nodes.extend(parse_file(sources, &file)?);
    }
    let expanded = jisp_expand::expand_module(&nodes)?;
    Ok(Lowerer.lower_module(&expanded.nodes)?)
}

fn module_source_files(path: &Path) -> Result<Vec<PathBuf>, Error> {
    if path.is_dir() {
        module_files(path)
    } else {
        Ok(vec![canonicalize(path)?])
    }
}

fn parse_file(sources: &mut SourceMap, path: &Path) -> Result<Vec<Node>, Error> {
    let syntax =
        detect_syntax(path).ok_or_else(|| Error::UnknownSyntax(path.display().to_string()))?;
    let text = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    let source = sources.add(path.display().to_string(), text.clone());
    parse_nodes(source, syntax, &text)
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
