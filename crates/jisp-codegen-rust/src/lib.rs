//! Native Rust backend scaffold.
//!
//! The interpreter is the executable MVP. This crate deliberately exposes a
//! stable entry point so an agent can implement code generation without
//! changing the frontend.

mod emit;
#[cfg(test)]
mod emit_test;
mod layout;
#[cfg(test)]
mod layout_test;
#[cfg(test)]
mod lib_test;

use jisp_types::TypedModule;
use proc_macro2::TokenStream;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodegenError {
    #[error("native Rust code generation cannot classify layout: {0}")]
    Layout(String),

    #[error("native Rust code generation does not support {0} yet")]
    Unsupported(&'static str),
}

pub fn generate(module: &TypedModule) -> Result<TokenStream, CodegenError> {
    let _layout =
        layout::classify_module(module).map_err(|error| CodegenError::Layout(error.to_string()))?;
    emit::emit_module(module)
}
