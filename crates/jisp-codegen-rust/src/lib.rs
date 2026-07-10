//! Native Rust backend scaffold.
//!
//! The interpreter is the executable MVP. This crate deliberately exposes a
//! stable entry point so an agent can implement code generation without
//! changing the frontend.

mod emit;
#[cfg(test)]
mod emit_test;
mod enum_types;
mod layout;
#[cfg(test)]
mod layout_test;
#[cfg(test)]
mod lib_test;
mod patterns;
mod source_map;

use jisp_types::TypedModule;
use proc_macro2::TokenStream;
use thiserror::Error;

pub use source_map::{RustItemKind, RustSourceItem, RustSourceMap};

#[derive(Clone, Debug)]
pub struct GeneratedRust {
    pub tokens: TokenStream,
    pub source_map: RustSourceMap,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodegenError {
    #[error("native Rust code generation cannot classify layout: {0}")]
    Layout(String),

    #[error("native Rust code generation does not support {0} yet")]
    Unsupported(&'static str),
}

pub fn generate(module: &TypedModule) -> Result<TokenStream, CodegenError> {
    Ok(generate_detailed(module)?.tokens)
}

pub fn generate_detailed(module: &TypedModule) -> Result<GeneratedRust, CodegenError> {
    let _layout =
        layout::classify_module(module).map_err(|error| CodegenError::Layout(error.to_string()))?;
    emit::emit_module(module)
}
