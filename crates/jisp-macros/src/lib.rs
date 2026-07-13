//! Compile-time file inclusion entry points.
//!
//! These macros track direct and imported source files for Cargo rebuilds, then
//! emit native Rust tokens for the subset supported by `jisp-codegen-rust`.

use proc_macro::TokenStream;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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

/// Compile a Jisp file with an exported zero-argument `main` as a Rust
/// expression. The file and all resolved Jisp imports remain Cargo-tracked.
#[proc_macro]
pub fn lisp_expr(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let source_path = resolve_path(&path.value());
    let source_literal = LitStr::new(&source_path.display().to_string(), path.span());
    let generated = match generate_file(&source_path) {
        Ok(generated) => generated,
        Err(message) => return quote! { compile_error!(#message); }.into(),
    };
    let dependency_literals = generated
        .dependencies
        .iter()
        .map(|path| LitStr::new(&path.display().to_string(), proc_macro2::Span::call_site()));
    let tokens = generated.tokens;

    quote! {{
        const _: &str = include_str!(#source_literal);
        #(const _: &str = include_str!(#dependency_literals);)*
        mod __jisp_expression {
            #tokens
        }
        __jisp_expression::main()
    }}
    .into()
}

fn tracked_file(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let source_path = resolve_path(&path.value());
    let source_literal = LitStr::new(&source_path.display().to_string(), path.span());
    let generated = match generate_file(&source_path) {
        Ok(generated) => generated,
        Err(message) => {
            return quote! {
                compile_error!(#message);
            }
            .into()
        }
    };
    let dependency_literals = generated
        .dependencies
        .iter()
        .map(|path| LitStr::new(&path.display().to_string(), proc_macro2::Span::call_site()));
    let tokens = generated.tokens;

    quote! {
        const _: &str = include_str!(#source_literal);
        #(const _: &str = include_str!(#dependency_literals);)*
        #tokens
    }
    .into()
}

struct GeneratedFile {
    dependencies: Vec<PathBuf>,
    tokens: proc_macro2::TokenStream,
}

fn generate_file(path: &PathBuf) -> Result<GeneratedFile, String> {
    let text = read_source(path, "native code generation")?;
    let generated =
        jisp::emit_rust_detailed(path, &text).map_err(|error| format_module_error(path, &error))?;
    Ok(GeneratedFile {
        dependencies: generated.dependencies,
        tokens: generated.tokens,
    })
}

#[cfg(test)]
fn import_dependencies(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let text = read_source(path, "dependency tracking")?;
    jisp::import_dependencies(path, &text).map_err(|error| {
        format!(
            "failed to resolve Jisp dependencies for `{}`: {error}",
            path.display()
        )
    })
}

fn read_source(path: &PathBuf, purpose: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read Jisp source `{}` for {purpose}: {error}",
            path.display()
        )
    })
}

fn format_module_error(path: &Path, error: &jisp::ModuleError) -> String {
    let rendered = error
        .render_diagnostics()
        .unwrap_or_else(|| error.error.to_string());
    format!(
        "failed to generate native Rust for Jisp source `{}`:\n{rendered}",
        path.display()
    )
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
