use std::collections::BTreeSet;

use jisp_core::{ui_element, Node, NodeKind, Span};

use crate::{Definition, Expr, ExprKind, Literal, LowerError, Lowerer, Module};

use crate::lower::{error, expect_arity, expect_symbol, parse_fn_params};

pub(crate) fn lower_component(
    lowerer: &Lowerer,
    span: Span,
    items: &[Node],
    module: &mut Module,
) -> Result<(), LowerError> {
    expect_arity(items, 4, 4, span, "component")?;
    let name = expect_symbol(&items[1], "component name")?.to_owned();
    if ui_element(&name).is_some() {
        return Err(error(
            items[1].span,
            format!("component name `{name}` is reserved by the UI element registry"),
        ));
    }
    let (params, rest) = parse_fn_params(&items[2])?;
    let body = lower_ui_expr(lowerer, &items[3])?;
    module.definitions.push(Definition {
        name,
        public: false,
        value: Expr::new(
            ExprKind::Lambda {
                params,
                rest,
                body: Box::new(body),
            },
            span,
        ),
        span,
    });
    Ok(())
}

pub(crate) fn lower_ui_expr(lowerer: &Lowerer, node: &Node) -> Result<Expr, LowerError> {
    match &node.kind {
        NodeKind::Form(items) => match items.first().and_then(Node::as_symbol) {
            Some("text") => lower_text(lowerer, node.span, items),
            Some("if") => lower_ui_if(lowerer, node.span, items),
            Some("ui.local") => lower_ui_local(lowerer, node.span, items),
            Some(name) if ui_element(name).is_some() => {
                lower_ui_element(lowerer, node.span, name, &items[1..])
            }
            _ => lowerer.lower_expr(node),
        },
        _ => lowerer.lower_expr(node),
    }
}

/// Lower an opt-in component-local state boundary. The callback's body stays
/// in UI lowering so element/directive forms retain their normal meaning.
/// Runtime ownership is assigned by JUIR; this form never runs a host effect.
fn lower_ui_local(lowerer: &Lowerer, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
    expect_arity(items, 3, 3, span, "ui.local")?;
    let Some(callback) = items[2].as_form() else {
        return Err(error(items[2].span, "ui.local callback must be an fn form"));
    };
    if callback.first().and_then(Node::as_symbol) != Some("fn") {
        return Err(error(items[2].span, "ui.local callback must be an fn form"));
    }
    expect_arity(callback, 3, 3, items[2].span, "ui.local callback")?;
    let (params, rest) = parse_fn_params(&callback[1])?;
    if rest.is_some() || params.len() != 2 {
        return Err(error(
            callback[1].span,
            "ui.local callback must bind exactly `(state set-state)`",
        ));
    }
    Ok(Expr::new(
        ExprKind::Call {
            callee: Box::new(Expr::new(ExprKind::Name("ui.local".to_owned()), span)),
            arguments: vec![
                lowerer.lower_expr(&items[1])?,
                Expr::new(
                    ExprKind::Lambda {
                        params,
                        rest: None,
                        body: Box::new(lower_ui_expr(lowerer, &callback[2])?),
                    },
                    items[2].span,
                ),
            ],
        },
        span,
    ))
}

fn lower_ui_if(lowerer: &Lowerer, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
    expect_arity(items, 3, 4, span, "if")?;
    let else_branch = items
        .get(3)
        .map(|node| lower_ui_expr(lowerer, node))
        .transpose()?
        .unwrap_or_else(|| Expr::null(span));
    Ok(Expr::new(
        ExprKind::If {
            condition: Box::new(lowerer.lower_expr(&items[1])?),
            then_branch: Box::new(lower_ui_expr(lowerer, &items[2])?),
            else_branch: Box::new(else_branch),
        },
        span,
    ))
}

fn lower_text(lowerer: &Lowerer, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
    expect_arity(items, 2, 2, span, "text")?;
    Ok(ui_node(Expr::new(
        ExprKind::Object(vec![
            (string_literal("tag", span), string_literal("text", span)),
            (
                string_literal("value", span),
                lowerer.lower_expr(&items[1])?,
            ),
        ]),
        span,
    )))
}

fn lower_ui_element(
    lowerer: &Lowerer,
    span: Span,
    tag: &str,
    nodes: &[Node],
) -> Result<Expr, LowerError> {
    let parts = lower_ui_parts(lowerer, nodes)?;
    let mut fields = vec![(string_literal("tag", span), string_literal(tag, span))];
    if !parts.attrs.is_empty() {
        fields.push((
            string_literal("attrs", span),
            Expr::new(ExprKind::Object(parts.attrs), span),
        ));
    }
    if !parts.props.is_empty() {
        fields.push((
            string_literal("props", span),
            Expr::new(ExprKind::Object(parts.props), span),
        ));
    }
    if !parts.classes.is_empty() {
        fields.push((
            string_literal("classes", span),
            Expr::new(ExprKind::Object(parts.classes), span),
        ));
    }
    if !parts.events.is_empty() {
        fields.push((
            string_literal("events", span),
            Expr::new(ExprKind::Object(parts.events), span),
        ));
    }
    if let Some(key) = parts.key {
        fields.push((string_literal("key", span), key));
    }
    if !parts.children.is_empty() {
        fields.push((
            string_literal("children", span),
            lower_children(span, parts.children),
        ));
    }
    Ok(ui_node(Expr::new(ExprKind::Object(fields), span)))
}

fn ui_node(value: Expr) -> Expr {
    let span = value.span;
    Expr::new(
        ExprKind::Call {
            callee: Box::new(Expr::new(ExprKind::Name("ui.node".to_owned()), span)),
            arguments: vec![value],
        },
        span,
    )
}

fn lower_children(span: Span, children: Vec<UiChild>) -> Expr {
    let lists = children
        .into_iter()
        .map(|child| match child {
            UiChild::One(child) => Expr::new(ExprKind::List(vec![child]), span),
            UiChild::Many(children) => children,
        })
        .collect::<Vec<_>>();
    if lists.len() == 1 {
        return lists.into_iter().next().expect("one child list");
    }
    Expr::new(
        ExprKind::Call {
            callee: Box::new(Expr::new(ExprKind::Name("list.cat".to_owned()), span)),
            arguments: lists,
        },
        span,
    )
}

fn lower_ui_parts(lowerer: &Lowerer, nodes: &[Node]) -> Result<UiParts, LowerError> {
    let mut parts = UiParts::default();
    for node in nodes {
        if let Some(items) = node.as_form() {
            if lower_ui_directive(lowerer, &mut parts, node.span, items)? {
                continue;
            }
            if items.first().and_then(Node::as_symbol) == Some("for") {
                parts
                    .children
                    .push(UiChild::Many(lower_for(lowerer, node.span, items)?));
                continue;
            }
        }
        parts
            .children
            .push(UiChild::One(lower_ui_expr(lowerer, node)?));
    }
    Ok(parts)
}

fn lower_ui_directive(
    lowerer: &Lowerer,
    parts: &mut UiParts,
    span: Span,
    items: &[Node],
) -> Result<bool, LowerError> {
    let Some(head) = items.first().and_then(Node::as_symbol) else {
        return Ok(false);
    };
    match head {
        "attr" => {
            expect_arity(items, 3, 3, span, "attr")?;
            push_named(
                &mut parts.attrs,
                &mut parts.attr_names,
                ui_name(&items[1], "attribute name")?,
                items[1].span,
                lowerer.lower_expr(&items[2])?,
            )?;
            Ok(true)
        }
        "prop" => {
            expect_arity(items, 3, 3, span, "prop")?;
            push_named(
                &mut parts.props,
                &mut parts.prop_names,
                ui_name(&items[1], "property name")?,
                items[1].span,
                lowerer.lower_expr(&items[2])?,
            )?;
            Ok(true)
        }
        "class" => {
            if items.len() < 2 {
                return Err(error(span, "class expects at least one class name"));
            }
            for class in &items[1..] {
                let name = ui_name(class, "class name")?;
                push_named(
                    &mut parts.classes,
                    &mut parts.class_names,
                    name,
                    class.span,
                    bool_literal(true, class.span),
                )?;
            }
            Ok(true)
        }
        "class-if" => {
            expect_arity(items, 3, 3, span, "class-if")?;
            push_named(
                &mut parts.classes,
                &mut parts.class_names,
                ui_name(&items[1], "class name")?,
                items[1].span,
                lowerer.lower_expr(&items[2])?,
            )?;
            Ok(true)
        }
        "on" => {
            if items.len() < 3 {
                return Err(error(span, "on expects an event name and handler"));
            }
            push_named(
                &mut parts.events,
                &mut parts.event_names,
                ui_name(&items[1], "event name")?,
                items[1].span,
                lower_event(lowerer, span, &items[2..])?,
            )?;
            Ok(true)
        }
        "key" => {
            expect_arity(items, 2, 2, span, "key")?;
            if parts.key.is_some() {
                return Err(error(span, "UI element has more than one key directive"));
            }
            parts.key = Some(lowerer.lower_expr(&items[1])?);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn lower_event(lowerer: &Lowerer, span: Span, parts: &[Node]) -> Result<Expr, LowerError> {
    let (handler, modifiers) = parts
        .split_last()
        .expect("on has at least one handler after the event name");
    if modifiers.is_empty() {
        return lower_event_handler(lowerer, handler);
    }

    let mut policy = vec![];
    let mut seen = BTreeSet::new();
    for modifier in modifiers {
        let Some(items) = modifier.as_form() else {
            return Err(error(modifier.span, "event modifier must be a form"));
        };
        expect_arity(items, 1, 1, modifier.span, "event modifier")?;
        let name = expect_symbol(&items[0], "event modifier")?;
        if !matches!(name, "prevent-default" | "stop-propagation" | "capture") {
            return Err(error(
                modifier.span,
                format!(
                    "unknown event modifier `{name}`; expected prevent-default, stop-propagation, or capture"
                ),
            ));
        }
        if !seen.insert(name) {
            return Err(error(
                modifier.span,
                format!("duplicate event modifier `{name}`"),
            ));
        }
        policy.push((
            string_literal(name, modifier.span),
            bool_literal(true, modifier.span),
        ));
    }

    Ok(Expr::new(
        ExprKind::Object(vec![
            (
                string_literal("handler", span),
                lower_event_handler(lowerer, handler)?,
            ),
            (
                string_literal("policy", span),
                Expr::new(ExprKind::Object(policy), span),
            ),
        ]),
        span,
    ))
}

fn lower_event_handler(lowerer: &Lowerer, node: &Node) -> Result<Expr, LowerError> {
    let Some(items) = node.as_form() else {
        return lowerer.lower_expr(node);
    };
    if items.first().and_then(Node::as_symbol) != Some("emit") {
        return lowerer.lower_expr(node);
    }
    expect_arity(items, 2, 2, node.span, "emit")?;
    Ok(Expr::new(
        ExprKind::Lambda {
            params: vec!["event".to_owned()],
            rest: None,
            body: Box::new(lowerer.lower_expr(&items[1])?),
        },
        node.span,
    ))
}

fn lower_for(lowerer: &Lowerer, span: Span, items: &[Node]) -> Result<Expr, LowerError> {
    expect_arity(items, 4, 4, span, "for")?;
    let binding = expect_symbol(&items[1], "for binding")?.to_owned();
    Ok(Expr::new(
        ExprKind::Call {
            callee: Box::new(Expr::new(ExprKind::Name("list.map".to_owned()), span)),
            arguments: vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![binding],
                        rest: None,
                        body: Box::new(lower_ui_expr(lowerer, &items[3])?),
                    },
                    items[3].span,
                ),
                lowerer.lower_expr(&items[2])?,
            ],
        },
        span,
    ))
}

#[derive(Default)]
struct UiParts {
    attrs: Vec<(Expr, Expr)>,
    props: Vec<(Expr, Expr)>,
    classes: Vec<(Expr, Expr)>,
    events: Vec<(Expr, Expr)>,
    key: Option<Expr>,
    children: Vec<UiChild>,
    attr_names: BTreeSet<String>,
    prop_names: BTreeSet<String>,
    class_names: BTreeSet<String>,
    event_names: BTreeSet<String>,
}

enum UiChild {
    One(Expr),
    Many(Expr),
}

fn push_named(
    fields: &mut Vec<(Expr, Expr)>,
    names: &mut BTreeSet<String>,
    name: &str,
    span: Span,
    value: Expr,
) -> Result<(), LowerError> {
    if !names.insert(name.to_owned()) {
        return Err(error(span, format!("duplicate UI directive name `{name}`")));
    }
    fields.push((string_literal(name, span), value));
    Ok(())
}

fn ui_name<'a>(node: &'a Node, description: &str) -> Result<&'a str, LowerError> {
    node.as_symbol()
        .or_else(|| node.as_string())
        .ok_or_else(|| {
            error(
                node.span,
                format!("expected {description} to be a symbol or string"),
            )
        })
}

fn string_literal(value: impl Into<String>, span: Span) -> Expr {
    Expr::new(ExprKind::Literal(Literal::String(value.into())), span)
}

fn bool_literal(value: bool, span: Span) -> Expr {
    Expr::new(ExprKind::Literal(Literal::Bool(value)), span)
}
