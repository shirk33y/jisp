//! Compile-time file inclusion entry points.
//!
//! Native Jisp compilation is intentionally left for `jisp-codegen-rust`.
//! These macros currently establish dependency tracking and produce a clear
//! compile-time note instead of silently pretending native codegen exists.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn file(input: TokenStream) -> TokenStream { tracked_file(input) }

#[proc_macro]
pub fn json_file(input: TokenStream) -> TokenStream { tracked_file(input) }

#[proc_macro]
pub fn yaml_file(input: TokenStream) -> TokenStream { tracked_file(input) }

#[proc_macro]
pub fn lisp_file(input: TokenStream) -> TokenStream { tracked_file(input) }

fn tracked_file(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    quote!({
        const _: &str = include_str!(#path);
        compile_error!("Jisp native file macros are scaffolded; use the interpreter/CLI until jisp-codegen-rust is implemented");
    }).into()
}
