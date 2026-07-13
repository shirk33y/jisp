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
use jisp_ir::{Import, LowerError, Lowerer, Module};
use jisp_types::{ImportTypeEnvironments, Inferencer, Scheme, Type, TypeVar, Unifier};
use proc_macro2::TokenStream;
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
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
    #[error("registry dependency `{dependency}` requires a `jisp.lock` cache entry")]
    RegistryDependencyUnlocked { dependency: String },
    #[error(
        "registry dependency `{dependency}` lockfile version mismatch: manifest requires {requirement}, lockfile has {locked}"
    )]
    RegistryDependencyLockVersionMismatch {
        dependency: String,
        requirement: String,
        locked: String,
    },
    #[error(
        "registry dependency `{dependency}` lockfile checksum mismatch: manifest requires {requirement}, lockfile has {locked}"
    )]
    RegistryDependencyLockChecksumMismatch {
        dependency: String,
        requirement: String,
        locked: String,
    },
    #[error(
        "registry dependency `{dependency}` checksum mismatch for `{source_path}`: expected {expected}, got {actual}"
    )]
    RegistryDependencyChecksumMismatch {
        dependency: String,
        source_path: String,
        expected: String,
        actual: String,
    },
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
    pub nodes: Vec<Node>,
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
    parse_with_options_detailed(path, text, options).map_err(|error| error.error)
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
    let name = path.display().to_string();
    let lowered = lower_path_detailed(path, syntax, text)?;
    parsed_from_lowered(name, lowered, options)
}

pub fn parse_as_detailed(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
    options: ParseOptions,
) -> Result<ParsedModule, ModuleError> {
    let name = name.into();
    let lowered = lower_as_detailed(name.clone(), syntax, text)?;
    parsed_from_lowered(name, lowered, options)
}

fn parsed_from_lowered(
    name: String,
    lowered: LoweredModule,
    options: ParseOptions,
) -> Result<ParsedModule, ModuleError> {
    let LoweredModule {
        mut sources,
        nodes,
        module,
        expansion_map,
        macro_dependencies,
    } = lowered;
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
        dependencies = merge_dependencies(macro_dependencies, resolved_dependencies);
        resolved_modules = imported_modules;
        Some(inferred)
    } else {
        dependencies = macro_dependencies;
        None
    };
    Ok(ParsedModule {
        sources,
        nodes,
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
    let name = path.display().to_string();
    let lowered = lower_path_detailed(path, syntax, text)?;
    generated_rust_from_lowered(name, lowered)
}

pub fn emit_rust_as_detailed(
    name: impl Into<String>,
    syntax: Syntax,
    text: &str,
) -> Result<GeneratedRustModule, ModuleError> {
    let name = name.into();
    let lowered = lower_as_detailed(name.clone(), syntax, text)?;
    generated_rust_from_lowered(name, lowered)
}

fn generated_rust_from_lowered(
    name: String,
    lowered: LoweredModule,
) -> Result<GeneratedRustModule, ModuleError> {
    let LoweredModule {
        mut sources,
        nodes: _,
        module,
        expansion_map,
        macro_dependencies,
    } = lowered;
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
        dependencies: merge_dependencies(macro_dependencies, dependencies),
    })
}

struct LoweredModule {
    sources: SourceMap,
    nodes: Vec<Node>,
    module: Module,
    expansion_map: ExpansionMap,
    macro_dependencies: Vec<PathBuf>,
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
        Syntax::Ws => jisp_syntax_ws::WsParser.parse_module(source, text),
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
    let nodes = expanded.nodes;
    let module = match Lowerer.lower_module(&nodes) {
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
        nodes,
        module,
        expansion_map: expanded.expansion_map,
        macro_dependencies: vec![],
    })
}

fn lower_path_detailed(
    path: &Path,
    syntax: Syntax,
    text: &str,
) -> Result<LoweredModule, ModuleError> {
    let mut sources = SourceMap::default();
    let source = sources.add(path.display().to_string(), text.to_owned());
    let nodes = match parse_nodes(source, syntax, text) {
        Ok(nodes) => nodes,
        Err(error) => return Err(ModuleError::new(sources, ExpansionMap::default(), error)),
    };
    let macro_load = match load_macro_imports(&mut sources, path, &nodes) {
        Ok(macro_load) => macro_load,
        Err(error) => return Err(ModuleError::new(sources, ExpansionMap::default(), error)),
    };
    let expanded = match jisp_expand::expand_module_with_imported_macros(&nodes, &macro_load.macros)
    {
        Ok(expanded) => expanded,
        Err(error) => {
            return Err(ModuleError::new(
                sources,
                ExpansionMap::default(),
                error.into(),
            ))
        }
    };
    let nodes = expanded.nodes;
    let module = match Lowerer.lower_module(&nodes) {
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
        nodes,
        module,
        expansion_map: expanded.expansion_map,
        macro_dependencies: macro_load.dependencies,
    })
}

fn merge_dependencies(left: Vec<PathBuf>, right: Vec<PathBuf>) -> Vec<PathBuf> {
    left.into_iter()
        .chain(right)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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
    export_schema_with_type(path, text, export, None)
}

pub fn export_schema_with_type(
    path: impl AsRef<Path>,
    text: &str,
    export: &str,
    instantiation: Option<&str>,
) -> Result<JsonValue, Error> {
    let parsed = check(path, text)?;
    if !parsed.module.exports.iter().any(|name| name == export) {
        return Err(Error::Schema(format!("`{export}` is not a public export")));
    }
    let schemes = parsed
        .types
        .as_ref()
        .ok_or_else(|| Error::Schema("type information is unavailable".to_owned()))?;
    let scheme = schemes
        .get(export)
        .ok_or_else(|| Error::Schema(format!("export `{export}` has no value definition")))?;
    let ty = if let Some(instantiation) = instantiation {
        let requested = parse_schema_type(&parsed.module, instantiation, false)?;
        let mut unifier = Unifier::default();
        unifier
            .unify(scheme.body.clone(), requested)
            .map_err(|error| Error::Schema(format!("type instantiation mismatch: {error}")))?;
        unifier.substitution.apply(&scheme.body)
    } else if !scheme.variables.is_empty() {
        return Err(Error::Schema(format!(
            "export `{export}` is polymorphic and needs an explicit instantiation"
        )));
    } else {
        scheme.body.clone()
    };
    let schema_module = schema_module(&parsed);
    let mut builder = JsonSchemaBuilder::new(&schema_module);
    let schema = builder.schema_for_type(&ty)?;
    let mut envelope = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("Jisp export `{export}`"),
        "schema": schema,
        "dependencies": parsed.dependencies,
    });
    if !builder.definitions.is_empty() {
        envelope["$defs"] = JsonValue::Object(builder.definitions);
    }
    Ok(envelope)
}

fn schema_module(parsed: &ParsedModule) -> Module {
    let mut module = parsed.module.clone();
    for imported in parsed.resolved_modules.values() {
        module.types.extend(imported.types.clone());
    }
    module
}

struct JsonSchemaBuilder<'a> {
    module: &'a Module,
    definitions: serde_json::Map<String, JsonValue>,
    building: BTreeSet<String>,
}

impl<'a> JsonSchemaBuilder<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            definitions: serde_json::Map::new(),
            building: BTreeSet::new(),
        }
    }

    fn schema_for_type(&mut self, ty: &Type) -> Result<JsonValue, Error> {
        match ty {
            Type::Null => Ok(json!({ "type": "null" })),
            Type::Bool => Ok(json!({ "type": "boolean" })),
            Type::Int => Ok(json!({ "type": "integer" })),
            Type::BigInt => Ok(json!({ "type": "string", "pattern": "^-?[0-9]+$" })),
            Type::Float => Ok(json!({ "type": "number" })),
            Type::Str => Ok(json!({ "type": "string" })),
            Type::List(item) => {
                Ok(json!({ "type": "array", "items": self.schema_for_type(item)? }))
            }
            Type::Map(value) => Ok(json!({
                "type": "object",
                "additionalProperties": self.schema_for_type(value)?,
            })),
            Type::Object(row) if row.rest.is_none() => {
                let mut properties = serde_json::Map::new();
                for (name, field) in &row.fields {
                    properties.insert(name.clone(), self.schema_for_type(field)?);
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
            Type::Named { name, arguments } if arguments.is_empty() => {
                let definition = self.ensure_named_definition(name, arguments)?;
                Ok(json!({ "$ref": format!("#/$defs/{definition}") }))
            }
            Type::Named { name, arguments } => {
                let definition = self.ensure_named_definition(name, arguments)?;
                Ok(json!({ "$ref": format!("#/$defs/{definition}") }))
            }
        }
    }

    fn ensure_named_definition(&mut self, name: &str, arguments: &[Type]) -> Result<String, Error> {
        let definition_name = named_definition_key(name, arguments)?;
        if self.definitions.contains_key(&definition_name)
            || self.building.contains(&definition_name)
        {
            return Ok(definition_name);
        }
        let declaration = find_schema_type_declaration(self.module, name)?;
        let parameters = declaration_type_parameters(self.module, declaration)?;
        if parameters.len() != arguments.len() {
            return Err(Error::Schema(format!(
                "type `{name}` expects {} argument(s), got {}",
                parameters.len(),
                arguments.len()
            )));
        }
        let substitutions = parameters
            .into_iter()
            .zip(arguments.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        self.building.insert(definition_name.clone());
        let variants = declaration
            .variants
            .iter()
            .map(|variant| {
                let fields = variant
                    .field_types
                    .iter()
                    .map(|field| {
                        let field = parse_schema_type(self.module, field, true)?;
                        let field = rewrite_schema_type_name(&field, &declaration.name, name);
                        let field = substitute_schema_type(&field, &substitutions);
                        self.schema_for_type(&field)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let count = fields.len() + 1;
                let mut prefix_items = vec![json!({ "const": variant.name })];
                prefix_items.extend(fields);
                Ok(json!({
                    "type": "array",
                    "prefixItems": prefix_items,
                    "items": false,
                    "minItems": count,
                    "maxItems": count,
                }))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        self.building.remove(&definition_name);
        self.definitions
            .insert(definition_name.clone(), json!({ "oneOf": variants }));
        Ok(definition_name)
    }
}

fn find_schema_type_declaration<'a>(
    module: &'a Module,
    name: &str,
) -> Result<&'a jisp_ir::TypeDecl, Error> {
    if let Some(declaration) = module
        .types
        .iter()
        .find(|declaration| declaration.name == name)
    {
        return Ok(declaration);
    }
    let matches = module
        .types
        .iter()
        .filter(|declaration| declaration.name.rsplit('/').next() == Some(name))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [declaration] => Ok(*declaration),
        [] => Err(Error::Schema(format!("unknown named type `{name}`"))),
        _ => Err(Error::Schema(format!("ambiguous named type `{name}`"))),
    }
}

fn parse_schema_type(module: &Module, text: &str, allow_parameters: bool) -> Result<Type, Error> {
    let mut parser = SchemaTypeParser::new(module, allow_parameters);
    parser.parse(text)
}

struct SchemaTypeParser<'a> {
    module: &'a Module,
    allow_parameters: bool,
    variables: BTreeMap<String, TypeVar>,
    order: Vec<TypeVar>,
}

impl<'a> SchemaTypeParser<'a> {
    fn new(module: &'a Module, allow_parameters: bool) -> Self {
        Self {
            module,
            allow_parameters,
            variables: BTreeMap::new(),
            order: vec![],
        }
    }

    fn parse(&mut self, text: &str) -> Result<Type, Error> {
        let text = text.trim();
        Ok(match text {
            "never" => Type::Never,
            "null" => Type::Null,
            "bool" => Type::Bool,
            "int" => Type::Int,
            "bigint" => Type::BigInt,
            "float" => Type::Float,
            "str" | "string" => Type::Str,
            _ if is_parenthesized_type(text) => {
                let inner = &text[1..text.len() - 1];
                let items = split_schema_type_items(inner)?;
                let Some((head, tail)) = items.split_first() else {
                    return Err(Error::Schema("empty type form".to_owned()));
                };
                if *head == "list" && tail.len() == 1 {
                    Type::List(Box::new(self.parse(tail[0])?))
                } else if *head == "map" && tail.len() == 2 && tail[0] == "str" {
                    Type::Map(Box::new(self.parse(tail[1])?))
                } else {
                    Type::Named {
                        name: (*head).to_owned(),
                        arguments: tail
                            .iter()
                            .map(|item| self.parse(item))
                            .collect::<Result<Vec<_>, _>>()?,
                    }
                }
            }
            _ if self.is_named_type(text) => Type::Named {
                name: text.to_owned(),
                arguments: vec![],
            },
            _ if self.allow_parameters && is_type_parameter_name(text) => {
                Type::Var(self.type_parameter(text))
            }
            _ if is_type_name(text) => Type::Named {
                name: text.to_owned(),
                arguments: vec![],
            },
            _ => return Err(Error::Schema(format!("invalid type annotation `{text}`"))),
        })
    }

    fn is_named_type(&self, text: &str) -> bool {
        self.module
            .types
            .iter()
            .any(|declaration| declaration.name == text)
    }

    fn type_parameter(&mut self, name: &str) -> TypeVar {
        if let Some(var) = self.variables.get(name) {
            return *var;
        }
        let var = TypeVar(self.variables.len() as u32);
        self.variables.insert(name.to_owned(), var);
        self.order.push(var);
        var
    }
}

fn declaration_type_parameters(
    module: &Module,
    declaration: &jisp_ir::TypeDecl,
) -> Result<Vec<TypeVar>, Error> {
    let mut parser = SchemaTypeParser::new(module, true);
    for variant in &declaration.variants {
        for field in &variant.field_types {
            parser.parse(field)?;
        }
    }
    Ok(parser.order)
}

fn substitute_schema_type(ty: &Type, substitutions: &BTreeMap<TypeVar, Type>) -> Type {
    match ty {
        Type::Var(var) => substitutions.get(var).cloned().unwrap_or(Type::Var(*var)),
        Type::List(item) => Type::List(Box::new(substitute_schema_type(item, substitutions))),
        Type::Map(value) => Type::Map(Box::new(substitute_schema_type(value, substitutions))),
        Type::Object(row) => Type::Object(jisp_types::ObjectRow {
            fields: row
                .fields
                .iter()
                .map(|(name, ty)| (name.clone(), substitute_schema_type(ty, substitutions)))
                .collect(),
            rest: row.rest,
        }),
        Type::Function {
            parameters,
            rest,
            result,
        } => Type::Function {
            parameters: parameters
                .iter()
                .map(|ty| substitute_schema_type(ty, substitutions))
                .collect(),
            rest: rest
                .as_ref()
                .map(|ty| Box::new(substitute_schema_type(ty, substitutions))),
            result: Box::new(substitute_schema_type(result, substitutions)),
        },
        Type::Named { name, arguments } => Type::Named {
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(|ty| substitute_schema_type(ty, substitutions))
                .collect(),
        },
        other => other.clone(),
    }
}

fn rewrite_schema_type_name(ty: &Type, from: &str, to: &str) -> Type {
    match ty {
        Type::List(item) => Type::List(Box::new(rewrite_schema_type_name(item, from, to))),
        Type::Map(value) => Type::Map(Box::new(rewrite_schema_type_name(value, from, to))),
        Type::Object(row) => Type::Object(jisp_types::ObjectRow {
            fields: row
                .fields
                .iter()
                .map(|(name, ty)| (name.clone(), rewrite_schema_type_name(ty, from, to)))
                .collect(),
            rest: row.rest,
        }),
        Type::Function {
            parameters,
            rest,
            result,
        } => Type::Function {
            parameters: parameters
                .iter()
                .map(|ty| rewrite_schema_type_name(ty, from, to))
                .collect(),
            rest: rest
                .as_ref()
                .map(|ty| Box::new(rewrite_schema_type_name(ty, from, to))),
            result: Box::new(rewrite_schema_type_name(result, from, to)),
        },
        Type::Named { name, arguments } => Type::Named {
            name: if name == from {
                to.to_owned()
            } else {
                name.clone()
            },
            arguments: arguments
                .iter()
                .map(|ty| rewrite_schema_type_name(ty, from, to))
                .collect(),
        },
        other => other.clone(),
    }
}

fn named_definition_key(name: &str, arguments: &[Type]) -> Result<String, Error> {
    if arguments.is_empty() {
        return Ok(name.to_owned());
    }
    let arguments = arguments
        .iter()
        .map(schema_type_key)
        .collect::<Result<Vec<_>, _>>()?
        .join("_");
    Ok(format!("{name}_{arguments}"))
}

fn schema_type_key(ty: &Type) -> Result<String, Error> {
    Ok(match ty {
        Type::Null => "null".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Int => "int".to_owned(),
        Type::BigInt => "bigint".to_owned(),
        Type::Float => "float".to_owned(),
        Type::Str => "str".to_owned(),
        Type::List(item) => format!("list_{}", schema_type_key(item)?),
        Type::Map(value) => format!("map_str_{}", schema_type_key(value)?),
        Type::Named { name, arguments } => named_definition_key(name, arguments)?,
        Type::Never => {
            return Err(Error::Schema(
                "never values have no JSON representation".to_owned(),
            ))
        }
        Type::Var(_) => return Err(Error::Schema("unresolved type variable".to_owned())),
        Type::Object(_) => "object".to_owned(),
        Type::Function { .. } => {
            return Err(Error::Schema(
                "functions have no JSON representation".to_owned(),
            ))
        }
    })
}

fn is_parenthesized_type(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

fn split_schema_type_items(text: &str) -> Result<Vec<&str>, Error> {
    let mut items = vec![];
    let mut depth = 0usize;
    let mut start = None;

    for (index, ch) in text.char_indices() {
        match ch {
            '(' => {
                if start.is_none() {
                    start = Some(index);
                }
                depth += 1;
            }
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| Error::Schema("invalid type annotation".to_owned()))?;
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if let Some(item_start) = start.take() {
                    items.push(&text[item_start..index]);
                }
            }
            _ if start.is_none() => start = Some(index),
            _ => {}
        }
    }

    if depth != 0 {
        return Err(Error::Schema("invalid type annotation".to_owned()));
    }
    if let Some(item_start) = start {
        items.push(&text[item_start..]);
    }
    Ok(items)
}

fn is_type_parameter_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_type_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '/')
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

    if let Some(path) = package_dependency_path(base, import)? {
        if path.is_dir() || path.is_file() && detect_syntax(&path).is_some() {
            return canonicalize(&path);
        }
    }

    Err(Error::ImportNotFound {
        import: import.to_owned(),
        from: importer.display().to_string(),
    })
}

fn package_dependency_path(base: &Path, import: &str) -> Result<Option<PathBuf>, Error> {
    if Path::new(import).components().count() != 1 {
        return Ok(None);
    }
    for directory in base.ancestors() {
        let manifest = directory.join("jisp.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        match manifest_dependency_spec(&text, import) {
            Some(ManifestDependencySpec::Path(path)) => return Ok(Some(directory.join(path))),
            Some(ManifestDependencySpec::Registry {
                requirement,
                checksum,
            }) => {
                return registry_dependency_path(directory, import, requirement, checksum)
                    .map(Some);
            }
            None => {}
        }
    }
    Ok(None)
}

fn registry_dependency_path(
    directory: &Path,
    dependency: &str,
    requirement: &str,
    manifest_checksum: Option<&str>,
) -> Result<PathBuf, Error> {
    let lock_path = directory.join("jisp.lock");
    let lockfile = fs::read_to_string(&lock_path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => Error::RegistryDependencyUnlocked {
            dependency: dependency.to_owned(),
        },
        _ => Error::Read {
            path: lock_path.display().to_string(),
            source: error,
        },
    })?;
    let entry = registry_lock_entry(&lockfile, dependency).ok_or_else(|| {
        Error::RegistryDependencyUnlocked {
            dependency: dependency.to_owned(),
        }
    })?;
    if entry.version != Some(requirement) {
        return Err(Error::RegistryDependencyLockVersionMismatch {
            dependency: dependency.to_owned(),
            requirement: requirement.to_owned(),
            locked: entry.version.unwrap_or("<missing>").to_owned(),
        });
    }
    if let Some(manifest_checksum) = manifest_checksum {
        if !manifest_checksum.eq_ignore_ascii_case(entry.checksum) {
            return Err(Error::RegistryDependencyLockChecksumMismatch {
                dependency: dependency.to_owned(),
                requirement: manifest_checksum.to_ascii_lowercase(),
                locked: entry.checksum.to_ascii_lowercase(),
            });
        }
    }
    let source_path = directory.join(entry.source);
    let bytes = fs::read(&source_path).map_err(|source| Error::Read {
        path: source_path.display().to_string(),
        source,
    })?;
    let actual = sha256_checksum(&bytes);
    let expected = entry.checksum.to_ascii_lowercase();
    if actual != expected {
        return Err(Error::RegistryDependencyChecksumMismatch {
            dependency: dependency.to_owned(),
            source_path: source_path.display().to_string(),
            expected,
            actual,
        });
    }
    Ok(source_path)
}

#[derive(Debug, PartialEq, Eq)]
struct RegistryLockEntry<'a> {
    version: Option<&'a str>,
    source: &'a str,
    checksum: &'a str,
}

fn registry_lock_entry<'a>(lockfile: &'a str, dependency: &str) -> Option<RegistryLockEntry<'a>> {
    let header = format!("[registry.{dependency}]");
    let mut in_section = false;
    let mut version = None;
    let mut source = None;
    let mut checksum = None;
    for line in lockfile.lines() {
        let line = line.split('#').next()?.trim();
        if line.starts_with('[') {
            if in_section {
                break;
            }
            in_section = line == header;
            continue;
        }
        if !in_section || line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let value = quoted_toml_string(value.trim())?;
        match name.trim() {
            "version" => version = Some(value),
            "source" => source = Some(value),
            "checksum" => checksum = Some(value),
            _ => {}
        }
    }
    Some(RegistryLockEntry {
        version,
        source: source?,
        checksum: checksum?,
    })
}

fn quoted_toml_string(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
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

#[derive(Debug, PartialEq, Eq)]
enum ManifestDependencySpec<'a> {
    Path(&'a str),
    Registry {
        requirement: &'a str,
        checksum: Option<&'a str>,
    },
}

fn manifest_dependency_spec<'a>(
    manifest: &'a str,
    dependency: &str,
) -> Option<ManifestDependencySpec<'a>> {
    let mut dependencies = false;
    let mut target_inline_table = false;
    let mut target_path = None;
    let mut target_requirement = None;
    let mut target_checksum = None;
    for line in manifest.lines() {
        let line = line.split('#').next()?.trim();
        if line.starts_with('[') {
            dependencies = line == "[dependencies]";
            target_inline_table = false;
            target_path = None;
            target_requirement = None;
            target_checksum = None;
            continue;
        }
        if !dependencies {
            continue;
        }
        if target_inline_table {
            if line.starts_with('}') {
                target_inline_table = false;
                if let Some(path) = target_path {
                    return Some(ManifestDependencySpec::Path(path));
                }
                if let Some(requirement) = target_requirement {
                    return Some(ManifestDependencySpec::Registry {
                        requirement,
                        checksum: target_checksum,
                    });
                }
                target_checksum = None;
                continue;
            }
            if let Some(path) = manifest_inline_value(line, "path") {
                target_path = Some(path);
            }
            if let Some(requirement) = manifest_inline_value(line, "version") {
                target_requirement = Some(requirement);
            }
            if let Some(checksum) = manifest_inline_value(line, "checksum") {
                target_checksum = Some(checksum);
            }
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != dependency {
            continue;
        }
        let value = value.trim();
        if let Some(path) = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        {
            return Some(ManifestDependencySpec::Path(path));
        }
        let inline = value.strip_prefix('{')?;
        let inline = inline.strip_suffix('}').unwrap_or(inline).trim();
        if let Some(path) = manifest_inline_value(inline, "path") {
            return Some(ManifestDependencySpec::Path(path));
        }
        if let Some(requirement) = manifest_inline_value(inline, "version") {
            return Some(ManifestDependencySpec::Registry {
                requirement,
                checksum: manifest_inline_value(inline, "checksum"),
            });
        }
        if !value.contains('}') {
            target_inline_table = true;
            target_path = None;
            target_requirement = None;
            target_checksum = None;
        }
    }
    None
}

fn manifest_inline_value<'a>(inline: &'a str, key: &str) -> Option<&'a str> {
    inline.split(',').find_map(|item| {
        let (name, value) = item.split_once('=')?;
        let value = value.trim().trim_end_matches(',').trim();
        (name.trim() == key)
            .then_some(value)?
            .strip_prefix('"')?
            .strip_suffix('"')
    })
}

fn load_module(sources: &mut SourceMap, path: &Path) -> Result<Module, Error> {
    let mut nodes = vec![];
    for file in module_source_files(path)? {
        nodes.extend(parse_file(sources, &file)?);
    }
    let macro_load = load_macro_imports(sources, path, &nodes)?;
    let expanded = jisp_expand::expand_module_with_imported_macros(&nodes, &macro_load.macros)?;
    Ok(Lowerer.lower_module(&expanded.nodes)?)
}

struct MacroImportLoad {
    macros: Vec<(String, Vec<Node>)>,
    dependencies: Vec<PathBuf>,
}

fn load_macro_imports(
    sources: &mut SourceMap,
    importer: &Path,
    nodes: &[Node],
) -> Result<MacroImportLoad, Error> {
    load_macro_imports_inner(sources, importer, nodes, &mut vec![])
}

fn load_macro_imports_inner(
    sources: &mut SourceMap,
    importer: &Path,
    nodes: &[Node],
    stack: &mut Vec<PathBuf>,
) -> Result<MacroImportLoad, Error> {
    let mut macros = vec![];
    let mut dependencies = BTreeSet::new();
    for import in macro_imports(nodes)? {
        let path = resolve_import(importer, &import.path)?;
        let key = canonicalize(&path)?;
        if stack.contains(&key) {
            return Err(Error::ImportCycle(format_cycle(stack, &key)));
        }
        stack.push(key.clone());
        let mut definitions = vec![];
        for file in module_source_files(&path)? {
            dependencies.insert(canonicalize(&file)?);
            definitions.extend(parse_file(sources, &file)?);
        }
        let nested = load_macro_imports_inner(sources, &key, &definitions, stack)?;
        dependencies.extend(nested.dependencies);
        macros.extend(nested.macros);
        macros.push((import.alias, definitions));
        stack.pop();
    }
    Ok(MacroImportLoad {
        macros,
        dependencies: dependencies.into_iter().collect(),
    })
}

fn macro_imports(nodes: &[Node]) -> Result<Vec<Import>, Error> {
    let mut imports = vec![];
    for node in nodes {
        let Some(items) = node.as_form() else {
            continue;
        };
        if items.first().and_then(Node::as_symbol) != Some("macro-import") {
            continue;
        }
        if !(items.len() == 2 || items.len() == 3) {
            return Err(lower_error(
                node.span,
                format!(
                    "macro-import expects 1 or 2 argument(s), got {}",
                    items.len().saturating_sub(1)
                ),
            ));
        }
        let (alias, path_node) = if items.len() == 2 {
            let path = items[1]
                .as_string()
                .ok_or_else(|| lower_error(items[1].span, "macro-import path must be a string"))?;
            (default_import_alias(path), &items[1])
        } else {
            (
                items[1].as_symbol().ok_or_else(|| {
                    lower_error(items[1].span, "macro-import alias must be a symbol")
                })?,
                &items[2],
            )
        };
        let path = path_node
            .as_string()
            .ok_or_else(|| lower_error(path_node.span, "macro-import path must be a string"))?;
        imports.push(Import {
            alias: alias.to_owned(),
            path: path.to_owned(),
            span: node.span,
        });
    }
    Ok(imports)
}

fn lower_error(span: Span, message: impl Into<String>) -> Error {
    Error::Lower(LowerError::new(vec![
        Diagnostic::error(span, message).with_code("JISP-LOWER")
    ]))
}

fn default_import_alias(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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
        Syntax::Ws => jisp_syntax_ws::WsParser.parse_module(source, text)?,
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
        for extension in ["lisp", "jisp", "ws", "json", "yaml", "yml"] {
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

#[cfg(test)]
mod manifest_tests {
    use super::{
        check_detailed, manifest_dependency_spec, registry_lock_entry, sha256_checksum, Error,
        ManifestDependencySpec, ModuleError, RegistryLockEntry,
    };

    #[test]
    fn manifest_dependencies_support_local_path_specs() {
        let manifest = r#"
[package]
name = "app"

[dependencies]
math = { path = "../math" }
util = "../util"
"#;

        assert_eq!(
            manifest_dependency_spec(manifest, "math"),
            Some(ManifestDependencySpec::Path("../math"))
        );
        assert_eq!(
            manifest_dependency_spec(manifest, "util"),
            Some(ManifestDependencySpec::Path("../util"))
        );
    }

    #[test]
    fn manifest_dependencies_parse_registry_specs_without_resolving_them() {
        let manifest = r#"
[dependencies]
math = {
  registry = "jisp",
  package = "math",
  version = "1.2.3",
  checksum = "sha256:abc"
}
"#;

        assert_eq!(
            manifest_dependency_spec(manifest, "math"),
            Some(ManifestDependencySpec::Registry {
                requirement: "1.2.3",
                checksum: Some("sha256:abc")
            })
        );
    }

    #[test]
    fn registry_dependencies_require_a_lockfile_cache_entry() {
        let directory = std::env::temp_dir().join(format!(
            "jisp-registry-unlocked-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"jisp\", version = \"1.2.3\", checksum = \"sha256:abc\" }\n",
        )
        .unwrap();
        let entry = directory.join("main.lisp");
        let text = "(import math \"math\")\n(export main (fn () 1))";

        let error = match check_detailed(&entry, text) {
            Ok(_) => panic!("registry dependency unexpectedly resolved"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ModuleError {
                error: Error::RegistryDependencyUnlocked {
                    dependency,
                },
                ..
            } if dependency == "math"
        ));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn registry_lock_entries_parse_source_and_checksum() {
        let lockfile = r#"
version = 1

[registry.math]
registry = "jisp"
package = "math"
version = "1.2.3"
source = "cache/math.lisp"
checksum = "sha256:abc"

[source.0]
path = "main.lisp"
"#;

        assert_eq!(
            registry_lock_entry(lockfile, "math"),
            Some(RegistryLockEntry {
                version: Some("1.2.3"),
                source: "cache/math.lisp",
                checksum: "sha256:abc"
            })
        );
    }

    #[test]
    fn registry_dependencies_resolve_from_locked_cache_with_checksum() {
        let directory =
            std::env::temp_dir().join(format!("jisp-registry-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("cache")).unwrap();
        let cached_source = "(export inc (fn (value) (+ value 1)))\n";
        std::fs::write(directory.join("cache/math.lisp"), cached_source).unwrap();
        let checksum = sha256_checksum(cached_source.as_bytes());
        std::fs::write(
            directory.join("jisp.toml"),
            format!("[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{ registry = \"jisp\", package = \"math\", version = \"1.2.3\", checksum = \"{checksum}\" }}\n"),
        )
        .unwrap();
        std::fs::write(
            directory.join("jisp.lock"),
            format!(
                "version = 1\n\n[registry.math]\nregistry = \"jisp\"\npackage = \"math\"\nversion = \"1.2.3\"\nsource = \"cache/math.lisp\"\nchecksum = \"{checksum}\"\n"
            ),
        )
        .unwrap();
        let entry = directory.join("main.lisp");
        let text = "(import math \"math\")\n(export main (fn () (math.inc 41)))";

        let parsed = check_detailed(&entry, text).unwrap();

        assert_eq!(
            parsed.dependencies,
            vec![directory.join("cache/math.lisp").canonicalize().unwrap()]
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn registry_dependencies_reject_lockfile_version_mismatches() {
        let directory =
            std::env::temp_dir().join(format!("jisp-registry-version-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("cache")).unwrap();
        let cached_source = "(export inc (fn (value) (+ value 1)))\n";
        std::fs::write(directory.join("cache/math.lisp"), cached_source).unwrap();
        let checksum = sha256_checksum(cached_source.as_bytes());
        std::fs::write(
            directory.join("jisp.toml"),
            format!("[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = {{ registry = \"jisp\", version = \"2.0.0\", checksum = \"{checksum}\" }}\n"),
        )
        .unwrap();
        std::fs::write(
            directory.join("jisp.lock"),
            format!(
                "version = 1\n\n[registry.math]\nversion = \"1.2.3\"\nsource = \"cache/math.lisp\"\nchecksum = \"{checksum}\"\n"
            ),
        )
        .unwrap();
        let entry = directory.join("main.lisp");
        let text = "(import math \"math\")\n(export main (fn () (math.inc 41)))";

        let error = match check_detailed(&entry, text) {
            Ok(_) => panic!("registry dependency unexpectedly resolved"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ModuleError {
                error: Error::RegistryDependencyLockVersionMismatch {
                    dependency,
                    requirement,
                    locked,
                },
                ..
            } if dependency == "math" && requirement == "2.0.0" && locked == "1.2.3"
        ));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn registry_dependencies_reject_manifest_lock_checksum_mismatches() {
        let directory = std::env::temp_dir().join(format!(
            "jisp-registry-lock-checksum-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("cache")).unwrap();
        let cached_source = "(export inc (fn (value) (+ value 1)))\n";
        std::fs::write(directory.join("cache/math.lisp"), cached_source).unwrap();
        let checksum = sha256_checksum(cached_source.as_bytes());
        std::fs::write(
            directory.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"jisp\", version = \"1.2.3\", checksum = \"sha256:deadbeef\" }\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("jisp.lock"),
            format!(
                "version = 1\n\n[registry.math]\nversion = \"1.2.3\"\nsource = \"cache/math.lisp\"\nchecksum = \"{checksum}\"\n"
            ),
        )
        .unwrap();
        let entry = directory.join("main.lisp");
        let text = "(import math \"math\")\n(export main (fn () (math.inc 41)))";

        let error = match check_detailed(&entry, text) {
            Ok(_) => panic!("registry dependency unexpectedly resolved"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ModuleError {
                error: Error::RegistryDependencyLockChecksumMismatch {
                    dependency,
                    requirement,
                    locked,
                },
                ..
            } if dependency == "math"
                && requirement == "sha256:deadbeef"
                && locked == checksum
        ));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn registry_dependencies_reject_checksum_mismatches() {
        let directory = std::env::temp_dir().join(format!(
            "jisp-registry-mismatch-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("cache")).unwrap();
        let cached_source = "(export inc (fn (value) (+ value 1)))\n";
        std::fs::write(directory.join("cache/math.lisp"), cached_source).unwrap();
        std::fs::write(
            directory.join("jisp.toml"),
            "[package]\nname = \"app\"\nentry = \"main.lisp\"\n\n[dependencies]\nmath = { registry = \"jisp\", version = \"1.2.3\" }\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("jisp.lock"),
            "version = 1\n\n[registry.math]\nversion = \"1.2.3\"\nsource = \"cache/math.lisp\"\nchecksum = \"sha256:deadbeef\"\n",
        )
        .unwrap();
        let entry = directory.join("main.lisp");
        let text = "(import math \"math\")\n(export main (fn () (math.inc 41)))";

        let error = match check_detailed(&entry, text) {
            Ok(_) => panic!("registry dependency unexpectedly resolved"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ModuleError {
                error: Error::RegistryDependencyChecksumMismatch {
                    dependency,
                    expected,
                    actual,
                    ..
                },
                ..
            } if dependency == "math"
                && expected == "sha256:deadbeef"
                && actual == sha256_checksum(cached_source.as_bytes())
        ));

        let _ = std::fs::remove_dir_all(&directory);
    }
}
