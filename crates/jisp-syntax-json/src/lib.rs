use jisp_core::{Diagnostic, Node, NodeKind, ParseError, SourceId, Span, Syntax, SyntaxParser};

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonParser;

impl SyntaxParser for JsonParser {
    fn syntax(&self) -> Syntax {
        Syntax::Json
    }

    fn parse_module(&self, source: SourceId, text: &str) -> Result<Vec<Node>, ParseError> {
        let mut reader = Reader::new(source, text);
        let raw = reader.parse_value()?;
        reader.skip_ws();
        if !reader.is_eof() {
            return Err(reader.error_here("unexpected content after the JSON document"));
        }
        let node = normalize(raw)?;
        module_from_root(node)
    }
}

#[derive(Clone, Debug)]
struct RawNode {
    kind: RawKind,
    span: Span,
}

#[derive(Clone, Debug)]
enum RawKind {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<RawNode>),
}

fn normalize(raw: RawNode) -> Result<Node, ParseError> {
    match raw.kind {
        RawKind::Null => Ok(Node::new(NodeKind::Null, raw.span)),
        RawKind::Bool(value) => Ok(Node::new(NodeKind::Bool(value), raw.span)),
        RawKind::Int(value) => Ok(Node::new(NodeKind::Int(value), raw.span)),
        RawKind::Float(value) => Ok(Node::new(NodeKind::Float(value), raw.span)),
        RawKind::String(value) => Ok(Node::symbol(value, raw.span)),
        RawKind::Array(items) => normalize_array(items, raw.span),
    }
}

fn normalize_array(items: Vec<RawNode>, span: Span) -> Result<Node, ParseError> {
    let head = items.first().and_then(|item| match &item.kind {
        RawKind::String(value) => Some(value.as_str()),
        _ => None,
    });

    if head == Some("str") {
        if items.len() == 1 {
            return Ok(Node::string("", span));
        }
        if items.len() == 2 {
            if let RawKind::String(value) = &items[1].kind {
                return Ok(Node::string(value.clone(), span));
            }
        }

        let mut normalized = Vec::with_capacity(items.len());
        normalized.push(Node::symbol("str", items[0].span));
        for item in items.into_iter().skip(1) {
            match item.kind {
                RawKind::String(value) => normalized.push(Node::string(value, item.span)),
                _ => normalized.push(normalize(item)?),
            }
        }
        return Ok(Node::form(normalized, span));
    }

    if head == Some("str.lines") {
        let mut normalized = Vec::with_capacity(items.len());
        normalized.push(Node::symbol("str.lines", items[0].span));
        for item in items.into_iter().skip(1) {
            match item.kind {
                RawKind::String(value) => normalized.push(Node::string(value, item.span)),
                _ => normalized.push(normalize(item)?),
            }
        }
        return Ok(Node::form(normalized, span));
    }

    let normalized = items
        .into_iter()
        .map(normalize)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Node::form(normalized, span))
}

fn module_from_root(root: Node) -> Result<Vec<Node>, ParseError> {
    let Some(items) = root.as_form() else {
        return Err(ParseError::single(
            Diagnostic::error(root.span, "a Jisp JSON module must be an array")
                .with_code("JISP-J001"),
        ));
    };

    let is_single_top_level = items
        .first()
        .and_then(Node::as_symbol)
        .is_some_and(|head| matches!(head, "def" | "export" | "import" | "type"));

    if is_single_top_level {
        return Ok(vec![root]);
    }

    if items.iter().all(|node| node.as_form().is_some()) {
        return Ok(items.to_vec());
    }

    Err(ParseError::single(
        Diagnostic::error(
            root.span,
            "a canonical JSON module must contain top-level forms",
        )
        .with_code("JISP-J002")
        .with_note("Wrap each definition in its own array."),
    ))
}

struct Reader<'a> {
    source: SourceId,
    text: &'a str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(source: SourceId, text: &'a str) -> Self {
        Self {
            source,
            text,
            pos: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.text.len()
    }

    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        ParseError::single(
            Diagnostic::error(Span::empty(self.source, self.pos), message).with_code("JISP-J000"),
        )
    }

    fn parse_value(&mut self) -> Result<RawNode, ParseError> {
        self.skip_ws();
        let start = self.pos;
        match self.peek() {
            Some('n') => {
                self.expect_keyword("null")?;
                Ok(RawNode {
                    kind: RawKind::Null,
                    span: Span::new(self.source, start, self.pos),
                })
            }
            Some('t') => {
                self.expect_keyword("true")?;
                Ok(RawNode {
                    kind: RawKind::Bool(true),
                    span: Span::new(self.source, start, self.pos),
                })
            }
            Some('f') => {
                self.expect_keyword("false")?;
                Ok(RawNode {
                    kind: RawKind::Bool(false),
                    span: Span::new(self.source, start, self.pos),
                })
            }
            Some('"') => self.parse_string(),
            Some('[') => self.parse_array(),
            Some('{') => Err(ParseError::single(
                Diagnostic::error(
                    Span::new(self.source, start, start + 1),
                    "JSON objects are reserved and currently unsupported",
                )
                .with_code("JISP-J003")
                .with_note("Construct runtime objects with [\"obj\", key, value, ...].")
                .with_note("The future meaning of raw {} metadata is intentionally undecided."),
            )),
            Some('-' | '0'..='9') => self.parse_number(),
            Some(ch) => Err(self.error_here(format!("unexpected JSON token `{ch}`"))),
            None => Err(self.error_here("unexpected end of JSON input")),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        if self.text[self.pos..].starts_with(keyword) {
            self.pos += keyword.len();
            Ok(())
        } else {
            Err(self.error_here(format!("expected `{keyword}`")))
        }
    }

    fn parse_string(&mut self) -> Result<RawNode, ParseError> {
        let start = self.pos;
        self.bump();
        let mut escaped = false;
        while let Some(ch) = self.bump() {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => {
                    let end = self.pos;
                    let slice = &self.text[start..end];
                    let value: String = serde_json::from_str(slice).map_err(|error| {
                        ParseError::single(
                            Diagnostic::error(
                                Span::new(self.source, start, end),
                                format!("invalid JSON string: {error}"),
                            )
                            .with_code("JISP-J004"),
                        )
                    })?;
                    return Ok(RawNode {
                        kind: RawKind::String(value),
                        span: Span::new(self.source, start, end),
                    });
                }
                _ => {}
            }
        }
        Err(ParseError::single(
            Diagnostic::error(
                Span::new(self.source, start, self.pos),
                "unterminated JSON string",
            )
            .with_code("JISP-J005"),
        ))
    }

    fn parse_array(&mut self) -> Result<RawNode, ParseError> {
        let start = self.pos;
        self.bump();
        self.skip_ws();
        let mut items = vec![];
        if self.peek() == Some(']') {
            self.bump();
            return Ok(RawNode {
                kind: RawKind::Array(items),
                span: Span::new(self.source, start, self.pos),
            });
        }

        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(',') => {
                    self.skip_ws();
                    if self.peek() == Some(']') {
                        return Err(self.error_here("trailing commas are not valid canonical JSON"));
                    }
                }
                Some(']') => break,
                Some(ch) => {
                    return Err(
                        self.error_here(format!("expected `,` or `]` in array, found `{ch}`"))
                    )
                }
                None => return Err(self.error_here("unterminated JSON array")),
            }
        }

        Ok(RawNode {
            kind: RawKind::Array(items),
            span: Span::new(self.source, start, self.pos),
        })
    }

    fn parse_number(&mut self) -> Result<RawNode, ParseError> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
        {
            self.bump();
        }
        let slice = &self.text[start..self.pos];
        let span = Span::new(self.source, start, self.pos);
        if slice.contains(['.', 'e', 'E']) {
            let value = slice.parse::<f64>().map_err(|_| {
                ParseError::single(
                    Diagnostic::error(span, format!("invalid floating-point literal `{slice}`"))
                        .with_code("JISP-J006"),
                )
            })?;
            Ok(RawNode {
                kind: RawKind::Float(value),
                span,
            })
        } else {
            let value = slice.parse::<i64>().map_err(|_| {
                ParseError::single(
                    Diagnostic::error(span, format!("invalid integer literal `{slice}`"))
                        .with_code("JISP-J007"),
                )
            })?;
            Ok(RawNode {
                kind: RawKind::Int(value),
                span,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_strings_are_distinct_from_symbols() {
        let module = JsonParser
            .parse_module(
                SourceId(0),
                r#"[["def","x",["str","hello"]],["def","y","hello"]]"#,
            )
            .unwrap();
        let x = module[0].as_form().unwrap()[2].clone();
        let y = module[1].as_form().unwrap()[2].clone();
        assert!(matches!(x.kind, NodeKind::String(_)));
        assert!(matches!(y.kind, NodeKind::Symbol(_)));
    }

    #[test]
    fn preserves_string_template_fragments() {
        let module = JsonParser
            .parse_module(
                SourceId(0),
                r#"[["def","x",["str","Hello, ",[",","name"],"!"]]]"#,
            )
            .unwrap();
        let template = &module[0].as_form().unwrap()[2];
        assert_eq!(template.as_form().unwrap()[0].as_symbol(), Some("str"));
        assert_eq!(template.as_form().unwrap()[1].as_string(), Some("Hello, "));
    }

    #[test]
    fn rejects_raw_objects() {
        let error = JsonParser
            .parse_module(SourceId(0), r#"[{"type":"later"}]"#)
            .unwrap_err();
        assert!(error.diagnostics[0].message.contains("reserved"));
    }
}
