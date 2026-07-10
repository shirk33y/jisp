use jisp_core::Span;

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustSourceItem {
    pub kind: RustItemKind,
    pub rust_name: String,
    pub source_span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustItemKind {
    Function,
    Struct,
    Enum,
}
