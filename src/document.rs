use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet as StdHashSet,
    fs::File,
    hash::{BuildHasherDefault, Hasher},
    io::{Read, Write},
    path::Path,
};

/// Hasher for layer and token identifiers. Identifiers are trusted integers, so a
/// multiplicative hash is enough and much cheaper than the default SipHash on the
/// per-layer selection checks made every frame.
#[derive(Default)]
pub struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(*byte)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0 ^ value).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// Set of layer or token identifiers.
pub type IdSet = StdHashSet<u64, BuildHasherDefault<IdHasher>>;

pub const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_NODES: usize = 10_000;
/// Largest coordinate and layer size a document accepts.
pub const MAX_COORDINATE: f32 = 100_000.0;
/// Largest text layer content, in UTF-8 bytes.
pub const MAX_TEXT_BYTES: usize = 100_000;
/// Page color shown around the artboard when a document does not specify one.
pub const DEFAULT_CANVAS: [u8; 4] = [40, 40, 40, 255];

/// Newest document version this build reads and writes. See `docs/document-format.md`.
pub const CURRENT_VERSION: u32 = 3;
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
fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}
fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Frame,
    #[default]
    Rectangle,
    Ellipse,
    Triangle,
    Path,
    Text,
}

impl NodeKind {
    pub fn is_shape(self) -> bool {
        matches!(self, Self::Rectangle | Self::Ellipse | Self::Triangle)
    }
    pub fn is_frame(self) -> bool {
        self == Self::Frame
    }
    pub fn is_path(self) -> bool {
        self == Self::Path
    }
    pub fn supports_radius(self) -> bool {
        matches!(self, Self::Rectangle | Self::Frame)
    }
    pub fn supports_shader(self) -> bool {
        matches!(
            self,
            Self::Frame | Self::Rectangle | Self::Ellipse | Self::Triangle
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Triangle => "triangle",
            Self::Path => "path",
            Self::Text => "text",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Frame => "Frame",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Path => "Path",
            Self::Text => "Text",
        }
    }
    pub fn from_shape_name(s: &str) -> Option<Self> {
        match s {
            "rectangle" => Some(Self::Rectangle),
            "ellipse" => Some(Self::Ellipse),
            "triangle" => Some(Self::Triangle),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderKind {
    #[default]
    LinearGradient,
    RadialGradient,
    Mesh,
    Grain,
    Halftone,
}

impl ShaderKind {
    pub const ALL: [Self; 5] = [
        Self::LinearGradient,
        Self::RadialGradient,
        Self::Mesh,
        Self::Grain,
        Self::Halftone,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::LinearGradient => "linear_gradient",
            Self::RadialGradient => "radial_gradient",
            Self::Mesh => "mesh",
            Self::Grain => "grain",
            Self::Halftone => "halftone",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::LinearGradient => "Linear",
            Self::RadialGradient => "Radial",
            Self::Mesh => "Mesh",
            Self::Grain => "Grain",
            Self::Halftone => "Halftone",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "linear_gradient" | "linear" => Some(Self::LinearGradient),
            "radial_gradient" | "radial" => Some(Self::RadialGradient),
            "mesh" => Some(Self::Mesh),
            "grain" => Some(Self::Grain),
            "halftone" => Some(Self::Halftone),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderFill {
    pub kind: ShaderKind,
    pub secondary: [u8; 4],
    pub angle: f32,
    pub intensity: f32,
}

impl Default for ShaderFill {
    fn default() -> Self {
        Self {
            kind: ShaderKind::LinearGradient,
            secondary: [255, 107, 43, 255],
            angle: 45.0,
            intensity: 0.65,
        }
    }
}

/// An agent/human spatial directive or annotation pin anchored to a layer or canvas point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directive {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<u64>,
    pub x: f32,
    pub y: f32,
    pub message: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resolved: bool,
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
    /// Baseline-to-baseline distance of text, in document units (version 3). `None`
    /// keeps the font's natural line spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    /// Horizontal alignment of text rows within the layer (version 3).
    #[serde(default, skip_serializing_if = "TextAlign::is_left")]
    pub text_align: TextAlign,
    /// Optional parent Frame identifier (version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    /// Whether a Frame clips its children to its bounds (version 3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub clip: bool,
    /// Optional stroke RGBA color (version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<[u8; 4]>,
    /// Optional color token bound to `stroke` (version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_token: Option<u64>,
    /// Stroke width in document units (version 3). `0.0` means no stroke.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub stroke_width: f32,
    /// Normalized `[0.0..=1.0, 0.0..=1.0]` points within the layer bounds for `NodeKind::Path` (version 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f32; 2]>,
    /// Whether a `NodeKind::Path` contour is closed (version 3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    /// Optional procedural shader fill (version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<ShaderFill>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Rectangle".into(),
            kind: NodeKind::Rectangle,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            fill: [210, 214, 220, 255],
            radius: 0.0,
            text: String::new(),
            font_size: 28.0,
            hidden: false,
            locked: false,
            fill_token: None,
            line_height: None,
            text_align: TextAlign::Left,
            parent: None,
            clip: false,
            stroke: None,
            stroke_token: None,
            stroke_width: 0.0,
            points: Vec::new(),
            closed: false,
            shader: None,
        }
    }
}

/// Default normalized control points for a newly placed `NodeKind::Path` layer.
pub fn default_path_points() -> Vec<[f32; 2]> {
    vec![[0.08, 0.78], [0.36, 0.18], [0.66, 0.82], [0.92, 0.22]]
}

/// Parses a simple SVG `M x y L x y ... [Z]` path string into normalized `[0.0..=1.0]` points
/// and a `closed` flag.
pub fn parse_svg_path_points(d: &str) -> Result<(Vec<[f32; 2]>, bool), String> {
    if !d.trim_start().starts_with('M') || d.chars().filter(|c| *c == 'M').count() != 1 {
        return Err("SVG input supports one absolute M/L contour only. Multiple subpaths and relative commands are unsupported.".into());
    }
    let mut closed = false;
    let mut cleaned = String::with_capacity(d.len());
    let mut segments = Vec::new();
    for ch in d.chars() {
        if closed && !ch.is_whitespace() {
            return Err("SVG input supports one contour with an optional final Z only.".into());
        }
        match ch {
            'Z' | 'z' => {
                closed = true;
                cleaned.push(' ');
            }
            'M' | 'L' => {
                cleaned.push(' ');
                segments.push(cleaned.len());
            }
            ',' => cleaned.push(' '),
            'm' | 'l' | 'C' | 'c' | 'Q' | 'q' | 'H' | 'h' | 'V' | 'v' | 'S' | 's' | 'T' | 't'
            | 'A' | 'a' => {
                return Err(format!(
                    "SVG command {ch} is unsupported. Use an absolute M/L polyline; curve authoring is not implemented."
                ));
            }
            _ => cleaned.push(ch),
        }
    }
    segments.push(cleaned.len());
    for range in segments.windows(2) {
        let count = cleaned[range[0]..range[1]].split_whitespace().count();
        if count < 2 || !count.is_multiple_of(2) {
            return Err("Each SVG M/L command requires complete coordinate pairs.".into());
        }
    }
    let mut coords = Vec::new();
    for token in cleaned.split_whitespace() {
        let value: f32 = token
            .parse()
            .map_err(|_| format!("Invalid SVG path coordinate “{token}”."))?;
        if !value.is_finite() {
            return Err("SVG path coordinates must be finite.".into());
        }
        coords.push(value);
    }
    if coords.len() < 4 || coords.len() % 2 != 0 {
        return Err("SVG path must contain at least 2 coordinate pairs (M x y L x y).".into());
    }
    if coords.len() > 20_000 {
        return Err("SVG path exceeds the 10,000 point limit.".into());
    }
    let mut raw: Vec<[f32; 2]> = coords.as_chunks::<2>().0.to_vec();
    let already_normalized = raw
        .iter()
        .all(|[x, y]| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y));
    if !already_normalized {
        let in_percent = raw
            .iter()
            .all(|[x, y]| (0.0..=100.0).contains(x) && (0.0..=100.0).contains(y));
        if in_percent {
            for [x, y] in &mut raw {
                *x = (*x / 100.0).clamp(0.0, 1.0);
                *y = (*y / 100.0).clamp(0.0, 1.0);
            }
        } else {
            let min_x = raw.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let max_x = raw.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
            let min_y = raw.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            let max_y = raw.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
            let span_x = (max_x - min_x).max(1.0);
            let span_y = (max_y - min_y).max(1.0);
            for [x, y] in &mut raw {
                *x = ((*x - min_x) / span_x).clamp(0.0, 1.0);
                *y = ((*y - min_y) / span_y).clamp(0.0, 1.0);
            }
        }
    }
    Ok((raw, closed))
}

/// Horizontal alignment of text rows within a text layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub fn is_left(&self) -> bool {
        *self == Self::Left
    }
    /// Fraction of the layer width at which rows are anchored.
    pub fn anchor(self) -> f32 {
        match self {
            Self::Left => 0.0,
            Self::Center => 0.5,
            Self::Right => 1.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Accepted explicit line heights, in document units.
pub const LINE_HEIGHT: std::ops::RangeInclusive<f32> = 1.0..=4096.0;

impl Node {
    /// Authored world bounds `[left, top, right, bottom]`. Parent links do not
    /// change the absolute-coordinate contract of released v3 files.
    pub fn bounds(&self) -> [f32; 4] {
        [self.x, self.y, self.x + self.width, self.y + self.height]
    }

    /// Whether the layer uses a version 3 field or node kind.
    pub fn uses_v3(&self) -> bool {
        self.line_height.is_some()
            || !self.text_align.is_left()
            || matches!(
                self.kind,
                NodeKind::Frame | NodeKind::Ellipse | NodeKind::Triangle | NodeKind::Path
            )
            || self.parent.is_some()
            || self.clip
            || self.stroke.is_some()
            || self.stroke_token.is_some()
            || self.stroke_width > 0.0
            || !self.points.is_empty()
            || self.closed
            || self.shader.is_some()
    }

    /// Changes the geometry of a shape layer. Only rectangles keep a corner radius.
    pub fn set_shape(&mut self, kind: NodeKind) {
        debug_assert!(self.kind.is_shape() && kind.is_shape());
        self.kind = kind;
        if kind != NodeKind::Rectangle {
            self.radius = 0.0;
        }
    }
}

/// Content of a newly placed text layer.
pub const NEW_TEXT: &str = "New text";

/// Text properties a new text layer inherits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub font_size: f32,
    pub fill: [u8; 4],
    pub fill_token: Option<u64>,
    pub line_height: Option<f32>,
    pub text_align: TextAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 28.0,
            fill: [35, 40, 48, 255],
            fill_token: None,
            line_height: None,
            text_align: TextAlign::Left,
        }
    }
}

impl TextStyle {
    /// The style of a text layer; `None` for other kinds.
    pub fn of(node: &Node) -> Option<Self> {
        (node.kind == NodeKind::Text).then_some(Self {
            font_size: node.font_size,
            fill: node.fill,
            fill_token: node.fill_token,
            line_height: node.line_height,
            text_align: node.text_align,
        })
    }
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
    /// Stroke color of a layer.
    Stroke(u64),
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
    /// Next directive identifier (version 3). Omitted while no directives exist.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub next_directive_id: u64,
    /// Spatial directives and agent/human annotations (version 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
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
            next_directive_id: 1,
            directives: Vec::new(),
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
                line_height: None,
                text_align: TextAlign::Left,
                ..Node::default()
            });
            doc.next_id += 1;
        }
        doc
    }

    /// Lowest format version that can represent this document.
    pub fn required_version(&self) -> u32 {
        if self.nodes.iter().any(Node::uses_v3)
            || !self.directives.is_empty()
            || self.next_directive_id > 1
        {
            3
        } else if self.tokens.is_empty() {
            1
        } else {
            2
        }
    }

    fn has_bindings(&self) -> bool {
        self.background_token.is_some()
            || self.canvas_token.is_some()
            || self
                .nodes
                .iter()
                .any(|n| n.fill_token.is_some() || n.stroke_token.is_some())
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
            if let Some(token) = node
                .stroke_token
                .and_then(|id| self.tokens.iter().find(|t| t.id == id))
            {
                node.stroke = Some(token.value);
            }
        }
        if self.tokens.is_empty() {
            self.next_token_id = 1;
        }
        if self.directives.is_empty() {
            self.next_directive_id = 1;
        }
        // Preserve unsupported versions so validation rejects them instead of downgrading.
        if (1..=CURRENT_VERSION).contains(&self.version) {
            self.version = self.required_version();
        }
    }

    pub fn token(&self, id: u64) -> Option<&ColorToken> {
        self.tokens.iter().find(|t| t.id == id)
    }

    /// Token whose name matches `name` case-insensitively, ignoring surrounding spaces.
    pub fn token_by_name(&self, name: &str) -> Option<&ColorToken> {
        let name = name.trim();
        self.tokens
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Aligns the visible, unlocked layers among `ids`. Two or more align to their shared
    /// bounds; a single layer aligns to the artboard.
    pub fn align_nodes(&mut self, ids: &[u64], align: Align) -> Result<(), String> {
        for &id in ids {
            if !self.nodes.iter().any(|n| n.id == id) {
                return Err(format!("No layer {id}."));
            }
        }
        let target_ids: IdSet = ids.iter().copied().collect();
        let arrangeable: IdSet = self
            .nodes
            .iter()
            .filter(|n| target_ids.contains(&n.id) && !n.hidden && !n.locked)
            .map(|n| n.id)
            .collect();
        if arrangeable.is_empty() {
            return Err("No unlocked visible layers to align.".into());
        }
        let reference = if arrangeable.len() == 1 {
            [0.0, 0.0, self.width, self.height]
        } else {
            self.nodes
                .iter()
                .filter(|n| arrangeable.contains(&n.id))
                .map(node_rect)
                .reduce(|a, b| {
                    [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[2].max(b[2]),
                        a[3].max(b[3]),
                    ]
                })
                .expect("two or more arrangeable layers")
        };
        for node in self
            .nodes
            .iter_mut()
            .filter(|n| arrangeable.contains(&n.id))
        {
            match align {
                Align::Left => node.x = reference[0],
                Align::CenterX => node.x = (reference[0] + reference[2] - node.width) / 2.0,
                Align::Right => node.x = reference[2] - node.width,
                Align::Top => node.y = reference[1],
                Align::CenterY => node.y = (reference[1] + reference[3] - node.height) / 2.0,
                Align::Bottom => node.y = reference[3] - node.height,
            }
        }
        Ok(())
    }

    /// Gives three or more visible, unlocked layers among `ids` equal gaps along `axis`.
    /// The first and last layer along the axis stay in place.
    pub fn distribute_nodes(&mut self, ids: &[u64], axis: Axis) -> Result<(), String> {
        for &id in ids {
            if !self.nodes.iter().any(|n| n.id == id) {
                return Err(format!("No layer {id}."));
            }
        }
        let target_ids: IdSet = ids.iter().copied().collect();
        let arrangeable: IdSet = self
            .nodes
            .iter()
            .filter(|n| target_ids.contains(&n.id) && !n.hidden && !n.locked)
            .map(|n| n.id)
            .collect();
        if arrangeable.len() < 3 {
            return Err("Distribute requires at least 3 unlocked visible layers.".into());
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
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| arrangeable.contains(&n.id))
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
        let mut position = first;
        let count = items.len();
        for (order, (index, _, size)) in items.into_iter().enumerate() {
            let node = &mut self.nodes[index];
            // The last layer keeps its position exactly.
            if order + 1 < count {
                if horizontal {
                    node.x = position;
                } else {
                    node.y = position;
                }
            }
            position += size + gap;
        }
        Ok(())
    }

    /// Copies the layers among `ids` above the existing layers, in paint order, offset by
    /// `dx`, `dy`, and returns the new identifiers.
    pub fn duplicate_nodes(&mut self, ids: &[u64], dx: f32, dy: f32) -> Result<Vec<u64>, String> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err("Offset must be finite.".into());
        }
        for &id in ids {
            if !self.nodes.iter().any(|n| n.id == id) {
                return Err(format!("No layer {id}."));
            }
        }
        let target_ids: IdSet = ids.iter().copied().collect();
        let originals: Vec<Node> = self
            .nodes
            .iter()
            .filter(|n| target_ids.contains(&n.id))
            .cloned()
            .collect();
        if originals.is_empty() {
            return Err("ids must not be empty.".into());
        }
        let next = self
            .next_id
            .checked_add(originals.len() as u64)
            .ok_or("Layer identifiers are exhausted.")?;
        let first = self.next_id;
        let id_map: Vec<(u64, u64)> = originals
            .iter()
            .enumerate()
            .map(|(offset, node)| (node.id, first + offset as u64))
            .collect();
        let mut copies = Vec::with_capacity(originals.len());
        for (offset, mut node) in originals.into_iter().enumerate() {
            node.id = first + offset as u64;
            if let Some(pid) = node.parent
                && let Some(&(_, mapped)) = id_map.iter().find(|&&(old, _)| old == pid)
            {
                node.parent = Some(mapped);
            }
            node.x = (node.x + dx).clamp(-MAX_COORDINATE, MAX_COORDINATE);
            node.y = (node.y + dy).clamp(-MAX_COORDINATE, MAX_COORDINATE);
            let mut name_end = node.name.len().min(251);
            while !node.name.is_char_boundary(name_end) {
                name_end -= 1;
            }
            node.name.truncate(name_end);
            node.name.push_str(" copy");
            copies.push(node.id);
            self.nodes.push(node);
        }
        self.next_id = next;
        Ok(copies)
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
    pub fn node_stroke(&self, node: &Node) -> Option<[u8; 4]> {
        if node.stroke_width <= 0.0 {
            return None;
        }
        if let Some(token) = node.stroke_token.and_then(|id| self.token(id)) {
            return Some(token.value);
        }
        node.stroke
    }
    pub fn background_color(&self) -> [u8; 4] {
        self.resolve(self.background, self.background_token)
    }
    pub fn canvas_color(&self) -> [u8; 4] {
        self.resolve(self.canvas, self.canvas_token)
    }

    /// Intersects `[left, top, right, bottom]` for all ancestor Frames of `node` where `clip` is `true`.
    pub fn ancestors_clip_rect(&self, node: &Node) -> Option<[f32; 4]> {
        let mut clip: Option<[f32; 4]> = None;
        let mut current = node.parent;
        let mut hops = 0;
        while let Some(pid) = current {
            if hops > self.nodes.len() {
                break;
            }
            hops += 1;
            let Some(parent) = self.nodes.iter().find(|n| n.id == pid) else {
                break;
            };
            if parent.kind == NodeKind::Frame && parent.clip {
                let rect = node_rect(parent);
                clip = Some(match clip {
                    None => rect,
                    Some(c) => [
                        c[0].max(rect[0]),
                        c[1].max(rect[1]),
                        c[2].min(rect[2]),
                        c[3].min(rect[3]),
                    ],
                });
            }
            current = parent.parent;
        }
        clip
    }

    /// Returns `true` if `node.hidden` or any ancestor Frame of `node` is hidden.
    pub fn is_effectively_hidden(&self, node: &Node) -> bool {
        if node.hidden {
            return true;
        }
        let mut current = node.parent;
        let mut hops = 0;
        while let Some(pid) = current {
            if hops > self.nodes.len() {
                break;
            }
            hops += 1;
            let Some(parent) = self.nodes.iter().find(|n| n.id == pid) else {
                break;
            };
            if parent.hidden {
                return true;
            }
            current = parent.parent;
        }
        false
    }

    /// Inherited locking for workspace selection. Mutation inheritance is handled
    /// separately by each model operation until the hierarchy milestone.
    pub fn is_effectively_locked(&self, node: &Node) -> bool {
        if node.locked {
            return true;
        }
        let mut current = node.parent;
        for _ in 0..=self.nodes.len() {
            let Some(layer) = current.and_then(|id| self.nodes.iter().find(|n| n.id == id)) else {
                break;
            };
            if layer.locked {
                return true;
            }
            current = layer.parent;
        }
        false
    }

    /// Legacy artboard: a background and default export region, not workspace bounds.
    pub fn artboard_bounds(&self) -> [f32; 4] {
        [0.0, 0.0, self.width, self.height]
    }

    /// Visible axis-aligned world bounds after rectangular ancestor clipping.
    pub fn visible_bounds(&self, node: &Node) -> Option<[f32; 4]> {
        if self.is_effectively_hidden(node) {
            return None;
        }
        let mut bounds = node.bounds();
        if let Some(clip) = self.ancestors_clip_rect(node) {
            bounds = [
                bounds[0].max(clip[0]),
                bounds[1].max(clip[1]),
                bounds[2].min(clip[2]),
                bounds[3].min(clip[3]),
            ];
        }
        (bounds[0] < bounds[2] && bounds[1] < bounds[3]).then_some(bounds)
    }

    pub fn content_bounds(&self) -> Option<[f32; 4]> {
        self.nodes
            .iter()
            .filter_map(|n| self.visible_bounds(n))
            .reduce(union_bounds)
    }

    /// Bounding-box selection in paint order. Shape/stroke-aware hits are a later
    /// slice; all canvas hit consumers share this explicit contract.
    pub fn hit_test(&self, point: [f32; 2]) -> Option<u64> {
        self.nodes
            .iter()
            .rev()
            .find(|node| {
                !self.is_effectively_locked(node)
                    && self.visible_bounds(node).is_some_and(|b| {
                        point[0] >= b[0] && point[0] <= b[2] && point[1] >= b[1] && point[1] <= b[3]
                    })
            })
            .map(|node| node.id)
    }

    /// Returns all node IDs whose `parent` chain includes `frame_id`.
    pub fn descendants_of(&self, frame_id: u64) -> IdSet {
        let mut descendants = IdSet::default();
        for node in &self.nodes {
            if descendants.contains(&node.id) {
                continue;
            }
            let mut current = node.parent;
            let mut hops = 0;
            while let Some(pid) = current {
                if pid == frame_id || descendants.contains(&pid) {
                    descendants.insert(node.id);
                    break;
                }
                if hops > self.nodes.len() {
                    break;
                }
                hops += 1;
                current = self
                    .nodes
                    .iter()
                    .find(|n| n.id == pid)
                    .and_then(|n| n.parent);
            }
        }
        descendants
    }

    /// Wraps the unlocked, visible layers among `ids` in a new `NodeKind::Frame` inserted at the
    /// earliest index among `ids`, and sets `node.parent = Some(frame_id)` on the wrapped layers.
    pub fn wrap_in_frame(&mut self, ids: &[u64], name: Option<&str>) -> Result<u64, String> {
        for &id in ids {
            if !self.nodes.iter().any(|n| n.id == id) {
                return Err(format!("No layer {id}."));
            }
        }
        let target_ids: IdSet = ids.iter().copied().collect();
        let arrangeable: IdSet = self
            .nodes
            .iter()
            .filter(|n| target_ids.contains(&n.id) && !n.hidden && !n.locked)
            .map(|n| n.id)
            .collect();
        if arrangeable.is_empty() {
            return Err("No unlocked visible layers to wrap in a frame.".into());
        }
        let bounds = self
            .nodes
            .iter()
            .filter(|n| arrangeable.contains(&n.id))
            .map(node_rect)
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
            .expect("one or more arrangeable layers");
        let left = (bounds[0] - 16.0).clamp(0.0, MAX_COORDINATE);
        let top = (bounds[1] - 16.0).clamp(0.0, MAX_COORDINATE);
        let right = (bounds[2] + 16.0).clamp(0.0, MAX_COORDINATE);
        let bottom = (bounds[3] + 16.0).clamp(0.0, MAX_COORDINATE);
        let width = (right - left).clamp(1.0, MAX_COORDINATE);
        let height = (bottom - top).clamp(1.0, MAX_COORDINATE);
        let frame_id = self.next_id;
        let next = frame_id
            .checked_add(1)
            .ok_or("Layer identifiers are exhausted.")?;
        let first_index = self
            .nodes
            .iter()
            .position(|n| arrangeable.contains(&n.id))
            .expect("one or more arrangeable layers");
        for node in &mut self.nodes {
            if arrangeable.contains(&node.id) {
                node.parent = Some(frame_id);
            }
        }
        self.nodes.insert(
            first_index,
            Node {
                id: frame_id,
                name: name
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Frame")
                    .into(),
                kind: NodeKind::Frame,
                x: left,
                y: top,
                width,
                height,
                fill: [255, 255, 255, 255],
                radius: 12.0,
                clip: true,
                ..Node::default()
            },
        );
        self.next_id = next;
        Ok(frame_id)
    }

    pub fn add_directive(
        &mut self,
        layer_id: Option<u64>,
        x: f32,
        y: f32,
        message: &str,
    ) -> Result<u64, String> {
        if self.directives.len() >= 1_000 {
            return Err("Document exceeds the 1,000 directive limit.".into());
        }
        if let Some(lid) = layer_id
            && !self.nodes.iter().any(|n| n.id == lid)
        {
            return Err(format!("No layer {lid}."));
        }
        if !x.is_finite() || !y.is_finite() {
            return Err("Directive coordinates must be finite.".into());
        }
        let message = message.trim();
        if message.is_empty() || message.len() > 4_000 {
            return Err("Directive message must contain 1 to 4,000 bytes.".into());
        }
        let id = self.next_directive_id;
        let next = id
            .checked_add(1)
            .ok_or("Directive identifiers are exhausted.")?;
        self.directives.push(Directive {
            id,
            layer_id,
            x: clamp_coordinate(x),
            y: clamp_coordinate(y),
            message: message.to_owned(),
            resolved: false,
        });
        self.next_directive_id = next;
        Ok(id)
    }

    pub fn resolve_directive(&mut self, id: u64, resolved: bool) -> Result<(), String> {
        let directive = self
            .directives
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| format!("No directive {id}."))?;
        directive.resolved = resolved;
        Ok(())
    }

    pub fn delete_directive(&mut self, id: u64) -> Result<(), String> {
        let before = self.directives.len();
        self.directives.retain(|d| d.id != id);
        if self.directives.len() == before {
            return Err(format!("No directive {id}."));
        }
        if self.directives.is_empty() {
            self.next_directive_id = 1;
        }
        Ok(())
    }

    /// Limits one shared translation so every layer position stays within range.
    pub(crate) fn clamp_translation(&self, ids: &IdSet, delta: [f32; 2]) -> [f32; 2] {
        self.nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .fold(delta, |[dx, dy], node| {
                [
                    dx.clamp(-MAX_COORDINATE - node.x, MAX_COORDINATE - node.x),
                    dy.clamp(-MAX_COORDINATE - node.y, MAX_COORDINATE - node.y),
                ]
            })
    }

    pub fn binding(&self, target: ColorTarget) -> Option<u64> {
        match target {
            ColorTarget::Canvas => self.canvas_token,
            ColorTarget::Background => self.background_token,
            ColorTarget::Node(id) => self.nodes.iter().find(|n| n.id == id)?.fill_token,
            ColorTarget::Stroke(id) => self.nodes.iter().find(|n| n.id == id)?.stroke_token,
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
            ColorTarget::Stroke(id) => {
                let node = self.nodes.iter().find(|n| n.id == id)?;
                Some(
                    node.stroke_token
                        .and_then(|tid| self.token(tid))
                        .map_or(node.stroke.unwrap_or([0, 0, 0, 255]), |t| t.value),
                )
            }
        }
    }

    pub(crate) fn target_mut(
        &mut self,
        target: ColorTarget,
    ) -> Option<(&mut [u8; 4], &mut Option<u64>)> {
        match target {
            ColorTarget::Canvas => Some((&mut self.canvas, &mut self.canvas_token)),
            ColorTarget::Background => Some((&mut self.background, &mut self.background_token)),
            ColorTarget::Node(id) => self
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .map(|n| (&mut n.fill, &mut n.fill_token)),
            ColorTarget::Stroke(id) => self
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .map(|n| (n.stroke.get_or_insert([0, 0, 0, 255]), &mut n.stroke_token)),
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
        for node in &self.nodes {
            if node.fill_token == Some(id) {
                usage.push(ColorTarget::Node(node.id));
            }
            if node.stroke_token == Some(id) {
                usage.push(ColorTarget::Stroke(node.id));
            }
        }
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
        if self.version < 3
            && (self.nodes.iter().any(Node::uses_v3)
                || !self.directives.is_empty()
                || self.next_directive_id > 1)
        {
            return Err(
                "Version 1 and 2 documents cannot contain line height, text alignment, ellipses or triangles."
                    .into(),
            );
        }
        if self.tokens.len() > MAX_TOKENS {
            return Err("Document exceeds the 1,000 token limit.".into());
        }
        let mut token_ids = StdHashSet::new();
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
            .chain(
                self.nodes
                    .iter()
                    .flat_map(|n| [n.fill_token, n.stroke_token]),
            )
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
        let mut ids = StdHashSet::new();
        let mut frame_ids = StdHashSet::new();
        for node in &self.nodes {
            if node.id == 0 || node.id >= self.next_id || !ids.insert(node.id) {
                return Err(
                    "Layer identifiers must be unique and below the next identifier.".into(),
                );
            }
            if let Some(pid) = node.parent
                && !frame_ids.contains(&pid)
            {
                return Err("A layer parent must be an earlier Frame layer.".into());
            }
            if node.name.len() > 256 || node.text.len() > MAX_TEXT_BYTES {
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
                || !node.stroke_width.is_finite()
                || !(0.0..=1000.0).contains(&node.stroke_width)
            {
                return Err("Layer geometry contains an invalid or out-of-range number.".into());
            }
            if node.kind.is_shape() {
                if !node.text.is_empty() {
                    return Err("Shape layers cannot contain text.".into());
                }
                if node.line_height.is_some() || !node.text_align.is_left() {
                    return Err("Only text layers have line height and text alignment.".into());
                }
                if node.kind != NodeKind::Rectangle && node.radius != 0.0 {
                    return Err("Only rectangles have a corner radius.".into());
                }
            }
            if node.kind == NodeKind::Path {
                if !(2..=10_000).contains(&node.points.len()) {
                    return Err("Path layers must contain between 2 and 10,000 points.".into());
                }
                if !node.points.iter().all(|[x, y]| {
                    x.is_finite()
                        && y.is_finite()
                        && (0.0..=1.0).contains(x)
                        && (0.0..=1.0).contains(y)
                }) {
                    return Err("Path points must be normalized coordinates in 0..=1.".into());
                }
            } else if !node.points.is_empty() || node.closed {
                return Err("Only path layers can have path points or closed contours.".into());
            }
            if let Some(shader) = &node.shader {
                if !node.kind.supports_shader() {
                    return Err("This layer kind does not support shader fills.".into());
                }
                if !shader.angle.is_finite()
                    || !shader.intensity.is_finite()
                    || !(0.0..=1.0).contains(&shader.intensity)
                {
                    return Err(
                        "Shader angle must be finite and intensity must be in 0..=1.".into(),
                    );
                }
            }
            if node
                .line_height
                .is_some_and(|h| !h.is_finite() || !LINE_HEIGHT.contains(&h))
            {
                return Err("Line height must be between 1 and 4,096.".into());
            }
            if node.kind == NodeKind::Frame {
                frame_ids.insert(node.id);
            }
        }
        if self.next_id == 0 {
            return Err("The next layer identifier is invalid.".into());
        }
        if self.next_directive_id == 0 {
            return Err("The next directive identifier is invalid.".into());
        }
        if self.directives.len() > 1_000 {
            return Err("Document exceeds the 1,000 directive limit.".into());
        }
        let mut directive_ids = StdHashSet::new();
        for directive in &self.directives {
            if directive.id == 0
                || directive.id >= self.next_directive_id
                || !directive_ids.insert(directive.id)
            {
                return Err(
                    "Directive identifiers must be unique and below the next identifier.".into(),
                );
            }
            if ![directive.x, directive.y]
                .iter()
                .all(|n| n.is_finite() && n.abs() <= MAX_COORDINATE)
            {
                return Err("Directive coordinates are out of range.".into());
            }
            if directive.message.is_empty() || directive.message.len() > 4_000 {
                return Err("Directive message must contain 1 to 4,000 bytes.".into());
            }
            if let Some(lid) = directive.layer_id
                && !ids.contains(&lid)
            {
                return Err(format!("Directive references missing layer {lid}."));
            }
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
                || fields["nodes"].as_array().is_some_and(|nodes| {
                    nodes.iter().any(|node| {
                        node.get("fill_token").is_some() || node.get("stroke_token").is_some()
                    })
                }))
        {
            return Err("Version 1 documents cannot contain color token fields.".into());
        }
        // A frozen schema may not carry newer keys even at default/null values.
        // Value-based validation alone cannot distinguish omitted from explicit defaults.
        if version.is_some_and(|v| v < 3)
            && (["directives", "next_directive_id"]
                .iter()
                .any(|key| fields.get(key).is_some())
                || fields["nodes"].as_array().is_some_and(|nodes| {
                    nodes.iter().any(|node| {
                        [
                            "line_height",
                            "text_align",
                            "parent",
                            "clip",
                            "stroke",
                            "stroke_token",
                            "stroke_width",
                            "points",
                            "closed",
                            "shader",
                        ]
                        .iter()
                        .any(|key| node.get(key).is_some())
                    })
                }))
        {
            return Err("Version 1 and 2 documents cannot contain version 3 fields, even at their defaults.".into());
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

    fn css_background(&self, node: &Node) -> String {
        let fill = self.css_value(node.fill, node.fill_token);
        if let Some(shader) = &node.shader {
            let sec = css_color(shader.secondary);
            match shader.kind {
                ShaderKind::LinearGradient | ShaderKind::Grain | ShaderKind::Halftone => {
                    format!("linear-gradient({}deg,{fill},{sec})", shader.angle)
                }
                ShaderKind::RadialGradient | ShaderKind::Mesh => {
                    format!("radial-gradient(circle,{fill},{sec})")
                }
            }
        } else {
            fill
        }
    }

    fn css_border(&self, node: &Node) -> String {
        if node.stroke_width > 0.0 && (node.stroke.is_some() || node.stroke_token.is_some()) {
            let stroke = self.css_value(node.stroke.unwrap_or([0, 0, 0, 255]), node.stroke_token);
            format!(";border:{}px solid {stroke}", node.stroke_width)
        } else {
            String::new()
        }
    }

    fn svg_paint(&self, node: &Node) -> String {
        let mut paint = svg_fill(self.node_fill(node));
        if let Some(sc) = self.node_stroke(node) {
            paint.push_str(&format!(
                " stroke=\"#{:02x}{:02x}{:02x}\" stroke-opacity=\"{:.4}\" stroke-width=\"{}\"",
                sc[0],
                sc[1],
                sc[2],
                sc[3] as f32 / 255.0,
                node.stroke_width
            ));
        }
        paint
    }

    fn validate_vector_export(&self) -> Result<(), String> {
        self.validate()?;
        for node in self.nodes.iter().filter(|n| !self.is_effectively_hidden(n)) {
            if node.shader.is_some() {
                return Err(format!(
                    "Layer {} has a shader unsupported by HTML/SVG export. Use PNG or remove the shader.",
                    node.id
                ));
            }
            if self.ancestors_clip_rect(node).is_some() {
                return Err(format!(
                    "Layer {} has ancestor clipping unsupported by HTML/SVG export. Use PNG or disable ancestor clipping.",
                    node.id
                ));
            }
        }
        Ok(())
    }

    pub fn export_html(&self) -> Result<String, String> {
        self.validate_vector_export()?;
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
        for node in self.nodes.iter().filter(|n| !self.is_effectively_hidden(n)) {
            let style = format!(
                "position:absolute;box-sizing:border-box;overflow:hidden;left:{}px;top:{}px;width:{}px;height:{}px;",
                node.x, node.y, node.width, node.height
            );
            let fill = self.css_value(node.fill, node.fill_token);
            let bg = self.css_background(node);
            let border = self.css_border(node);
            match node.kind {
                NodeKind::Frame => html.push_str(&format!("<div class=\"layer frame\" aria-label=\"{}\" style=\"{style}background:{bg};border-radius:{}px{border}\"></div>\n", escape(&node.name), node.radius)),
                NodeKind::Rectangle => html.push_str(&format!("<div aria-label=\"{}\" style=\"{style}background:{bg};border-radius:{}px{border}\"></div>\n", escape(&node.name), node.radius)),
                NodeKind::Ellipse => html.push_str(&format!("<div class=\"layer ellipse\" aria-label=\"{}\" style=\"{style}background:{bg};border-radius:50%{border};\"></div>\n", escape(&node.name))),
                NodeKind::Triangle => html.push_str(&format!("<div class=\"layer triangle\" aria-label=\"{}\" style=\"{style}background:{bg};clip-path:polygon(50% 0%,100% 100%,0% 100%){border};\"></div>\n", escape(&node.name))),
                NodeKind::Path => {
                    let d = svg_path_d(node, true);
                    let stroke_attr = if let Some(sc) = self.node_stroke(node) {
                        format!(
                            " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                            css_color(sc),
                            node.stroke_width
                        )
                    } else {
                        String::new()
                    };
                    html.push_str(&format!(
                        "<svg class=\"layer path\" aria-label=\"{}\" style=\"{style}\" viewBox=\"0 0 {} {}\"><path d=\"{d}\" fill=\"{fill}\"{stroke_attr}/></svg>\n",
                        escape(&node.name),
                        node.width,
                        node.height
                    ));
                }
                NodeKind::Text => {
                    let line_height = node.line_height.map_or("1.2".into(), |h| format!("{h}px"));
                    let align = match node.text_align {
                        TextAlign::Left => String::new(),
                        align => format!(";text-align:{}", align.name()),
                    };
                    html.push_str(&format!("<div style=\"{style}white-space:pre-wrap;overflow-wrap:break-word;font:{}px sans-serif;line-height:{line_height};color:{fill}{align}\">{}</div>\n", node.font_size, escape(&node.text)));
                }
            }
        }
        html.push_str("</main></body></html>\n");
        Ok(html)
    }

    /// Exports the artboard in document units. Text keeps explicit line breaks;
    /// fonts are resolved by the receiving application.
    pub fn export_svg(&self) -> Result<String, String> {
        self.validate_vector_export()?;
        let visible = self.nodes.iter().filter(|n| !self.is_effectively_hidden(n));
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
                NodeKind::Frame | NodeKind::Rectangle => svg.push_str(&format!(
                    "<rect {label} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" {}/>\n",
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    node.radius.min(node.width.min(node.height) / 2.0),
                    self.svg_paint(node)
                )),
                NodeKind::Ellipse => svg.push_str(&format!(
                    "<ellipse {label} cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {}/>\n",
                    node.x + node.width / 2.0,
                    node.y + node.height / 2.0,
                    node.width / 2.0,
                    node.height / 2.0,
                    self.svg_paint(node)
                )),
                NodeKind::Triangle => svg.push_str(&format!(
                    "<polygon {label} points=\"{},{} {},{} {},{}\" {}/>\n",
                    node.x + node.width / 2.0,
                    node.y,
                    node.x + node.width,
                    node.y + node.height,
                    node.x,
                    node.y + node.height,
                    self.svg_paint(node)
                )),
                NodeKind::Path => svg.push_str(&format!(
                    "<path {label} d=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" {}/>\n",
                    svg_path_d(node, false),
                    self.svg_paint(node)
                )),
                NodeKind::Text => {
                    let anchor = match node.text_align {
                        TextAlign::Left => "",
                        TextAlign::Center => " text-anchor=\"middle\"",
                        TextAlign::Right => " text-anchor=\"end\"",
                    };
                    svg.push_str(&format!(
                        "<defs><clipPath id=\"clip-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>\n<text {label} clip-path=\"url(#clip-{})\" font-family=\"sans-serif\" font-size=\"{}\"{anchor} xml:space=\"preserve\" {}>",
                        node.id, node.x, node.y, node.width, node.height,
                        node.id, node.font_size, svg_fill(self.node_fill(node))
                    ));
                    // shortcut: explicit lines only; share native text layout when wrapped SVG text is required.
                    let text = node.text.replace("\r\n", "\n").replace('\r', "\n");
                    let x = node.x + node.width * node.text_align.anchor();
                    // Without an authored line height, lines keep the earlier 1.2 × font size.
                    let step = node.line_height.unwrap_or(node.font_size * 1.2);
                    for (line, text) in text.split('\n').enumerate() {
                        svg.push_str(&format!(
                            "<tspan x=\"{x}\" y=\"{}\">{}</tspan>",
                            node.y + node.font_size + line as f32 * step,
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

fn svg_path_d(node: &Node, relative: bool) -> String {
    let mut d = String::new();
    let ox = if relative { 0.0 } else { node.x };
    let oy = if relative { 0.0 } else { node.y };
    for (i, &[px, py]) in node.points.iter().enumerate() {
        let cmd = if i == 0 { "M" } else { " L" };
        let x = ox + px * node.width;
        let y = oy + py * node.height;
        d.push_str(&format!("{cmd} {x} {y}"));
    }
    if node.closed {
        d.push_str(" Z");
    }
    d
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
    /// Membership index of `selection`, so per-layer checks stay constant time.
    selected: IdSet,
    past: Vec<(Document, u64)>,
    future: Vec<(Document, u64)>,
    /// Identifies the committed state; restored by undo and redo.
    revision: u64,
    next_revision: u64,
}

fn clamp_coordinate(value: f32) -> f32 {
    value.clamp(-MAX_COORDINATE, MAX_COORDINATE)
}

fn node_rect(node: &Node) -> [f32; 4] {
    node.bounds()
}

fn union_bounds(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            selection: Vec::new(),
            selected: IdSet::default(),
            past: Vec::new(),
            future: Vec::new(),
            revision: 0,
            next_revision: 1,
        }
    }
    /// Identifier of the committed document state. Every committed change gets a new
    /// value; undo and redo restore the value of the state they return to. Direct edits
    /// to `document` do not change it until they are committed.
    pub fn revision(&self) -> u64 {
        self.revision
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
        self.selected.contains(&id)
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
        self.selected.clear();
        let mut ids = ids.into_iter().peekable();
        if ids.peek().is_none() {
            return;
        }
        let existing: IdSet = self.document.nodes.iter().map(|n| n.id).collect();
        for id in ids {
            if existing.contains(&id) && self.selected.insert(id) {
                self.selection.push(id);
            }
        }
    }
    /// Adds `id` to the selection, or removes it when already selected.
    pub fn toggle_selection(&mut self, id: u64) {
        if self.selected.remove(&id) {
            self.selection.retain(|s| *s != id);
        } else if self.document.nodes.iter().any(|n| n.id == id) {
            self.selected.insert(id);
            self.selection.push(id);
        }
    }
    /// Selects every visible, unlocked layer — the layers a canvas click can select.
    pub fn select_all(&mut self) {
        let ids: Vec<_> = self
            .document
            .nodes
            .iter()
            .filter(|n| {
                !self.document.is_effectively_hidden(n) && !self.document.is_effectively_locked(n)
            })
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
            .filter(|n| {
                self.is_selected(n.id)
                    && !self.document.is_effectively_hidden(n)
                    && !self.document.is_effectively_locked(n)
            })
            .map(|n| n.id)
            .collect()
    }

    /// Union bounds `[left, top, right, bottom]` of the arrangeable selection.
    pub fn selection_bounds(&self) -> Option<[f32; 4]> {
        self.document
            .nodes
            .iter()
            .filter(|n| {
                self.is_selected(n.id)
                    && !self.document.is_effectively_hidden(n)
                    && !self.document.is_effectively_locked(n)
            })
            .map(node_rect)
            .reduce(union_bounds)
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
            self.past.push((before, self.revision));
            self.future.clear();
            self.revision = self.next_revision;
            self.next_revision += 1;
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

    /// Applies a fallible `edit` as one history step. When `edit` returns an error or
    /// leaves the document invalid, the document is restored and history is unchanged.
    pub fn apply<T>(
        &mut self,
        edit: impl FnOnce(&mut Document) -> Result<T, String>,
    ) -> Result<T, String> {
        let before = self.document.clone();
        match edit(&mut self.document) {
            Ok(value) => {
                self.commit(before)?;
                self.check_selection();
                Ok(value)
            }
            Err(error) => {
                self.document = before;
                Err(error)
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some((doc, revision)) = self.past.pop() {
            let current = std::mem::replace(&mut self.document, doc);
            self.future
                .push((current, std::mem::replace(&mut self.revision, revision)));
            self.check_selection();
        }
    }
    pub fn redo(&mut self) {
        if let Some((doc, revision)) = self.future.pop() {
            let current = std::mem::replace(&mut self.document, doc);
            self.past
                .push((current, std::mem::replace(&mut self.revision, revision)));
            self.check_selection();
        }
    }
    /// Drops selected identifiers that no longer exist.
    fn check_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let existing: IdSet = self.document.nodes.iter().map(|n| n.id).collect();
        self.selection.retain(|id| existing.contains(id));
        self.selected.retain(|id| existing.contains(id));
    }

    pub fn add(&mut self, kind: NodeKind) -> Result<(), String> {
        let (width, height) = if kind == NodeKind::Frame {
            (320.0, 220.0)
        } else {
            (220.0, 80.0)
        };
        self.add_at(kind, 80.0, 80.0, width, height)
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
        self.add_configured_at(
            kind,
            [x, y, width, height],
            TextStyle::default(),
            None,
            None,
        )
        .map(|_| ())
    }

    /// Adds a text layer using `style`, as one history step, selects it and returns its
    /// identifier. A token that no longer exists is not bound.
    pub fn add_text_at(&mut self, geometry: [f32; 4], style: TextStyle) -> Result<u64, String> {
        self.add_configured_at(NodeKind::Text, geometry, style, None, None)
    }

    /// Initializes all creation fields before validation/history, including fields
    /// previously appended by the GUI after a committed insertion.
    pub fn add_configured_at(
        &mut self,
        kind: NodeKind,
        geometry: [f32; 4],
        style: TextStyle,
        parent: Option<u64>,
        shader: Option<ShaderFill>,
    ) -> Result<u64, String> {
        let [x, y, width, height] = geometry.map(|v| v.round());
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
        let text = kind == NodeKind::Text;
        let fill_token = style
            .fill_token
            .filter(|token| text && self.document.token(*token).is_some());
        let fill = match kind {
            NodeKind::Text => match fill_token.and_then(|t| self.document.token(t)) {
                Some(token) => token.value,
                None => style.fill,
            },
            NodeKind::Frame => [244, 242, 236, 255],
            NodeKind::Path => [255, 107, 43, 0],
            _ => [124, 155, 214, 255],
        };
        self.document.nodes.push(Node {
            id,
            name: kind.label().into(),
            kind,
            x: clamp_coordinate(x),
            y: clamp_coordinate(y),
            width: width.clamp(1.0, MAX_COORDINATE),
            height: height.clamp(1.0, MAX_COORDINATE),
            fill,
            radius: if kind == NodeKind::Frame { 12.0 } else { 0.0 },
            text: if text { NEW_TEXT.into() } else { String::new() },
            font_size: if text { style.font_size } else { 28.0 },
            hidden: false,
            locked: false,
            fill_token,
            line_height: if text { style.line_height } else { None },
            text_align: if text {
                style.text_align
            } else {
                TextAlign::Left
            },
            parent,
            clip: kind == NodeKind::Frame,
            stroke: (kind == NodeKind::Path).then_some([255, 107, 43, 255]),
            stroke_token: None,
            stroke_width: if kind == NodeKind::Path { 3.0 } else { 0.0 },
            points: if kind == NodeKind::Path {
                default_path_points()
            } else {
                Vec::new()
            },
            closed: false,
            shader,
        });
        self.document.next_id = next;
        self.commit(before)?;
        self.select(Some(id));
        Ok(id)
    }

    /// Wraps the arrangeable selection in a new Frame layer as one history step and selects it.
    pub fn wrap_selection_in_frame(&mut self) -> Result<u64, String> {
        let ids = self.arrangeable();
        if ids.is_empty() {
            return Err("Select at least one unlocked visible layer to wrap in a frame.".into());
        }
        let frame_id = self.apply(|doc| doc.wrap_in_frame(&ids, None))?;
        self.select(Some(frame_id));
        Ok(frame_id)
    }

    /// Deletes every selected unlocked layer (hidden ones included) as one step.
    pub fn delete(&mut self) -> Result<(), String> {
        let mut selection = self.selected.clone();
        for &id in &self.selection {
            if self
                .document
                .nodes
                .iter()
                .any(|n| n.id == id && !n.locked && n.kind == NodeKind::Frame)
            {
                selection.extend(self.document.descendants_of(id));
            }
        }
        self.transaction(|doc| {
            doc.nodes.retain(|n| !selection.contains(&n.id) || n.locked);
            let remaining: IdSet = doc.nodes.iter().map(|n| n.id).collect();
            for node in &mut doc.nodes {
                if let Some(pid) = node.parent
                    && !remaining.contains(&pid)
                {
                    node.parent = None;
                }
            }
            doc.directives
                .retain(|d| d.layer_id.is_none_or(|lid| remaining.contains(&lid)));
        })?;
        self.check_selection();
        Ok(())
    }

    /// Duplicates every selected unlocked layer above the current layers, in their
    /// existing order, offset by 20 units, and selects the copies. One history step.
    pub fn duplicate(&mut self) -> Result<(), String> {
        let ids: Vec<u64> = self
            .document
            .nodes
            .iter()
            .filter(|n| self.is_selected(n.id) && !n.locked)
            .map(|n| n.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let copies = self.apply(|doc| doc.duplicate_nodes(&ids, 20.0, 20.0))?;
        self.set_selection(copies);
        Ok(())
    }

    /// Changes shape layer `id` to `kind` as one history step. Leaving the rectangle
    /// kind clears the corner radius.
    pub fn set_shape(&mut self, id: u64, kind: NodeKind) -> Result<(), String> {
        self.apply(|doc| {
            let node = doc
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .ok_or_else(|| format!("No layer {id}."))?;
            if !node.kind.is_shape() || !kind.is_shape() {
                return Err("Only shape layers can change shape.".into());
            }
            if node.locked {
                return Err("Unlock the layer to change its shape.".into());
            }
            node.set_shape(kind);
            Ok(())
        })
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
        let mut ids: IdSet = self.arrangeable().into_iter().collect();
        for id in ids.clone() {
            for d in self.document.descendants_of(id) {
                if self.document.nodes.iter().any(|n| n.id == d && !n.locked) {
                    ids.insert(d);
                }
            }
        }
        let [dx, dy] = self.document.clamp_translation(&ids, [dx, dy]);
        self.transaction(|doc| {
            for node in doc.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                node.x += dx;
                node.y += dy;
            }
        })
    }

    /// Shows all selected layers when any is hidden; otherwise hides them. One step.
    pub fn toggle_hidden(&mut self) -> Result<(), String> {
        let selection = self.selected.clone();
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
        let selection = self.selected.clone();
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
        if ids.is_empty() {
            return Ok(());
        }
        self.apply(|doc| doc.align_nodes(&ids, align))
    }

    /// Gives three or more arrangeable layers equal gaps along `axis`. Layers are ordered
    /// by their leading edge (paint order breaks ties); the first and last stay in place.
    /// Overlapping layers can produce negative gaps. One step.
    pub fn distribute(&mut self, axis: Axis) -> Result<(), String> {
        let ids = self.arrangeable();
        if ids.len() < 3 {
            return Ok(());
        }
        self.apply(|doc| doc.distribute_nodes(&ids, axis))
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
        matches!(target, ColorTarget::Node(id) | ColorTarget::Stroke(id)
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
