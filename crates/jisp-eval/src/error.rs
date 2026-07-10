use jisp_core::Span;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
#[error("{message}")]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<Span>,
    pub stack: Vec<Span>,
}

impl RuntimeError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            stack: vec![],
        }
    }

    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            stack: vec![],
        }
    }

    pub fn push_frame(mut self, span: Span) -> Self {
        self.stack.push(span);
        self
    }
}
