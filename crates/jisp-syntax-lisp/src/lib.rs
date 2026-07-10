use jisp_core::{
    Diagnostic, Node, NodeKind, ParseError, SourceId, Span, Syntax, SyntaxParser,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct LispParser;

impl SyntaxParser for LispParser {
    fn syntax(&self) -> Syntax {
        Syntax::Lisp
    }

    fn parse_module(&self, source: SourceId, text: &str) -> Result<Vec<Node>, ParseError> {
        let mut reader = Reader::new(source, text);
        let mut forms = vec![];
        reader.skip_layout();
        while !reader.is_eof() {
            forms.push(reader.parse_node()?);
            reader.skip_layout();
        }
        Ok(forms)
    }
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
                .with_code("JISP-L000"),
        )
    }

    fn skip_layout(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if self.peek() == Some(';') {
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
            Some('(') => self.parse_form(),
            Some('"') => self.parse_string(),
            Some('`') => self.parse_prefix("`", 1),
            Some(',') => {
                if self.text[self.pos..].starts_with(",@") {
                    self.parse_prefix(",@", 2)
                } else {
                    self.parse_prefix(",", 1)
                }
            }
            Some(')') => Err(self.error_here("unexpected `)`")),
            Some(_) => self.parse_atom(),
            None => Err(self.error_here("unexpected end of Lisp input")),
        }
    }

    fn parse_prefix(&mut self, name: &'static str, bytes: usize) -> Result<Node, ParseError> {
        let start = self.pos;
        self.pos += bytes;
        let value = self.parse_node()?;
        let head = Node::symbol(name, Span::new(self.source, start, start + bytes));
        let span = Span::new(self.source, start, value.span.end);
        Ok(Node::form(vec![head, value], span))
    }

    fn parse_form(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        self.bump();
        let mut items = vec![];
        loop {
            self.skip_layout();
            match self.peek() {
                Some(')') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(ParseError::single(
                        Diagnostic::error(
                            Span::new(self.source, start, self.pos),
                            "unterminated Lisp form",
                        )
                        .with_code("JISP-L001"),
                    ))
                }
                _ => items.push(self.parse_node()?),
            }
        }
        Ok(Node::form(
            items,
            Span::new(self.source, start, self.pos),
        ))
    }

    fn parse_string(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        self.bump();
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
                                format!("invalid string literal: {error}"),
                            )
                            .with_code("JISP-L002"),
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

        Err(ParseError::single(
            Diagnostic::error(
                Span::new(self.source, start, self.pos),
                "unterminated string literal",
            )
            .with_code("JISP-L003"),
        ))
    }

    fn parse_atom(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        while self.peek().is_some_and(|ch| {
            !ch.is_whitespace() && !matches!(ch, '(' | ')' | '"' | ';' | '`' | ',')
        }) {
            self.bump();
        }
        let value = &self.text[start..self.pos];
        let span = Span::new(self.source, start, self.pos);
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
    fn reads_multiple_top_level_forms() {
        let module = LispParser
            .parse_module(SourceId(0), "(def x 1)\n(def y 2)")
            .unwrap();
        assert_eq!(module.len(), 2);
    }

    #[test]
    fn expands_reader_sugar_to_forms() {
        let module = LispParser
            .parse_module(SourceId(0), "`(a ,b ,@c)")
            .unwrap();
        let outer = module[0].as_form().unwrap();
        assert_eq!(outer[0].as_symbol(), Some("`"));
        let quoted = outer[1].as_form().unwrap();
        assert_eq!(quoted[1].as_form().unwrap()[0].as_symbol(), Some(","));
        assert_eq!(quoted[2].as_form().unwrap()[0].as_symbol(), Some(",@"));
    }
}
