use jisp_core::Span;
use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RustSourceMap {
    pub items: Vec<RustSourceItem>,
}

impl RustSourceMap {
    pub fn item(&self, kind: RustItemKind, rust_name: &str) -> Option<&RustSourceItem> {
        self.items
            .iter()
            .find(|item| item.kind == kind && item.rust_name == rust_name)
    }

    pub fn item_at(&self, generated_offset: usize) -> Option<&RustSourceItem> {
        self.items
            .iter()
            .filter(|item| {
                item.generated_range
                    .as_ref()
                    .is_some_and(|range| range.contains(&generated_offset))
            })
            .min_by_key(|item| {
                item.generated_range
                    .as_ref()
                    .map_or(usize::MAX, |range| range.end - range.start)
            })
    }

    pub(crate) fn locate_generated_ranges(&mut self, rendered: &str) {
        for item in &mut self.items {
            item.generated_range = locate_item_range(rendered, item.kind, &item.rust_name);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustSourceItem {
    pub kind: RustItemKind,
    pub rust_name: String,
    pub source_span: Span,
    pub generated_range: Option<Range<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustItemKind {
    Function,
    Struct,
    Enum,
    Expression,
}

fn locate_item_range(rendered: &str, kind: RustItemKind, name: &str) -> Option<Range<usize>> {
    if kind == RustItemKind::Expression {
        return locate_expression_range(rendered, name);
    }
    let keyword = match kind {
        RustItemKind::Function => "fn",
        RustItemKind::Struct => "struct",
        RustItemKind::Enum => "enum",
        RustItemKind::Expression => unreachable!("expressions are handled above"),
    };
    let needle = format!("{keyword} {name}");
    let start = rendered.find(&needle)?;
    let body = rendered[start..].find('{')? + start;
    let mut depth = 0usize;
    for (offset, character) in rendered[body..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start..body + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn locate_expression_range(rendered: &str, name: &str) -> Option<Range<usize>> {
    let binding = format!("let {name} =");
    let binding_start = rendered.find(&binding)?;
    let start = rendered[..binding_start].rfind('{')?;
    let end_marker = format!("; {name} }}");
    let end = binding_start + rendered[binding_start..].find(&end_marker)? + end_marker.len();
    Some(start..end)
}
