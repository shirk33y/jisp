//! Compile-time file inclusion entry points.
//!
//! Native Jisp compilation is intentionally left for `jisp-codegen-rust`.
//! These macros currently establish dependency tracking and produce a clear
//! compile-time note instead of silently pretending native codegen exists.

use proc_macro::TokenStream;
use std::{env, fs, path::PathBuf};

use quote::quote;
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn file(input: TokenStream) -> TokenStream {
    tracked_file(input)
}

#[proc_macro]
pub fn json_file(input: TokenStream) -> TokenStream {
    tracked_file(input)
}

#[proc_macro]
pub fn yaml_file(input: TokenStream) -> TokenStream {
    tracked_file(input)
}

#[proc_macro]
pub fn lisp_file(input: TokenStream) -> TokenStream {
    tracked_file(input)
}

fn tracked_file(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let source_path = resolve_path(&path.value());
    let source_literal = LitStr::new(&source_path.display().to_string(), path.span());
    let dependencies = match import_dependencies(&source_path) {
        Ok(dependencies) => dependencies,
        Err(message) => {
            return quote!({
                compile_error!(#message);
            })
            .into()
        }
    };
    let dependency_literals = dependencies
        .iter()
        .map(|path| LitStr::new(&path.display().to_string(), proc_macro2::Span::call_site()));

    quote!({
        const _: &str = include_str!(#source_literal);
        #(const _: &str = include_str!(#dependency_literals);)*
        compile_error!("Jisp native file macros are scaffolded; use the interpreter/CLI until jisp-codegen-rust is implemented");
    }).into()
}

fn import_dependencies(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read Jisp source `{}` for dependency tracking: {error}",
            path.display()
        )
    })?;
    jisp::import_dependencies(path, &text).map_err(|error| {
        format!(
            "failed to resolve Jisp dependencies for `{}`: {error}",
            path.display()
        )
    })
}

fn resolve_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned())).join(path)
    }
}

#[cfg(test)]
mod lib_test;
