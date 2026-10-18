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

/// Newest document version this build reads and writes. See `docs/document-format.md`.
pub const CURRENT_VERSION: u32 = 2;
const MAX_TOKENS: usize = 1_000;
const MAX_TOKEN_NAME: usize = 64;

fn default_canvas() -> [u8; 4] {
    DEFAULT_CANVAS
}
fn one() -> u64 {
    1
}
fn is_one(value: &u64) -> bool {
    *value == 1
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
    /// Color token bound to `fill` (version 2). `fill` keeps the synchronized value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_token: Option<u64>,
}

/// A named color shared by bound properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorToken {
    pub id: u64,
    pub name: String,
    pub value: [u8; 4],
}

/// A color property that can be bound to a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorTarget {
    /// Page color around the artboard.
    Canvas,
    /// Artboard background.
    Background,
    /// Rectangle fill or text color of a layer.
    Node(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    /// Horizontal centers.
    CenterX,
    Right,
    Top,
    /// Vertical centers.
    CenterY,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
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
    /// Color token bound to `background` (version 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_token: Option<u64>,
    /// Color token bound to `canvas` (version 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_token: Option<u64>,
    pub next_id: u64,
    /// Next color token identifier (version 2). Omitted while no tokens exist.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub next_token_id: u64,
    /// Color tokens (version 2), in Theme panel order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<ColorToken>,
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
            background_token: None,
            canvas_token: None,
            next_id: 1,
            next_token_id: 1,
            tokens: Vec::new(),
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
                fill_token: None,
            });
            doc.next_id += 1;
        }
        doc
    }

    /// Lowest format version that can represent this document.
    pub fn required_version(&self) -> u32 {
        if self.tokens.is_empty() { 1 } else { 2 }
    }

    fn has_bindings(&self) -> bool {
        self.background_token.is_some()
            || self.canvas_token.is_some()
            || self.nodes.iter().any(|n| n.fill_token.is_some())
    }

    /// Synchronizes bound literals with their tokens and sets the lowest representable
    /// version. Unbound literals are unchanged.
    pub fn normalize(&mut self) {
        let resolve = |tokens: &[ColorToken], literal: &mut [u8; 4], binding: Option<u64>| {
            if let Some(token) = binding.and_then(|id| tokens.iter().find(|t| t.id == id)) {
                *literal = token.value;
            }
        };
        resolve(&self.tokens, &mut self.background, self.background_token);
        resolve(&self.tokens, &mut self.canvas, self.canvas_token);
        for node in &mut self.nodes {
            resolve(&self.tokens, &mut node.fill, node.fill_token);
        }
        // Preserve unsupported versions so validation rejects them instead of downgrading.
        if (1..=CURRENT_VERSION).contains(&self.version) {
            self.version = self.required_version();
        }
        if self.tokens.is_empty() {
            self.next_token_id = 1;
        }
    }

    pub fn token(&self, id: u64) -> Option<&ColorToken> {
        self.tokens.iter().find(|t| t.id == id)
    }

    /// Effective color of a literal and its optional binding. A missing token falls back
    /// to the literal; validation prevents saving such a reference.
    fn resolve(&self, literal: [u8; 4], binding: Option<u64>) -> [u8; 4] {
        binding
            .and_then(|id| self.token(id))
            .map_or(literal, |t| t.value)
    }

    pub fn node_fill(&self, node: &Node) -> [u8; 4] {
        self.resolve(node.fill, node.fill_token)
    }
    pub fn background_color(&self) -> [u8; 4] {
        self.resolve(self.background, self.background_token)
    }
    pub fn canvas_color(&self) -> [u8; 4] {
        self.resolve(self.canvas, self.canvas_token)
    }

    pub fn binding(&self, target: ColorTarget) -> Option<u64> {
        match target {
            ColorTarget::Canvas => self.canvas_token,
            ColorTarget::Background => self.background_token,
            ColorTarget::Node(id) => self.nodes.iter().find(|n| n.id == id)?.fill_token,
        }
    }

    /// Effective color of `target`, or `None` for a missing layer.
    pub fn target_color(&self, target: ColorTarget) -> Option<[u8; 4]> {
        match target {
            ColorTarget::Canvas => Some(self.canvas_color()),
            ColorTarget::Background => Some(self.background_color()),
            ColorTarget::Node(id) => self
                .nodes
                .iter()
                .find(|n| n.id == id)
                .map(|n| self.node_fill(n)),
        }
    }

    fn target_mut(&mut self, target: ColorTarget) -> Option<(&mut [u8; 4], &mut Option<u64>)> {
        match target {
            ColorTarget::Canvas => Some((&mut self.canvas, &mut self.canvas_token)),
            ColorTarget::Background => Some((&mut self.background, &mut self.background_token)),
            ColorTarget::Node(id) => self
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .map(|n| (&mut n.fill, &mut n.fill_token)),
        }
    }

    /// Properties bound to the token `id`.
    pub fn token_usage(&self, id: u64) -> Vec<ColorTarget> {
        let mut usage = Vec::new();
        if self.canvas_token == Some(id) {
            usage.push(ColorTarget::Canvas);
        }
        if self.background_token == Some(id) {
            usage.push(ColorTarget::Background);
        }
        usage.extend(
            self.nodes
                .iter()
                .filter(|n| n.fill_token == Some(id))
                .map(|n| ColorTarget::Node(n.id)),
        );
        usage
    }

    /// Checks a token name: 1–64 bytes of Unicode letters, digits, `-` or `_`, unique
    /// among the other tokens without regard to case.
    pub fn check_token_name(&self, name: &str, except: Option<u64>) -> Result<(), String> {
        if name.is_empty() || name.len() > MAX_TOKEN_NAME {
            return Err("Token names must contain 1 to 64 bytes.".into());
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Token names may contain only letters, digits, - and _.".into());
        }
        let lower = name.to_lowercase();
        if self
            .tokens
            .iter()
            .any(|t| Some(t.id) != except && t.name.to_lowercase() == lower)
        {
            return Err(format!("A token named “{name}” already exists."));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version == 0 || self.version > CURRENT_VERSION {
            return Err(
                "This document was created by a newer PaperCraft or has an unsupported version."
                    .into(),
            );
        }
        if self.version == 1 && (!self.tokens.is_empty() || self.has_bindings()) {
            return Err("Version 1 documents cannot contain color tokens.".into());
        }
        if self.tokens.len() > MAX_TOKENS {
            return Err("Document exceeds the 1,000 token limit.".into());
        }
        let mut token_ids = HashSet::new();
        for token in &self.tokens {
            if token.id == 0 || token.id >= self.next_token_id || !token_ids.insert(token.id) {
                return Err(
                    "Token identifiers must be unique and below the next identifier.".into(),
                );
            }
            self.check_token_name(&token.name, Some(token.id))?;
        }
        let bound = [self.background_token, self.canvas_token]
            .into_iter()
            .chain(self.nodes.iter().map(|n| n.fill_token))
            .flatten();
        for id in bound {
            if !token_ids.contains(&id) {
                return Err("A color binding refers to a token that does not exist.".into());
            }
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
        // Check the version first so newer files get a clear message rather than an
        // unknown-field error.
        let fields: serde_json::Value = serde_json::from_slice(&data)
            .map_err(|e| format!("Invalid PaperCraft document: {e}"))?;
        let version = fields["version"].as_u64();
        if version.is_some_and(|version| version > u64::from(CURRENT_VERSION)) {
            return Err(
                "This document was created by a newer PaperCraft and cannot be opened.".into(),
            );
        }
        if version == Some(1)
            && ([
                "tokens",
                "next_token_id",
                "background_token",
                "canvas_token",
            ]
            .iter()
            .any(|key| fields.get(key).is_some())
                || fields["nodes"]
                    .as_array()
                    .is_some_and(|nodes| nodes.iter().any(|node| node.get("fill_token").is_some())))
        {
            return Err("Version 1 documents cannot contain color token fields.".into());
        }
        let mut doc: Self = serde_json::from_slice(&data)
            .map_err(|e| format!("Invalid PaperCraft document: {e}"))?;
        doc.validate()?;
        doc.normalize();
        Ok(doc)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut doc = self.clone();
        doc.normalize();
        doc.validate()?;
        let data =
            serde_json::to_vec_pretty(&doc).map_err(|e| format!("Serialization failed: {e}"))?;
        if data.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Document exceeds the 16 MiB size limit.".into());
        }
        atomic_write(path, &data)
    }

    /// CSS value for a color property: a custom property for bound tokens (defined in
    /// the exported `:root`), otherwise the literal.
    fn css_value(&self, literal: [u8; 4], binding: Option<u64>) -> String {
        match binding.and_then(|id| self.token(id)) {
            Some(token) => format!("var(--color-{})", token.name),
            None => css_color(literal),
        }
    }

    pub fn export_html(&self) -> Result<String, String> {
        self.validate()?;
        // Token names are restricted to CSS identifier characters by validation.
        let theme = if self.tokens.is_empty() {
            String::new()
        } else {
            let variables: String = self
                .tokens
                .iter()
                .map(|t| format!("--color-{}:{};", t.name, css_color(t.value)))
                .collect();
            format!("<style>:root{{{variables}}}</style>")
        };
        let mut html = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title>{theme}</head>\n<body style=\"margin:0;background:{}\"><main style=\"position:relative;overflow:hidden;width:{}px;height:{}px;background:{}\">\n",
            escape(&self.name),
            self.css_value(self.canvas, self.canvas_token),
            self.width,
            self.height,
            self.css_value(self.background, self.background_token)
        );
        for node in self.nodes.iter().filter(|n| !n.hidden) {
            let style = format!(
                "position:absolute;box-sizing:border-box;left:{}px;top:{}px;width:{}px;height:{}px;overflow:hidden;",
                node.x, node.y, node.width, node.height
            );
            let fill = self.css_value(node.fill, node.fill_token);
            match node.kind {
                NodeKind::Rectangle => html.push_str(&format!("<div aria-label=\"{}\" style=\"{style}background:{fill};border-radius:{}px\"></div>\n", escape(&node.name), node.radius)),
                NodeKind::Text => html.push_str(&format!("<div style=\"{style}white-space:pre-wrap;overflow-wrap:break-word;font:{}px sans-serif;line-height:1.2;color:{fill}\">{}</div>\n", node.font_size, escape(&node.text))),
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
            svg_fill(self.background_color())
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
                    svg_fill(self.node_fill(node))
                )),
                NodeKind::Text => {
                    svg.push_str(&format!(
                        "<defs><clipPath id=\"clip-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>\n<text {label} clip-path=\"url(#clip-{})\" font-family=\"sans-serif\" font-size=\"{}\" xml:space=\"preserve\" {}>",
                        node.id, node.x, node.y, node.width, node.height,
                        node.id, node.font_size, svg_fill(self.node_fill(node))
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
    /// Selected layer identifiers in selection order, without duplicates.
    selection: Vec<u64>,
    past: Vec<Document>,
    future: Vec<Document>,
}

fn clamp_coordinate(value: f32) -> f32 {
    value.clamp(-MAX_COORDINATE, MAX_COORDINATE)
}

fn node_rect(node: &Node) -> [f32; 4] {
    [node.x, node.y, node.x + node.width, node.y + node.height]
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            selection: Vec::new(),
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

    pub fn selection(&self) -> &[u64] {
        &self.selection
    }
    pub fn is_selected(&self, id: u64) -> bool {
        self.selection.contains(&id)
    }
    /// The selected layer when exactly one is selected.
    pub fn selected(&self) -> Option<u64> {
        match self.selection.as_slice() {
            [id] => Some(*id),
            _ => None,
        }
    }
    /// The selected node when exactly one is selected.
    pub fn selected_node(&self) -> Option<&Node> {
        let id = self.selected()?;
        self.document.nodes.iter().find(|n| n.id == id)
    }
    /// Replaces the selection with `id`, or clears it.
    pub fn select(&mut self, id: Option<u64>) {
        self.set_selection(id);
    }
    /// Replaces the selection; unknown and repeated identifiers are ignored.
    pub fn set_selection(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.selection.clear();
        for id in ids {
            if !self.selection.contains(&id) && self.document.nodes.iter().any(|n| n.id == id) {
                self.selection.push(id);
            }
        }
    }
    /// Adds `id` to the selection, or removes it when already selected.
    pub fn toggle_selection(&mut self, id: u64) {
        if let Some(index) = self.selection.iter().position(|s| *s == id) {
            self.selection.remove(index);
        } else if self.document.nodes.iter().any(|n| n.id == id) {
            self.selection.push(id);
        }
    }
    /// Selects every visible, unlocked layer — the layers a canvas click can select.
    pub fn select_all(&mut self) {
        let ids: Vec<_> = self
            .document
            .nodes
            .iter()
            .filter(|n| !n.hidden && !n.locked)
            .map(|n| n.id)
            .collect();
        self.set_selection(ids);
    }

    /// Selected layers that move, align and distribute: visible and unlocked, in
    /// document (paint) order.
    pub fn arrangeable(&self) -> Vec<u64> {
        self.document
            .nodes
            .iter()
            .filter(|n| self.is_selected(n.id) && !n.hidden && !n.locked)
            .map(|n| n.id)
            .collect()
    }

    /// Union bounds `[left, top, right, bottom]` of the arrangeable selection.
    pub fn selection_bounds(&self) -> Option<[f32; 4]> {
        self.document
            .nodes
            .iter()
            .filter(|n| self.is_selected(n.id) && !n.hidden && !n.locked)
            .map(node_rect)
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    }

    pub fn commit(&mut self, before: Document) -> Result<(), String> {
        self.document.normalize();
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

    /// Applies `edit` to the document as one history step.
    fn transaction<T>(&mut self, edit: impl FnOnce(&mut Document) -> T) -> Result<T, String> {
        let before = self.document.clone();
        let value = edit(&mut self.document);
        self.commit(before)?;
        Ok(value)
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
    /// Drops selected identifiers that no longer exist.
    fn check_selection(&mut self) {
        let nodes = &self.document.nodes;
        self.selection
            .retain(|id| nodes.iter().any(|n| n.id == *id));
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
            x: clamp_coordinate(x),
            y: clamp_coordinate(y),
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
            fill_token: None,
        });
        self.document.next_id = next;
        self.commit(before)?;
        self.select(Some(id));
        Ok(())
    }

    /// Deletes every selected unlocked layer (hidden ones included) as one step.
    pub fn delete(&mut self) -> Result<(), String> {
        let selection = self.selection.clone();
        self.transaction(|doc| doc.nodes.retain(|n| !selection.contains(&n.id) || n.locked))?;
        self.check_selection();
        Ok(())
    }

    /// Duplicates every selected unlocked layer above the current layers, in their
    /// existing order, offset by 20 units, and selects the copies. One history step.
    pub fn duplicate(&mut self) -> Result<(), String> {
        let originals: Vec<Node> = self
            .document
            .nodes
            .iter()
            .filter(|n| self.is_selected(n.id) && !n.locked)
            .cloned()
            .collect();
        if originals.is_empty() {
            return Ok(());
        }
        let next = self
            .document
            .next_id
            .checked_add(originals.len() as u64)
            .ok_or("Layer identifiers are exhausted.")?;
        let first = self.document.next_id;
        let copies = self.transaction(|doc| {
            let mut copies = Vec::new();
            for (offset, mut node) in originals.into_iter().enumerate() {
                node.id = first + offset as u64;
                node.x = (node.x + 20.0).min(MAX_COORDINATE);
                node.y = (node.y + 20.0).min(MAX_COORDINATE);
                let mut name_end = node.name.len().min(251);
                while !node.name.is_char_boundary(name_end) {
                    name_end -= 1;
                }
                node.name.truncate(name_end);
                node.name.push_str(" copy");
                copies.push(node.id);
                doc.nodes.push(node);
            }
            doc.next_id = next;
            copies
        })?;
        self.set_selection(copies);
        Ok(())
    }

    /// Moves the single selected unlocked layer one step in paint order.
    pub fn reorder(&mut self, forward: bool) -> Result<(), String> {
        let Some(index) = self
            .document
            .nodes
            .iter()
            .position(|n| Some(n.id) == self.selected() && !n.locked)
        else {
            return Ok(());
        };
        let other = if forward {
            (index + 1).min(self.document.nodes.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        self.transaction(|doc| doc.nodes.swap(index, other))
    }

    /// Moves the arrangeable selection by `dx`, `dy` as one step.
    pub fn nudge(&mut self, dx: f32, dy: f32) -> Result<(), String> {
        let ids = self.arrangeable();
        self.transaction(|doc| {
            for node in doc.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                node.x = clamp_coordinate(node.x + dx);
                node.y = clamp_coordinate(node.y + dy);
            }
        })
    }

    /// Shows all selected layers when any is hidden; otherwise hides them. One step.
    pub fn toggle_hidden(&mut self) -> Result<(), String> {
        let selection = self.selection.clone();
        self.transaction(|doc| {
            let selected = |n: &&mut Node| selection.contains(&n.id);
            let hide = doc.nodes.iter_mut().filter(selected).all(|n| !n.hidden);
            doc.nodes
                .iter_mut()
                .filter(selected)
                .for_each(|n| n.hidden = hide);
        })
    }

    /// Unlocks all selected layers when every one is locked; otherwise locks them all.
    /// One step.
    pub fn toggle_locked(&mut self) -> Result<(), String> {
        let selection = self.selection.clone();
        self.transaction(|doc| {
            let selected = |n: &&mut Node| selection.contains(&n.id);
            let lock = !doc.nodes.iter_mut().filter(selected).all(|n| n.locked);
            doc.nodes
                .iter_mut()
                .filter(selected)
                .for_each(|n| n.locked = lock);
        })
    }

    /// Aligns the arrangeable selection. Two or more layers align to their shared
    /// bounds; a single layer aligns to the artboard. One step.
    pub fn align(&mut self, align: Align) -> Result<(), String> {
        let ids = self.arrangeable();
        let reference = match ids.len() {
            0 => return Ok(()),
            1 => [0.0, 0.0, self.document.width, self.document.height],
            _ => self
                .selection_bounds()
                .expect("non-empty selection has bounds"),
        };
        self.transaction(|doc| {
            for node in doc.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                match align {
                    Align::Left => node.x = reference[0],
                    Align::CenterX => node.x = (reference[0] + reference[2] - node.width) / 2.0,
                    Align::Right => node.x = reference[2] - node.width,
                    Align::Top => node.y = reference[1],
                    Align::CenterY => node.y = (reference[1] + reference[3] - node.height) / 2.0,
                    Align::Bottom => node.y = reference[3] - node.height,
                }
                node.x = clamp_coordinate(node.x);
                node.y = clamp_coordinate(node.y);
            }
        })
    }

    /// Gives three or more arrangeable layers equal gaps along `axis`. Layers are ordered
    /// by their leading edge (paint order breaks ties); the first and last stay in place.
    /// Overlapping layers can produce negative gaps. One step.
    pub fn distribute(&mut self, axis: Axis) -> Result<(), String> {
        let ids = self.arrangeable();
        if ids.len() < 3 {
            return Ok(());
        }
        let horizontal = axis == Axis::Horizontal;
        let extent = |n: &Node| {
            if horizontal {
                (n.x, n.width)
            } else {
                (n.y, n.height)
            }
        };
        let mut items: Vec<(usize, f32, f32)> = self
            .document
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| ids.contains(&n.id))
            .map(|(index, n)| {
                let (start, size) = extent(n);
                (index, start, size)
            })
            .collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let first = items[0].1;
        let last = items.last().expect("three or more items");
        let span = last.1 + last.2 - first;
        let sizes: f32 = items.iter().map(|item| item.2).sum();
        let gap = (span - sizes) / (items.len() - 1) as f32;
        self.transaction(|doc| {
            let mut position = first;
            let count = items.len();
            for (order, (index, _, size)) in items.into_iter().enumerate() {
                let node = &mut doc.nodes[index];
                // The last layer keeps its position exactly.
                if order + 1 < count {
                    let value = clamp_coordinate(position);
                    if horizontal {
                        node.x = value;
                    } else {
                        node.y = value;
                    }
                }
                position += size + gap;
            }
        })
    }

    // Color tokens. Each operation is one history step.

    fn unique_token_name(&self) -> String {
        (1..)
            .map(|n| format!("color-{n}"))
            .find(|name| self.document.check_token_name(name, None).is_ok())
            .expect("an unused token name exists")
    }

    /// Creates a color token named `color-N` and returns its identifier.
    pub fn create_token(&mut self, value: [u8; 4]) -> Result<u64, String> {
        let id = self.document.next_token_id;
        let next = id
            .checked_add(1)
            .ok_or("Token identifiers are exhausted.")?;
        let name = self.unique_token_name();
        self.transaction(|doc| {
            doc.tokens.push(ColorToken { id, name, value });
            doc.next_token_id = next;
        })?;
        Ok(id)
    }

    /// Creates a token from the effective color of `target` and binds `target` to it.
    pub fn create_token_from(&mut self, target: ColorTarget) -> Result<Option<u64>, String> {
        if self.target_locked(target) {
            return Ok(None);
        }
        let Some(value) = self.document.target_color(target) else {
            return Ok(None);
        };
        let id = self.document.next_token_id;
        let next = id
            .checked_add(1)
            .ok_or("Token identifiers are exhausted.")?;
        let name = self.unique_token_name();
        self.transaction(|doc| {
            doc.tokens.push(ColorToken { id, name, value });
            doc.next_token_id = next;
            if let Some((_, binding)) = doc.target_mut(target) {
                *binding = Some(id);
            }
        })?;
        Ok(Some(id))
    }

    pub fn rename_token(&mut self, id: u64, name: &str) -> Result<(), String> {
        let name = name.trim();
        self.document.check_token_name(name, Some(id))?;
        self.transaction(|doc| {
            if let Some(token) = doc.tokens.iter_mut().find(|t| t.id == id) {
                token.name = name.to_owned();
            }
        })
    }

    /// Changes a token's value; every bound property follows.
    pub fn set_token_value(&mut self, id: u64, value: [u8; 4]) -> Result<(), String> {
        self.transaction(|doc| {
            if let Some(token) = doc.tokens.iter_mut().find(|t| t.id == id) {
                token.value = value;
            }
        })
    }

    fn target_locked(&self, target: ColorTarget) -> bool {
        matches!(target, ColorTarget::Node(id)
            if self.document.nodes.iter().any(|n| n.id == id && n.locked))
    }

    /// Binds `target` to `token`, or detaches it with `None`. Detaching keeps the
    /// current effective color as a literal. Locked layers and missing tokens are
    /// left unchanged.
    pub fn bind(&mut self, target: ColorTarget, token: Option<u64>) -> Result<(), String> {
        if self.target_locked(target) {
            return Ok(());
        }
        if let Some(id) = token
            && self.document.token(id).is_none()
        {
            return Err("That token no longer exists.".into());
        }
        let Some(current) = self.document.target_color(target) else {
            return Ok(());
        };
        self.transaction(|doc| {
            if let Some((literal, binding)) = doc.target_mut(target) {
                *literal = current;
                *binding = token;
            }
        })
    }

    /// Deletes a token. Every bound property, including those of locked layers, is
    /// detached and keeps the token's color as its literal value.
    pub fn delete_token(&mut self, id: u64) -> Result<(), String> {
        let Some(value) = self.document.token(id).map(|t| t.value) else {
            return Ok(());
        };
        self.transaction(|doc| {
            for target in doc.token_usage(id) {
                if let Some((literal, binding)) = doc.target_mut(target) {
                    *literal = value;
                    *binding = None;
                }
            }
            doc.tokens.retain(|t| t.id != id);
        })
    }
}
