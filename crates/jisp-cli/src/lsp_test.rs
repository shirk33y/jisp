use std::{io::Cursor, path::Path};

use serde_json::{json, Value};

use crate::{lsp_definition, lsp_diagnostics, lsp_hover, remapped_cargo_errors};

#[test]
fn diagnostics_keep_jisp_codes_and_utf16_ranges() {
    let text = "(export main (fn () \"🙂\" \"unterminated";
    let diagnostics = lsp_diagnostics("file:///unicode.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["source"], "jisp");
    assert_eq!(diagnostics[0]["code"], "JISP-L003");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(
        diagnostics[0]["range"]["start"]["character"],
        "(export main (fn () \"🙂\" ".encode_utf16().count()
    );
}

#[test]
fn diagnostics_keep_parser_codes_and_eof_ranges() {
    let text = "\"unterminated";
    let diagnostics = lsp_diagnostics("file:///unterminated.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["source"], "jisp");
    assert_eq!(diagnostics[0]["code"], "JISP-L003");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 0);
    assert_eq!(
        diagnostics[0]["range"]["end"]["character"],
        text.encode_utf16().count()
    );
}

#[test]
fn diagnostics_keep_multiline_ranges_after_unicode_text() {
    let text = "(def label \"🙂\")\n(export main\n  (fn ()\n    (+ 1 true)))\n";
    let diagnostics = lsp_diagnostics("file:///multiline-unicode.lisp", text);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "JISP-TYPE");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 3);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 4);
}

#[test]
fn native_remapping_ignores_errors_outside_generated_rust() {
    let generated = jisp::emit_rust_detailed("main.lisp", "(export main (fn () 1))").unwrap();
    let cargo_json = r#"{
        "reason":"compiler-message",
        "message":{
            "level":"error",
            "message":"foreign compiler error",
            "spans":[{
                "is_primary":true,
                "file_name":"dependency.rs",
                "byte_start":0
            }]
        }
    }"#;

    assert!(remapped_cargo_errors(cargo_json, &generated, Path::new("src/lib.rs")).is_empty());
}

#[test]
fn hover_reports_an_inferred_top_level_type() {
    let text = "(def answer 42)\n(export main (fn () answer))\n";
    let hover = lsp_hover("file:///main.lisp", text, 0, 6).unwrap();

    assert_eq!(hover["contents"]["kind"], "markdown");
    assert_eq!(hover["contents"]["value"], "**answer** — `int`");
}

#[test]
fn definition_resolves_a_local_top_level_name() {
    let text = "(def answer 42)\n(export main (fn () (+ answer 1)))\n";
    let definition = lsp_definition("file:///main.lisp", text, 1, 24).unwrap();

    assert_eq!(definition["uri"], "file:///main.lisp");
    assert_eq!(definition["range"]["start"]["line"], 0);
    assert_eq!(definition["range"]["start"]["character"], 5);
}

#[test]
fn definition_resolves_lambda_and_sequential_let_bindings() {
    let text =
        "(export main (fn (value) (let (offset 1 total (+ value offset)) (+ total value))))\n";

    let parameter = lsp_definition("file:///main.lisp", text, 0, 52).unwrap();
    let offset = lsp_definition("file:///main.lisp", text, 0, 58).unwrap();
    let total = lsp_definition("file:///main.lisp", text, 0, 70).unwrap();

    assert_eq!(parameter["range"]["start"]["character"], 18);
    assert_eq!(offset["range"]["start"]["character"], 31);
    assert_eq!(total["range"]["start"]["character"], 40);
}

#[test]
fn definition_resolves_case_pattern_bindings() {
    let text = "(export main (fn () (case (some 1) ((some value) (+ value 1)))))\n";
    let use_offset = text.rfind("value").unwrap();
    let declaration = lsp_definition("file:///main.lisp", text, 0, use_offset).unwrap();

    assert_eq!(
        declaration["range"]["start"]["character"],
        text.find("value").unwrap()
    );
}

#[test]
fn definition_ignores_unknown_and_non_name_symbols() {
    let text = "(export main (fn () 42))\n";

    assert!(lsp_definition("file:///main.lisp", text, 0, 1).is_none());
    assert!(lsp_definition("file:///main.lisp", text, 0, 19).is_none());
}

#[test]
fn definition_resolves_a_qualified_import() {
    let directory =
        std::env::temp_dir().join(format!("jisp-lsp-definition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let math = directory.join("math.lisp");
    std::fs::write(&math, "(export increment (fn (value) (+ value 1)))\n").unwrap();
    let text = "(import math \"math\")\n(export main (fn () (math.increment 41)))\n";
    std::fs::write(&main, text).unwrap();

    let uri = format!("file://{}", main.display());
    let definition = lsp_definition(&uri, text, 1, 29).unwrap();

    assert_eq!(definition["uri"], format!("file://{}", math.display()));
    assert_eq!(definition["range"]["start"]["line"], 0);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn definition_resolves_an_imported_macro() {
    let directory =
        std::env::temp_dir().join(format!("jisp-lsp-macro-definition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let macros = directory.join("macros.lisp");
    std::fs::write(&macros, "(def wrap (~ (fn (value) `(+ ,value 1))))\n").unwrap();
    let text = "(macro-import m \"macros.lisp\")\n(export main (fn () (m.wrap 41)))\n";
    std::fs::write(&main, text).unwrap();

    let uri = format!("file://{}", main.display());
    let position = text.lines().nth(1).unwrap().find("m.wrap").unwrap() + 2;
    let definition = lsp_definition(&uri, text, 1, position).unwrap();

    assert_eq!(definition["uri"], format!("file://{}", macros.display()));
    assert_eq!(definition["range"]["start"]["line"], 0);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn protocol_advertises_incremental_sync_and_uses_changed_document_text() {
    let messages = protocol_messages(&[
        request(1, "initialize", json!({})),
        notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": "file:///main.lisp",
                    "version": 1,
                    "text": "(def answer 1)"
                }
            }),
        ),
        request(
            2,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": "file:///main.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///main.lisp", "version": 2 },
                "contentChanges": [{ "text": "(def updated 1)" }]
            }),
        ),
        request(
            3,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": "file:///main.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        request(
            4,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": "file:///main.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        request(5, "shutdown", json!({})),
        notification("exit", json!({})),
    ]);

    assert_eq!(messages[0]["result"]["capabilities"]["textDocumentSync"], 2);
    assert_eq!(
        messages[2]["result"]["contents"]["value"],
        "**answer** — `int`"
    );
    assert_eq!(
        messages[4]["result"]["contents"]["value"],
        "**updated** — `int`"
    );
    assert_eq!(messages[5]["result"]["uri"], "file:///main.lisp");
    assert_eq!(messages[5]["result"]["range"]["start"]["character"], 5);
}

#[test]
fn protocol_applies_utf16_and_ordered_incremental_changes_atomically() {
    let source = "(export main (fn () (let (emoji \"🙂\") (+ 1 true))))";
    let true_start = source.find("true").unwrap();
    let true_character = source[..true_start].encode_utf16().count();
    let messages = protocol_messages(&[
        request(1, "initialize", json!({})),
        notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": "file:///unicode.lisp", "version": 1, "text": source }
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///unicode.lisp", "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 0, "character": true_character },
                        "end": { "line": 0, "character": true_character + 4 }
                    },
                    "text": "1"
                }]
            }),
        ),
        notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": "file:///ordered.lisp", "version": 1, "text": "(def one \"a\")" }
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///ordered.lisp", "version": 2 },
                "contentChanges": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 5 },
                            "end": { "line": 0, "character": 8 }
                        },
                        "text": "answer"
                    },
                    {
                        "range": {
                            "start": { "line": 0, "character": 12 },
                            "end": { "line": 0, "character": 15 }
                        },
                        "text": "42"
                    }
                ]
            }),
        ),
        request(
            2,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": "file:///ordered.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        request(3, "shutdown", json!({})),
        notification("exit", json!({})),
    ]);

    assert_eq!(messages[1]["params"]["diagnostics"][0]["code"], "JISP-TYPE");
    assert_eq!(messages[2]["params"]["diagnostics"], json!([]));
    assert_eq!(messages[4]["params"]["diagnostics"], json!([]));
    assert_eq!(
        messages[5]["result"]["contents"]["value"],
        "**answer** — `int`"
    );
}

#[test]
fn protocol_preserves_newest_valid_document_after_stale_or_invalid_edits() {
    let messages = protocol_messages(&[
        request(1, "initialize", json!({})),
        notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": "file:///versions.lisp", "version": 1, "text": "(def answer 1)" }
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///versions.lisp", "version": 3 },
                "contentChanges": [{ "text": "(def latest \"ok\")" }]
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///versions.lisp", "version": 2 },
                "contentChanges": [{ "text": "(def stale 0)" }]
            }),
        ),
        notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": "file:///versions.lisp", "version": 4 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 0, "character": 999 },
                        "end": { "line": 0, "character": 999 }
                    },
                    "text": "!"
                }]
            }),
        ),
        request(
            2,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": "file:///versions.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        request(3, "shutdown", json!({})),
        notification("exit", json!({})),
    ]);

    assert_eq!(messages.len(), 5);
    assert_eq!(
        messages[3]["result"]["contents"]["value"],
        "**latest** — `str`"
    );
}

#[test]
fn protocol_clears_closed_documents_and_enforces_session_lifecycle() {
    let messages = protocol_messages(&[
        request(1, "textDocument/completion", json!({})),
        request(2, "initialize", json!({})),
        notification("unknown/notification", json!({})),
        notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": "file:///close.lisp", "version": 1, "text": "(def answer 1)" }
            }),
        ),
        notification(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": "file:///close.lisp" } }),
        ),
        request(
            3,
            "textDocument/hover",
            json!({
                "textDocument": { "uri": "file:///close.lisp" },
                "position": { "line": 0, "character": 6 }
            }),
        ),
        request(4, "unknown/request", json!({})),
        request(5, "shutdown", json!({})),
        request(6, "textDocument/completion", json!({})),
        notification("exit", json!({})),
    ]);

    assert_eq!(messages.len(), 8);
    assert_eq!(messages[0]["error"]["code"], -32002);
    assert_eq!(messages[3]["params"]["diagnostics"], json!([]));
    assert_eq!(messages[4]["result"], Value::Null);
    assert_eq!(messages[5]["error"]["code"], -32601);
    assert_eq!(messages[7]["error"]["code"], -32600);
}

#[test]
fn protocol_rejects_malformed_headers_and_json_without_panicking() {
    let mut output = Vec::new();
    let header_error = crate::lsp::run(
        &mut Cursor::new(b"Content-Length: nope\r\n\r\n"),
        &mut output,
    )
    .unwrap_err()
    .to_string();
    assert!(header_error.contains("parse LSP Content-Length"));

    let mut output = Vec::new();
    let json_error = crate::lsp::run(&mut Cursor::new(b"Content-Length: 1\r\n\r\n{"), &mut output)
        .unwrap_err()
        .to_string();
    assert!(json_error.contains("parse LSP JSON"));
    assert!(output.is_empty());
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

fn protocol_messages(messages: &[Value]) -> Vec<Value> {
    let input = messages.iter().fold(Vec::new(), |mut bytes, message| {
        let body = serde_json::to_vec(message).unwrap();
        bytes.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        bytes.extend_from_slice(&body);
        bytes
    });
    let mut output = Vec::new();
    crate::lsp::run(&mut Cursor::new(input), &mut output).unwrap();
    decode_protocol_messages(&output)
}

fn decode_protocol_messages(mut bytes: &[u8]) -> Vec<Value> {
    let mut messages = vec![];
    while !bytes.is_empty() {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap();
        let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let length = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let body_start = header_end + 4;
        let body_end = body_start + length;
        messages.push(serde_json::from_slice(&bytes[body_start..body_end]).unwrap());
        bytes = &bytes[body_end..];
    }
    messages
}
