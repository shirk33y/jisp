mod ir;
mod lower;

#[cfg(test)]
mod lower_test;

pub use ir::{
    CaseBranch, Definition, Expr, ExprKind, Import, Literal, Module, Pattern, StringPart, TypeDecl,
    VariantDecl,
};
pub use lower::{LowerError, Lowerer};
