use jisp_core::{Diagnostic, Node, NodeKind, ParseError, SourceId, Span, Syntax, SyntaxParser};
use jisp_syntax_lisp::LispParser;

#[derive(Clone, Copy, Debug, Default)]
pub struct WsParser;

impl SyntaxParser for WsParser {
    fn syntax(&self) -> Syntax {
        Syntax::Ws
    }

    fn parse_module(&self, source: SourceId, text: &str) -> Result<Vec<Node>, ParseError> {
        Reader::new(source, text).parse_module()
    }
}

struct Reader<'a> {
    source: SourceId,
    text: &'a str,
}

#[derive(Clone, Debug)]
struct Token {
    text: String,
    span: Span,
}

#[derive(Debug)]
struct Line {
    indent: isize,
    tokens: Vec<Token>,
    children: Vec<usize>,
}

impl<'a> Reader<'a> {
    fn new(source: SourceId, text: &'a str) -> Self {
        Self { source, text }
    }

    fn parse_module(&self) -> Result<Vec<Node>, ParseError> {
        let lines = self.parse_lines()?;
        let mut nodes = Vec::with_capacity(lines[0].children.len());
        for &child in &lines[0].children {
            if is_continuation(&lines[child]) {
                return Err(self.error(
                    lines[child].tokens[0].span,
                    "layout continuation `...` has no parent form",
                ));
            }
            nodes.push(self.line_to_node(&lines, child)?);
        }
        Ok(nodes)
    }

    fn parse_lines(&self) -> Result<Vec<Line>, ParseError> {
        let mut lines = vec![Line {
            indent: -2,
            tokens: vec![],
            children: vec![],
        }];
        let mut stack = vec![0usize];
        let mut line_start = 0;

        for physical in self.text.split_inclusive('\n') {
            let raw_start = line_start;
            line_start += physical.len();
            let raw = trim_line_ending(physical);
            self.reject_unsupported_whitespace(raw, raw_start)?;

            let raw = self.strip_comment(raw);
            if raw.trim().is_empty() {
                continue;
            }

            let indent = raw.bytes().take_while(|byte| *byte == b' ').count();
            if indent % 2 != 0 {
                return Err(self.error(
                    Span::empty(self.source, raw_start + indent),
                    "ws indentation must use multiples of two spaces",
                ));
            }

            let tokens = self.scan_tokens(raw, raw_start, indent)?;
            if tokens.is_empty() {
                continue;
            }
            if tokens[0].text.starts_with("...") && tokens[0].text != "..." {
                return Err(self.error(
                    tokens[0].span,
                    "line-leading ellipsis-like tokens are reserved; use `... token` for continuation",
                ));
            }

            let indent = indent as isize;
            while lines[*stack.last().unwrap()].indent >= indent {
                stack.pop();
            }
            let parent = *stack.last().unwrap();
            if lines[parent].indent + 2 < indent {
                return Err(self.error(
                    tokens[0].span,
                    "ws indentation cannot jump more than one level",
                ));
            }

            let index = lines.len();
            lines.push(Line {
                indent,
                tokens,
                children: vec![],
            });
            lines[parent].children.push(index);
            if !is_continuation(&lines[index]) {
                stack.push(index);
            }
        }

        Ok(lines)
    }

    fn line_to_node(&self, lines: &[Line], index: usize) -> Result<Node, ParseError> {
        let line = &lines[index];
        if is_continuation(line) {
            return Err(self.error(
                line.tokens[0].span,
                "layout continuation `...` cannot be used as a nested form",
            ));
        }

        let mut items = line
            .tokens
            .iter()
            .map(|token| self.token_to_node(token))
            .collect::<Result<Vec<_>, _>>()?;

        if line.children.is_empty() && items.len() == 1 {
            return Ok(items.remove(0));
        }

        let start = line.tokens[0].span.start;
        let mut end = items
            .last()
            .map(|node| node.span.end)
            .unwrap_or(line.tokens[0].span.end);

        for &child_index in &line.children {
            let child = &lines[child_index];
            if is_continuation(child) {
                if !child.children.is_empty() {
                    return Err(self.error(
                        child.tokens[0].span,
                        "layout continuation `...` cannot have children",
                    ));
                }
                if child.tokens.len() == 1 {
                    return Err(self.error(
                        child.tokens[0].span,
                        "layout continuation `...` must provide at least one token",
                    ));
                }
                for token in &child.tokens[1..] {
                    let node = self.token_to_node(token)?;
                    end = end.max(node.span.end);
                    items.push(node);
                }
            } else {
                let node = self.line_to_node(lines, child_index)?;
                end = end.max(node.span.end);
                items.push(node);
            }
        }

        Ok(Node::form(items, Span::new(self.source, start, end)))
    }

    fn scan_tokens(
        &self,
        line: &str,
        line_start: usize,
        mut index: usize,
    ) -> Result<Vec<Token>, ParseError> {
        let mut tokens = vec![];
        while index < line.len() {
            while line[index..].starts_with(' ') {
                index += 1;
                if index >= line.len() {
                    return Ok(tokens);
                }
            }

            let start = index;
            let end = if line[index..].starts_with('"') {
                self.scan_string(line, line_start, index)?
            } else if let Some(form_start) = reader_prefixed_form_start(line, index) {
                self.scan_form(line, line_start, form_start)?
            } else if line[index..].starts_with('(') {
                self.scan_form(line, line_start, index)?
            } else {
                while index < line.len() && !line[index..].starts_with(' ') {
                    index += line[index..].chars().next().unwrap().len_utf8();
                }
                index
            };

            index = end;
            tokens.push(Token {
                text: line[start..end].to_owned(),
                span: Span::new(self.source, line_start + start, line_start + end),
            });
        }
        Ok(tokens)
    }

    fn scan_string(
        &self,
        line: &str,
        line_start: usize,
        mut index: usize,
    ) -> Result<usize, ParseError> {
        let start = index;
        index += 1;
        let mut escaped = false;
        while index < line.len() {
            let ch = line[index..].chars().next().unwrap();
            index += ch.len_utf8();
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => return Ok(index),
                _ => {}
            }
        }
        Err(self.error(
            Span::new(self.source, line_start + start, line_start + index),
            "unterminated string literal",
        ))
    }

    fn scan_form(
        &self,
        line: &str,
        line_start: usize,
        mut index: usize,
    ) -> Result<usize, ParseError> {
        let start = index;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;

        while index < line.len() {
            let ch = line[index..].chars().next().unwrap();
            index += ch.len_utf8();
            if in_string {
                match (escaped, ch) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Ok(index);
                    }
                }
                _ => {}
            }
        }

        Err(self.error(
            Span::new(self.source, line_start + start, line_start + index),
            "unterminated explicit form; explicit `(...)` islands must close on the same line",
        ))
    }

    fn token_to_node(&self, token: &Token) -> Result<Node, ParseError> {
        if has_paren_outside_string(&token.text)
            && !starts_explicit_or_reader_prefixed_form(&token.text)
        {
            return Err(self.error(
                token.span,
                "parentheses inside ws atoms are not allowed; write an explicit `(...)` token",
            ));
        }

        let nodes = LispParser
            .parse_module(self.source, &token.text)
            .map_err(|error| rebase_error(error, token.span.start))?;
        match nodes.as_slice() {
            [node] => Ok(shift_node(node.clone(), token.span.start)),
            _ => Err(self.error(token.span, "ws token must contain exactly one Jisp datum")),
        }
    }

    fn reject_unsupported_whitespace(
        &self,
        line: &str,
        line_start: usize,
    ) -> Result<(), ParseError> {
        let mut in_string = false;
        let mut escaped = false;
        for (index, ch) in line.char_indices() {
            if in_string {
                match (escaped, ch) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '\t' => {
                    return Err(self.error(
                        Span::empty(self.source, line_start + index),
                        "tabs are not allowed in ws layout",
                    ))
                }
                _ if ch.is_whitespace() && ch != ' ' => {
                    return Err(self.error(
                        Span::empty(self.source, line_start + index),
                        "only ASCII spaces are allowed as ws layout whitespace",
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn strip_comment<'line>(&self, line: &'line str) -> &'line str {
        let mut in_string = false;
        let mut escaped = false;
        for (index, ch) in line.char_indices() {
            if in_string {
                match (escaped, ch) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                ';' => return &line[..index],
                '#' if is_hash_comment_marker(line, index) => return &line[..index],
                _ => {}
            }
        }
        line
    }

    fn error(&self, span: Span, message: impl Into<String>) -> ParseError {
        ParseError::single(Diagnostic::error(span, message).with_code("JISP-W000"))
    }
}

fn trim_line_ending(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn is_continuation(line: &Line) -> bool {
    line.tokens.first().is_some_and(|token| token.text == "...")
}

fn reader_prefixed_form_start(line: &str, index: usize) -> Option<usize> {
    if line[index..].starts_with('`') {
        let form = index + 1;
        return line
            .get(form..)
            .is_some_and(|rest| rest.starts_with('('))
            .then_some(form);
    }
    if line[index..].starts_with(",@") {
        let form = index + 2;
        return line
            .get(form..)
            .is_some_and(|rest| rest.starts_with('('))
            .then_some(form);
    }
    if line[index..].starts_with(',') {
        let form = index + 1;
        return line
            .get(form..)
            .is_some_and(|rest| rest.starts_with('('))
            .then_some(form);
    }
    None
}

fn starts_explicit_or_reader_prefixed_form(text: &str) -> bool {
    text.starts_with('(')
        || text.starts_with("`(")
        || text.starts_with(",(")
        || text.starts_with(",@(")
}

fn has_paren_outside_string(text: &str) -> bool {
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | ')' => return true,
            _ => {}
        }
    }
    false
}

fn is_hash_comment_marker(line: &str, index: usize) -> bool {
    let before = index == 0
        || line[..index]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
    let after_index = index + 1;
    let after = after_index >= line.len()
        || line[after_index..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace);
    before && after
}

fn shift_node(node: Node, base: usize) -> Node {
    let span = shift_span(node.span, base);
    match node.kind {
        NodeKind::Form(items) => Node::form(
            items
                .into_iter()
                .map(|item| shift_node(item, base))
                .collect(),
            span,
        ),
        kind => Node::new(kind, span),
    }
}

fn shift_span(span: Span, base: usize) -> Span {
    Span::new(span.source, span.start + base, span.end + base)
}

fn rebase_error(error: ParseError, base: usize) -> ParseError {
    ParseError::new(
        error
            .diagnostics
            .into_iter()
            .map(|mut diagnostic| {
                diagnostic.primary.span = shift_span(diagnostic.primary.span, base);
                for secondary in &mut diagnostic.secondary {
                    secondary.span = shift_span(secondary.span, base);
                }
                diagnostic
            })
            .collect(),
    )
}

#[cfg(test)]
mod layout_model_test;
#[cfg(test)]
mod lib_test;
