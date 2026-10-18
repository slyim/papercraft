use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    path::Path,
};

pub const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_NODES: usize = 10_000;
const MAX_COORDINATE: f32 = 100_000.0;
/// Page color shown around the artboard when a document does not specify one.
pub const DEFAULT_CANVAS: [u8; 4] = [40, 40, 40, 255];

fn default_canvas() -> [u8; 4] {
    DEFAULT_CANVAS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Rectangle,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: u64,
    pub name: String,
    pub kind: NodeKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: [u8; 4],
    pub radius: f32,
    pub text: String,
    pub font_size: f32,
    pub hidden: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub background: [u8; 4],
    /// Page color around the artboard. Older version 1 files omit it.
    #[serde(default = "default_canvas")]
    pub canvas: [u8; 4],
    pub next_id: u64,
    pub nodes: Vec<Node>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            name: "Untitled".into(),
            width: 960.0,
            height: 640.0,
            background: [248, 247, 243, 255],
            canvas: DEFAULT_CANVAS,
            next_id: 1,
            nodes: Vec::new(),
        }
    }
}

impl Document {
    pub fn example() -> Self {
        let mut doc = Self {
            name: "A place to begin".into(),
            ..Self::default()
        };
        let items = [
            (
                NodeKind::Rectangle,
                "Surface",
                56.0,
                56.0,
                848.0,
                528.0,
                [226, 233, 220, 255],
                "",
                20.0,
                20.0,
            ),
            (
                NodeKind::Text,
                "Heading",
                96.0,
                100.0,
                720.0,
                150.0,
                [30, 52, 40, 255],
                "A canvas for\nwhat comes next.",
                58.0,
                0.0,
            ),
            (
                NodeKind::Text,
                "Description",
                100.0,
                286.0,
                640.0,
                76.0,
                [64, 81, 67, 255],
                "Shape an idea. Give it space.\nKeep the work on your own computer.",
                23.0,
                0.0,
            ),
            (
                NodeKind::Rectangle,
                "Button background",
                100.0,
                414.0,
                196.0,
                54.0,
                [39, 65, 48, 255],
                "",
                20.0,
                10.0,
            ),
            (
                NodeKind::Text,
                "Button label",
                124.0,
                430.0,
                160.0,
                30.0,
                [255, 255, 255, 255],
                "Make something",
                18.0,
                0.0,
            ),
            (
                NodeKind::Text,
                "Footer",
                100.0,
                520.0,
                600.0,
                26.0,
                [64, 81, 67, 255],
                "An original starter document · PaperCraft",
                14.0,
                0.0,
            ),
        ];
        for (kind, name, x, y, width, height, fill, text, font_size, radius) in items {
            doc.nodes.push(Node {
                id: doc.next_id,
                name: name.into(),
                kind,
                x,
                y,
                width,
                height,
                fill,
                radius,
                text: text.into(),
                font_size,
                hidden: false,
                locked: false,
            });
            doc.next_id += 1;
        }
        doc
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("This document version is not supported.".into());
        }
        if self.name.len() > 512 || self.nodes.len() > MAX_NODES {
            return Err("Document exceeds the name or layer limit.".into());
        }
        if !valid_size(self.width) || !valid_size(self.height) {
            return Err("Artboard dimensions must be between 1 and 100,000.".into());
        }
        let mut ids = HashSet::new();
        for node in &self.nodes {
            if node.id == 0 || node.id >= self.next_id || !ids.insert(node.id) {
                return Err(
                    "Layer identifiers must be unique and below the next identifier.".into(),
                );
            }
            if node.name.len() > 256 || node.text.len() > 100_000 {
                return Err("Layer name or text is too long.".into());
            }
            if ![node.x, node.y]
                .iter()
                .all(|n| n.is_finite() && n.abs() <= MAX_COORDINATE)
                || !valid_size(node.width)
                || !valid_size(node.height)
                || !node.radius.is_finite()
                || !(0.0..=MAX_COORDINATE).contains(&node.radius)
                || !node.font_size.is_finite()
                || !(1.0..=512.0).contains(&node.font_size)
            {
                return Err("Layer geometry contains an invalid or out-of-range number.".into());
            }
        }
        if self.next_id == 0 {
            return Err("The next layer identifier is invalid.".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let mut data = Vec::new();
        File::open(path)
            .map_err(|e| format!("Open failed: {e}"))?
            .take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("Read failed: {e}"))?;
        if data.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Document exceeds the 16 MiB size limit.".into());
        }
        let doc: Self = serde_json::from_slice(&data)
            .map_err(|e| format!("Invalid PaperCraft document: {e}"))?;
        doc.validate()?;
        Ok(doc)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let data =
            serde_json::to_vec_pretty(self).map_err(|e| format!("Serialization failed: {e}"))?;
        if data.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Document exceeds the 16 MiB size limit.".into());
        }
        atomic_write(path, &data)
    }

    pub fn export_html(&self) -> Result<String, String> {
        self.validate()?;
        let mut html = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title></head>\n<body style=\"margin:0;background:{}\"><main style=\"position:relative;overflow:hidden;width:{}px;height:{}px;background:{}\">\n",
            escape(&self.name),
            css_color(self.canvas),
            self.width,
            self.height,
            css_color(self.background)
        );
        for node in self.nodes.iter().filter(|n| !n.hidden) {
            let style = format!(
                "position:absolute;box-sizing:border-box;left:{}px;top:{}px;width:{}px;height:{}px;overflow:hidden;",
                node.x, node.y, node.width, node.height
            );
            match node.kind {
                NodeKind::Rectangle => html.push_str(&format!("<div aria-label=\"{}\" style=\"{style}background:{};border-radius:{}px\"></div>\n", escape(&node.name), css_color(node.fill), node.radius)),
                NodeKind::Text => html.push_str(&format!("<div style=\"{style}white-space:pre-wrap;overflow-wrap:break-word;font:{}px sans-serif;line-height:1.2;color:{}\">{}</div>\n", node.font_size, css_color(node.fill), escape(&node.text))),
            }
        }
        html.push_str("</main></body></html>\n");
        Ok(html)
    }

    /// Exports the artboard in document units. Text keeps explicit line breaks;
    /// fonts are resolved by the receiving application.
    pub fn export_svg(&self) -> Result<String, String> {
        self.validate()?;
        let visible = self.nodes.iter().filter(|n| !n.hidden);
        if std::iter::once(self.name.as_str())
            .chain(visible.clone().flat_map(|n| [n.name.as_str(), n.text.as_str()]))
            .any(|value| value.chars().any(|c| !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')))
        {
            return Err("SVG export failed: remove XML-incompatible control characters from names or text.".into());
        }
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" overflow=\"hidden\" role=\"img\" aria-labelledby=\"document-title\">\n<title id=\"document-title\">{}</title>\n<rect width=\"{}\" height=\"{}\" {}/>\n",
            self.width,
            self.height,
            self.width,
            self.height,
            escape(&self.name),
            self.width,
            self.height,
            svg_fill(self.background)
        );
        for node in visible {
            let label = format!(
                "id=\"layer-{}\" aria-label=\"{}\"",
                node.id,
                escape(&node.name)
            );
            match node.kind {
                NodeKind::Rectangle => svg.push_str(&format!(
                    "<rect {label} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" {}/>\n",
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    node.radius.min(node.width.min(node.height) / 2.0),
                    svg_fill(node.fill)
                )),
                NodeKind::Text => {
                    svg.push_str(&format!(
                        "<defs><clipPath id=\"clip-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>\n<text {label} clip-path=\"url(#clip-{})\" font-family=\"sans-serif\" font-size=\"{}\" xml:space=\"preserve\" {}>",
                        node.id, node.x, node.y, node.width, node.height,
                        node.id, node.font_size, svg_fill(node.fill)
                    ));
                    // shortcut: explicit lines only; share native text layout when wrapped SVG text is required.
                    let text = node.text.replace("\r\n", "\n").replace('\r', "\n");
                    for (line, text) in text.split('\n').enumerate() {
                        svg.push_str(&format!(
                            "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                            node.x,
                            node.y + node.font_size * (1.0 + line as f32 * 1.2),
                            escape(text)
                        ));
                    }
                    svg.push_str("</text>\n");
                }
            }
        }
        svg.push_str("</svg>\n");
        Ok(svg)
    }
}

fn valid_size(n: f32) -> bool {
    n.is_finite() && (1.0..=MAX_COORDINATE).contains(&n)
}
fn css_color(c: [u8; 4]) -> String {
    format!(
        "rgba({},{},{},{:.4})",
        c[0],
        c[1],
        c[2],
        c[3] as f32 / 255.0
    )
}
fn svg_fill(c: [u8; 4]) -> String {
    format!(
        "fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.4}\"",
        c[0],
        c[1],
        c[2],
        c[3] as f32 / 255.0
    )
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("Cannot prepare save: {e}"))?;
    temporary
        .write_all(data)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| format!("Cannot write save: {e}"))?;
    temporary
        .persist(path)
        .map_err(|e| format!("Cannot replace destination: {}", e.error))?;
    Ok(())
}

pub struct Editor {
    pub document: Document,
    pub selected: Option<u64>,
    past: Vec<Document>,
    future: Vec<Document>,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            selected: None,
            past: Vec::new(),
            future: Vec::new(),
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
    pub fn selected_node(&self) -> Option<&Node> {
        self.document
            .nodes
            .iter()
            .find(|n| Some(n.id) == self.selected)
    }

    pub fn commit(&mut self, before: Document) -> Result<(), String> {
        if let Err(error) = self.document.validate() {
            self.document = before;
            return Err(error);
        }
        if before != self.document {
            // shortcut: up to 100 full snapshots; use command history when large documents need it.
            if self.past.len() == 100 {
                self.past.remove(0);
            }
            self.past.push(before);
            self.future.clear();
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(doc) = self.past.pop() {
            self.future.push(std::mem::replace(&mut self.document, doc));
            self.check_selection();
        }
    }
    pub fn redo(&mut self) {
        if let Some(doc) = self.future.pop() {
            self.past.push(std::mem::replace(&mut self.document, doc));
            self.check_selection();
        }
    }
    fn check_selection(&mut self) {
        if self.selected_node().is_none() {
            self.selected = None;
        }
    }

    pub fn add(&mut self, kind: NodeKind) -> Result<(), String> {
        self.add_at(kind, 80.0, 80.0, 220.0, 80.0)
    }

    /// Adds a layer with the given artboard geometry as one history step and selects it.
    /// Values are rounded and clamped to the document limits.
    pub fn add_at(
        &mut self,
        kind: NodeKind,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let [x, y, width, height] = [x, y, width, height].map(|v| v.round());
        if ![x, y, width, height].iter().all(|v| v.is_finite()) {
            return Err("Layer geometry contains an invalid number.".into());
        }
        let next = self
            .document
            .next_id
            .checked_add(1)
            .ok_or("Layer identifiers are exhausted.")?;
        let before = self.document.clone();
        let id = self.document.next_id;
        self.document.nodes.push(Node {
            id,
            name: match kind {
                NodeKind::Rectangle => "Rectangle",
                NodeKind::Text => "Text",
            }
            .into(),
            kind,
            x: x.clamp(-MAX_COORDINATE, MAX_COORDINATE),
            y: y.clamp(-MAX_COORDINATE, MAX_COORDINATE),
            width: width.clamp(1.0, MAX_COORDINATE),
            height: height.clamp(1.0, MAX_COORDINATE),
            fill: if kind == NodeKind::Text {
                [35, 40, 48, 255]
            } else {
                [124, 155, 214, 255]
            },
            radius: 0.0,
            text: if kind == NodeKind::Text {
                "New text".into()
            } else {
                String::new()
            },
            font_size: 28.0,
            hidden: false,
            locked: false,
        });
        self.document.next_id = next;
        self.commit(before)?;
        self.selected = Some(id);
        Ok(())
    }

    pub fn delete(&mut self) -> Result<(), String> {
        let before = self.document.clone();
        self.document
            .nodes
            .retain(|n| Some(n.id) != self.selected || n.locked);
        self.commit(before)?;
        self.check_selection();
        Ok(())
    }

    pub fn duplicate(&mut self) -> Result<(), String> {
        let Some(mut node) = self.selected_node().filter(|n| !n.locked).cloned() else {
            return Ok(());
        };
        let next = self
            .document
            .next_id
            .checked_add(1)
            .ok_or("Layer identifiers are exhausted.")?;
        let before = self.document.clone();
        node.id = self.document.next_id;
        node.x = (node.x + 20.0).min(MAX_COORDINATE);
        node.y = (node.y + 20.0).min(MAX_COORDINATE);
        let mut name_end = node.name.len().min(251);
        while !node.name.is_char_boundary(name_end) {
            name_end -= 1;
        }
        node.name.truncate(name_end);
        node.name.push_str(" copy");
        let id = node.id;
        self.document.nodes.push(node);
        self.document.next_id = next;
        self.commit(before)?;
        self.selected = Some(id);
        Ok(())
    }

    pub fn reorder(&mut self, forward: bool) -> Result<(), String> {
        let Some(index) = self
            .document
            .nodes
            .iter()
            .position(|n| Some(n.id) == self.selected && !n.locked)
        else {
            return Ok(());
        };
        let other = if forward {
            (index + 1).min(self.document.nodes.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        let before = self.document.clone();
        self.document.nodes.swap(index, other);
        self.commit(before)
    }
}
