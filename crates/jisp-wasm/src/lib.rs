//! Browser-facing WebAssembly entry points for the interpreter-backed playground.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_html(source: &str) -> Result<String, JsValue> {
    render_html_source(source).map_err(|error| JsValue::from_str(&error))
}

pub(crate) fn render_html_source(source: &str) -> Result<String, String> {
    let value = jisp::run_main("playground.lisp", source).map_err(|error| error.to_string())?;
    match value {
        jisp::jisp_eval::Value::Str(html) => Ok(html.to_string()),
        other => Err(format!(
            "playground main must return HTML text through ui.html, got {}",
            other.type_name()
        )),
    }
}

#[cfg(test)]
mod lib_test;
