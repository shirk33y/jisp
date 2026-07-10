use jisp_core::{
    Diagnostic, Node, NodeKind, ParseError, SourceId, Span, Syntax, SyntaxParser,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct YamlParser;

impl SyntaxParser for YamlParser {
    fn syntax(&self) -> Syntax {
        Syntax::Yaml
    }

    fn parse_module(&self, source: SourceId, text: &str) -> Result<Vec<Node>, ParseError> {
        let mut reader = Reader::new(source, text);
        let root = reader.parse_node()?;
        reader.skip_layout();
        if !reader.is_eof() {
            return Err(reader.error_here("unexpected content after YAML-like document"));
        }
        module_from_root(root)
    }
}

fn module_from_root(root: Node) -> Result<Vec<Node>, ParseError> {
    let Some(items) = root.as_form() else {
        return Err(ParseError::single(
            Diagnostic::error(root.span, "a YAML-like Jisp module must be a flow sequence")
                .with_code("JISP-Y001"),
        ));
    };

    let single = items
        .first()
        .and_then(Node::as_symbol)
        .is_some_and(|head| matches!(head, "def" | "export" | "import" | "type"));

    if single {
        return Ok(vec![root]);
    }
    if items.iter().all(|item| item.as_form().is_some()) {
        return Ok(items.to_vec());
    }

    Err(ParseError::single(
        Diagnostic::error(
            root.span,
            "the outer YAML-like sequence must contain top-level forms",
        )
        .with_code("JISP-Y002"),
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

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        ParseError::single(
            Diagnostic::error(Span::empty(self.source, self.pos), message)
                .with_code("JISP-Y000"),
        )
    }

    fn skip_layout(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if self.peek() == Some('#') {
                while self.peek().is_some_and(|ch| ch != '\n') {
                    self.bump();
                }
                continue;
            }
            break;
        }
    }

    fn parse_node(&mut self) -> Result<Node, ParseError> {
        self.skip_layout();
        match self.peek() {
            Some('[') => self.parse_sequence(),
            Some('{') => Err(ParseError::single(
                Diagnostic::error(
                    Span::new(self.source, self.pos, self.pos + 1),
                    "YAML maps `{}` are reserved and currently unsupported",
                )
                .with_code("JISP-Y003")
                .with_note("Use [obj, \"key\", value, ...] for runtime objects."),
            )),
            Some('"') | Some('\'') => self.parse_quoted(),
            Some(_) => self.parse_plain(),
            None => Err(self.error_here("unexpected end of YAML-like input")),
        }
    }

    fn parse_sequence(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        self.bump();
        self.skip_layout();
        let mut items = vec![];

        if self.peek() == Some(']') {
            self.bump();
            return Ok(Node::form(items, Span::new(self.source, start, self.pos)));
        }

        loop {
            items.push(self.parse_node()?);
            self.skip_layout();
            match self.bump() {
                Some(',') => {
                    self.skip_layout();
                    if self.peek() == Some(']') {
                        self.bump();
                        break;
                    }
                }
                Some(']') => break,
                Some(ch) => {
                    return Err(self.error_here(format!(
                        "expected `,` or `]` in flow sequence, found `{ch}`"
                    )))
                }
                None => return Err(self.error_here("unterminated flow sequence")),
            }
        }

        Ok(Node::form(
            items,
            Span::new(self.source, start, self.pos),
        ))
    }

    fn parse_quoted(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        let quote = self.bump().unwrap();
        if quote == '"' {
            let mut escaped = false;
            while let Some(ch) = self.bump() {
                match (escaped, ch) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => {
                        let slice = &self.text[start..self.pos];
                        let value: String = serde_json::from_str(slice).map_err(|error| {
                            ParseError::single(
                                Diagnostic::error(
                                    Span::new(self.source, start, self.pos),
                                    format!("invalid quoted string: {error}"),
                                )
                                .with_code("JISP-Y004"),
                            )
                        })?;
                        return Ok(Node::string(
                            value,
                            Span::new(self.source, start, self.pos),
                        ));
                    }
                    _ => {}
                }
            }
        } else {
            let mut value = String::new();
            while let Some(ch) = self.bump() {
                if ch == '\'' {
                    if self.peek() == Some('\'') {
                        self.bump();
                        value.push('\'');
                        continue;
                    }
                    return Ok(Node::string(
                        value,
                        Span::new(self.source, start, self.pos),
                    ));
                }
                value.push(ch);
            }
        }

        Err(ParseError::single(
            Diagnostic::error(
                Span::new(self.source, start, self.pos),
                "unterminated quoted scalar",
            )
            .with_code("JISP-Y005"),
        ))
    }

    fn parse_plain(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        while self.peek().is_some_and(|ch| {
            !ch.is_whitespace() && !matches!(ch, ',' | ']' | '[' | '{' | '}' | '#')
        }) {
            self.bump();
        }
        let value = &self.text[start..self.pos];
        let span = Span::new(self.source, start, self.pos);
        if value.is_empty() {
            return Err(self.error_here("expected a scalar"));
        }

        let kind = match value {
            "null" => NodeKind::Null,
            "true" => NodeKind::Bool(true),
            "false" => NodeKind::Bool(false),
            _ if looks_like_float(value) => value
                .parse::<f64>()
                .map(NodeKind::Float)
                .unwrap_or_else(|_| NodeKind::Symbol(value.into())),
            _ if looks_like_int(value) => value
                .parse::<i64>()
                .map(NodeKind::Int)
                .unwrap_or_else(|_| NodeKind::Symbol(value.into())),
            _ => NodeKind::Symbol(value.into()),
        };
        Ok(Node::new(kind, span))
    }
}

fn looks_like_int(value: &str) -> bool {
    let rest = value.strip_prefix('-').unwrap_or(value);
    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
}

fn looks_like_float(value: &str) -> bool {
    value.contains(['.', 'e', 'E'])
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_scalars_are_strings_and_plain_scalars_are_symbols() {
        let module = YamlParser
            .parse_module(SourceId(0), r#"[[def, x, "hello"], [def, y, hello]]"#)
            .unwrap();
        let x = &module[0].as_form().unwrap()[2];
        let y = &module[1].as_form().unwrap()[2];
        assert!(matches!(x.kind, NodeKind::String(_)));
        assert!(matches!(y.kind, NodeKind::Symbol(_)));
    }

    #[test]
    fn supports_comments() {
        let module = YamlParser
            .parse_module(SourceId(0), "[\n # comment\n [def, x, 1]\n]")
            .unwrap();
        assert_eq!(module.len(), 1);
    }

    #[test]
    fn rejects_maps_until_metadata_is_designed() {
        assert!(YamlParser
            .parse_module(SourceId(0), "[{type: x}]")
            .is_err());
    }
}
