//! Public facade for parsing, lowering, and interpreting Jisp modules.

use std::{collections::BTreeMap, path::Path};

use jisp_core::{detect_syntax, SourceMap, Syntax, SyntaxParser};
use jisp_eval::{Evaluator, LoadedModule, RuntimeError, Value};
use jisp_ir::{LowerError, Lowerer, Module};
use jisp_types::{Inferencer, Scheme};
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
    let source = sources.add(name, text.to_owned());
    let nodes = match syntax {
        Syntax::Json => jisp_syntax_json::JsonParser.parse_module(source, text)?,
        Syntax::Yaml => jisp_syntax_yaml::YamlParser.parse_module(source, text)?,
        Syntax::Lisp => jisp_syntax_lisp::LispParser.parse_module(source, text)?,
    };
    let module = Lowerer.lower_module(&nodes)?;
    let types = if options.infer_types {
        Some(Inferencer::with_prelude().infer_module(&module)?)
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
