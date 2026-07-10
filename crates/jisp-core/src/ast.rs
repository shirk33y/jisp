use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Span;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(Arc<str>);

impl Symbol {
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}

impl Node {
    pub const fn new(kind: NodeKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn symbol(value: impl Into<Arc<str>>, span: Span) -> Self {
        Self::new(NodeKind::Symbol(Symbol::new(value)), span)
    }

    pub fn string(value: impl Into<Arc<str>>, span: Span) -> Self {
        Self::new(NodeKind::String(value.into()), span)
    }

    pub fn form(items: Vec<Node>, span: Span) -> Self {
        Self::new(NodeKind::Form(items), span)
    }

    pub fn as_symbol(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Symbol(symbol) => Some(symbol.as_str()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_form(&self) -> Option<&[Node]> {
        match &self.kind {
            NodeKind::Form(items) => Some(items),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Symbol(Symbol),
    String(Arc<str>),
    Form(Vec<Node>),
}
