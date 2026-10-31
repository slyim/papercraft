//! Local Model Context Protocol server over stdin/stdout (newline-delimited JSON-RPC).
//!
//! The server edits documents through the same [`Editor`] transactions as the GUI:
//! each `document_apply` batch is validated as a whole and becomes one undo step, or
//! is rejected without changing the document. Every result reports the revision and
//! saved state so a client can tell exactly what is on disk. Diagnostics go to stderr.

use crate::cli::{describe, export_bytes, same_file};
use crate::document::{
    Align, Axis, ColorTarget, ColorToken, Document, Editor, Node, NodeKind, ShaderFill, ShaderKind,
    TextAlign,
};

/// Operation fields that apply only to text layers.
const TEXT_ONLY: [&str; 5] = ["text", "font_size", "line_height", "text_align", "fit_text"];
use crate::render;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::PathBuf,
};

const PROTOCOL_VERSION: &str = "2025-06-18";
/// Largest accepted request line.
pub(crate) const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;
/// Largest number of operations in one batch.
const MAX_OPERATIONS: usize = 10_000;

struct Session {
    editor: Editor,
    path: Option<PathBuf>,
    /// Revision last written to `path`; `None` when never saved.
    saved_revision: Option<u64>,
    /// Format version of the file at `path` when it was opened or last saved.
    file_version: u32,
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
            "selected_ids": self.editor.selection(),
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
        handle_request(line, |params| self.call(params))
    }

    fn call(&mut self, params: &Value) -> Value {
        tool_result(params, |name, args| self.tool(name, args))
    }

    fn tool(&mut self, name: &str, args: &Map<String, Value>) -> Result<Vec<Value>, String> {
        let text = |value: Value| Ok(vec![json!({ "type": "text", "text": value.to_string() })]);
        if matches!(name, "document_save" | "document_export")
            && let Some(path) = opt_str(args, "path")?
            && self.sessions.iter().any(|(id, session)| {
                Some(id.as_str()) != args.get("document_id").and_then(Value::as_str)
                    && session
                        .path
                        .as_ref()
                        .is_some_and(|p| same_file(p, std::path::Path::new(path)))
            })
        {
            return Err(
                "The destination belongs to another open document. Choose another path.".into(),
            );
        }
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
                    file_version: 0,
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
                let file_version = document.required_version();
                let id = self.insert(Session {
                    editor: Editor::new(document),
                    path: Some(path),
                    saved_revision: Some(0),
                    file_version,
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
                let mut selection_update = None;
                let mut refs = BTreeMap::new();
                let mut created = session.editor.apply(|doc| {
                    let mut created = Vec::new();
                    for (index, operation) in operations.iter().enumerate() {
                        apply_referenced_operation(
                            doc,
                            operation,
                            &mut created,
                            &mut selection_update,
                            &mut refs,
                        )
                        .map_err(|e| {
                            format!("Operation {index} failed: {e} No changes were applied.")
                        })?;
                    }
                    Ok(created)
                })?;
                if let Some(ids) = selection_update {
                    session.editor.set_selection(ids);
                }
                for item in &mut created {
                    if created_kind(item["op"].as_str().unwrap_or("")) == Some("layer")
                        && let Some(node) = session
                            .editor
                            .document
                            .nodes
                            .iter()
                            .find(|n| Some(n.id) == item["id"].as_u64())
                    {
                        item["bounds"] = json!(node.bounds());
                    }
                }
                let mut value = session.state(&id);
                value["created"] = json!(created);
                value["refs"] = json!(
                    refs.into_iter()
                        .map(|(label, (_, id))| (label, id))
                        .collect::<BTreeMap<_, _>>()
                );
                value["content_bounds"] = json!(session.editor.document.content_bounds());
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
                allow(args, &["document_id", "scale", "region", "frame_id"])?;
                let scale = opt_f32(args, "scale")?.unwrap_or(1.0);
                let (id, session) = self.session(args)?;
                let bounds = render_bounds(&session.editor.document, args)?;
                let (width, height) = render::png_region_size(bounds, scale)?;
                let png = render::export_png_region(&session.editor.document, bounds, scale)?;
                let mut value = session.state(&id);
                value["width"] = json!(width);
                value["height"] = json!(height);
                value["bounds"] = json!(bounds);
                Ok(vec![
                    json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" }),
                    json!({ "type": "text", "text": value.to_string() }),
                ])
            }
            "document_export" => {
                allow(
                    args,
                    &[
                        "document_id",
                        "format",
                        "path",
                        "scale",
                        "region",
                        "frame_id",
                    ],
                )?;
                let format = req_str(args, "format")?.to_ascii_lowercase();
                let path = PathBuf::from(req_str(args, "path")?);
                let scale = opt_f32(args, "scale")?.unwrap_or(1.0);
                let (id, session) = self.session(args)?;
                if session.path.as_ref().is_some_and(|p| same_file(p, &path)) {
                    return Err("The export path is the document's own file.".into());
                }
                let bounds = render_bounds(&session.editor.document, args)?;
                let has_region = args.contains_key("region") || args.contains_key("frame_id");
                if format != "png" && (has_region || args.contains_key("scale")) {
                    return Err("region, frame_id and scale apply only to PNG export. Use document_render or export PNG for a bounded world crop.".into());
                }
                let data = if format == "png" {
                    render::export_png_region(&session.editor.document, bounds, scale)?
                } else {
                    export_bytes(&session.editor.document, &format, scale)?
                };
                crate::document::atomic_write(&path, &data)?;
                let mut value = session.state(&id);
                value["bounds"] = json!(bounds);
                if format == "png" {
                    let (width, height) = render::png_region_size(bounds, scale)?;
                    value["width"] = json!(width);
                    value["height"] = json!(height);
                }
                value["exported"] = json!(path.display().to_string());
                value["bytes"] = json!(data.len());
                text(value)
            }
            "document_save" => {
                allow(args, &["document_id", "path", "allow_upgrade"])?;
                let allow_upgrade = opt_bool(args, "allow_upgrade")?.unwrap_or(false);
                let path = opt_str(args, "path")?.map(PathBuf::from);
                let (id, session) = self.session_mut(args)?;
                let path = path
                    .or_else(|| session.path.clone())
                    .ok_or("This document has no file yet; pass path.")?;
                let version = session.editor.document.required_version();
                let upgraded = session
                    .path
                    .as_ref()
                    .is_some_and(|p| same_file(p, &path) && version > session.file_version);
                if upgraded && !allow_upgrade {
                    return Err("Saving would upgrade this file. Use a new path or pass allow_upgrade: true to explicitly replace the original.".into());
                }
                session.editor.document.save(&path)?;
                let previous = std::mem::replace(&mut session.file_version, version);
                session.path = Some(path);
                session.saved_revision = Some(session.editor.revision());
                let mut value = session.state(&id);
                value["version"] = json!(version);
                if upgraded {
                    value["upgraded_from"] = json!(previous);
                }
                text(value)
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

pub(crate) fn handle_request(line: &str, mut call: impl FnMut(&Value) -> Value) -> Option<Value> {
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
        "tools/call" => Ok(call(&params)),
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

pub(crate) fn tool_result(
    params: &Value,
    mut call: impl FnMut(&str, &Map<String, Value>) -> Result<Vec<Value>, String>,
) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let empty = Map::new();
    let args = params
        .get("arguments")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    match call(name, args) {
        Ok(content) => json!({ "content": content, "isError": false }),
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    }
}

/// Temporarily lends the GUI's authoritative editor to the existing tool handler.
/// The editor, including selection and both history stacks, is always returned.
pub(crate) fn editor_tool(
    name: &str,
    args: &Map<String, Value>,
    id: &str,
    editor: &mut Editor,
    path: &mut Option<PathBuf>,
    saved_revision: &mut Option<u64>,
) -> Result<Vec<Value>, String> {
    let file_version = editor.document.required_version();
    let mut server = Server::default();
    server.sessions.insert(
        id.to_owned(),
        Session {
            editor: std::mem::replace(editor, Editor::new(Document::default())),
            path: path.clone(),
            saved_revision: *saved_revision,
            file_version,
        },
    );
    let result = server.tool(name, args);
    let session = server
        .sessions
        .remove(id)
        .expect("document lifecycle is handled by GUI");
    *editor = session.editor;
    *path = session.path;
    *saved_revision = session.saved_revision;
    result
}

/// Resolves an explicit render crop without changing any authored geometry.
fn render_bounds(doc: &Document, args: &Map<String, Value>) -> Result<[f32; 4], String> {
    if args.contains_key("region") && args.contains_key("frame_id") {
        return Err("Pass region or frame_id, not both.".into());
    }
    if let Some(value) = args.get("region") {
        let region = value
            .as_object()
            .ok_or("region must be {x, y, width, height}.")?;
        allow(region, &["x", "y", "width", "height"])?;
        let number =
            |key| opt_f32(region, key)?.ok_or_else(|| format!("region.{key} is required."));
        let (x, y, w, h) = (
            number("x")?,
            number("y")?,
            number("width")?,
            number("height")?,
        );
        let bounds = [x, y, x + w, y + h];
        // Geometry and pixel budgets are checked together at the requested scale
        // by the renderer, before allocation or writing.
        return Ok(bounds);
    }
    if let Some(id) = opt_u64(args, "frame_id")? {
        let frame = doc
            .nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| format!("No frame {id}."))?;
        if frame.kind != NodeKind::Frame {
            return Err(format!(
                "Layer {id} is not a frame. Use region for a custom crop."
            ));
        }
        return Ok(frame.bounds());
    }
    Ok(doc.artboard_bounds())
}

fn created_kind(op: &str) -> Option<&'static str> {
    match op {
        "create_token" => Some("token"),
        "create_directive" => Some("directive"),
        "create_frame" | "create_rectangle" | "create_ellipse" | "create_triangle"
        | "create_shape" | "create_path" | "create_text" | "wrap_in_frame" | "duplicate_layers" => {
            Some("layer")
        }
        _ => None,
    }
}

type BatchRefs = BTreeMap<String, (&'static str, u64)>;

fn resolve_ref(value: &mut Value, expected: &str, refs: &BatchRefs) -> Result<(), String> {
    if let Some(object) = value.as_object() {
        allow(object, &["ref"])?;
        let label = req_str(object, "ref")?;
        let &(kind, id) = refs.get(label).ok_or_else(|| {
            format!("Unknown batch reference {label}; only earlier results may be referenced.")
        })?;
        if kind != expected {
            return Err(format!("Reference {label} is a {kind}, not a {expected}."));
        }
        *value = json!(id);
    }
    Ok(())
}

fn apply_referenced_operation(
    doc: &mut Document,
    operation: &Value,
    created: &mut Vec<Value>,
    selection: &mut Option<Vec<u64>>,
    refs: &mut BatchRefs,
) -> Result<(), String> {
    let mut operation = operation.clone();
    let fields = operation
        .as_object_mut()
        .ok_or("Each operation must be an object.")?;
    let op = req_str(fields, "op")?.to_owned();
    let label = opt_str(fields, "ref")?.map(str::to_owned);
    if let Some(label) = &label {
        if label.is_empty()
            || label.len() > 64
            || !label
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("ref must contain 1–64 ASCII letters/digits, _ or -.".into());
        }
        if refs.contains_key(label) {
            return Err(format!("Duplicate batch reference {label}."));
        }
        if created_kind(&op).is_none() || op == "duplicate_layers" {
            return Err(
                "ref is supported only on single-result create operations and wrap_in_frame."
                    .into(),
            );
        }
        fields.remove("ref");
    }
    for (key, value) in fields.iter_mut() {
        let expected = match key.as_str() {
            "id" if matches!(op.as_str(), "update_token" | "delete_token") => "token",
            "id" if matches!(op.as_str(), "resolve_directive" | "delete_directive") => "directive",
            "id" | "parent" | "layer_id" => "layer",
            "fill_token" | "stroke_token" | "background_token" | "canvas_token" => "token",
            "ids" => {
                if let Some(ids) = value.as_array_mut() {
                    for id in ids {
                        resolve_ref(id, "layer", refs)?;
                    }
                }
                continue;
            }
            _ => continue,
        };
        resolve_ref(value, expected, refs)?;
    }
    apply_operation(doc, &operation, created, selection)?;
    if let Some(label) = label {
        let id = created
            .last()
            .and_then(|v| v["id"].as_u64())
            .ok_or("Operation returned no identifier.")?;
        refs.insert(label, (created_kind(&op).expect("checked above"), id));
    }
    Ok(())
}

/// Applies one batch operation directly to `doc`. Validation of the whole document
/// happens when the batch commits.
fn apply_operation(
    doc: &mut Document,
    operation: &Value,
    created: &mut Vec<Value>,
    selection_update: &mut Option<Vec<u64>>,
) -> Result<(), String> {
    let op = operation
        .as_object()
        .ok_or("Each operation must be an object.")?;
    let kind = req_str(op, "op")?;
    match kind {
        "create_frame" | "create_rectangle" | "create_ellipse" | "create_triangle"
        | "create_shape" | "create_path" | "create_text" => {
            let text = kind == "create_text";
            allow(
                op,
                &[
                    "op",
                    "name",
                    "shape",
                    "parent",
                    "clip",
                    "x",
                    "y",
                    "width",
                    "height",
                    "fill",
                    "fill_token",
                    "stroke",
                    "stroke_token",
                    "stroke_width",
                    "radius",
                    "points",
                    "anchors",
                    "svg_path",
                    "closed",
                    "shader",
                    "text",
                    "font_size",
                    "line_height",
                    "text_align",
                    "fit_text",
                    "hidden",
                    "locked",
                    "index",
                ],
            )?;
            if !text && TEXT_ONLY.iter().any(|k| op.contains_key(*k)) {
                return Err(format!(
                    "{} apply only to create_text.",
                    TEXT_ONLY.join(", ")
                ));
            }
            if matches!(kind, "create_text" | "create_frame" | "create_path")
                && op.contains_key("shape")
            {
                return Err("shape applies only to shape layers.".into());
            }
            let node_kind = match kind {
                "create_text" => NodeKind::Text,
                "create_frame" => NodeKind::Frame,
                "create_path" => NodeKind::Path,
                _ => {
                    let default_kind = match kind {
                        "create_ellipse" => NodeKind::Ellipse,
                        "create_triangle" => NodeKind::Triangle,
                        _ => NodeKind::Rectangle,
                    };
                    match opt_str(op, "shape")? {
                        Some(shape) => parse_shape(shape)?,
                        None => default_kind,
                    }
                }
            };
            if !node_kind.supports_radius() && op.contains_key("radius") {
                return Err(format!(
                    "radius applies only to rectangles and frames, not {}.",
                    node_kind.name()
                ));
            }
            let id = doc.next_id;
            doc.next_id = id
                .checked_add(1)
                .ok_or("Layer identifiers are exhausted.")?;
            let default_name = node_kind.label().into();
            let mut node = match node_kind {
                NodeKind::Frame => Node {
                    id,
                    name: default_name,
                    kind: node_kind,
                    width: 320.0,
                    height: 220.0,
                    fill: [244, 242, 236, 255],
                    radius: 12.0,
                    clip: true,
                    ..Node::default()
                },
                NodeKind::Path => Node {
                    id,
                    name: default_name,
                    kind: node_kind,
                    width: 100.0,
                    height: 100.0,
                    fill: [255, 107, 43, 0],
                    stroke: Some([255, 107, 43, 255]),
                    stroke_width: 3.0,
                    points: crate::document::default_path_points(),
                    ..Node::default()
                },
                NodeKind::Text => Node {
                    id,
                    name: default_name,
                    kind: node_kind,
                    width: 220.0,
                    height: 40.0,
                    fill: [35, 40, 48, 255],
                    text: "Text".into(),
                    font_size: 28.0,
                    ..Node::default()
                },
                _ => Node {
                    id,
                    name: default_name,
                    kind: node_kind,
                    width: 100.0,
                    height: 100.0,
                    fill: [210, 214, 220, 255],
                    font_size: 28.0,
                    ..Node::default()
                },
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
                    "shape",
                    "parent",
                    "clip",
                    "x",
                    "y",
                    "width",
                    "height",
                    "fill",
                    "fill_token",
                    "stroke",
                    "stroke_token",
                    "stroke_width",
                    "radius",
                    "points",
                    "anchors",
                    "svg_path",
                    "closed",
                    "shader",
                    "text",
                    "font_size",
                    "line_height",
                    "text_align",
                    "fit_text",
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
            if doc.is_effectively_locked(&node)
                && ["x", "y", "width", "height", "points", "anchors", "svg_path"]
                    .iter()
                    .any(|k| op.contains_key(*k))
            {
                return Err("Unlock the layer and its ancestors before editing geometry.".into());
            }
            if let Some(shape) = opt_str(op, "shape")? {
                if !node.kind.is_shape() {
                    return Err(format!(
                        "Layer {id} is {}; shape does not apply.",
                        node.kind.name()
                    ));
                }
                node.set_shape(parse_shape(shape)?);
            }
            if node.kind != NodeKind::Text && TEXT_ONLY.iter().any(|k| op.contains_key(*k)) {
                return Err(format!(
                    "Layer {id} is a {}; {} do not apply.",
                    node.kind.name(),
                    TEXT_ONLY.join(", ")
                ));
            }
            if !node.kind.supports_radius() && op.contains_key("radius") {
                return Err(format!(
                    "Layer {id} is {}; radius applies only to rectangles and frames.",
                    node.kind.name()
                ));
            }
            set_node_fields(doc, &mut node, op)?;
            let old = &doc.nodes[position];
            if [node.x, node.y, node.width, node.height] != [old.x, old.y, old.width, old.height] {
                doc.set_geometry(id, [node.x, node.y, node.width, node.height])?;
            }
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
            doc.delete_nodes(&ids)?;
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
        "select_layers" => {
            allow(op, &["op", "ids"])?;
            let ids = req_ids(op, "ids")?;
            for id in &ids {
                if !doc.nodes.iter().any(|n| n.id == *id) {
                    return Err(format!("No layer {id}."));
                }
            }
            *selection_update = Some(ids);
        }
        "align_layers" => {
            allow(op, &["op", "ids", "align"])?;
            let ids = req_ids(op, "ids")?;
            let align_name = req_str(op, "align")?;
            let align = match align_name {
                "left" => Align::Left,
                "center" | "center_x" | "horizontal_center" => Align::CenterX,
                "right" => Align::Right,
                "top" => Align::Top,
                "middle" | "center_y" | "vertical_center" => Align::CenterY,
                "bottom" => Align::Bottom,
                _ => {
                    return Err(format!(
                        "align must be left, center, right, top, middle or bottom, not {align_name}."
                    ));
                }
            };
            doc.align_nodes(&ids, align)?;
        }
        "distribute_layers" => {
            allow(op, &["op", "ids", "axis"])?;
            let ids = req_ids(op, "ids")?;
            let axis_name = req_str(op, "axis")?;
            let axis = match axis_name {
                "horizontal" => Axis::Horizontal,
                "vertical" => Axis::Vertical,
                _ => {
                    return Err(format!(
                        "axis must be horizontal or vertical, not {axis_name}."
                    ));
                }
            };
            doc.distribute_nodes(&ids, axis)?;
        }
        "duplicate_layers" => {
            allow(op, &["op", "ids", "offset_x", "offset_y"])?;
            let ids = req_ids(op, "ids")?;
            let dx = opt_f32(op, "offset_x")?.unwrap_or(20.0);
            let dy = opt_f32(op, "offset_y")?.unwrap_or(20.0);
            let new_ids = doc.duplicate_nodes(&ids, dx, dy)?;
            for &id in &new_ids {
                created.push(json!({ "op": "duplicate_layers", "id": id }));
            }
            *selection_update = Some(new_ids);
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
            let target = op
                .get("id")
                .or_else(|| op.get("name"))
                .ok_or("id is required.")?;
            let id = resolve_token_id(doc, target, "id")?;
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
            allow(op, &["op", "id", "name"])?;
            let target = op
                .get("id")
                .or_else(|| op.get("name"))
                .ok_or("id is required.")?;
            let id = resolve_token_id(doc, target, "id")?;
            let value = doc
                .token(id)
                .map(|t| t.value)
                .expect("resolved token exists");
            // Bound properties keep the token's color as a literal.
            for target in doc.token_usage(id) {
                if let Some((literal, binding)) = doc.target_mut(target) {
                    *literal = value;
                    *binding = None;
                }
            }
            doc.tokens.retain(|t| t.id != id);
        }
        "wrap_in_frame" => {
            allow(op, &["op", "ids", "name"])?;
            let ids = req_ids(op, "ids")?;
            let name = opt_str(op, "name")?;
            let frame_id = doc.wrap_in_frame(&ids, name)?;
            created.push(json!({ "op": "wrap_in_frame", "id": frame_id }));
            *selection_update = Some(vec![frame_id]);
        }
        "create_directive" => {
            allow(op, &["op", "layer_id", "x", "y", "message"])?;
            let layer_id = match op.get("layer_id") {
                None | Some(Value::Null) => None,
                Some(_) => opt_u64(op, "layer_id")?,
            };
            let layer_pos = layer_id
                .and_then(|id| doc.nodes.iter().find(|n| n.id == id))
                .map(|n| (n.x, n.y));
            let x = opt_f32(op, "x")?.unwrap_or_else(|| layer_pos.map_or(40.0, |(lx, _)| lx));
            let y = opt_f32(op, "y")?.unwrap_or_else(|| layer_pos.map_or(40.0, |(_, ly)| ly));
            let message = req_str(op, "message")?;
            let id = doc.add_directive(layer_id, x, y, message)?;
            created.push(json!({ "op": "create_directive", "id": id }));
        }
        "resolve_directive" => {
            allow(op, &["op", "id", "resolved"])?;
            let id = req_u64(op, "id")?;
            let resolved = opt_bool(op, "resolved")?.unwrap_or(true);
            doc.resolve_directive(id, resolved)?;
        }
        "delete_directive" => {
            allow(op, &["op", "id"])?;
            let id = req_u64(op, "id")?;
            doc.delete_directive(id)?;
        }
        _ => return Err(format!("Unknown op {kind}.")),
    }
    // Keep bound literals in sync for operations later in the same batch.
    doc.normalize();
    Ok(())
}

fn parse_shape(name: &str) -> Result<NodeKind, String> {
    NodeKind::from_shape_name(name)
        .ok_or_else(|| format!("shape must be rectangle, ellipse or triangle, not {name}."))
}

fn resolve_token_id(doc: &Document, value: &Value, field: &str) -> Result<u64, String> {
    if let Some(id) = value.as_u64() {
        doc.token(id).ok_or_else(|| format!("No token {id}."))?;
        Ok(id)
    } else if let Some(name) = value.as_str() {
        doc.token_by_name(name)
            .map(|t| t.id)
            .ok_or_else(|| format!("No token {name}."))
    } else {
        Err(format!("{field} must be a token id, token name or null."))
    }
}

fn set_node_fields(doc: &Document, node: &mut Node, op: &Map<String, Value>) -> Result<(), String> {
    if ["anchors", "points", "svg_path"]
        .iter()
        .filter(|key| op.contains_key(**key))
        .count()
        > 1
    {
        return Err("Path geometry inputs anchors/points/svg_path are mutually exclusive.".into());
    }
    if let Some(name) = opt_str(op, "name")? {
        node.name = name.to_owned();
    }
    match op.get("parent") {
        None => {}
        Some(Value::Null) => node.parent = None,
        Some(value) => {
            let parent_id = value
                .as_u64()
                .ok_or("parent must be a frame layer id or null.")?;
            if parent_id == node.id {
                return Err("A layer cannot be its own parent.".into());
            }
            let parent = doc
                .nodes
                .iter()
                .find(|n| n.id == parent_id)
                .ok_or_else(|| format!("No layer {parent_id}."))?;
            if !parent.kind.is_frame() {
                return Err(format!("Layer {parent_id} is not a frame."));
            }
            node.parent = Some(parent_id);
        }
    }
    if let Some(clip) = opt_bool(op, "clip")? {
        if !node.kind.is_frame() {
            return Err(format!(
                "clip applies only to frames, not {}.",
                node.kind.name()
            ));
        }
        node.clip = clip;
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
    match op.get("line_height") {
        None => {}
        Some(Value::Null) => node.line_height = None,
        Some(_) => node.line_height = opt_f32(op, "line_height")?,
    }
    if let Some(align) = opt_str(op, "text_align")? {
        node.text_align = match align {
            "left" => TextAlign::Left,
            "center" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => {
                return Err(format!(
                    "text_align must be left, center or right, not {align}."
                ));
            }
        };
    }
    if let Some(size) = opt_f32(op, "font_size")? {
        node.font_size = size;
    }
    if let Some(text) = opt_str(op, "text")? {
        node.text = text.to_owned();
    }
    if opt_bool(op, "fit_text")?.unwrap_or(false) {
        let metrics = render::measure_text(node);
        node.height = metrics.height.ceil().max(1.0);
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
            let id = resolve_token_id(doc, value, "fill_token")?;
            let token = doc.token(id).expect("resolved token exists");
            node.fill = token.value;
            node.fill_token = Some(id);
        }
    }
    match op.get("stroke") {
        None => {}
        Some(Value::Null) => {
            node.stroke = None;
            node.stroke_token = None;
        }
        Some(value) => {
            node.stroke = Some(parse_color(value)?);
            node.stroke_token = None;
            if node.stroke_width == 0.0 {
                node.stroke_width = 1.0;
            }
        }
    }
    match op.get("stroke_token") {
        None => {}
        Some(Value::Null) => node.stroke_token = None,
        Some(value) => {
            let id = resolve_token_id(doc, value, "stroke_token")?;
            let token = doc.token(id).expect("resolved token exists");
            node.stroke = Some(token.value);
            node.stroke_token = Some(id);
            if node.stroke_width == 0.0 {
                node.stroke_width = 1.0;
            }
        }
    }
    if let Some(width) = opt_f32(op, "stroke_width")? {
        if !(0.0..=1000.0).contains(&width) {
            return Err("stroke_width must be between 0 and 1000.".into());
        }
        node.stroke_width = width;
    }
    if let Some(value) = op.get("anchors") {
        if !node.kind.is_path() || op.contains_key("points") || op.contains_key("svg_path") {
            return Err(
                "anchors apply only to paths and are mutually exclusive with points/svg_path."
                    .into(),
            );
        }
        node.anchors =
            serde_json::from_value(value.clone()).map_err(|e| format!("Invalid anchors: {e}"))?;
        if !(2..=crate::vector::MAX_ANCHORS).contains(&node.anchors.len()) {
            return Err("anchors requires 2–1,024 records.".into());
        }
        node.points.clear();
    }
    if let Some(d) = opt_str(op, "svg_path")? {
        if !node.kind.is_path() {
            return Err(format!(
                "svg_path applies only to path layers, not {}.",
                node.kind.name()
            ));
        }
        let (points, closed) = crate::document::parse_svg_path_points(d)?;
        node.points = points;
        node.anchors.clear();
        node.closed = closed;
    }
    if let Some(points_val) = op.get("points") {
        if !node.kind.is_path() {
            return Err(format!(
                "points apply only to path layers, not {}.",
                node.kind.name()
            ));
        }
        node.points = parse_points(points_val)?;
        node.anchors.clear();
    }
    if let Some(closed) = opt_bool(op, "closed")? {
        if !node.kind.is_path() {
            return Err(format!(
                "closed applies only to path layers, not {}.",
                node.kind.name()
            ));
        }
        node.closed = closed;
    }
    match op.get("shader") {
        None => {}
        Some(Value::Null) => {
            if !node.kind.supports_shader() {
                return Err(format!(
                    "shader does not apply to {} layers.",
                    node.kind.name()
                ));
            }
            node.shader = None;
        }
        Some(value) => {
            if !node.kind.supports_shader() {
                return Err(format!(
                    "shader does not apply to {} layers.",
                    node.kind.name()
                ));
            }
            node.shader = Some(parse_shader(value, node.shader.as_ref())?);
        }
    }
    Ok(())
}

fn parse_points(value: &Value) -> Result<Vec<[f32; 2]>, String> {
    let items = value
        .as_array()
        .ok_or("points must be an array of [x, y] pairs.")?;
    if items.len() < 2 {
        return Err("points must contain at least 2 [x, y] pairs.".into());
    }
    let mut points = Vec::with_capacity(items.len());
    for item in items {
        let pair = item
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or("Each point in points must be an [x, y] pair.")?;
        let x = pair[0]
            .as_f64()
            .map(|n| n as f32)
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            .ok_or("Point coordinates in points must be numbers in 0.0..=1.0.")?;
        let y = pair[1]
            .as_f64()
            .map(|n| n as f32)
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            .ok_or("Point coordinates in points must be numbers in 0.0..=1.0.")?;
        points.push([x, y]);
    }
    Ok(points)
}

fn parse_shader(value: &Value, existing: Option<&ShaderFill>) -> Result<ShaderFill, String> {
    let obj = value
        .as_object()
        .ok_or("shader must be an object or null.")?;
    allow(obj, &["kind", "secondary", "angle", "intensity"])?;
    let mut shader = existing.cloned().unwrap_or_default();
    if let Some(kind_name) = opt_str(obj, "kind")? {
        shader.kind = ShaderKind::from_name(kind_name).ok_or_else(|| {
            format!(
                "shader.kind must be linear_gradient, radial_gradient, mesh, grain or halftone, not {kind_name}."
            )
        })?;
    } else if existing.is_none() {
        return Err("shader.kind is required.".into());
    }
    if let Some(secondary) = obj.get("secondary") {
        shader.secondary = parse_color(secondary)?;
    }
    if let Some(angle) = opt_f32(obj, "angle")? {
        shader.angle = angle;
    }
    if let Some(intensity) = opt_f32(obj, "intensity")? {
        if !(0.0..=1.0).contains(&intensity) {
            return Err("shader.intensity must be between 0.0 and 1.0.".into());
        }
        shader.intensity = intensity;
    }
    Ok(shader)
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
            let id = resolve_token_id(doc, value, token_key)?;
            let value = doc.token(id).expect("resolved token exists").value;
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
pub(crate) fn allow(fields: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match fields.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(key) => Err(format!(
            "Unknown field {key}; expected one of: {}.",
            allowed.join(", ")
        )),
        None => Ok(()),
    }
}

pub(crate) fn req_str<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    opt_str(fields, key)?.ok_or_else(|| format!("{key} is required."))
}
pub(crate) fn opt_str<'a>(
    fields: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    fields
        .get(key)
        .map(|v| v.as_str().ok_or_else(|| format!("{key} must be a string.")))
        .transpose()
}
pub(crate) fn opt_f32(fields: &Map<String, Value>, key: &str) -> Result<Option<f32>, String> {
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
pub(crate) fn opt_bool(fields: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
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
document_save. Node x/y are absolute world coordinates even with a parent. Name a single created \
result with ref and use {ref: label} in matching ID fields later in the same batch. \
When creating designs, choose text_align explicitly for each create_text (left, center, right), \
and set width to the intended text box. After rendering, review text alignment, wrapping and clipping \
before saving. To change existing text alignment, use update_layer with id and text_align. \
align_layers moves layer boxes instead. Text alignment is visible when width exceeds the text width. \
Use document_render with frame_id or region to inspect distant content. HTML/SVG remain artboard-only \
and reject shaders/ancestor clipping. Headless in-place format upgrades require allow_upgrade:true; \
live upgrades require GUI confirmation or a new path. State results report revision, saved_revision and dirty.";

fn tools() -> Value {
    let id = json!({ "type": "string", "description": "Identifier returned by document_open or document_new." });
    let color = json!({ "description": "\"#rgb\", \"#rrggbb\", \"#rrggbbaa\" or [r, g, b, a]." });
    let local_ref = json!({"type":"object", "additionalProperties":false, "required":["ref"], "properties":{"ref":{"type":"string","minLength":1,"maxLength":64,"pattern":"^[A-Za-z0-9_-]+$"}}});
    let layer_ref = json!({"oneOf":[{"type":"integer","minimum":1},local_ref]});
    let token_ref = json!({ "oneOf":[{"type":"integer","minimum":1},{"type":"string"},{"type":"null"},local_ref], "description": "Token id, name, earlier typed batch reference, or null to detach." });
    let region = json!({"type":"object","additionalProperties":false,"required":["x","y","width","height"],"properties":{
        "x":{"type":"number","minimum":-100000,"maximum":100000}, "y":{"type":"number","minimum":-100000,"maximum":100000},
        "width":{"type":"number","minimum":1,"maximum":100000}, "height":{"type":"number","minimum":1,"maximum":100000}
    }});
    let mut op_properties = json!({
        "op": { "type": "string", "enum": [
            "create_frame", "create_rectangle", "create_ellipse", "create_triangle", "create_shape",
            "create_path", "create_text", "update_layer", "delete_layers", "move_layer",
            "select_layers", "align_layers", "distribute_layers", "duplicate_layers",
            "wrap_in_frame", "set_artboard", "create_token", "update_token", "delete_token",
            "create_directive", "resolve_directive", "delete_directive"
        ] },
        "ref": { "type":"string", "minLength":1, "maxLength":64, "pattern":"^[A-Za-z0-9_-]+$", "description":"Name a single created result for use later in this batch as {ref: label}; not persisted." },
        "id": { "oneOf":[{"type":"integer","minimum":1},{"type":"string"},local_ref], "description": "Layer/token/directive id or typed batch reference; token names only on token operations." },
        "ids": { "type": "array", "items": layer_ref, "description": "Layer ids or earlier layer references." },
        "name": { "type": "string" },
        "shape": { "type": "string", "enum": ["rectangle", "ellipse", "triangle"] },
        "x": { "type": "number", "minimum":-100000, "maximum":100000, "description":"Absolute world x, including children." },
        "y": { "type": "number", "minimum":-100000, "maximum":100000, "description":"Absolute world y, including children." },
        "width": { "type": "number", "minimum":1, "maximum":100000 },
        "height": { "type": "number", "minimum":1, "maximum":100000 },
        "fill": color.clone(),
        "fill_token": token_ref.clone(),
        "radius": { "type": "number", "minimum":0, "maximum":100000, "description": "Corner radius (rectangle and frame layers only)." },
        "text": { "type": "string" },
        "font_size": { "type": "number", "minimum":1, "maximum":512 },
    });
    op_properties
        .as_object_mut()
        .expect("object")
        .extend(
            json!({
                "line_height": { "type": ["number", "null"], "minimum":1, "maximum":4096 },
                "text_align": { "type": "string", "enum": ["left", "center", "right"], "description": "Choose explicitly on create_text; omitted values default to left. Aligns text horizontally inside the layer width. Change with update_layer {id, text_align}; preserves position and size. Applies only to text. Full justification and vertical alignment are unsupported." },
                "fit_text": { "type": "boolean", "description": "When true on a text layer, sets height to the measured text height." },
                "hidden": { "type": "boolean" },
                "locked": { "type": "boolean" },
                "index": { "type": "integer" },
                "background": color.clone(),
                "background_token": token_ref.clone(),
                "canvas": color.clone(),
                "canvas_token": token_ref.clone(),
                "value": color.clone(),
                "align": { "type": "string", "enum": ["left", "center", "center_x", "horizontal_center", "right", "top", "middle", "center_y", "vertical_center", "bottom"], "description": "For align_layers: moves layer boxes. To align text within a box, use update_layer with text_align." },
                "axis": { "type": "string", "enum": ["horizontal", "vertical"] },
                "offset_x": { "type": "number" },
                "offset_y": { "type": "number" }
            })
            .as_object()
            .expect("object")
            .clone(),
        );
    op_properties
        .as_object_mut()
        .expect("object")
        .extend(
            json!({
                "parent": { "oneOf":[layer_ref,{"type":"null"}], "description": "Earlier frame id or batch reference, or null. Child x/y stay absolute world coordinates." },
                "clip": { "type": "boolean", "description": "Clip child layers to frame bounds (frame layers only)." },
                "stroke": color,
                "stroke_token": token_ref,
                "stroke_width": { "type": "number", "minimum": 0, "maximum": 1000 },
                "anchors": {"type":"array","minItems":2,"maxItems":1024,"description":"v4 normalized cubic anchors; mutually exclusive with points/svg_path; handles may extend outside 0–1","items":{"type":"object","additionalProperties":false,"required":["position"],"properties":{"position":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"incoming":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"outgoing":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"mode":{"enum":["corner","smooth","symmetric"]}}}},
                "points": { "type": "array", "minItems":2, "maxItems":10000, "items": { "type": "array", "minItems":2, "maxItems":2, "items": { "type": "number", "minimum":0, "maximum":1 } }, "description": "Normalized straight path vertices; not curve control points (path layers only)." },
                "svg_path": { "type": "string", "description": "One absolute M/L contour with optional final Z, normalized into vertices. Relative/curve/multiple-subpath input rejects (path layers only)." },
                "closed": { "type": "boolean", "description": "Whether the vector path is closed (path layers only)." },
                "shader": { "type": ["object", "null"], "description": "Procedural shader fill {kind: linear_gradient|radial_gradient|mesh|grain|halftone, secondary?, angle?, intensity?} or null." },
                "layer_id": { "oneOf":[layer_ref,{"type":"null"}], "description": "Optional target layer id or earlier batch reference for a directive." },
                "message": { "type": "string", "description": "Agent directive note or prompt." },
                "resolved": { "type": "boolean", "description": "Whether the directive is resolved (default true)." }
            })
            .as_object()
            .expect("object")
            .clone(),
        );
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
            "description": "Inspect absolute world geometry, artboard/content/layer bounds, effective visibility/locking, capabilities/limitations, ordered layers (first is bottom), tokens, directives and revision/selection state. Colors are resolved #rrggbbaa values.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id } }
        },
        {
            "name": "document_apply",
            "description": "Apply operations as one all-or-nothing batch and one undo step. For each create_text, choose text_align explicitly (left, center, right) and set width to the intended text box. Example: {\"op\":\"create_text\",\"text\":\"Heading\",\"width\":400,\"text_align\":\"center\"}. Edit existing text with update_layer {id,text_align}; align_layers moves boxes. Render and review alignment, wrapping and clipping before saving. Operations (field \"op\"): create_frame, create_rectangle, create_ellipse, create_triangle, create_shape, create_path, create_text {name,shape,parent,clip,x,y,width,height,fill,fill_token,stroke,stroke_token,stroke_width,radius,points,anchors,svg_path,closed,shader|text,font_size,line_height,text_align,fit_text,hidden,locked,index}; update_layer {id,...same fields}; delete_layers {ids}; move_layer {id,index}; select_layers {ids}; align_layers {ids,align}; distribute_layers {ids,axis}; duplicate_layers {ids,offset_x,offset_y}; wrap_in_frame {ids,name?}; set_artboard {name,width,height,background,background_token,canvas,canvas_token}; create_token {name,value}; update_token {id,name,value}; delete_token {id|name}; create_directive {layer_id?,x?,y?,message}; resolve_directive {id,resolved?}; delete_directive {id}. Tokens may be referenced by id or name. A literal fill/stroke detaches a token; fill_token/stroke_token null detaches and keeps the color. Unknown fields are errors. Single-result creations/wrap may add ref; later matching IDs accept {ref:label}, typed and batch-local. Returns ordered created ids with final layer bounds, refs, content_bounds and state.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id", "operations"],
                "properties": {
                    "document_id": id,
                    "expected_revision": { "type": "integer", "minimum":0, "description": "Reject the batch unless the document is at this revision." },
                    "operations": { "type": "array", "minItems":1, "maxItems":10000, "items": { "type": "object", "additionalProperties":false, "required": ["op"],
                        "properties": op_properties } } } }
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
            "description": "Render PNG with the canvas painter. Default is the artboard; pass region or frame_id (not both) for a world crop of the entire visible composition. Outside the artboard is transparent. Returns image, world bounds, dimensions and state.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "not":{"required":["region","frame_id"]},
                "properties": { "document_id": id, "region":region, "frame_id":{"type":"integer","minimum":1}, "scale": { "type": "number", "minimum": 0.1, "maximum": 8, "default": 1 } } }
        },
        {
            "name": "document_export",
            "description": "Export atomically. HTML/SVG are artboard-only with text font/wrap limitations; shaders and ancestor clipping reject (use PNG). PNG accepts region or frame_id for a bounded world crop. Open source paths are protected.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id", "format", "path"],
                "not":{"required":["region","frame_id"]},
                "properties": { "document_id": id, "format": { "type": "string", "enum": ["html", "svg", "png"] },
                    "path": { "type": "string" }, "region":region, "frame_id":{"type":"integer","minimum":1}, "scale": { "type": "number", "minimum": 0.1, "maximum": 8 } } }
        },
        {
            "name": "document_save",
            "description": "Validate and save atomically. Headless in-place upgrades require allow_upgrade:true; live upgrades require GUI confirmation or a new path. Other open source files are protected.",
            "inputSchema": { "type": "object", "additionalProperties": false, "required": ["document_id"],
                "properties": { "document_id": id, "path": { "type": "string" }, "allow_upgrade":{"type":"boolean","default":false} } }
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
