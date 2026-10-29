//! Local Model Context Protocol server over stdin/stdout (newline-delimited JSON-RPC).
//!
//! The server edits documents through the same [`Editor`] transactions as the GUI:
//! each `document_apply` batch is validated as a whole and becomes one undo step, or
//! is rejected without changing the document. Every result reports the revision and
//! saved state so a client can tell exactly what is on disk. Diagnostics go to stderr.

use crate::cli::{describe, export_bytes, same_file};
use crate::document::{ColorTarget, ColorToken, Document, Editor, Node, NodeKind};
use crate::render;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::PathBuf,
};

const PROTOCOL_VERSION: &str = "2025-06-18";
/// Largest accepted request line.
const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;
/// Largest number of operations in one batch.
const MAX_OPERATIONS: usize = 10_000;

struct Session {
    editor: Editor,
    path: Option<PathBuf>,
    /// Revision last written to `path`; `None` when never saved.
    saved_revision: Option<u64>,
}

impl Session {
    fn dirty(&self) -> bool {
        self.saved_revision != Some(self.editor.revision())
    }
    fn state(&self, id: &str) -> Value {
        json!({
            "document_id": id,
            "path": self.path.as_ref().map(|p| p.display().to_string()),
            "revision": self.editor.revision(),
            "saved_revision": self.saved_revision,
            "dirty": self.dirty(),
            "can_undo": self.editor.can_undo(),
            "can_redo": self.editor.can_redo(),
        })
    }
}

#[derive(Default)]
pub struct Server {
    sessions: BTreeMap<String, Session>,
    next_session: u64,
}

/// Serves requests until stdin closes.
pub fn serve() -> Result<(), String> {
    let mut server = Server::default();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let mut line = String::new();
    let mut input = stdin.lock();
    loop {
        line.clear();
        let read = input
            .read_line(&mut line)
            .map_err(|e| format!("Cannot read request: {e}"))?;
        if read == 0 {
            return Ok(());
        }
        if line.len() > MAX_REQUEST_BYTES {
            let error = error_response(Value::Null, -32600, "Request is too large.");
            writeln!(stdout, "{error}").map_err(|e| e.to_string())?;
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .map_err(|e| format!("Cannot write response: {e}"))?;
        }
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

impl Server {
    /// Handles one JSON-RPC message; notifications produce no response.
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let request: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(e) => {
                return Some(error_response(
                    Value::Null,
                    -32700,
                    &format!("Parse error: {e}"),
                ));
            }
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str);
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let Some(method) = method else {
            return Some(error_response(
                id.unwrap_or(Value::Null),
                -32600,
                "Missing method.",
            ));
        };
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "papercraft", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(self.call(&params)),
            _ if method.starts_with("notifications/") => return None,
            _ => Err((-32601, format!("Unknown method {method}."))),
        };
        // Notifications (no id) get no response.
        let id = id?;
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    fn call(&mut self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let empty = Map::new();
        let args = params
            .get("arguments")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        match self.tool(name, args) {
            Ok(content) => json!({ "content": content, "isError": false }),
            Err(message) => json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            }),
        }
    }

    fn tool(&mut self, name: &str, args: &Map<String, Value>) -> Result<Vec<Value>, String> {
        let text = |value: Value| Ok(vec![json!({ "type": "text", "text": value.to_string() })]);
        match name {
            "document_new" => {
                allow(args, &["name", "width", "height"])?;
                let mut document = Document::default();
                if let Some(name) = opt_str(args, "name")? {
                    document.name = name.to_owned();
                }
                if let Some(width) = opt_f32(args, "width")? {
                    document.width = width;
                }
                if let Some(height) = opt_f32(args, "height")? {
                    document.height = height;
                }
                document.validate()?;
                let id = self.insert(Session {
                    editor: Editor::new(document),
                    path: None,
                    saved_revision: None,
                });
                text(self.sessions[&id].state(&id))
            }
            "document_open" => {
                allow(args, &["path"])?;
                let path = PathBuf::from(req_str(args, "path")?);
                if let Some((id, session)) = self
                    .sessions
                    .iter()
                    .find(|(_, s)| s.path.as_ref().is_some_and(|p| same_file(p, &path)))
                {
                    return text(session.state(id));
                }
                let document = Document::load(&path)?;
                let id = self.insert(Session {
                    editor: Editor::new(document),
                    path: Some(path),
                    saved_revision: Some(0),
                });
                text(self.sessions[&id].state(&id))
            }
            "document_list" => {
                allow(args, &[])?;
                text(json!(
                    self.sessions
                        .iter()
                        .map(|(id, s)| s.state(id))
                        .collect::<Vec<_>>()
                ))
            }
            "document_inspect" => {
                allow(args, &["document_id"])?;
                let (id, session) = self.session(args)?;
                let mut value = describe(&session.editor.document);
                value["state"] = session.state(&id);
                text(value)
            }
            "document_apply" => {
                allow(args, &["document_id", "operations", "expected_revision"])?;
                let operations = args
                    .get("operations")
                    .and_then(Value::as_array)
                    .ok_or("operations must be an array.")?
                    .clone();
                if operations.is_empty() || operations.len() > MAX_OPERATIONS {
                    return Err(format!(
                        "operations must contain 1 to {MAX_OPERATIONS} items."
                    ));
                }
                let expected = opt_u64(args, "expected_revision")?;
                let (id, session) = self.session_mut(args)?;
                if let Some(expected) = expected
                    && expected != session.editor.revision()
                {
                    return Err(format!(
                        "Revision mismatch: expected {expected}, document is at {}. Inspect it before editing.",
                        session.editor.revision()
                    ));
                }
                let created = session.editor.apply(|doc| {
                    let mut created = Vec::new();
                    for (index, operation) in operations.iter().enumerate() {
                        apply_operation(doc, operation, &mut created).map_err(|e| {
                            format!("Operation {index} failed: {e} No changes were applied.")
                        })?;
                    }
                    Ok(created)
                })?;
                let mut value = session.state(&id);
                value["created"] = json!(created);
                text(value)
            }
            "document_undo" | "document_redo" => {
                allow(args, &["document_id"])?;
                let (id, session) = self.session_mut(args)?;
                let before = session.editor.revision();
                if name == "document_undo" {
                    session.editor.undo();
                } else {
                    session.editor.redo();
                }
                let mut value = session.state(&id);
                value["changed"] = json!(before != session.editor.revision());
                text(value)
            }
            "document_render" => {
                allow(args, &["document_id", "scale"])?;
                let scale = opt_f32(args, "scale")?.unwrap_or(1.0);
                let (id, session) = self.session(args)?;
                let png = render::export_png(&session.editor.document, scale)?;
                let (width, height) = render::png_size(&session.editor.document, scale)?;
                let mut value = session.state(&id);
                value["width"] = json!(width);
                value["height"] = json!(height);
                Ok(vec![
                    json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" }),
                    json!({ "type": "text", "text": value.to_string() }),
                ])
            }
            "document_export" => {
                allow(args, &["document_id", "format", "path", "scale"])?;
                let format = req_str(args, "format")?.to_ascii_lowercase();
                let path = PathBuf::from(req_str(args, "path")?);
                let scale = opt_f32(args, "scale")?.unwrap_or(1.0);
                let (id, session) = self.session(args)?;
                if session.path.as_ref().is_some_and(|p| same_file(p, &path)) {
                    return Err("The export path is the document's own file.".into());
                }
                let data = export_bytes(&session.editor.document, &format, scale)?;
                crate::document::atomic_write(&path, &data)?;
                let mut value = session.state(&id);
                value["exported"] = json!(path.display().to_string());
                value["bytes"] = json!(data.len());
                text(value)
            }
            "document_save" => {
                allow(args, &["document_id", "path"])?;
                let path = opt_str(args, "path")?.map(PathBuf::from);
                let (id, session) = self.session_mut(args)?;
                let path = path
                    .or_else(|| session.path.clone())
                    .ok_or("This document has no file yet; pass path.")?;
                session.editor.document.save(&path)?;
                session.path = Some(path);
                session.saved_revision = Some(session.editor.revision());
                text(session.state(&id))
            }
            "document_close" => {
                allow(args, &["document_id", "discard"])?;
                let discard = opt_bool(args, "discard")?.unwrap_or(false);
                let (id, session) = self.session(args)?;
                if session.dirty() && !discard {
                    return Err(
                        "The document has unsaved changes; save it or pass discard: true.".into(),
                    );
                }
                self.sessions.remove(&id);
                text(json!({ "closed": id }))
            }
            _ => Err(format!("Unknown tool {name}.")),
        }
    }

    fn insert(&mut self, session: Session) -> String {
        self.next_session += 1;
        let id = format!("doc-{}", self.next_session);
        self.sessions.insert(id.clone(), session);
        id
    }

    fn session(&self, args: &Map<String, Value>) -> Result<(String, &Session), String> {
        let id = req_str(args, "document_id")?;
        self.sessions
            .get(id)
            .map(|s| (id.to_owned(), s))
            .ok_or_else(|| format!("No open document {id}; use document_open or document_new."))
    }

    fn session_mut(&mut self, args: &Map<String, Value>) -> Result<(String, &mut Session), String> {
        let id = req_str(args, "document_id")?.to_owned();
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or_else(|| format!("No open document {id}; use document_open or document_new."))?;
        Ok((id, session))
    }
}

/// Applies one batch operation directly to `doc`. Validation of the whole document
/// happens when the batch commits.
fn apply_operation(
    doc: &mut Document,
    operation: &Value,
    created: &mut Vec<Value>,
) -> Result<(), String> {
    let op = operation
        .as_object()
        .ok_or("Each operation must be an object.")?;
    let kind = req_str(op, "op")?;
    match kind {
        "create_rectangle" | "create_text" => {
            let text = kind == "create_text";
            allow(
                op,
                &[
                    "op",
                    "name",
                    "x",
                    "y",
                    "width",
                    "height",
                    "fill",
                    "fill_token",
                    "radius",
                    "text",
                    "font_size",
                    "hidden",
                    "locked",
                    "index",
                ],
            )?;
            if !text && (op.contains_key("text") || op.contains_key("font_size")) {
                return Err("text and font_size apply only to create_text.".into());
            }
            if text && op.contains_key("radius") {
                return Err("radius applies only to create_rectangle.".into());
            }
            let id = doc.next_id;
            doc.next_id = id
                .checked_add(1)
                .ok_or("Layer identifiers are exhausted.")?;
            let mut node = Node {
                id,
                name: if text {
                    "Text".into()
                } else {
                    "Rectangle".into()
                },
                kind: if text {
                    NodeKind::Text
                } else {
                    NodeKind::Rectangle
                },
                x: 0.0,
                y: 0.0,
                width: if text { 220.0 } else { 100.0 },
                height: if text { 40.0 } else { 100.0 },
                fill: if text {
                    [35, 40, 48, 255]
                } else {
                    [210, 214, 220, 255]
                },
                radius: 0.0,
                text: if text { "Text".into() } else { String::new() },
                font_size: 28.0,
                hidden: false,
                locked: false,
                fill_token: None,
            };
            set_node_fields(doc, &mut node, op)?;
            let index = opt_u64(op, "index")?
                .map_or(doc.nodes.len(), |i| (i as usize).min(doc.nodes.len()));
            doc.nodes.insert(index, node);
            created.push(json!({ "op": kind, "id": id }));
        }
        "update_layer" => {
            allow(
                op,
                &[
                    "op",
                    "id",
                    "name",
                    "x",
                    "y",
                    "width",
                    "height",
                    "fill",
                    "fill_token",
                    "radius",
                    "text",
                    "font_size",
                    "hidden",
                    "locked",
                ],
            )?;
            let id = req_u64(op, "id")?;
            let position = doc
                .nodes
                .iter()
                .position(|n| n.id == id)
                .ok_or_else(|| format!("No layer {id}."))?;
            let mut node = doc.nodes[position].clone();
            match node.kind {
                NodeKind::Rectangle if op.contains_key("text") || op.contains_key("font_size") => {
                    return Err(format!(
                        "Layer {id} is a rectangle; text and font_size do not apply."
                    ));
                }
                NodeKind::Text if op.contains_key("radius") => {
                    return Err(format!("Layer {id} is text; radius does not apply."));
                }
                _ => {}
            }
            set_node_fields(doc, &mut node, op)?;
            doc.nodes[position] = node;
        }
        "delete_layers" => {
            allow(op, &["op", "ids"])?;
            let ids = req_ids(op, "ids")?;
            for id in &ids {
                if !doc.nodes.iter().any(|n| n.id == *id) {
                    return Err(format!("No layer {id}."));
                }
            }
            doc.nodes.retain(|n| !ids.contains(&n.id));
        }
        "move_layer" => {
            allow(op, &["op", "id", "index"])?;
            let id = req_u64(op, "id")?;
            let from = doc
                .nodes
                .iter()
                .position(|n| n.id == id)
                .ok_or_else(|| format!("No layer {id}."))?;
            let node = doc.nodes.remove(from);
            let to = (req_u64(op, "index")? as usize).min(doc.nodes.len());
            doc.nodes.insert(to, node);
        }
        "set_artboard" => {
            allow(
                op,
                &[
                    "op",
                    "name",
                    "width",
                    "height",
                    "background",
                    "background_token",
                    "canvas",
                    "canvas_token",
                ],
            )?;
            if let Some(name) = opt_str(op, "name")? {
                doc.name = name.to_owned();
            }
            if let Some(width) = opt_f32(op, "width")? {
                doc.width = width;
            }
            if let Some(height) = opt_f32(op, "height")? {
                doc.height = height;
            }
            for (literal_key, token_key, target) in [
                ("background", "background_token", ColorTarget::Background),
                ("canvas", "canvas_token", ColorTarget::Canvas),
            ] {
                set_color(doc, op, literal_key, token_key, target)?;
            }
        }
        "create_token" => {
            allow(op, &["op", "name", "value"])?;
            let name = req_str(op, "name")?.trim().to_owned();
            doc.check_token_name(&name, None)?;
            let value = parse_color(op.get("value").ok_or("value is required.")?)?;
            let id = doc.next_token_id;
            doc.next_token_id = id
                .checked_add(1)
                .ok_or("Token identifiers are exhausted.")?;
            doc.tokens.push(ColorToken { id, name, value });
            created.push(json!({ "op": kind, "id": id }));
        }
        "update_token" => {
            allow(op, &["op", "id", "name", "value"])?;
            let id = req_u64(op, "id")?;
            if doc.token(id).is_none() {
                return Err(format!("No token {id}."));
            }
            if let Some(name) = opt_str(op, "name")? {
                let name = name.trim().to_owned();
                doc.check_token_name(&name, Some(id))?;
                doc.tokens
                    .iter_mut()
                    .find(|t| t.id == id)
                    .expect("checked")
                    .name = name;
            }
            if let Some(value) = op.get("value") {
                let value = parse_color(value)?;
                doc.tokens
                    .iter_mut()
                    .find(|t| t.id == id)
                    .expect("checked")
                    .value = value;
            }
        }
        "delete_token" => {
            allow(op, &["op", "id"])?;
            let id = req_u64(op, "id")?;
            let value = doc
                .token(id)
                .map(|t| t.value)
                .ok_or_else(|| format!("No token {id}."))?;
            // Bound properties keep the token's color as a literal.
            for target in doc.token_usage(id) {
                if let Some((literal, binding)) = doc.target_mut(target) {
                    *literal = value;
                    *binding = None;
                }
            }
            doc.tokens.retain(|t| t.id != id);
        }
        _ => return Err(format!("Unknown op {kind}.")),
    }
    // Keep bound literals in sync for operations later in the same batch.
    doc.normalize();
    Ok(())
}

fn set_node_fields(doc: &Document, node: &mut Node, op: &Map<String, Value>) -> Result<(), String> {
    if let Some(name) = opt_str(op, "name")? {
        node.name = name.to_owned();
    }
    for (key, field) in [
        ("x", &mut node.x),
        ("y", &mut node.y),
        ("width", &mut node.width),
        ("height", &mut node.height),
    ] {
        if let Some(value) = opt_f32(op, key)? {
            *field = value;
        }
    }
    if let Some(radius) = opt_f32(op, "radius")? {
        node.radius = radius;
    }
    if let Some(size) = opt_f32(op, "font_size")? {
        node.font_size = size;
    }
    if let Some(text) = opt_str(op, "text")? {
        node.text = text.to_owned();
    }
    if let Some(hidden) = opt_bool(op, "hidden")? {
        node.hidden = hidden;
    }
    if let Some(locked) = opt_bool(op, "locked")? {
        node.locked = locked;
    }
    // A literal fill detaches the token unless a token is given in the same operation.
    if let Some(fill) = op.get("fill") {
        node.fill = parse_color(fill)?;
        node.fill_token = None;
    }
    match op.get("fill_token") {
        None => {}
        Some(Value::Null) => node.fill_token = None,
        Some(value) => {
            let id = value
                .as_u64()
                .ok_or("fill_token must be a token id or null.")?;
            let token = doc.token(id).ok_or_else(|| format!("No token {id}."))?;
            node.fill = token.value;
            node.fill_token = Some(id);
        }
    }
    Ok(())
}

fn set_color(
    doc: &mut Document,
    op: &Map<String, Value>,
    literal_key: &str,
    token_key: &str,
    target: ColorTarget,
) -> Result<(), String> {
    let literal = op.get(literal_key).map(parse_color).transpose()?;
    let token = match op.get(token_key) {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            let id = value
                .as_u64()
                .ok_or_else(|| format!("{token_key} must be a token id or null."))?;
            let value = doc
                .token(id)
                .ok_or_else(|| format!("No token {id}."))?
                .value;
            Some(Some((id, value)))
        }
    };
    let Some((color, binding)) = doc.target_mut(target) else {
        return Ok(());
    };
    if let Some(literal) = literal {
        *color = literal;
        *binding = None;
    }
    match token {
        None => {}
        Some(None) => *binding = None,
        Some(Some((id, value))) => {
            *color = value;
            *binding = Some(id);
        }
    }
    Ok(())
}

/// Accepts `#rgb`, `#rrggbb`, `#rrggbbaa` or `[r, g, b, a]`.
fn parse_color(value: &Value) -> Result<[u8; 4], String> {
    let invalid =
        || format!("Invalid color {value}; use \"#rrggbb\", \"#rrggbbaa\" or [r, g, b, a].");
    match value {
        Value::String(s) => {
            let hex = s.strip_prefix('#').ok_or_else(invalid)?;
            let digits: Vec<u8> = hex
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect::<Option<_>>()
                .ok_or_else(invalid)?;
            match digits.as_slice() {
                [r, g, b] => Ok([r * 17, g * 17, b * 17, 255]),
                [r1, r2, g1, g2, b1, b2] => Ok([r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2, 255]),
                [r1, r2, g1, g2, b1, b2, a1, a2] => {
                    Ok([r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2, a1 * 16 + a2])
                }
                _ => Err(invalid()),
            }
        }
        Value::Array(items) if items.len() == 4 => {
            let mut color = [0u8; 4];
            for (slot, item) in color.iter_mut().zip(items) {
                *slot = item.as_u64().filter(|n| *n <= 255).ok_or_else(invalid)? as u8;
            }
            Ok(color)
        }
        _ => Err(invalid()),
    }
}

/// Rejects fields outside `allowed`, so misspelled keys are not silently ignored.
fn allow(fields: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match fields.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(key) => Err(format!(
            "Unknown field {key}; expected one of: {}.",
            allowed.join(", ")
        )),
        None => Ok(()),
    }
}

fn req_str<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    opt_str(fields, key)?.ok_or_else(|| format!("{key} is required."))
}
fn opt_str<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    fields
        .get(key)
        .map(|v| v.as_str().ok_or_else(|| format!("{key} must be a string.")))
        .transpose()
}
fn opt_f32(fields: &Map<String, Value>, key: &str) -> Result<Option<f32>, String> {
    fields
        .get(key)
        .map(|v| {
            v.as_f64()
                .map(|n| n as f32)
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("{key} must be a finite number."))
        })
        .transpose()
}
fn opt_u64(fields: &Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    fields
        .get(key)
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| format!("{key} must be a non-negative integer."))
        })
        .transpose()
}
fn req_u64(fields: &Map<String, Value>, key: &str) -> Result<u64, String> {
    opt_u64(fields, key)?.ok_or_else(|| format!("{key} is required."))
}
fn opt_bool(fields: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    fields
        .get(key)
        .map(|v| {
            v.as_bool()
                .ok_or_else(|| format!("{key} must be true or false."))
        })
        .transpose()
}
fn req_ids(fields: &Map<String, Value>, key: &str) -> Result<Vec<u64>, String> {
    fields
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be an array of layer ids."))?
        .iter()
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| format!("{key} must contain layer ids."))
        })
        .collect()
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

const INSTRUCTIONS: &str = "PaperCraft edits local .papercraft documents. Open or create a document, \
inspect it, then send edits with document_apply as one batch per logical change; pass \
expected_revision from your last inspect to avoid editing a state you have not seen. A batch is \
all-or-nothing and one undo step. Use document_render to check the result visually, then \
document_save. Every result reports revision, saved_revision and dirty.";

fn tools() -> Value {
    let id = json!({ "type": "string", "description": "Identifier returned by document_open or document_new." });
    let color = json!({ "description": "\"#rrggbb\", \"#rrggbbaa\" or [r, g, b, a]." });
    json!([
        {
            "name": "document_new",
            "description": "Create an unsaved document with one artboard.",
            "inputSchema": { "type": "object", "additionalProperties": false, "properties": {
                "name": { "type": "string" }, "width": { "type": "number" }, "height": { "type": "number" } } }
        },
        {
            "name": "document_open",
            "description": "Open a .papercraft (or legacy .crumbs) file. Opening an already open file returns its existing document_id.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["path"],
                "properties": { "path": { "type": "string" } } }
        },
        {
            "name": "document_list",
            "description": "List open documents with their revision and saved state.",
            "inputSchema": { "type": "object", "additionalProperties": false, "properties": {} }
        },
        {
            "name": "document_inspect",
            "description": "Describe the artboard, ordered layers (paint order, first is bottom) and color tokens. Colors are resolved #rrggbbaa values.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id } }
        },
        {
            "name": "document_apply",
            "description": "Apply operations as one all-or-nothing batch and one undo step. Operations (field \"op\"): create_rectangle, create_text {name,x,y,width,height,fill,fill_token,radius|text,font_size,hidden,locked,index}; update_layer {id,...same fields}; delete_layers {ids}; move_layer {id,index}; set_artboard {name,width,height,background,background_token,canvas,canvas_token}; create_token {name,value}; update_token {id,name,value}; delete_token {id}. A literal fill detaches a token; fill_token null detaches and keeps the color. Unknown fields are errors. Created ids are returned in order.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id", "operations"],
                "properties": {
                    "document_id": id,
                    "expected_revision": { "type": "integer", "description": "Reject the batch unless the document is at this revision." },
                    "operations": { "type": "array", "items": { "type": "object", "required": ["op"],
                        "properties": { "op": { "type": "string", "enum": [
                            "create_rectangle", "create_text", "update_layer", "delete_layers", "move_layer",
                            "set_artboard", "create_token", "update_token", "delete_token"] },
                            "fill": color, "value": color } } } } }
        },
        {
            "name": "document_undo",
            "description": "Undo the last batch or editor change.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id } }
        },
        {
            "name": "document_redo",
            "description": "Redo the last undone change.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id } }
        },
        {
            "name": "document_render",
            "description": "Render the artboard to PNG with the editor's own text layout, independent of any window. Returns the image.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id, "scale": { "type": "number", "minimum": 0.1, "maximum": 8, "default": 1 } } }
        },
        {
            "name": "document_export",
            "description": "Export the artboard to html, svg or png at path (written atomically).",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id", "format", "path"],
                "properties": { "document_id": id, "format": { "type": "string", "enum": ["html", "svg", "png"] },
                    "path": { "type": "string" }, "scale": { "type": "number", "minimum": 0.1, "maximum": 8 } } }
        },
        {
            "name": "document_save",
            "description": "Validate and write the document atomically to its file, or to path (which becomes its file).",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id, "path": { "type": "string" } } }
        },
        {
            "name": "document_close",
            "description": "Close a document. Refuses unsaved changes unless discard is true.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id, "discard": { "type": "boolean" } } }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn colors_parse_in_every_accepted_form() {
        assert_eq!(parse_color(&json!("#f60")).unwrap(), [255, 102, 0, 255]);
        assert_eq!(parse_color(&json!("#291D35")).unwrap(), [41, 29, 53, 255]);
        assert_eq!(parse_color(&json!("#291d3580")).unwrap(), [41, 29, 53, 128]);
        assert_eq!(parse_color(&json!([1, 2, 3, 4])).unwrap(), [1, 2, 3, 4]);
        for bad in [
            json!("291d35"),
            json!("#12345"),
            json!([1, 2, 3]),
            json!([0, 0, 0, 256]),
            json!(7),
        ] {
            assert!(parse_color(&bad).is_err(), "{bad}");
        }
    }
}
