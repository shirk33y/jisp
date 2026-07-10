//! Native Rust backend scaffold.
//!
//! The interpreter is the executable MVP. This crate deliberately exposes a
//! stable entry point so an agent can implement code generation without
//! changing the frontend.

use jisp_ir::Module;
use proc_macro2::TokenStream;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("native Rust code generation is not implemented yet")]
pub struct CodegenError;

pub fn generate(_module: &Module) -> Result<TokenStream, CodegenError> {
    Err(CodegenError)
}
