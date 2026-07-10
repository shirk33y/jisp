use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub source: SourceId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(source: SourceId, start: usize, end: usize) -> Self {
        Self { source, start, end }
    }

    pub const fn empty(source: SourceId, offset: usize) -> Self {
        Self::new(source, offset, offset)
    }

    pub fn merge(self, other: Self) -> Self {
        debug_assert_eq!(self.source, other.source);
        Self::new(
            self.source,
            self.start.min(other.start),
            self.end.max(other.end),
        )
    }
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    id: SourceId,
    name: Arc<str>,
    text: Arc<str>,
    line_starts: Arc<[usize]>,
}

impl SourceFile {
    pub fn new(id: SourceId, name: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
        let name = name.into();
        let text = text.into();
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            id,
            name,
            text,
            line_starts: line_starts.into(),
        }
    }

    pub fn id(&self) -> SourceId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn slice(&self, span: Span) -> &str {
        debug_assert_eq!(self.id, span.source);
        &self.text[span.start.min(self.text.len())..span.end.min(self.text.len())]
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line_index = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_index];
        (line_index + 1, offset - line_start + 1)
    }

    pub fn line_text(&self, one_based_line: usize) -> Option<&str> {
        let index = one_based_line.checked_sub(1)?;
        let start = *self.line_starts.get(index)?;
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.text.len());
        Some(self.text[start..end].trim_end_matches(['\r', '\n']))
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    next_id: u32,
    files: HashMap<SourceId, SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, name: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> SourceId {
        let id = SourceId(self.next_id);
        self.next_id += 1;
        self.files.insert(id, SourceFile::new(id, name, text));
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(&id)
    }

    pub fn span_text(&self, span: Span) -> Option<&str> {
        self.get(span.source).map(|file| file.slice(span))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_line_and_column() {
        let file = SourceFile::new(SourceId(0), "x", "one\ntwo\nthree");
        assert_eq!(file.line_col(0), (1, 1));
        assert_eq!(file.line_col(4), (2, 1));
        assert_eq!(file.line_col(8), (3, 1));
    }
}
