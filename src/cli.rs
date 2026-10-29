//! Headless command line: inspection, export and the local MCP server.
//!
//! Commands write machine-readable output to stdout and messages to stderr. Exit
//! status: 0 success, 1 document or file error, 2 usage error.

use crate::document::{Document, Node, NodeKind, atomic_write};
use crate::render;
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const USAGE: &str = "\
Usage:
  papercraft [FILE]                     Open the editor, optionally with FILE
  papercraft inspect FILE               Print the document as JSON
  papercraft export FILE --format FORMAT --output PATH [--scale N]
                                        Export html, svg or png (scale 0.1–8, default 1)
  papercraft mcp                        Serve the local MCP protocol (live editor if open)
  papercraft mcp --live [SESSION_FILE]  Connect MCP to a running editor
  papercraft mcp --headless             Serve an isolated headless MCP session
  papercraft --help | --version
";

/// Exit status of a command.
pub enum Exit {
    Ok,
    Failure(String),
    Usage(String),
}

/// Returns `None` when the arguments open the editor; otherwise runs the command.
pub fn run(args: &[OsString]) -> Option<i32> {
    let command = args.first()?.to_str()?;
    if !matches!(
        command,
        "inspect" | "export" | "mcp" | "help" | "--help" | "-h" | "--version" | "-V"
    ) {
        return None;
    }
    attach_console();
    let exit = match command {
        "inspect" => inspect(&args[1..]),
        "export" => export(&args[1..]),
        "mcp" if args.len() == 1 => match crate::live::discover_optional() {
            Ok(Some(path)) => match crate::live::serve(&path) {
                Ok(()) => Exit::Ok,
                Err(error) => Exit::Failure(error),
            },
            Ok(None) => match crate::mcp::serve() {
                Ok(()) => Exit::Ok,
                Err(error) => Exit::Failure(error),
            },
            Err(error) => Exit::Failure(error),
        },
        "mcp" if args.get(1).is_some_and(|v| v == "--headless") && args.len() == 2 => {
            match crate::mcp::serve() {
                Ok(()) => Exit::Ok,
                Err(error) => Exit::Failure(error),
            }
        }
        "mcp" if args.get(1).is_some_and(|v| v == "--live") && args.len() <= 3 => {
            let path = args
                .get(2)
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(crate::live::discover);
            match path.and_then(|p| crate::live::serve(&p)) {
                Ok(()) => Exit::Ok,
                Err(error) => Exit::Failure(error),
            }
        }
        "mcp" => Exit::Usage("Use mcp, mcp --live [SESSION_FILE] or mcp --headless.".into()),
        "--version" | "-V" => {
            println!("papercraft {}", env!("CARGO_PKG_VERSION"));
            Exit::Ok
        }
        _ => {
            print!("{USAGE}");
            Exit::Ok
        }
    };
    Some(match exit {
        Exit::Ok => 0,
        Exit::Failure(message) => {
            eprintln!("papercraft: {message}");
            1
        }
        Exit::Usage(message) => {
            eprintln!("papercraft: {message}\n\n{USAGE}");
            2
        }
    })
}

fn inspect(args: &[OsString]) -> Exit {
    let [path] = args else {
        return Exit::Usage("inspect takes exactly one FILE.".into());
    };
    match Document::load(Path::new(path)) {
        Ok(document) => {
            let value = describe(&document);
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("JSON values serialize")
            );
            Exit::Ok
        }
        Err(error) => Exit::Failure(error),
    }
}

fn export(args: &[OsString]) -> Exit {
    let mut file = None;
    let (mut format, mut output, mut scale) = (None, None, None::<f32>);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(flag @ ("--format" | "--output" | "--scale")) => {
                let Some(value) = args.next() else {
                    return Exit::Usage(format!("{flag} needs a value."));
                };
                match flag {
                    "--format" => format = value.to_str().map(str::to_ascii_lowercase),
                    "--output" => output = Some(PathBuf::from(value)),
                    _ => match value.to_str().and_then(|v| v.parse().ok()) {
                        Some(value) => scale = Some(value),
                        None => return Exit::Usage("--scale needs a number.".into()),
                    },
                }
            }
            Some(flag) if flag.starts_with("--") => {
                return Exit::Usage(format!("Unknown option {flag}."));
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => return Exit::Usage("export takes exactly one FILE.".into()),
        }
    }
    let (Some(file), Some(format), Some(output)) = (file, format, output) else {
        return Exit::Usage("export needs FILE, --format and --output.".into());
    };
    if !matches!(format.as_str(), "html" | "svg" | "png") {
        return Exit::Usage(format!("Unknown format {format}; use html, svg or png."));
    }
    if format != "png" && scale.is_some() {
        return Exit::Usage("--scale applies only to png.".into());
    }
    match export_file(&file, &format, &output, scale.unwrap_or(1.0)) {
        Ok(()) => Exit::Ok,
        Err(error) => Exit::Failure(error),
    }
}

/// Exports `file` to `output`. The destination is written atomically and only after
/// the export succeeds; the source is never written.
pub fn export_file(file: &Path, format: &str, output: &Path, scale: f32) -> Result<(), String> {
    if same_file(file, output) {
        return Err("The output path is the source document.".into());
    }
    let document = Document::load(file)?;
    let data = export_bytes(&document, format, scale)?;
    atomic_write(output, &data)
}

pub fn export_bytes(document: &Document, format: &str, scale: f32) -> Result<Vec<u8>, String> {
    match format {
        "html" => document.export_html().map(String::into_bytes),
        "svg" => document.export_svg().map(String::into_bytes),
        "png" => render::export_png(document, scale),
        _ => Err(format!("Unknown format {format}; use html, svg or png.")),
    }
}

/// Whether `a` and `b` name the same existing file.
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `#rrggbbaa`.
pub fn hex(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}

/// Deterministic, machine-readable description of a document. Colors are resolved
/// `#rrggbbaa` values; bindings name token identifiers.
pub fn describe(document: &Document) -> Value {
    json!({
        "format": "papercraft",
        "version": document.required_version(),
        "name": document.name,
        "coordinate_space": "world",
        "artboard_bounds": document.artboard_bounds(),
        "content_bounds": document.content_bounds(),
        "capabilities": {
            "workspace": "off-artboard absolute world geometry",
            "selection": "clipped bounding boxes; not shape/stroke-aware",
            "png": {"regions":true,"frame_crop":true,"max_side":render::MAX_PNG_SIDE,"max_pixels":render::MAX_PNG_PIXELS},
            "html_svg": {"bounds":"artboard only","ancestor_clipping":false,"shaders":false,"text":"explicit SVG lines; fonts and wrapping differ"},
            "vectors": "partial normalized straight polylines; closed fills are convex-only, stroke parity incomplete; Path preset is not authored Pen, no curve or point editing",
            "hierarchy": "partial earlier-frame parent links with absolute child coordinates; no local transforms, group or reparent operations",
            "layout": false, "images":false, "components":false, "pages":false,
            "embedded_agent_execution": false
        },
        "artboard": {
            "width": document.width,
            "height": document.height,
            "background": hex(document.background_color()),
            "background_token": document.background_token,
            "background_token_name": document.background_token.and_then(|id| document.token(id)).map(|t| &t.name),
            "canvas": hex(document.canvas_color()),
            "canvas_token": document.canvas_token,
            "canvas_token_name": document.canvas_token.and_then(|id| document.token(id)).map(|t| &t.name),
        },
        "tokens": document.tokens.iter().map(|t| json!({
            "id": t.id,
            "name": t.name,
            "value": hex(t.value),
        })).collect::<Vec<_>>(),
        "layers": document.nodes.iter().enumerate()
            .map(|(index, node)| describe_node(document, index, node))
            .collect::<Vec<_>>(),
        "directives": document.directives.iter().map(|d| json!({
            "id": d.id,
            "layer_id": d.layer_id,
            "x": d.x,
            "y": d.y,
            "message": d.message,
            "resolved": d.resolved,
        })).collect::<Vec<_>>(),
    })
}

fn describe_node(document: &Document, index: usize, node: &Node) -> Value {
    let mut value = json!({
        "index": index,
        "id": node.id,
        "name": node.name,
        "kind": node.kind.name(),
        "x": node.x,
        "y": node.y,
        "width": node.width,
        "height": node.height,
        "bounds": node.bounds(),
        "visible_bounds": document.visible_bounds(node),
        "effective_hidden": document.is_effectively_hidden(node),
        "effective_locked": document.is_effectively_locked(node),
        "fill": hex(document.node_fill(node)),
        "fill_token": node.fill_token,
        "fill_token_name": node.fill_token.and_then(|id| document.token(id)).map(|t| &t.name),
        "hidden": node.hidden,
        "locked": node.locked,
    });
    let fields = value.as_object_mut().expect("object");
    if node.parent.is_some() {
        fields.insert("parent".into(), json!(node.parent));
    }
    if node.stroke_width > 0.0 || node.stroke.is_some() || node.stroke_token.is_some() {
        fields.insert("stroke".into(), json!(document.node_stroke(node).map(hex)));
        fields.insert("stroke_token".into(), json!(node.stroke_token));
        fields.insert(
            "stroke_token_name".into(),
            json!(
                node.stroke_token
                    .and_then(|id| document.token(id))
                    .map(|t| t.name.as_str())
            ),
        );
        fields.insert("stroke_width".into(), json!(node.stroke_width));
    }
    if let Some(shader) = &node.shader {
        fields.insert(
            "shader".into(),
            json!({
                "kind": shader.kind.name(),
                "secondary": hex(shader.secondary),
                "angle": shader.angle,
                "intensity": shader.intensity,
            }),
        );
    }
    match node.kind {
        NodeKind::Frame => {
            fields.insert("clip".into(), json!(node.clip));
            fields.insert("radius".into(), json!(node.radius));
        }
        NodeKind::Rectangle => {
            fields.insert("radius".into(), json!(node.radius));
        }
        NodeKind::Ellipse | NodeKind::Triangle => {}
        NodeKind::Path => {
            fields.insert("points".into(), json!(node.points));
            fields.insert("closed".into(), json!(node.closed));
        }
        NodeKind::Text => {
            let metrics = render::measure_text(node);
            fields.insert("text".into(), json!(node.text));
            fields.insert("font_size".into(), json!(node.font_size));
            fields.insert("line_height".into(), json!(node.line_height));
            fields.insert("text_align".into(), json!(node.text_align.name()));
            fields.insert("measured_width".into(), json!(metrics.width));
            fields.insert("measured_height".into(), json!(metrics.height));
            fields.insert("line_count".into(), json!(metrics.line_count));
            fields.insert("clipped".into(), json!(metrics.clipped));
        }
    }
    value
}

/// Release builds on Windows use the GUI subsystem, which starts without a console.
/// Attach to the parent console when standard output is not already redirected so
/// command output is visible in a terminal.
#[cfg(windows)]
fn attach_console() {
    unsafe extern "system" {
        fn GetStdHandle(id: u32) -> *mut core::ffi::c_void;
        fn AttachConsole(process: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: both functions take plain integers and have no preconditions.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() || handle as isize == -1 {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}
