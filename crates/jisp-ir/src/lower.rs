use std::collections::BTreeMap;

use jisp_core::{Diagnostic, Node, NodeKind, Span};
use thiserror::Error;

use crate::{
    CaseBranch, Definition, Expr, ExprKind, Import, Literal, Module, Pattern, StringPart, TypeDecl,
    VariantDecl,
};

#[derive(Debug, Error)]
#[error("lowering failed with {count} error(s)")]
pub struct LowerError {
    pub diagnostics: Vec<Diagnostic>,
    count: usize,
}

impl LowerError {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let count = diagnostics.len();
        Self { diagnostics, count }
    }

    fn single(diagnostic: Diagnostic) -> Self {
        Self::new(vec![diagnostic])
    }
}

#[derive(Clone, Debug, Default)]
pub struct Lowerer;

impl Lowerer {
    pub fn lower_module(&self, nodes: &[Node]) -> Result<Module, LowerError> {
        let mut module = Module::empty();
        let mut diagnostics = vec![];

        for node in nodes {
            if let Err(error) = self.lower_top_level(node, &mut module) {
                diagnostics.extend(error.diagnostics);
            }
        }
        validate_module_names(&module, &mut diagnostics);

        if diagnostics.is_empty() {
            Ok(module)
        } else {
            Err(LowerError::new(diagnostics))
        }
    }

    fn lower_top_level(&self, node: &Node, module: &mut Module) -> Result<(), LowerError> {
        let items = expect_form(node, "top-level item")?;
        let Some(head) = items.first().and_then(Node::as_symbol) else {
            return Err(error(node.span, "top-level form must start with a symbol"));
        };

        match head {
            "def" => {
                expect_arity(items, 3, 3, node.span, "def")?;
                let name = expect_symbol(&items[1], "definition name")?.to_owned();
                let value = self.lower_expr(&items[2])?;
                module.definitions.push(Definition {
                    name,
                    public: false,
                    value,
                    span: node.span,
                });
                Ok(())
            }
            "export" => {
                expect_arity(items, 2, 3, node.span, "export")?;
                let name = expect_symbol(&items[1], "export name")?.to_owned();
                if items.len() == 3 {
                    let value = self.lower_expr(&items[2])?;
                    module.definitions.push(Definition {
                        name: name.clone(),
                        public: true,
                        value,
                        span: node.span,
                    });
                }
                if !module.exports.contains(&name) {
                    module.exports.push(name);
                }
                Ok(())
            }
            "import" => {
                expect_arity(items, 2, 3, node.span, "import")?;
                let (alias, path_node) = if items.len() == 2 {
                    let path = expect_string(&items[1], "import path")?;
                    (default_alias(path), &items[1])
                } else {
                    (
                        expect_symbol(&items[1], "import alias")?.to_owned(),
                        &items[2],
                    )
                };
                let path = expect_string(path_node, "import path")?.to_owned();
                module.imports.push(Import {
                    alias,
                    path,
                    span: node.span,
                });
                Ok(())
            }
            "type" => {
                if items.len() < 3 {
                    return Err(error(
                        node.span,
                        "type expects a name and at least one variant",
                    ));
                }
                let name = expect_symbol(&items[1], "type name")?.to_owned();
                let variants = items[2..]
                    .iter()
                    .map(lower_variant)
                    .collect::<Result<Vec<_>, _>>()?;
                module.types.push(TypeDecl {
                    name,
                    variants,
                    span: node.span,
                });
                Ok(())
            }
            _ => Err(error(
                node.span,
                format!(
                    "top-level expression `{head}` is not allowed; use def, export, import, or type"
                ),
            )),
        }
    }

    pub fn lower_expr(&self, node: &Node) -> Result<Expr, LowerError> {
        match &node.kind {
            NodeKind::Null => Ok(Expr::new(ExprKind::Literal(Literal::Null), node.span)),
            NodeKind::Bool(value) => Ok(Expr::new(
                ExprKind::Literal(Literal::Bool(*value)),
                node.span,
            )),
            NodeKind::Int(value) => Ok(Expr::new(
                ExprKind::Literal(Literal::Int(*value)),
                node.span,
            )),
            NodeKind::Float(value) => Ok(Expr::new(
                ExprKind::Literal(Literal::Float(*value)),
                node.span,
            )),
            NodeKind::String(value) => Ok(Expr::new(
                ExprKind::Literal(Literal::String(value.to_string())),
                node.span,
            )),
            NodeKind::Symbol(symbol) => {
                Ok(Expr::new(ExprKind::Name(symbol.to_string()), node.span))
            }
            NodeKind::Form(items) => self.lower_form(node.span, items),
        }
    }

    fn lower_form(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if items.is_empty() {
            return Err(error(span, "an empty form cannot be evaluated"));
        }

        match items[0].as_symbol() {
            Some("fn") => self.lower_fn(span, items),
            Some("let") => self.lower_let(span, items),
            Some("do") => Ok(Expr::new(
                ExprKind::Do(
                    items[1..]
                        .iter()
                        .map(|node| self.lower_expr(node))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Some("if") => self.lower_if(span, items),
            Some("and") => Ok(Expr::new(
                ExprKind::And(
                    items[1..]
                        .iter()
                        .map(|node| self.lower_expr(node))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Some("or") => Ok(Expr::new(
                ExprKind::Or(
                    items[1..]
                        .iter()
                        .map(|node| self.lower_expr(node))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Some("not") => {
                expect_arity(items, 2, 2, span, "not")?;
                Ok(Expr::new(
                    ExprKind::Not(Box::new(self.lower_expr(&items[1])?)),
                    span,
                ))
            }
            Some("list") => Ok(Expr::new(
                ExprKind::List(
                    items[1..]
                        .iter()
                        .map(|node| self.lower_expr(node))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Some("obj") => self.lower_obj(span, items),
            Some(".") => {
                expect_arity(items, 3, 3, span, ".")?;
                Ok(Expr::new(
                    ExprKind::Field {
                        object: Box::new(self.lower_expr(&items[1])?),
                        key: Box::new(self.lower_expr(&items[2])?),
                    },
                    span,
                ))
            }
            Some("str") => self.lower_string_template(span, false, &items[1..]),
            Some("str.lines") => self.lower_string_template(span, true, &items[1..]),
            Some("case") => self.lower_case(span, items),
            Some("use") => self.lower_use(span, items),
            Some("quote" | "quasiquote" | "`" | "macro" | "~") => Err(error(
                span,
                "macro-phase syntax must be expanded before lowering",
            )),
            Some("unquote" | "," | "unquote-splicing" | ",@") => Err(error(
                span,
                "unquote is only valid inside quasiquote or a string template",
            )),
            _ => self.lower_call(span, items),
        }
    }

    fn lower_fn(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if items.len() < 3 {
            return Err(error(span, "fn expects a parameter list and a body"));
        }
        let params_node = expect_form(&items[1], "fn parameter list")?;
        let mut params = vec![];
        let mut rest = None;
        let mut index = 0;

        while index < params_node.len() {
            if params_node[index].as_symbol() == Some("...") {
                let Some(name_node) = params_node.get(index + 1) else {
                    return Err(error(
                        params_node[index].span,
                        "`...` must be followed by a name",
                    ));
                };
                rest = Some(expect_symbol(name_node, "rest parameter")?.to_owned());
                if index + 2 != params_node.len() {
                    return Err(error(
                        name_node.span,
                        "rest parameter must be the final parameter",
                    ));
                }
                break;
            }
            params.push(expect_symbol(&params_node[index], "parameter")?.to_owned());
            index += 1;
        }

        let body = self.lower_body(&items[2..], span)?;
        Ok(Expr::new(
            ExprKind::Lambda {
                params,
                rest,
                body: Box::new(body),
            },
            span,
        ))
    }

    fn lower_let(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if items.len() < 3 {
            return Err(error(span, "let expects bindings and a body"));
        }
        let flat = expect_form(&items[1], "let binding list")?;
        if flat.len() % 2 != 0 {
            return Err(error(
                items[1].span,
                "let binding list must contain alternating names and values",
            ));
        }

        let mut bindings = vec![];
        for pair in flat.chunks_exact(2) {
            let name = expect_symbol(&pair[0], "binding name")?.to_owned();
            bindings.push((name, self.lower_expr(&pair[1])?));
        }

        Ok(Expr::new(
            ExprKind::Let {
                bindings,
                body: Box::new(self.lower_body(&items[2..], span)?),
            },
            span,
        ))
    }

    fn lower_if(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        expect_arity(items, 3, 4, span, "if")?;
        let else_branch = if let Some(node) = items.get(3) {
            self.lower_expr(node)?
        } else {
            Expr::null(span)
        };
        Ok(Expr::new(
            ExprKind::If {
                condition: Box::new(self.lower_expr(&items[1])?),
                then_branch: Box::new(self.lower_expr(&items[2])?),
                else_branch: Box::new(else_branch),
            },
            span,
        ))
    }

    fn lower_obj(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if !(items.len() - 1).is_multiple_of(2) {
            return Err(error(
                span,
                "obj expects alternating key and value expressions",
            ));
        }
        let mut fields = vec![];
        let mut keys = BTreeMap::new();
        for pair in items[1..].chunks_exact(2) {
            let key = self.lower_expr(&pair[0])?;
            let value = self.lower_expr(&pair[1])?;
            if let Some(name) = static_string_key(&key) {
                if let Some(first) = keys.insert(name.clone(), key.span) {
                    return Err(LowerError::single(
                        Diagnostic::error(key.span, format!("duplicate object key `{name}`"))
                            .with_code("JISP-LOWER")
                            .with_secondary(first, "first defined here"),
                    ));
                }
            }
            fields.push((key, value));
        }
        Ok(Expr::new(ExprKind::Object(fields), span))
    }

    fn lower_string_template(
        &self,
        span: Span,
        lines: bool,
        nodes: &[Node],
    ) -> Result<Expr, LowerError> {
        let mut parts = vec![];
        for node in nodes {
            if let Some(value) = node.as_string() {
                parts.push(StringPart::Literal(value.to_owned()));
                continue;
            }
            let form = node.as_form().ok_or_else(|| {
                error(
                    node.span,
                    "str accepts literal fragments or unquote forms only",
                )
            })?;
            match form.first().and_then(Node::as_symbol) {
                Some("," | "unquote") if form.len() == 2 => {
                    parts.push(StringPart::Expr(self.lower_expr(&form[1])?));
                }
                Some(",@" | "unquote-splicing") if form.len() == 2 => {
                    parts.push(StringPart::Splice(self.lower_expr(&form[1])?));
                }
                _ => {
                    return Err(error(
                        node.span,
                        "interpolate with [\",\", expression] or [\",@\", expression]",
                    ))
                }
            }
        }
        Ok(Expr::new(ExprKind::StringTemplate { lines, parts }, span))
    }

    fn lower_case(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if items.len() < 3 {
            return Err(error(
                span,
                "case expects a subject and at least one branch",
            ));
        }
        let subject = Box::new(self.lower_expr(&items[1])?);
        let mut branches = vec![];
        for branch in &items[2..] {
            let pair = expect_form(branch, "case branch")?;
            if pair.len() < 2 {
                return Err(error(branch.span, "case branch expects a pattern and body"));
            }
            branches.push(CaseBranch {
                pattern: lower_pattern(&pair[0])?,
                body: self.lower_body(&pair[1..], branch.span)?,
                span: branch.span,
            });
        }
        Ok(Expr::new(ExprKind::Case { subject, branches }, span))
    }

    fn lower_use(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        if items.len() < 4 {
            return Err(error(span, "use expects bindings, a call, and a body"));
        }
        let (params, rest) = parse_use_bindings(&items[1])?;
        if rest.is_some() {
            return Err(error(items[1].span, "use bindings cannot be variadic"));
        }

        let call_items = expect_form(&items[2], "use call")?;
        if call_items.is_empty() {
            return Err(error(items[2].span, "use call cannot be empty"));
        }

        let callback = Expr::new(
            ExprKind::Lambda {
                params,
                rest: None,
                body: Box::new(self.lower_body(&items[3..], span)?),
            },
            items[1].span,
        );
        let callee = Box::new(self.lower_expr(&call_items[0])?);
        let mut arguments = call_items[1..]
            .iter()
            .map(|node| self.lower_expr(node))
            .collect::<Result<Vec<_>, _>>()?;
        arguments.push(callback);

        Ok(Expr::new(ExprKind::Call { callee, arguments }, span))
    }

    fn lower_call(&self, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
        Ok(Expr::new(
            ExprKind::Call {
                callee: Box::new(self.lower_expr(&items[0])?),
                arguments: items[1..]
                    .iter()
                    .map(|node| self.lower_expr(node))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            span,
        ))
    }

    fn lower_body(&self, nodes: &[Node], span: Span) -> Result<Expr, LowerError> {
        match nodes {
            [] => Ok(Expr::null(span)),
            [single] => self.lower_expr(single),
            many => Ok(Expr::new(
                ExprKind::Do(
                    many.iter()
                        .map(|node| self.lower_expr(node))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
        }
    }
}

fn lower_variant(node: &Node) -> Result<VariantDecl, LowerError> {
    let items = expect_form(node, "type variant")?;
    let Some(name) = items.first().and_then(Node::as_symbol) else {
        return Err(error(
            node.span,
            "type variant must start with a constructor name",
        ));
    };
    Ok(VariantDecl {
        name: name.to_owned(),
        field_types: items[1..].iter().map(render_type_node).collect(),
        span: node.span,
    })
}

fn render_type_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => format!("{value:?}"),
        NodeKind::Form(items) => {
            let inner = items
                .iter()
                .map(render_type_node)
                .collect::<Vec<_>>()
                .join(" ");
            format!("({inner})")
        }
    }
}

fn lower_pattern(node: &Node) -> Result<Pattern, LowerError> {
    match &node.kind {
        NodeKind::Null => Ok(Pattern::Literal(Literal::Null)),
        NodeKind::Bool(value) => Ok(Pattern::Literal(Literal::Bool(*value))),
        NodeKind::Int(value) => Ok(Pattern::Literal(Literal::Int(*value))),
        NodeKind::Float(value) => Ok(Pattern::Literal(Literal::Float(*value))),
        NodeKind::String(value) => Ok(Pattern::Literal(Literal::String(value.to_string()))),
        NodeKind::Symbol(symbol) if symbol.as_str() == "_" => Ok(Pattern::Wildcard),
        NodeKind::Symbol(symbol) => Ok(Pattern::Bind(symbol.to_string())),
        NodeKind::Form(items) => {
            let Some(head) = items.first().and_then(Node::as_symbol) else {
                return Err(error(node.span, "pattern form must start with a symbol"));
            };
            match head {
                "list" => lower_list_pattern(node.span, &items[1..]),
                "obj" => lower_object_pattern(node.span, &items[1..]),
                "or" | "as" | "when" => Err(error(
                    node.span,
                    "pattern alternatives, aliases, and guards are post-MVP features",
                )),
                tag => Ok(Pattern::Variant {
                    tag: tag.to_owned(),
                    fields: items[1..]
                        .iter()
                        .map(lower_pattern)
                        .collect::<Result<Vec<_>, _>>()?,
                }),
            }
        }
    }
}

fn lower_list_pattern(span: Span, nodes: &[Node]) -> Result<Pattern, LowerError> {
    let mut prefix = vec![];
    let mut rest = None;
    let mut index = 0;
    while index < nodes.len() {
        if nodes[index].as_symbol() == Some("...") {
            let Some(rest_node) = nodes.get(index + 1) else {
                return Err(error(nodes[index].span, "`...` must be followed by a name"));
            };
            rest = Some(expect_symbol(rest_node, "list rest binding")?.to_owned());
            if index + 2 != nodes.len() {
                return Err(error(
                    rest_node.span,
                    "list rest binding must be the final pattern",
                ));
            }
            break;
        }
        prefix.push(lower_pattern(&nodes[index])?);
        index += 1;
    }
    let _ = span;
    Ok(Pattern::List { prefix, rest })
}

fn lower_object_pattern(span: Span, nodes: &[Node]) -> Result<Pattern, LowerError> {
    if !nodes.len().is_multiple_of(2) {
        return Err(error(
            span,
            "obj pattern expects alternating string keys and patterns",
        ));
    }
    let mut fields = vec![];
    let mut keys = BTreeMap::new();
    for pair in nodes.chunks_exact(2) {
        let key = expect_string(&pair[0], "object pattern key")?.to_owned();
        if let Some(first) = keys.insert(key.clone(), pair[0].span) {
            return Err(LowerError::single(
                Diagnostic::error(
                    pair[0].span,
                    format!("duplicate object pattern key `{key}`"),
                )
                .with_code("JISP-LOWER")
                .with_secondary(first, "first defined here"),
            ));
        }
        fields.push((key, lower_pattern(&pair[1])?));
    }
    Ok(Pattern::Object(fields))
}

fn parse_use_bindings(node: &Node) -> Result<(Vec<String>, Option<String>), LowerError> {
    if let Some(name) = node.as_symbol() {
        return Ok((vec![name.to_owned()], None));
    }
    let nodes = expect_form(node, "use binding list")?;
    let params = nodes
        .iter()
        .map(|node| expect_symbol(node, "use binding").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((params, None))
}

fn expect_form<'a>(node: &'a Node, description: &str) -> Result<&'a [Node], LowerError> {
    node.as_form()
        .ok_or_else(|| error(node.span, format!("expected {description}")))
}

fn expect_symbol<'a>(node: &'a Node, description: &str) -> Result<&'a str, LowerError> {
    node.as_symbol()
        .ok_or_else(|| error(node.span, format!("expected {description}")))
}

fn expect_string<'a>(node: &'a Node, description: &str) -> Result<&'a str, LowerError> {
    node.as_string()
        .ok_or_else(|| error(node.span, format!("expected {description} to be a string")))
}

fn expect_arity(
    items: &[Node],
    min: usize,
    max: usize,
    span: Span,
    name: &str,
) -> Result<(), LowerError> {
    if (min..=max).contains(&items.len()) {
        Ok(())
    } else {
        Err(error(
            span,
            format!(
                "{name} expects {} argument(s), got {}",
                min.saturating_sub(1),
                items.len().saturating_sub(1)
            ),
        ))
    }
}

fn default_alias(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

fn validate_module_names(module: &Module, diagnostics: &mut Vec<Diagnostic>) {
    let mut values = BTreeMap::new();
    for definition in &module.definitions {
        record_unique_name(
            &mut values,
            &definition.name,
            definition.span,
            "value declaration",
            diagnostics,
        );
    }
    for declaration in &module.types {
        for variant in &declaration.variants {
            record_unique_name(
                &mut values,
                &variant.name,
                variant.span,
                "value declaration",
                diagnostics,
            );
        }
    }

    let mut aliases = BTreeMap::new();
    for import in &module.imports {
        record_unique_name(
            &mut aliases,
            &import.alias,
            import.span,
            "import alias",
            diagnostics,
        );
    }

    let mut types = BTreeMap::new();
    for declaration in &module.types {
        record_unique_name(
            &mut types,
            &declaration.name,
            declaration.span,
            "type declaration",
            diagnostics,
        );
    }
}

fn record_unique_name(
    names: &mut BTreeMap<String, Span>,
    name: &str,
    span: Span,
    kind: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(first) = names.get(name).copied() {
        diagnostics.push(
            Diagnostic::error(span, format!("duplicate {kind} `{name}`"))
                .with_code("JISP-LOWER")
                .with_secondary(first, "first defined here"),
        );
    } else {
        names.insert(name.to_owned(), span);
    }
}

fn static_string_key(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value.clone()),
        ExprKind::StringTemplate { lines, parts }
            if parts
                .iter()
                .all(|part| matches!(part, StringPart::Literal(_))) =>
        {
            let fragments = parts.iter().map(|part| match part {
                StringPart::Literal(value) => value.as_str(),
                StringPart::Expr(_) | StringPart::Splice(_) => unreachable!(),
            });
            Some(if *lines {
                fragments.collect::<Vec<_>>().join("\n")
            } else {
                fragments.collect()
            })
        }
        _ => None,
    }
}

fn error(span: Span, message: impl Into<String>) -> LowerError {
    LowerError::single(Diagnostic::error(span, message).with_code("JISP-LOWER"))
}
