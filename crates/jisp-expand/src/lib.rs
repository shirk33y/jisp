use std::collections::{HashMap, HashSet};

use jisp_core::{Diagnostic, Node, NodeKind, Span};
use thiserror::Error;

const MAX_ORIGIN_DEPTH: usize = 256;
const MAX_MACRO_EXPANSIONS: usize = 1_024;

#[derive(Clone, Debug)]
struct UserMacro {
    params: Vec<String>,
    rest: Option<String>,
    template: MacroTemplate,
}

#[derive(Clone, Debug)]
enum MacroTemplate {
    Quote(Node),
    Quasiquote(Node),
}

#[derive(Clone, Debug)]
enum SyntaxValue {
    Node(Node),
    Nodes(Vec<Node>),
}

#[derive(Clone, Debug, Default)]
pub struct ExpansionMap {
    origins: HashMap<Span, Span>,
}

impl ExpansionMap {
    pub fn record(&mut self, generated: Span, origin: Span) {
        self.origins.insert(generated, origin);
    }

    pub fn origin(&self, span: Span) -> Span {
        self.origin_chain(span).last().copied().unwrap_or(span)
    }

    pub fn origin_chain(&self, span: Span) -> Vec<Span> {
        let mut current = span;
        let mut origins = Vec::new();
        for _ in 0..MAX_ORIGIN_DEPTH {
            let Some(next) = self.origins.get(&current).copied() else {
                break;
            };
            if next == current || origins.contains(&next) {
                break;
            }
            origins.push(next);
            current = next;
        }
        origins
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct ExpandedModule {
    pub nodes: Vec<Node>,
    pub expansion_map: ExpansionMap,
}

#[derive(Debug, Error)]
#[error("macro expansion failed with {count} error(s)")]
pub struct ExpandError {
    pub diagnostics: Vec<Diagnostic>,
    count: usize,
}

impl ExpandError {
    fn single(span: Span, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![Diagnostic::error(span, message).with_code("JISP-EXPAND")],
            count: 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Expander {
    expansion_map: ExpansionMap,
    macros: HashMap<String, UserMacro>,
    macro_expansions: usize,
    next_hygienic_id: usize,
}

impl Expander {
    pub fn expand_module(mut self, nodes: &[Node]) -> Result<ExpandedModule, ExpandError> {
        let mut expanded = vec![];
        for node in nodes.iter().cloned() {
            if let Some((name, user_macro)) = parse_macro_definition(&node)? {
                self.macros.insert(name, user_macro);
            } else {
                expanded.push(self.expand_node(node)?);
            }
        }
        Ok(ExpandedModule {
            nodes: expanded,
            expansion_map: self.expansion_map,
        })
    }

    fn expand_node(&mut self, node: Node) -> Result<Node, ExpandError> {
        let NodeKind::Form(items) = &node.kind else {
            return Ok(node);
        };
        let Some(head) = items.first().and_then(Node::as_symbol) else {
            return self.expand_children(node);
        };

        if let Some(user_macro) = self.macros.get(head).cloned() {
            return self.expand_macro_call(node.span, user_macro, &items[1..]);
        }

        match head {
            "quote" => {
                expect_arity(items, 2, node.span, "quote")?;
                Ok(self.originated(items[1].clone(), node.span))
            }
            "quasiquote" | "`" => {
                expect_arity(items, 2, node.span, "quasiquote")?;
                let expanded = self.expand_quasiquote(&items[1])?;
                Ok(self.originated(expanded, node.span))
            }
            "unquote" | "," | "unquote-splicing" | ",@" => Err(ExpandError::single(
                node.span,
                "unquote is only valid inside quasiquote or a string template",
            )),
            "str" | "str.lines" => self.expand_string_template(node.span, items),
            "macro" | "~" => Err(ExpandError::single(
                node.span,
                "user macro evaluation is not implemented yet",
            )),
            _ => self.expand_children(node),
        }
    }

    fn expand_macro_call(
        &mut self,
        call_span: Span,
        user_macro: UserMacro,
        arguments: &[Node],
    ) -> Result<Node, ExpandError> {
        self.macro_expansions += 1;
        if self.macro_expansions > MAX_MACRO_EXPANSIONS {
            return Err(ExpandError::single(
                call_span,
                "macro expansion exceeded 1024 steps (possible recursive macro)",
            ));
        }

        let bindings = bind_macro_arguments(&user_macro, arguments, call_span)?;
        let caller_spans = caller_spans(arguments);
        let expanded = match &user_macro.template {
            MacroTemplate::Quote(node) => node.clone(),
            MacroTemplate::Quasiquote(node) => self.expand_macro_quasiquote(node, &bindings)?,
        };
        let expanded = self.hygienize_macro_template(expanded, &caller_spans);
        let originated = self.originated(expanded, call_span);
        self.expand_node(originated)
    }

    fn hygienize_macro_template(&mut self, node: Node, caller_spans: &HashSet<Span>) -> Node {
        self.hygienize_node(node, &HashMap::new(), caller_spans)
    }

    fn hygienize_node(
        &mut self,
        node: Node,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        match node.kind {
            NodeKind::Symbol(symbol) if caller_spans.contains(&node.span) => {
                Node::new(NodeKind::Symbol(symbol), node.span)
            }
            NodeKind::Symbol(symbol) => environment
                .get(symbol.as_str())
                .map(|name| Node::symbol(name.clone(), node.span))
                .unwrap_or_else(|| Node::new(NodeKind::Symbol(symbol), node.span)),
            NodeKind::Form(items) => {
                let Some(head) = items.first().and_then(Node::as_symbol) else {
                    return Node::form(
                        items
                            .into_iter()
                            .map(|item| self.hygienize_node(item, environment, caller_spans))
                            .collect(),
                        node.span,
                    );
                };
                match head {
                    "fn" => self.hygienize_fn(node.span, items, environment, caller_spans),
                    "let" => self.hygienize_let(node.span, items, environment, caller_spans),
                    "case" => self.hygienize_case(node.span, items, environment, caller_spans),
                    "use" => self.hygienize_use(node.span, items, environment, caller_spans),
                    "do" | "if" | "and" | "or" | "not" | "list" | "obj" | "." | "str"
                    | "str.lines" => Node::form(
                        items
                            .into_iter()
                            .enumerate()
                            .map(|(index, item)| {
                                if index == 0 {
                                    item
                                } else {
                                    self.hygienize_node(item, environment, caller_spans)
                                }
                            })
                            .collect(),
                        node.span,
                    ),
                    _ => Node::form(
                        items
                            .into_iter()
                            .map(|item| self.hygienize_node(item, environment, caller_spans))
                            .collect(),
                        node.span,
                    ),
                }
            }
            kind => Node::new(kind, node.span),
        }
    }

    fn hygienize_fn(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        if items.len() < 3 {
            return self.hygienize_children(span, items, environment, caller_spans);
        }
        let mut output = Vec::with_capacity(items.len());
        output.push(items[0].clone());
        let mut inner = environment.clone();
        let params = self.hygienize_binding_list(items[1].clone(), &mut inner, caller_spans);
        output.push(params);
        output.extend(
            items
                .into_iter()
                .skip(2)
                .map(|item| self.hygienize_node(item, &inner, caller_spans)),
        );
        Node::form(output, span)
    }

    fn hygienize_let(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        if items.len() < 3 {
            return self.hygienize_children(span, items, environment, caller_spans);
        }
        let mut output = Vec::with_capacity(items.len());
        output.push(items[0].clone());
        let mut inner = environment.clone();
        let bindings = match items[1].kind.clone() {
            NodeKind::Form(bindings) => {
                let mut rewritten = Vec::with_capacity(bindings.len());
                for pair in bindings.chunks(2) {
                    let value = pair
                        .get(1)
                        .map(|value| self.hygienize_node(value.clone(), &inner, caller_spans));
                    if let Some(name) = pair.first() {
                        rewritten.push(self.hygienize_binding_name(
                            name.clone(),
                            &mut inner,
                            caller_spans,
                        ));
                    }
                    if let Some(value) = value {
                        rewritten.push(value);
                    }
                }
                Node::form(rewritten, items[1].span)
            }
            _ => self.hygienize_node(items[1].clone(), environment, caller_spans),
        };
        output.push(bindings);
        output.extend(
            items
                .into_iter()
                .skip(2)
                .map(|item| self.hygienize_node(item, &inner, caller_spans)),
        );
        Node::form(output, span)
    }

    fn hygienize_case(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        if items.len() < 3 {
            return self.hygienize_children(span, items, environment, caller_spans);
        }
        let mut output = Vec::with_capacity(items.len());
        output.push(items[0].clone());
        output.push(self.hygienize_node(items[1].clone(), environment, caller_spans));
        for branch in items.into_iter().skip(2) {
            output.push(self.hygienize_case_branch(branch, environment, caller_spans));
        }
        Node::form(output, span)
    }

    fn hygienize_case_branch(
        &mut self,
        branch: Node,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        let NodeKind::Form(items) = branch.kind else {
            return self.hygienize_node(branch, environment, caller_spans);
        };
        if items.is_empty() {
            return Node::form(items, branch.span);
        }
        let mut branch_environment = environment.clone();
        let mut output = Vec::with_capacity(items.len());
        let pattern = if is_when_pattern(&items[0]) {
            let NodeKind::Form(when_items) = items[0].kind.clone() else {
                unreachable!("is_when_pattern requires a form");
            };
            let mut rewritten = Vec::with_capacity(when_items.len());
            rewritten.push(when_items[0].clone());
            if let Some(pattern) = when_items.get(1) {
                rewritten.push(self.hygienize_pattern(
                    pattern.clone(),
                    &mut branch_environment,
                    caller_spans,
                ));
            }
            if let Some(guard) = when_items.get(2) {
                rewritten.push(self.hygienize_node(
                    guard.clone(),
                    &branch_environment,
                    caller_spans,
                ));
            }
            rewritten.extend(when_items.into_iter().skip(3));
            Node::form(rewritten, items[0].span)
        } else {
            self.hygienize_pattern(items[0].clone(), &mut branch_environment, caller_spans)
        };
        output.push(pattern);
        output.extend(
            items
                .into_iter()
                .skip(1)
                .map(|item| self.hygienize_node(item, &branch_environment, caller_spans)),
        );
        Node::form(output, branch.span)
    }

    fn hygienize_use(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        if items.len() < 4 {
            return self.hygienize_children(span, items, environment, caller_spans);
        }
        let mut output = Vec::with_capacity(items.len());
        output.push(items[0].clone());
        let mut inner = environment.clone();
        output.push(self.hygienize_use_bindings(items[1].clone(), &mut inner, caller_spans));
        output.push(self.hygienize_node(items[2].clone(), environment, caller_spans));
        output.extend(
            items
                .into_iter()
                .skip(3)
                .map(|item| self.hygienize_node(item, &inner, caller_spans)),
        );
        Node::form(output, span)
    }

    fn hygienize_children(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        Node::form(
            items
                .into_iter()
                .map(|item| self.hygienize_node(item, environment, caller_spans))
                .collect(),
            span,
        )
    }

    fn hygienize_binding_list(
        &mut self,
        node: Node,
        environment: &mut HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        let NodeKind::Form(items) = node.kind else {
            return node;
        };
        let mut output = Vec::with_capacity(items.len());
        let mut index = 0;
        while index < items.len() {
            if items[index].as_symbol() == Some("...") {
                output.push(items[index].clone());
                if let Some(rest) = items.get(index + 1) {
                    output.push(self.hygienize_binding_name(
                        rest.clone(),
                        environment,
                        caller_spans,
                    ));
                }
                output.extend(items.into_iter().skip(index + 2));
                return Node::form(output, node.span);
            }
            output.push(self.hygienize_binding_name(
                items[index].clone(),
                environment,
                caller_spans,
            ));
            index += 1;
        }
        Node::form(output, node.span)
    }

    fn hygienize_use_bindings(
        &mut self,
        node: Node,
        environment: &mut HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        if matches!(node.kind, NodeKind::Symbol(_)) {
            return self.hygienize_binding_name(node, environment, caller_spans);
        }
        let NodeKind::Form(items) = node.kind else {
            return node;
        };
        let span = node.span;
        Node::form(
            items
                .into_iter()
                .map(|item| self.hygienize_binding_name(item, environment, caller_spans))
                .collect(),
            span,
        )
    }

    fn hygienize_pattern(
        &mut self,
        node: Node,
        environment: &mut HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        match node.kind {
            NodeKind::Symbol(symbol) if symbol.as_str() == "_" => {
                Node::new(NodeKind::Symbol(symbol), node.span)
            }
            NodeKind::Symbol(_) => self.hygienize_binding_name(node, environment, caller_spans),
            NodeKind::Form(items) => {
                let Some(head) = items.first().and_then(Node::as_symbol) else {
                    return Node::form(items, node.span);
                };
                match head {
                    "list" => {
                        self.hygienize_list_pattern(node.span, items, environment, caller_spans)
                    }
                    "obj" => {
                        let mut output = vec![items[0].clone()];
                        for pair in items[1..].chunks(2) {
                            if let Some(key) = pair.first() {
                                output.push(key.clone());
                            }
                            if let Some(pattern) = pair.get(1) {
                                output.push(self.hygienize_pattern(
                                    pattern.clone(),
                                    environment,
                                    caller_spans,
                                ));
                            }
                        }
                        Node::form(output, node.span)
                    }
                    "as" => {
                        let mut output = vec![items[0].clone()];
                        if let Some(pattern) = items.get(1) {
                            output.push(self.hygienize_pattern(
                                pattern.clone(),
                                environment,
                                caller_spans,
                            ));
                        }
                        if let Some(alias) = items.get(2) {
                            output.push(self.hygienize_binding_name(
                                alias.clone(),
                                environment,
                                caller_spans,
                            ));
                        }
                        output.extend(items.into_iter().skip(3));
                        Node::form(output, node.span)
                    }
                    "or" => {
                        let mut output = vec![items[0].clone()];
                        output.extend(
                            items.into_iter().skip(1).map(|item| {
                                self.hygienize_pattern(item, environment, caller_spans)
                            }),
                        );
                        Node::form(output, node.span)
                    }
                    _ => {
                        let mut output = vec![items[0].clone()];
                        output.extend(
                            items.into_iter().skip(1).map(|item| {
                                self.hygienize_pattern(item, environment, caller_spans)
                            }),
                        );
                        Node::form(output, node.span)
                    }
                }
            }
            kind => Node::new(kind, node.span),
        }
    }

    fn hygienize_list_pattern(
        &mut self,
        span: Span,
        items: Vec<Node>,
        environment: &mut HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        let mut output = vec![items[0].clone()];
        let mut index = 1;
        while index < items.len() {
            if items[index].as_symbol() == Some("...") {
                output.push(items[index].clone());
                if let Some(rest) = items.get(index + 1) {
                    output.push(self.hygienize_binding_name(
                        rest.clone(),
                        environment,
                        caller_spans,
                    ));
                }
                output.extend(items.into_iter().skip(index + 2));
                return Node::form(output, span);
            }
            output.push(self.hygienize_pattern(items[index].clone(), environment, caller_spans));
            index += 1;
        }
        Node::form(output, span)
    }

    fn hygienize_binding_name(
        &mut self,
        node: Node,
        environment: &mut HashMap<String, String>,
        caller_spans: &HashSet<Span>,
    ) -> Node {
        let Some(name) = node.as_symbol().map(str::to_owned) else {
            return node;
        };
        if caller_spans.contains(&node.span) || name == "_" {
            return node;
        }
        let renamed = environment
            .entry(name.clone())
            .or_insert_with(|| {
                let id = self.next_hygienic_id;
                self.next_hygienic_id += 1;
                format!("__jisp_macro_{id}_{name}")
            })
            .clone();
        Node::symbol(renamed, node.span)
    }

    fn expand_macro_quasiquote(
        &self,
        node: &Node,
        bindings: &HashMap<String, SyntaxValue>,
    ) -> Result<Node, ExpandError> {
        let Some(items) = node.as_form() else {
            return Ok(node.clone());
        };

        match items.first().and_then(Node::as_symbol) {
            Some("unquote" | ",") => {
                expect_arity(items, 2, node.span, "unquote")?;
                let name = expect_macro_parameter(&items[1], "unquote")?;
                match bindings.get(name) {
                    Some(SyntaxValue::Node(value)) => Ok(value.clone()),
                    Some(SyntaxValue::Nodes(_)) => Err(ExpandError::single(
                        items[1].span,
                        format!("macro rest parameter `{name}` must be spliced with ,@"),
                    )),
                    None => Err(ExpandError::single(
                        items[1].span,
                        format!("unknown macro parameter `{name}`"),
                    )),
                }
            }
            Some("unquote-splicing" | ",@") => Err(ExpandError::single(
                node.span,
                "unquote-splicing is only valid as an item in a quasiquoted form",
            )),
            _ => {
                let mut expanded = vec![];
                for item in items {
                    if let Some(values) = macro_splice_arg(item, bindings)? {
                        expanded.extend(values);
                    } else {
                        expanded.push(self.expand_macro_quasiquote(item, bindings)?);
                    }
                }
                Ok(Node::form(expanded, node.span))
            }
        }
    }

    fn expand_children(&mut self, node: Node) -> Result<Node, ExpandError> {
        let NodeKind::Form(items) = node.kind else {
            return Ok(node);
        };
        let items = items
            .into_iter()
            .map(|item| self.expand_node(item))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Node::form(items, node.span))
    }

    fn expand_string_template(&mut self, span: Span, items: &[Node]) -> Result<Node, ExpandError> {
        let mut expanded = vec![items[0].clone()];
        for item in &items[1..] {
            let Some(parts) = item.as_form() else {
                expanded.push(item.clone());
                continue;
            };
            match parts.first().and_then(Node::as_symbol) {
                Some("," | "unquote" | ",@" | "unquote-splicing") if parts.len() == 2 => {
                    expanded.push(Node::form(
                        vec![parts[0].clone(), self.expand_node(parts[1].clone())?],
                        item.span,
                    ));
                }
                _ => expanded.push(item.clone()),
            }
        }
        Ok(Node::form(expanded, span))
    }

    fn expand_quasiquote(&mut self, node: &Node) -> Result<Node, ExpandError> {
        let NodeKind::Form(items) = &node.kind else {
            return Ok(node.clone());
        };
        let Some(head) = items.first().and_then(Node::as_symbol) else {
            return self.expand_quasiquote_form(node.span, items);
        };

        match head {
            "unquote" | "," => {
                expect_arity(items, 2, node.span, "unquote")?;
                self.expand_node(items[1].clone())
            }
            "unquote-splicing" | ",@" => Err(ExpandError::single(
                node.span,
                "unquote-splicing is only valid inside a quasiquoted form",
            )),
            _ => self.expand_quasiquote_form(node.span, items),
        }
    }

    fn expand_quasiquote_form(&mut self, span: Span, items: &[Node]) -> Result<Node, ExpandError> {
        let mut expanded = vec![];
        for item in items {
            if let Some(splice) = unquote_splice_arg(item)? {
                let value = self.expand_node(splice.clone())?;
                let Some(items) = value.as_form() else {
                    return Err(ExpandError::single(
                        splice.span,
                        "unquote-splicing expects syntax for a form",
                    ));
                };
                expanded.extend(items.iter().cloned());
            } else {
                expanded.push(self.expand_quasiquote(item)?);
            }
        }
        Ok(Node::form(expanded, span))
    }

    fn originated(&mut self, node: Node, origin: Span) -> Node {
        record_origin(&node, origin, &mut self.expansion_map);
        node
    }
}

pub fn expand_module(nodes: &[Node]) -> Result<ExpandedModule, ExpandError> {
    Expander::default().expand_module(nodes)
}

fn parse_macro_definition(node: &Node) -> Result<Option<(String, UserMacro)>, ExpandError> {
    let Some(items) = node.as_form() else {
        return Ok(None);
    };
    if items.first().and_then(Node::as_symbol) != Some("def") {
        return Ok(None);
    }
    let Some(value) = items.get(2) else {
        return Ok(None);
    };
    let Some(marker) = value
        .as_form()
        .and_then(|items| items.first())
        .and_then(Node::as_symbol)
    else {
        return Ok(None);
    };
    if !matches!(marker, "macro" | "~") {
        return Ok(None);
    }

    expect_arity(items, 3, node.span, "def")?;
    let name = items[1].as_symbol().ok_or_else(|| {
        ExpandError::single(items[1].span, "macro definition name must be a symbol")
    })?;
    let marker_items = value.as_form().expect("macro marker was checked above");
    expect_arity(marker_items, 2, value.span, "macro")?;
    let function = marker_items[1].as_form().ok_or_else(|| {
        ExpandError::single(marker_items[1].span, "macro expects an fn expression")
    })?;
    if function.first().and_then(Node::as_symbol) != Some("fn") {
        return Err(ExpandError::single(
            marker_items[1].span,
            "macro expects an fn expression",
        ));
    }
    if function.len() != 3 {
        return Err(ExpandError::single(
            marker_items[1].span,
            "macro fn expects one parameter list and one quote or quasiquote body",
        ));
    }

    let (params, rest) = parse_macro_parameters(&function[1])?;
    let body = function[2].as_form().ok_or_else(|| {
        ExpandError::single(
            function[2].span,
            "macro body must be a quote or quasiquote expression",
        )
    })?;
    let template = match body.first().and_then(Node::as_symbol) {
        Some("quote") => {
            expect_arity(body, 2, function[2].span, "quote")?;
            MacroTemplate::Quote(body[1].clone())
        }
        Some("quasiquote" | "`") => {
            expect_arity(body, 2, function[2].span, "quasiquote")?;
            MacroTemplate::Quasiquote(body[1].clone())
        }
        _ => {
            return Err(ExpandError::single(
                function[2].span,
                "macro body must be a quote or quasiquote expression",
            ))
        }
    };

    Ok(Some((
        name.to_owned(),
        UserMacro {
            params,
            rest,
            template,
        },
    )))
}

fn parse_macro_parameters(node: &Node) -> Result<(Vec<String>, Option<String>), ExpandError> {
    let params = node
        .as_form()
        .ok_or_else(|| ExpandError::single(node.span, "macro fn parameter list must be a form"))?;
    let mut names = HashSet::new();
    let mut fixed = vec![];
    let mut rest = None;
    let mut index = 0;

    while index < params.len() {
        if params[index].as_symbol() == Some("...") {
            let Some(rest_node) = params.get(index + 1) else {
                return Err(ExpandError::single(
                    params[index].span,
                    "`...` must be followed by a macro rest parameter",
                ));
            };
            if index + 2 != params.len() {
                return Err(ExpandError::single(
                    rest_node.span,
                    "macro rest parameter must be the final parameter",
                ));
            }
            let name = rest_node.as_symbol().ok_or_else(|| {
                ExpandError::single(rest_node.span, "macro rest parameter must be a symbol")
            })?;
            if !names.insert(name.to_owned()) {
                return Err(ExpandError::single(
                    rest_node.span,
                    format!("duplicate macro parameter `{name}`"),
                ));
            }
            rest = Some(name.to_owned());
            break;
        }

        let name = params[index].as_symbol().ok_or_else(|| {
            ExpandError::single(params[index].span, "macro parameter must be a symbol")
        })?;
        if !names.insert(name.to_owned()) {
            return Err(ExpandError::single(
                params[index].span,
                format!("duplicate macro parameter `{name}`"),
            ));
        }
        fixed.push(name.to_owned());
        index += 1;
    }

    Ok((fixed, rest))
}

fn bind_macro_arguments(
    user_macro: &UserMacro,
    arguments: &[Node],
    call_span: Span,
) -> Result<HashMap<String, SyntaxValue>, ExpandError> {
    let fixed_count = user_macro.params.len();
    let valid = if user_macro.rest.is_some() {
        arguments.len() >= fixed_count
    } else {
        arguments.len() == fixed_count
    };
    if !valid {
        let expectation = match user_macro.rest {
            Some(_) => format!("at least {fixed_count}"),
            None => fixed_count.to_string(),
        };
        return Err(ExpandError::single(
            call_span,
            format!(
                "macro expects {expectation} argument(s), got {}",
                arguments.len()
            ),
        ));
    }

    let mut bindings = HashMap::new();
    for (name, argument) in user_macro.params.iter().zip(arguments) {
        bindings.insert(name.clone(), SyntaxValue::Node(argument.clone()));
    }
    if let Some(rest) = &user_macro.rest {
        bindings.insert(
            rest.clone(),
            SyntaxValue::Nodes(arguments[fixed_count..].to_vec()),
        );
    }
    Ok(bindings)
}

fn caller_spans(arguments: &[Node]) -> HashSet<Span> {
    let mut spans = HashSet::new();
    for argument in arguments {
        collect_spans(argument, &mut spans);
    }
    spans
}

fn collect_spans(node: &Node, spans: &mut HashSet<Span>) {
    spans.insert(node.span);
    if let NodeKind::Form(items) = &node.kind {
        for item in items {
            collect_spans(item, spans);
        }
    }
}

fn is_when_pattern(node: &Node) -> bool {
    node.as_form()
        .is_some_and(|items| items.first().and_then(Node::as_symbol) == Some("when"))
}

fn expect_macro_parameter<'a>(node: &'a Node, form: &'static str) -> Result<&'a str, ExpandError> {
    node.as_symbol().ok_or_else(|| {
        ExpandError::single(
            node.span,
            format!("{form} in a macro template expects a macro parameter"),
        )
    })
}

fn macro_splice_arg(
    node: &Node,
    bindings: &HashMap<String, SyntaxValue>,
) -> Result<Option<Vec<Node>>, ExpandError> {
    let Some(items) = node.as_form() else {
        return Ok(None);
    };
    if !matches!(
        items.first().and_then(Node::as_symbol),
        Some("unquote-splicing" | ",@")
    ) {
        return Ok(None);
    }
    expect_arity(items, 2, node.span, "unquote-splicing")?;
    let name = expect_macro_parameter(&items[1], "unquote-splicing")?;
    match bindings.get(name) {
        Some(SyntaxValue::Nodes(values)) => Ok(Some(values.clone())),
        Some(SyntaxValue::Node(_)) => Err(ExpandError::single(
            items[1].span,
            format!("macro parameter `{name}` is not variadic and cannot be spliced"),
        )),
        None => Err(ExpandError::single(
            items[1].span,
            format!("unknown macro parameter `{name}`"),
        )),
    }
}

fn unquote_splice_arg(node: &Node) -> Result<Option<&Node>, ExpandError> {
    let Some(items) = node.as_form() else {
        return Ok(None);
    };
    match items.first().and_then(Node::as_symbol) {
        Some("unquote-splicing" | ",@") => {
            expect_arity(items, 2, node.span, "unquote-splicing")?;
            Ok(Some(&items[1]))
        }
        _ => Ok(None),
    }
}

fn expect_arity(
    items: &[Node],
    expected: usize,
    span: Span,
    form: &'static str,
) -> Result<(), ExpandError> {
    if items.len() == expected {
        Ok(())
    } else {
        Err(ExpandError::single(
            span,
            format!("{form} expects {} argument(s)", expected - 1),
        ))
    }
}

fn record_origin(node: &Node, origin: Span, map: &mut ExpansionMap) {
    map.record(node.span, origin);
    if let NodeKind::Form(items) = &node.kind {
        for item in items {
            record_origin(item, origin, map);
        }
    }
}

#[cfg(test)]
mod lib_test;
