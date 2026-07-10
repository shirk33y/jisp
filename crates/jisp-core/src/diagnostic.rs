use crate::{SourceMap, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub primary: Label,
    pub secondary: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            primary: Label {
                span,
                message: String::new(),
            },
            secondary: vec![],
            notes: vec![],
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_primary_message(mut self, message: impl Into<String>) -> Self {
        self.primary.message = message.into();
        self
    }

    pub fn with_secondary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.secondary.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn render(&self, sources: &SourceMap) -> String {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        let code = self
            .code
            .as_deref()
            .map(|code| format!("[{code}]"))
            .unwrap_or_default();

        let Some(file) = sources.get(self.primary.span.source) else {
            return format!("{severity}{code}: {}", self.message);
        };
        let (line, column) = file.line_col(self.primary.span.start);
        let line_text = file.line_text(line).unwrap_or_default();
        let width = self
            .primary
            .span
            .end
            .saturating_sub(self.primary.span.start)
            .max(1);
        let caret = format!("{}{}", " ".repeat(column.saturating_sub(1)), "^".repeat(width));

        let mut out = format!(
            "{severity}{code}: {}\n  --> {}:{line}:{column}\n   |\n{line:>3} | {line_text}\n   | {caret}",
            self.message,
            file.name(),
        );
        if !self.primary.message.is_empty() {
            out.push(' ');
            out.push_str(&self.primary.message);
        }
        for note in &self.notes {
            out.push_str("\n   = note: ");
            out.push_str(note);
        }
        out
    }
}
