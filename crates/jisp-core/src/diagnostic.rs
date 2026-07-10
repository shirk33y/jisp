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

        let mut out = format!(
            "{severity}{code}: {}\n  --> {}:{line}:{column}\n   |",
            self.message,
            file.name(),
        );
        out.push('\n');
        out.push_str(&render_label(sources, &self.primary, '^', false));
        for secondary in &self.secondary {
            out.push('\n');
            out.push_str(&render_label(
                sources,
                secondary,
                '-',
                secondary.span.source != self.primary.span.source,
            ));
        }
        for note in &self.notes {
            out.push_str("\n   = note: ");
            out.push_str(note);
        }
        out
    }
}

fn render_label(sources: &SourceMap, label: &Label, marker: char, include_header: bool) -> String {
    let Some(file) = sources.get(label.span.source) else {
        return format!("   | {}{}", marker, label_suffix(label));
    };
    let (start_line, start_column) = file.line_col(label.span.start);
    let (end_line, end_column) = normalized_end(file, label.span, start_line);

    let mut out = String::new();
    if include_header {
        out.push_str(&format!(
            "  --> {}:{start_line}:{start_column}\n   |\n",
            file.name()
        ));
    }
    for line in start_line..=end_line {
        if line > start_line {
            out.push('\n');
        }
        let column = if line == start_line { start_column } else { 1 };
        let line_text = file.line_text(line).unwrap_or_default();
        let width = label_width(file, line, column, end_line, end_column);
        let suffix = if line == start_line {
            label_suffix(label)
        } else {
            String::new()
        };
        let caret = format!(
            "{}{}{}",
            " ".repeat(column.saturating_sub(1)),
            marker.to_string().repeat(width),
            suffix
        );
        out.push_str(&format!("{line:>3} | {line_text}\n   | {caret}"));
    }
    out
}

fn label_suffix(label: &Label) -> String {
    if label.message.is_empty() {
        String::new()
    } else {
        format!(" {}", label.message)
    }
}

fn normalized_end(file: &crate::SourceFile, span: Span, start_line: usize) -> (usize, usize) {
    let (line, column) = file.line_col(span.end);
    if span.end > span.start && column == 1 && line > start_line {
        let previous_line = line - 1;
        let previous_end = file
            .line_text(previous_line)
            .map(|text| text.len() + 1)
            .unwrap_or(1);
        (previous_line, previous_end)
    } else {
        (line, column)
    }
}

fn label_width(
    file: &crate::SourceFile,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
) -> usize {
    if end_line == line && end_column > column {
        return end_column - column;
    }
    file.line_text(line)
        .map(|text| text.len().saturating_sub(column.saturating_sub(1)).max(1))
        .unwrap_or(1)
}
