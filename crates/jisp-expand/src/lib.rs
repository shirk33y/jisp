use std::collections::HashMap;

use jisp_core::{Diagnostic, Node, NodeKind, Span};
use thiserror::Error;

const MAX_ORIGIN_DEPTH: usize = 256;

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
}

impl Expander {
    pub fn expand_module(mut self, nodes: &[Node]) -> Result<ExpandedModule, ExpandError> {
        let nodes = nodes
            .iter()
            .cloned()
            .map(|node| self.expand_node(node))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExpandedModule {
            nodes,
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
