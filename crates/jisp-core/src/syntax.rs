use std::path::Path;

use thiserror::Error;

use crate::{Diagnostic, Node, SourceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    Json,
    Yaml,
    Lisp,
    Ws,
}

impl Syntax {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Lisp => "lisp",
            Self::Ws => "ws",
        }
    }
}

pub fn detect_syntax(path: impl AsRef<Path>) -> Option<Syntax> {
    match path
        .as_ref()
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Some(Syntax::Json),
        "yaml" | "yml" => Some(Syntax::Yaml),
        "lisp" | "jisp" => Some(Syntax::Lisp),
        "ws" => Some(Syntax::Ws),
        _ => None,
    }
}

#[derive(Debug, Error)]
#[error("source contains {count} syntax error(s)")]
pub struct ParseError {
    pub diagnostics: Vec<Diagnostic>,
    count: usize,
}

impl ParseError {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let count = diagnostics.len();
        Self { diagnostics, count }
    }

    pub fn single(diagnostic: Diagnostic) -> Self {
        Self::new(vec![diagnostic])
    }
}

pub trait SyntaxParser {
    fn syntax(&self) -> Syntax;
    fn parse_module(&self, source: SourceId, text: &str) -> Result<Vec<Node>, ParseError>;
}
