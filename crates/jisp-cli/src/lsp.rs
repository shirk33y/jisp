use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

#[derive(Clone, Copy, Eq, PartialEq)]
enum SessionPhase {
    Uninitialized,
    Running,
    Shutdown,
}

struct OpenDocument {
    version: i64,
    text: String,
}

struct Server {
    phase: SessionPhase,
    documents: HashMap<String, OpenDocument>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            phase: SessionPhase::Uninitialized,
            documents: HashMap::new(),
        }
    }
}

pub(crate) fn stdio() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run(&mut stdin.lock(), &mut stdout.lock())
}

pub(crate) fn run(input: &mut impl BufRead, output: &mut impl Write) -> Result<()> {
    let mut server = Server::default();
    while let Some(message) = read_message(input)? {
        if !server.handle(message, output)? {
            break;
        }
    }
    Ok(())
}

impl Server {
    fn handle(&mut self, message: Value, output: &mut impl Write) -> Result<bool> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            self.error(
                output,
                message.get("id"),
                -32600,
                "invalid JSON-RPC request",
            )?;
            return Ok(true);
        };
        if method == "exit" {
            if self.phase != SessionPhase::Shutdown {
                bail!("LSP exit received before shutdown")
            }
            return Ok(false);
        }

        match self.phase {
            SessionPhase::Uninitialized => self.handle_uninitialized(method, &message, output),
            SessionPhase::Running => self.handle_running(method, &message, output),
            SessionPhase::Shutdown => {
                self.error(output, message.get("id"), -32600, "LSP server is shut down")?;
                Ok(true)
            }
        }
    }

    fn handle_uninitialized(
        &mut self,
        method: &str,
        message: &Value,
        output: &mut impl Write,
    ) -> Result<bool> {
        if method != "initialize" {
            self.error(
                output,
                message.get("id"),
                -32002,
                "LSP server is not initialized",
            )?;
            return Ok(true);
        }
        let Some(id) = message.get("id") else {
            return Ok(true);
        };
        self.phase = SessionPhase::Running;
        respond(
            output,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "capabilities": {
                    "textDocumentSync": 2,
                    "completionProvider": { "triggerCharacters": ["(", "."] },
                    "hoverProvider": true,
                    "definitionProvider": true
                } }
            }),
        )?;
        Ok(true)
    }

    fn handle_running(
        &mut self,
        method: &str,
        message: &Value,
        output: &mut impl Write,
    ) -> Result<bool> {
        match method {
            "initialize" => {
                self.error(
                    output,
                    message.get("id"),
                    -32600,
                    "initialize was already handled",
                )?;
            }
            "shutdown" => {
                let Some(id) = message.get("id") else {
                    return Ok(true);
                };
                self.phase = SessionPhase::Shutdown;
                respond(
                    output,
                    json!({ "jsonrpc": "2.0", "id": id, "result": null }),
                )?;
            }
            "textDocument/didOpen" => self.did_open(message, output)?,
            "textDocument/didChange" => self.did_change(message, output)?,
            "textDocument/didClose" => self.did_close(message, output)?,
            "textDocument/completion" => self.completion(message, output)?,
            "textDocument/hover" => self.hover(message, output)?,
            "textDocument/definition" => self.definition(message, output)?,
            _ => self.error(output, message.get("id"), -32601, "method not found")?,
        }
        Ok(true)
    }

    fn did_open(&mut self, message: &Value, output: &mut impl Write) -> Result<()> {
        let Some(document) = message.pointer("/params/textDocument") else {
            return Ok(());
        };
        let (Some(uri), Some(version), Some(text)) = (
            document.get("uri").and_then(Value::as_str),
            document.get("version").and_then(Value::as_i64),
            document.get("text").and_then(Value::as_str),
        ) else {
            return Ok(());
        };
        self.documents.insert(
            uri.to_owned(),
            OpenDocument {
                version,
                text: text.to_owned(),
            },
        );
        self.publish_diagnostics(output, uri, text)
    }

    fn did_change(&mut self, message: &Value, output: &mut impl Write) -> Result<()> {
        let Some(document) = message.pointer("/params/textDocument") else {
            return Ok(());
        };
        let (Some(uri), Some(version), Some(changes)) = (
            document.get("uri").and_then(Value::as_str),
            document.get("version").and_then(Value::as_i64),
            message
                .pointer("/params/contentChanges")
                .and_then(Value::as_array),
        ) else {
            return Ok(());
        };
        let Some(current) = self.documents.get(uri) else {
            return Ok(());
        };
        if version <= current.version {
            return Ok(());
        }
        let Ok(text) = apply_changes(&current.text, changes) else {
            return Ok(());
        };
        self.documents.insert(
            uri.to_owned(),
            OpenDocument {
                version,
                text: text.clone(),
            },
        );
        self.publish_diagnostics(output, uri, &text)
    }

    fn did_close(&mut self, message: &Value, output: &mut impl Write) -> Result<()> {
        let Some(uri) = message
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        self.documents.remove(uri);
        respond(
            output,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": { "uri": uri, "diagnostics": [] }
            }),
        )
    }

    fn completion(&self, message: &Value, output: &mut impl Write) -> Result<()> {
        self.result(
            output,
            message,
            Some(Value::Array(crate::lsp_completion_items())),
        )
    }

    fn hover(&self, message: &Value, output: &mut impl Write) -> Result<()> {
        let result = request_document(message, &self.documents)
            .and_then(|(uri, text, line, character)| crate::lsp_hover(uri, text, line, character));
        self.result(output, message, result)
    }

    fn definition(&self, message: &Value, output: &mut impl Write) -> Result<()> {
        let result =
            request_document(message, &self.documents).and_then(|(uri, text, line, character)| {
                crate::lsp_definition(uri, text, line, character)
            });
        self.result(output, message, result)
    }

    fn result(
        &self,
        output: &mut impl Write,
        message: &Value,
        result: Option<Value>,
    ) -> Result<()> {
        let Some(id) = message.get("id") else {
            return Ok(());
        };
        respond(
            output,
            json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        )
    }

    fn error(
        &self,
        output: &mut impl Write,
        id: Option<&Value>,
        code: i64,
        message: &str,
    ) -> Result<()> {
        let Some(id) = id else {
            return Ok(());
        };
        respond(
            output,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message }
            }),
        )
    }

    fn publish_diagnostics(&self, output: &mut impl Write, uri: &str, text: &str) -> Result<()> {
        respond(
            output,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": { "uri": uri, "diagnostics": crate::lsp_diagnostics(uri, text) }
            }),
        )
    }
}

fn request_document<'message, 'document>(
    message: &'message Value,
    documents: &'document HashMap<String, OpenDocument>,
) -> Option<(&'message str, &'document str, usize, usize)> {
    let uri = message
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)?;
    let line = message
        .pointer("/params/position/line")
        .and_then(Value::as_u64)? as usize;
    let character = message
        .pointer("/params/position/character")
        .and_then(Value::as_u64)? as usize;
    let document = documents.get(uri)?;
    Some((uri, &document.text, line, character))
}

fn apply_changes(text: &str, changes: &[Value]) -> Result<String> {
    let mut text = text.to_owned();
    for change in changes {
        let replacement = change
            .get("text")
            .and_then(Value::as_str)
            .context("LSP content change is missing text")?;
        let Some(range) = change.get("range") else {
            text = replacement.to_owned();
            continue;
        };
        let start = lsp_range_offset(&text, range.pointer("/start"))?;
        let end = lsp_range_offset(&text, range.pointer("/end"))?;
        if start > end {
            bail!("LSP content change has a reversed range")
        }
        text.replace_range(start..end, replacement);
    }
    Ok(text)
}

fn lsp_range_offset(text: &str, position: Option<&Value>) -> Result<usize> {
    let position = position.context("LSP content change is missing a range position")?;
    let line = position
        .get("line")
        .and_then(Value::as_u64)
        .context("LSP range line must be an unsigned integer")? as usize;
    let character = position
        .get("character")
        .and_then(Value::as_u64)
        .context("LSP range character must be an unsigned integer")? as usize;
    crate::lsp_byte_offset(text, line, character)
        .context("LSP range is outside the current UTF-16 document")
}

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            if length.is_some() {
                bail!("duplicate LSP Content-Length header")
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("parse LSP Content-Length")?,
            );
        }
    }
    let length = length.context("missing LSP Content-Length")?;
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .context("parse LSP JSON")
        .map(Some)
}

fn respond(output: &mut impl Write, message: Value) -> Result<()> {
    let body = serde_json::to_vec(&message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}
