mod ast;
mod diagnostic;
#[cfg(test)]
mod diagnostic_test;
mod registry;
mod schema;
mod source;
mod syntax;

pub use ast::{Node, NodeKind, Symbol};
pub use diagnostic::{Diagnostic, Label, Severity};
pub use registry::{
    special_form, ui_directive, ui_element, SpecialFormSpec, UiDirectiveSpec, UiElementSpec,
    SPECIAL_FORMS, UI_DIRECTIVES, UI_ELEMENTS,
};
pub use schema::core_schema;
pub use source::{SourceFile, SourceId, SourceMap, Span};
pub use syntax::{detect_syntax, ParseError, Syntax, SyntaxParser};
