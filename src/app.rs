use crate::document::{
    Align, Axis, ColorTarget, ColorToken, Document, Editor, IdSet, MAX_COORDINATE, MAX_TEXT_BYTES,
    NEW_TEXT, Node, NodeKind, TextStyle, atomic_write,
};
use crate::recents::Recents;
use crate::render::{color, export_png, font_definitions, paint_document};
use crate::snap::{self, Frame, Guide, Handle, SNAP_DISTANCE, Side, Snapper};
use crate::ui::{
    self, ACCENT, BODY, BORDER, CAPTION, CONTROL, ICON, ICON_SMALL, ROW, Row, SHELL, SPACE_L,
    SPACE_M, SPACE_S, SPACE_XS, TEXT, TEXT_DIM,
};
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, Painter, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};
use egui_phosphor::regular as icon;
use std::path::PathBuf;

mod dashboard;

/// Left inset of the tab strip. On macOS the strip shares the title bar with the window
/// controls.
const TAB_INSET: f32 = if cfg!(target_os = "macos") { 72.0 } else { 8.0 };
/// Default geometry for a rectangle placed with a single click.
const CLICK_SIZE: Vec2 = vec2(220.0, 80.0);
/// Extra artboard width prevents glyph rounding at other zoom levels from wrapping.
const AUTO_WIDTH_SLACK: f32 = 2.0;
/// Snapping guide color; distinct from the selection accent.
const GUIDE: Color32 = Color32::from_rgb(242, 78, 120);

#[derive(Clone, Debug, PartialEq)]
enum Intent {
    New,
    Open,
    OpenPath(PathBuf),
    CloseDocument,
    Close,
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Html,
    Svg,
    /// PNG at the given pixels per artboard unit.
    Png(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Editor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Rectangle,
    Text,
}

/// A tool rail entry. The rail and the single-key shortcuts both read [`TOOLS`].
pub struct ToolSpec {
    pub tool: Tool,
    pub glyph: &'static str,
    pub label: &'static str,
    pub key: Key,
    /// Rail group; a separator is drawn between groups.
    pub group: u8,
}

/// Tools in rail order. Only implemented tools are listed.
pub const TOOLS: [ToolSpec; 4] = [
    ToolSpec {
        tool: Tool::Select,
        glyph: icon::CURSOR,
        label: "Select",
        key: Key::V,
        group: 0,
    },
    ToolSpec {
        tool: Tool::Hand,
        glyph: icon::HAND,
        label: "Hand",
        key: Key::H,
        group: 0,
    },
    ToolSpec {
        tool: Tool::Rectangle,
        glyph: icon::SQUARE,
        label: "Rectangle",
        key: Key::R,
        group: 1,
    },
    ToolSpec {
        tool: Tool::Text,
        glyph: icon::TEXT_AA,
        label: "Text",
        key: Key::T,
        group: 1,
    },
];

impl Tool {
    fn creates(self) -> Option<NodeKind> {
        match self {
            Tool::Rectangle => Some(NodeKind::Rectangle),
            Tool::Text => Some(NodeKind::Text),
            Tool::Select | Tool::Hand => None,
        }
    }
}

struct Drag {
    before: Document,
    start: Pos2,
    /// Layers moved together, or the single resized layer.
    nodes: IdSet,
    /// Positions of `nodes` in the document; layer order is fixed during a gesture.
    indices: Vec<usize>,
    /// Resize handle being dragged, or `None` when moving.
    handle: Option<Handle>,
    /// Bounds of `nodes` when the gesture started, in artboard units.
    frame: Frame,
    /// Edges and centers of the other visible layers and the artboard.
    snapper: Snapper,
}

/// Marquee selection in progress, in artboard units.
struct Marquee {
    start: Pos2,
    current: Pos2,
    /// Selection kept when the marquee started with Shift.
    base: Vec<u64>,
}

/// A token change requested by an inspector color control, applied after the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TokenChoice {
    Bind(ColorTarget, Option<u64>),
    CreateFrom(ColorTarget),
}

/// The parts of a document that a panel edits in place during one frame: every field
/// except the layers, plus at most one layer. Taking it is much cheaper than cloning a
/// large document every frame.
struct Snapshot {
    header: Document,
    node: Option<(usize, Node)>,
}

impl Snapshot {
    fn take(document: &mut Document, node: Option<usize>) -> Self {
        let nodes = std::mem::take(&mut document.nodes);
        let header = document.clone();
        document.nodes = nodes;
        let node = node.and_then(|i| Some((i, document.nodes.get(i)?.clone())));
        Self { header, node }
    }

    /// The document as it was when the snapshot was taken, or `None` when the panel
    /// changed nothing.
    fn before(self, document: &mut Document) -> Option<Document> {
        let nodes = std::mem::take(&mut document.nodes);
        let header_changed = self.header != *document;
        document.nodes = nodes;
        let node_changed = self
            .node
            .as_ref()
            .is_some_and(|(i, node)| document.nodes.get(*i) != Some(node));
        if !header_changed && !node_changed {
            return None;
        }
        let mut before = self.header;
        before.nodes = document.nodes.clone();
        if let Some((i, node)) = self.node
            && let Some(slot) = before.nodes.get_mut(i)
        {
            *slot = node;
        }
        Some(before)
    }
}

/// A layer being drawn with the rectangle or text tool, in artboard coordinates.
struct Creation {
    kind: NodeKind,
    start: Pos2,
    current: Pos2,
}

/// A text layer edited in place on the canvas.
struct TextEditing {
    id: u64,
    /// Document before the edit; the whole edit is one history step.
    before: Document,
    /// The layer width follows its content (layers placed with a click).
    auto_width: bool,
    /// Focus and select all once possible.
    start: bool,
}

/// Rectangle drawn from `start` to `current`. `square` constrains it to equal sides;
/// `centered` draws it outward from `start`.
pub fn creation_rect(start: Pos2, current: Pos2, square: bool, centered: bool) -> Rect {
    let mut delta = current - start;
    if square {
        let side = delta.x.abs().max(delta.y.abs());
        delta = vec2(side.copysign(delta.x), side.copysign(delta.y));
    }
    if centered {
        Rect::from_center_size(start, delta.abs() * 2.0)
    } else {
        Rect::from_two_pos(start, start + delta)
    }
}

/// An open document that is not the active tab. The active tab's state lives in the
/// `PaperCraftApp` fields with the same names; its slot in `tabs` holds a placeholder.
/// Returns a process-unique key for a newly opened document.
fn document_key() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

struct Tab {
    editor: Editor,
    saved: Document,
    path: Option<PathBuf>,
    zoom: f32,
    pan: Vec2,
    fit: bool,
    file_version: u32,
    selected_token: Option<u64>,
    text_style: TextStyle,
    dirty: bool,
    key: u64,
}

impl Tab {
    fn placeholder() -> Self {
        let document = Document::default();
        Self {
            file_version: document.version,
            saved: document.clone(),
            editor: Editor::new(document),
            path: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit: true,
            selected_token: None,
            text_style: TextStyle::default(),
            dirty: false,
            key: document_key(),
        }
    }

    fn dirty(&self) -> bool {
        self.dirty
    }
}

pub struct PaperCraftApp {
    pub editor: Editor,
    saved: Document,
    /// Result of comparing the committed document with `saved`, keyed by the editor
    /// revision it was computed for. Comparing a large document every frame is costly.
    dirty_cache: std::cell::Cell<Option<(u64, bool)>>,
    path: Option<PathBuf>,
    status: String,
    zoom: f32,
    pan: Vec2,
    fit: bool,
    drag: Option<Drag>,
    marquee: Option<Marquee>,
    creation: Option<Creation>,
    text_edit: Option<TextEditing>,
    /// Style of the text layer most recently selected; new text layers use it.
    text_style: TextStyle,
    /// Moving and resizing snap to layer and artboard edges and centers.
    pub snapping: bool,
    /// Shift was held when the primary button was last pressed on the canvas.
    press_shift: bool,
    property_before: Option<Document>,
    pending: Option<Intent>,
    allow_close: bool,
    help: bool,
    pub view: View,
    pub tool: Tool,
    document_open: bool,
    /// One entry per open document, in tab order; empty when no document is open.
    tabs: Vec<Tab>,
    /// Index of the active tab in `tabs`.
    active: usize,
    recents: Recents,
    cards: dashboard::CardCache,
    search: String,
    list_view: bool,
    left_open: bool,
    pages_open: bool,
    layers_open: bool,
    logo: Option<egui::TextureHandle>,
    /// Left panel shows the Theme (tokens) tab instead of Design (pages and layers).
    pub theme: bool,
    colors_open: bool,
    pub selected_token: Option<u64>,
    /// Layer being renamed in the layer list, with its edited name.
    renaming: Option<(u64, String)>,
    /// Format version of the file at `path` when it was opened or last saved.
    file_version: u32,
    /// Identity of the active document, unique for the process. Layer and token IDs
    /// are document-local, so widget state keyed by them is namespaced with it.
    document_key: u64,
    /// Save is waiting for confirmation to upgrade a version 1 file.
    upgrade_prompt: bool,
}

impl PaperCraftApp {
    pub fn from_document(document: Document) -> Self {
        Self {
            saved: document.clone(),
            dirty_cache: Default::default(),
            editor: Editor::new(document),
            path: None,
            status: "Starter document · local and editable".into(),
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit: true,
            drag: None,
            marquee: None,
            creation: None,
            text_edit: None,
            text_style: TextStyle::default(),
            snapping: true,
            press_shift: false,
            property_before: None,
            pending: None,
            allow_close: false,
            help: false,
            view: View::Editor,
            tool: Tool::Select,
            document_open: true,
            tabs: vec![Tab::placeholder()],
            active: 0,
            recents: Recents::in_memory(),
            cards: Default::default(),
            search: String::new(),
            list_view: false,
            left_open: true,
            pages_open: true,
            layers_open: true,
            logo: None,
            theme: false,
            colors_open: true,
            selected_token: None,
            renaming: None,
            file_version: 1,
            document_key: document_key(),
            upgrade_prompt: false,
        }
    }

    /// Uses `recents` for the dashboard list instead of the in-memory default.
    pub fn with_recents(mut self, recents: Recents) -> Self {
        self.recents = recents;
        self.cards.invalidate();
        self
    }

    pub fn configure(ctx: &egui::Context) {
        ctx.set_fonts(font_definitions());
        ctx.set_visuals(egui::Visuals::dark());
        ctx.global_style_mut(|s| {
            s.spacing.item_spacing = vec2(8.0, 8.0);
            s.spacing.button_padding = vec2(SPACE_S, SPACE_XS);
            s.spacing.interact_size.y = ROW;
            s.spacing.menu_margin = egui::Margin::same(4);
            let v = &mut s.visuals;
            v.panel_fill = SHELL;
            v.window_fill = SHELL;
            v.window_stroke = Stroke::new(1.0, BORDER);
            v.window_corner_radius = egui::CornerRadius::same(ui::RADIUS_LARGE);
            v.menu_corner_radius = egui::CornerRadius::same(ui::RADIUS_LARGE);
            v.extreme_bg_color = Color32::from_rgb(30, 30, 30);
            v.faint_bg_color = Color32::from_rgb(46, 46, 46);
            v.override_text_color = None;
            v.selection.bg_fill = Color32::from_rgb(54, 77, 113);
            v.selection.stroke = Stroke::new(1.0, ACCENT);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
            v.widgets.noninteractive.fg_stroke.color = TEXT;
            for (w, fill) in [
                (&mut v.widgets.inactive, CONTROL),
                (&mut v.widgets.hovered, ui::PRESSED),
                (&mut v.widgets.active, Color32::from_rgb(70, 70, 70)),
                (&mut v.widgets.open, CONTROL),
            ] {
                w.weak_bg_fill = fill;
                w.bg_fill = fill;
                w.bg_stroke = Stroke::NONE;
                w.corner_radius = egui::CornerRadius::same(ui::RADIUS);
                w.expansion = 0.0;
            }
            v.widgets.inactive.fg_stroke.color = TEXT;
            v.widgets.hovered.fg_stroke.color = TEXT;
            v.widgets.active.fg_stroke.color = TEXT;
            v.widgets.open.fg_stroke.color = TEXT;
            s.text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(BODY));
            s.text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(BODY));
            s.text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(CAPTION));
            s.text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(SPACE_L));
        });
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure(&cc.egui_ctx);
        let recents = Recents::default_store()
            .map(Recents::load)
            .unwrap_or_default();
        let mut app = Self::from_document(Document::example()).with_recents(recents);
        if let Some(path) = std::env::args_os().nth(1) {
            app.open_path(PathBuf::from(path));
        }
        app
    }

    pub fn set_list_view(&mut self, list: bool) {
        self.list_view = list;
    }

    pub fn document_open(&self) -> bool {
        self.document_open
    }

    /// Number of open documents, one per window tab.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Index of the active document tab.
    pub fn active_tab(&self) -> usize {
        self.active
    }

    /// Closes the active document. Another open document becomes active; with none left
    /// the dashboard is shown. Refuses (returns `false`) when the document has unsaved
    /// changes; interactive closing asks first instead.
    pub fn close_document(&mut self) -> bool {
        self.flush_properties();
        if self.dirty() {
            return false;
        }
        self.close_tab();
        true
    }

    /// Removes the active tab without checking for unsaved changes.
    fn close_tab(&mut self) {
        self.clear_transient();
        self.cards.invalidate();
        self.status = "Document closed".into();
        if self.document_open {
            self.tabs.remove(self.active);
        }
        if self.tabs.is_empty() {
            self.replace_document(Document::default(), None);
            self.document_open = false;
            self.active = 0;
            self.view = View::Dashboard;
        } else {
            self.active = self.active.min(self.tabs.len() - 1);
            self.swap_tab(self.active);
            self.view = View::Editor;
        }
    }

    /// Makes tab `index` the active document and shows the editor.
    pub fn activate_tab(&mut self, index: usize) {
        if !self.document_open || index >= self.tabs.len() {
            return;
        }
        self.view = View::Editor;
        if index == self.active {
            return;
        }
        self.flush_properties();
        self.clear_transient();
        self.swap_tab(self.active);
        self.swap_tab(index);
        self.active = index;
    }

    /// Exchanges the active document state with the state stored in tab `index`.
    fn swap_tab(&mut self, index: usize) {
        let dirty = self.dirty();
        let tab = &mut self.tabs[index];
        std::mem::swap(&mut self.editor, &mut tab.editor);
        std::mem::swap(&mut self.saved, &mut tab.saved);
        std::mem::swap(&mut self.path, &mut tab.path);
        std::mem::swap(&mut self.zoom, &mut tab.zoom);
        std::mem::swap(&mut self.pan, &mut tab.pan);
        std::mem::swap(&mut self.fit, &mut tab.fit);
        std::mem::swap(&mut self.file_version, &mut tab.file_version);
        std::mem::swap(&mut self.selected_token, &mut tab.selected_token);
        std::mem::swap(&mut self.text_style, &mut tab.text_style);
        std::mem::swap(&mut self.document_key, &mut tab.key);
        self.dirty_cache
            .set(Some((self.editor.revision(), tab.dirty)));
        tab.dirty = dirty;
    }

    /// Drops in-progress gestures and edits that belong to the active document.
    fn clear_transient(&mut self) {
        self.property_before = None;
        self.text_edit = None;
        self.drag = None;
        self.creation = None;
        self.marquee = None;
        self.renaming = None;
        self.upgrade_prompt = false;
    }

    /// Opens `document` in a new tab after the active one and makes it active.
    fn open_tab(&mut self, document: Document, path: Option<PathBuf>) {
        if self.document_open {
            self.flush_properties();
            self.swap_tab(self.active);
            self.active += 1;
            self.tabs.insert(self.active, Tab::placeholder());
        } else {
            self.tabs = vec![Tab::placeholder()];
            self.active = 0;
        }
        self.replace_document(document, path);
        self.document_open = true;
        self.view = View::Editor;
        self.cards.invalidate();
    }

    fn dirty(&self) -> bool {
        if !self.document_open {
            return false;
        }
        // Uncommitted edits change the document without a new revision.
        if self.property_before.is_some() || self.drag.is_some() || self.text_edit.is_some() {
            return self.editor.document != self.saved;
        }
        let revision = self.editor.revision();
        match self.dirty_cache.get() {
            Some((cached, dirty)) if cached == revision => dirty,
            _ => {
                let dirty = self.editor.document != self.saved;
                self.dirty_cache.set(Some((revision, dirty)));
                dirty
            }
        }
    }

    fn set_saved(&mut self, document: Document) {
        self.saved = document;
        self.dirty_cache.set(None);
    }

    fn tab_dirty(&self, index: usize) -> bool {
        if index == self.active {
            self.dirty()
        } else {
            self.tabs[index].dirty()
        }
    }

    fn tab_document(&self, index: usize) -> &Document {
        if index == self.active {
            &self.editor.document
        } else {
            &self.tabs[index].editor.document
        }
    }

    fn tab_path(&self, index: usize) -> Option<&std::path::Path> {
        if index == self.active {
            self.path.as_deref()
        } else {
            self.tabs[index].path.as_deref()
        }
    }

    /// First tab with unsaved changes, preferring the active one.
    fn first_dirty_tab(&self) -> Option<usize> {
        if self.dirty() {
            return Some(self.active);
        }
        (0..self.tabs.len()).find(|&i| self.tab_dirty(i))
    }

    fn result(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.status = error;
        }
    }

    fn flush_properties(&mut self) {
        if let Some(edit) = self.text_edit.take() {
            let result = self.editor.commit(edit.before);
            self.result(result);
        }
        if let Some(before) = self.property_before.take() {
            let result = self.editor.commit(before);
            self.result(result);
        }
        if let Some((id, text)) = self.renaming.take() {
            let name = text.trim();
            if !name.is_empty()
                && let Some(index) = self.editor.document.nodes.iter().position(|n| n.id == id)
                && self.editor.document.nodes[index].name != name
            {
                let before = self.editor.document.clone();
                self.editor.document.nodes[index].name = name.into();
                let result = self.editor.commit(before);
                self.result(result);
            }
        }
    }

    fn remember(&mut self, path: &std::path::Path) {
        if let Err(error) = self.recents.touch(path) {
            self.status = format!("{} {error}", self.status);
        }
        self.cards.invalidate();
    }

    fn replace_document(&mut self, document: Document, path: Option<PathBuf>) {
        self.document_key = document_key();
        self.file_version = document.version;
        self.selected_token = None;
        self.renaming = None;
        self.upgrade_prompt = false;
        self.set_saved(document.clone());
        self.editor = Editor::new(document);
        self.path = path;
        self.fit = true;
        self.property_before = None;
        self.drag = None;
        self.marquee = None;
        self.creation = None;
        self.text_edit = None;
        self.text_style = TextStyle::default();
        self.tool = Tool::Select;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn save_document(&mut self, save_as: bool) -> bool {
        if !self.document_open {
            return false;
        }
        self.flush_properties();
        let path = if !save_as && self.path.is_some() {
            self.path.clone()
        } else {
            rfd::FileDialog::new()
                .add_filter("PaperCraft document", &["papercraft"])
                .set_file_name(format!("{}.papercraft", self.editor.document.name))
                .save_file()
        };
        let Some(path) = path else {
            return false;
        };
        // Saving in place must not silently make a version 1 file unreadable to older
        // PaperCraft; see docs/document-format.md.
        let upgrading = self
            .path
            .as_ref()
            .is_some_and(|p| document_path(p.clone()) == document_path(path.clone()))
            && self.file_version < self.editor.document.required_version();
        if upgrading {
            self.upgrade_prompt = true;
            return false;
        }
        self.write_document(path)
    }

    fn write_document(&mut self, path: PathBuf) -> bool {
        self.upgrade_prompt = false;
        let path = std::path::absolute(&path).unwrap_or(path);
        let identity = document_path(path.clone());
        if (0..self.tabs.len()).any(|i| {
            i != self.active
                && self
                    .tab_path(i)
                    .is_some_and(|p| document_path(p.to_path_buf()) == identity)
        }) {
            self.status =
                "This file is open in another tab. Close that tab before replacing it.".into();
            return false;
        }
        match self.editor.document.save(&path) {
            Ok(()) => {
                self.file_version = self.editor.document.required_version();
                self.set_saved(self.editor.document.clone());
                self.status = format!("Saved {}", path.display());
                self.remember(&path);
                self.path = Some(path);
                true
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let path = std::path::absolute(&path).unwrap_or(path);
        let identity = document_path(path.clone());
        if let Some(index) = (0..self.tabs.len()).find(|&i| {
            self.tab_path(i)
                .is_some_and(|p| document_path(p.to_path_buf()) == identity)
        }) {
            self.activate_tab(index);
            return;
        }
        match Document::load(&path) {
            Ok(document) => {
                self.remember(&path);
                self.open_tab(document, Some(path));
                self.status = "Document opened".into();
            }
            Err(error) => {
                self.status = if self.document_open {
                    format!("{error} Open documents were kept.")
                } else {
                    error
                }
            }
        }
    }

    fn request(&mut self, intent: Intent, ctx: &egui::Context) {
        self.flush_properties();
        match intent {
            // New and opened documents get their own tabs; only closing can lose work.
            Intent::CloseDocument if self.dirty() => self.pending = Some(intent),
            Intent::Close => match self.first_dirty_tab() {
                Some(index) => {
                    self.activate_tab(index);
                    self.pending = Some(intent);
                }
                None => self.perform(intent, ctx),
            },
            intent => self.perform(intent, ctx),
        }
    }

    fn perform(&mut self, intent: Intent, ctx: &egui::Context) {
        match intent {
            Intent::New => {
                self.open_tab(Document::default(), None);
                self.status = "New document".into();
            }
            Intent::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PaperCraft document", &["papercraft", "crumbs"])
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Intent::OpenPath(path) => self.open_path(path),
            // Unsaved changes were confirmed or discarded before reaching here.
            Intent::CloseDocument => self.close_tab(),
            Intent::Close => {
                // The active document was saved or discarded; ask about the next one.
                if self.dirty() {
                    self.close_tab();
                }
                if let Some(index) = self.first_dirty_tab() {
                    self.activate_tab(index);
                    self.pending = Some(Intent::Close);
                } else {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn export(&mut self, format: ExportFormat) {
        self.flush_properties();
        let (label, extension) = match format {
            ExportFormat::Html => ("HTML", "html"),
            ExportFormat::Svg => ("SVG", "svg"),
            ExportFormat::Png(_) => ("PNG", "png"),
        };
        // Check PNG limits before asking for a destination.
        if let ExportFormat::Png(scale) = format
            && let Err(error) = crate::render::png_size(&self.editor.document, scale)
        {
            self.status = error;
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[extension])
            .set_file_name(format!("{}.{extension}", self.editor.document.name))
            .save_file()
        else {
            return;
        };
        let data = match format {
            ExportFormat::Html => self.editor.document.export_html().map(String::into_bytes),
            ExportFormat::Svg => self.editor.document.export_svg().map(String::into_bytes),
            ExportFormat::Png(scale) => export_png(&self.editor.document, scale),
        };
        match data.and_then(|data| atomic_write(&path, &data)) {
            Ok(()) => self.status = format!("Exported {}", path.display()),
            Err(error) => self.status = error,
        }
    }

    fn export_menu(&mut self, ui: &mut egui::Ui) {
        for (label, format) in [
            ("Export HTML…", ExportFormat::Html),
            ("Export SVG…", ExportFormat::Svg),
            ("Export PNG 1×…", ExportFormat::Png(1.0)),
            ("Export PNG 2×…", ExportFormat::Png(2.0)),
        ] {
            if ui.button(label).clicked() {
                ui.close();
                self.export(format);
            }
        }
        ui.label(
            RichText::new("SVG keeps explicit text lines; fonts may differ.")
                .small()
                .color(TEXT_DIM),
        );
    }

    fn undo(&mut self) {
        self.flush_properties();
        self.editor.undo();
    }

    fn redo(&mut self) {
        self.flush_properties();
        self.editor.redo();
    }

    fn set_tool(&mut self, tool: Tool) {
        if self.text_edit.is_some() {
            self.flush_properties();
        }
        self.tool = tool;
        self.creation = None;
    }

    /// Edits text layer `id` in place. `auto_width` makes its width follow the content.
    fn begin_text_edit(&mut self, id: u64, auto_width: bool) {
        self.flush_properties();
        self.editor.select(Some(id));
        self.text_edit = Some(TextEditing {
            id,
            before: self.editor.document.clone(),
            auto_width,
            start: true,
        });
    }

    /// Keeps the inherited text style's color equal to its bound token and detaches the
    /// binding once the token is gone, so a later token reusing the ID is never bound.
    fn sync_text_style(&mut self) {
        let Some(id) = self.text_style.fill_token else {
            return;
        };
        match self.editor.document.token(id) {
            Some(token) => self.text_style.fill = token.value,
            None => self.text_style.fill_token = None,
        }
    }

    /// Adds a text layer at `at` and edits it. Without `rect` the layer is sized to its
    /// content and keeps following it while edited.
    fn place_text(&mut self, painter: &Painter, at: Pos2, rect: Option<Rect>) {
        self.sync_text_style();
        let style = self.text_style;
        let geometry = match rect {
            Some(rect) => [rect.min.x, rect.min.y, rect.width(), rect.height()],
            None => {
                let size = text_size(painter, NEW_TEXT, style.font_size, None);
                [at.x, at.y, size.x, size.y]
            }
        };
        match self.editor.add_text_at(geometry, style) {
            Ok(id) => self.begin_text_edit(id, rect.is_none()),
            Err(error) => self.status = error,
        }
    }

    /// Zooms around the canvas center by `factor`.
    fn zoom_by(&mut self, factor: f32) {
        let old = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.05, 4.0);
        self.pan *= self.zoom / old;
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let command = |key| egui::KeyboardShortcut::new(Modifiers::COMMAND, key);
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::S))) {
            self.save_document(false);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::O))) {
            self.request(Intent::Open, ctx);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::N))) {
            self.request(Intent::New, ctx);
        }
        if self.view == View::Dashboard {
            if ctx.input_mut(|i| i.consume_shortcut(&command(Key::F))) {
                ctx.memory_mut(|m| m.request_focus(dashboard::search_id()));
            }
            return;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Z,
            ))
        }) {
            self.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Z))) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::D))) {
            self.flush_properties();
            let result = self.editor.duplicate();
            self.result(result);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::A))) {
            self.flush_properties();
            self.editor.select_all();
        }
        let command_shift =
            |key| egui::KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key);
        if ctx.input_mut(|i| i.consume_shortcut(&command_shift(Key::H))) {
            self.flush_properties();
            let result = self.editor.toggle_hidden();
            self.result(result);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command_shift(Key::L))) {
            self.flush_properties();
            let result = self.editor.toggle_locked();
            self.result(result);
        }
        // Arrangement: ⌥A/D/W/S/H/V align, ⌃⌥H/V distribute.
        for (key, align) in [
            (Key::A, Align::Left),
            (Key::D, Align::Right),
            (Key::W, Align::Top),
            (Key::S, Align::Bottom),
            (Key::H, Align::CenterX),
            (Key::V, Align::CenterY),
        ] {
            if ctx.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::ALT, key))
            }) {
                self.arrange(Some(align), None);
            }
        }
        for (key, axis) in [(Key::H, Axis::Horizontal), (Key::V, Axis::Vertical)] {
            if ctx.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    Modifiers::CTRL | Modifiers::ALT,
                    key,
                ))
            }) {
                self.arrange(None, Some(axis));
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.flush_properties();
            let result = self.editor.delete();
            self.result(result);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            // egui has already taken focus from an in-place text edit; end it and keep
            // the layer selected.
            if self.text_edit.is_some() {
                self.flush_properties();
            } else if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            } else {
                self.editor.select(None);
            }
        }
        if !ctx.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt) {
            if let Some(id) = self
                .editor
                .selected_node()
                .filter(|n| n.kind == NodeKind::Text && !n.hidden && !n.locked)
                .map(|n| n.id)
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            {
                self.begin_text_edit(id, false);
            }
            for spec in &TOOLS {
                if ctx.input(|i| i.key_pressed(spec.key)) {
                    self.set_tool(spec.tool);
                }
            }
            if ctx.input(|i| i.key_pressed(Key::F)) {
                self.fit = true;
            }
        }
        let step = if ctx.input(|i| i.modifiers.shift) {
            10.0
        } else {
            1.0
        };
        let delta = ctx.input(|i| {
            vec2(
                (i.key_pressed(Key::ArrowRight) as u8 as f32
                    - i.key_pressed(Key::ArrowLeft) as u8 as f32)
                    * step,
                (i.key_pressed(Key::ArrowDown) as u8 as f32
                    - i.key_pressed(Key::ArrowUp) as u8 as f32)
                    * step,
            )
        });
        if delta != Vec2::ZERO && !self.editor.selection().is_empty() {
            self.flush_properties();
            let result = self.editor.nudge(delta.x, delta.y);
            self.result(result);
        }
    }

    /// Aligns or distributes the arrangeable selection as one history step.
    fn arrange(&mut self, align: Option<Align>, distribute: Option<Axis>) {
        self.flush_properties();
        let count = self.editor.arrangeable().len();
        let result = match (align, distribute) {
            (Some(align), _) if count >= 1 => self.editor.align(align),
            (_, Some(axis)) if count >= 3 => self.editor.distribute(axis),
            (Some(_), _) => Err("Select a visible, unlocked layer to align.".into()),
            _ => Err("Distribute needs three or more visible, unlocked layers.".into()),
        };
        self.result(result);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let finish_gesture_first =
            self.drag.is_some() || self.creation.is_some() || self.marquee.is_some();
        if !finish_gesture_first && self.pending.is_none() && !ctx.input(|i| i.pointer.any_down()) {
            self.shortcuts(&ctx);
        }
        let title = if self.document_open {
            format!(
                "{}{} — PaperCraft",
                self.editor.document.name,
                if self.dirty() { " •" } else { "" }
            )
        } else {
            "PaperCraft".into()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        egui::Panel::top("tabs")
            .exact_size(ui::TAB_BAR)
            .frame(egui::Frame::NONE.fill(SHELL))
            .show(ui, |ui| self.tab_bar(ui, &ctx));
        if self.view == View::Dashboard || !self.document_open {
            self.view = View::Dashboard;
            self.dashboard(ui, &ctx);
        } else {
            self.editor_view(ui, &ctx);
        }

        if finish_gesture_first && self.pending.is_none() && !ctx.input(|i| i.pointer.any_down()) {
            self.shortcuts(&ctx);
        }

        if let Some(intent) = self.pending.clone() {
            egui::Modal::new(egui::Id::new("unsaved")).show(&ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(RichText::new("Save changes before continuing?").size(16.0));
                ui.label(
                    RichText::new(format!(
                        "“{}” has unsaved changes.",
                        self.editor.document.name
                    ))
                    .color(TEXT_DIM),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Save and continue").clicked() && self.save_document(false) {
                        self.pending = None;
                        self.perform(intent.clone(), &ctx);
                    }
                    if ui.button("Discard changes").clicked() {
                        self.pending = None;
                        self.perform(intent.clone(), &ctx);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending = None;
                    }
                });
            });
        }
        if self.upgrade_prompt {
            egui::Modal::new(egui::Id::new("format_upgrade")).show(&ctx, |ui| {
                ui.set_max_width(400.0);
                ui.label(RichText::new("Save this file in the new format?").size(16.0));
                ui.label(
                    RichText::new(
                        "Color tokens need document format version 2. Earlier builds, including \
                         the published v0.1.1, cannot open it. Saving as a new file keeps \
                         the original unchanged.",
                    )
                    .color(TEXT_DIM),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let saved = if ui.button("Save as new file…").clicked() {
                        self.upgrade_prompt = false;
                        self.save_document(true)
                    } else if ui.button("Upgrade this file").clicked() {
                        self.path
                            .clone()
                            .is_some_and(|path| self.write_document(path))
                    } else {
                        if ui.button("Cancel").clicked() {
                            self.upgrade_prompt = false;
                        }
                        false
                    };
                    // Continue an action that was waiting for this save.
                    if saved && let Some(intent) = self.pending.take() {
                        self.perform(intent, &ctx);
                    }
                });
            });
        }
        egui::Window::new("Keyboard shortcuts")
            .open(&mut self.help)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                egui::Grid::new("shortcut_grid")
                    .num_columns(2)
                    .spacing([24.0, 6.0])
                    .show(ui, |ui| {
                        for (keys, action) in [
                            ("⌘ / Ctrl + N, O, S", "New, open, save"),
                            ("⌘ / Ctrl + Z", "Undo"),
                            ("⌘ / Ctrl + Shift + Z", "Redo"),
                            ("⌘ / Ctrl + D", "Duplicate"),
                            ("⌘ / Ctrl + F", "Search recent files"),
                            ("V / H", "Select / hand tool"),
                            ("R / T", "Rectangle / text tool"),
                            ("Esc", "Select tool, then clear selection"),
                            ("Shift + click", "Add or remove a layer from the selection"),
                            (
                                "Drag empty canvas",
                                "Select layers with a marquee (Shift: add)",
                            ),
                            ("Drag a handle", "Resize; several layers scale together"),
                            ("Shift + corner drag", "Resize keeping proportions"),
                            ("⌘ / Ctrl + drag", "Move or resize without snapping"),
                            ("⌘ / Ctrl + A", "Select visible, unlocked layers"),
                            ("Arrow keys", "Move selection (Shift: 10 px)"),
                            ("⌥ / Alt + A, H, D", "Align left, horizontal centers, right"),
                            ("⌥ / Alt + W, V, S", "Align top, vertical centers, bottom"),
                            (
                                "⌃ ⌥ / Ctrl + Alt + H, V",
                                "Distribute horizontally, vertically",
                            ),
                            ("⌘ / Ctrl + Shift + H, L", "Hide / lock selection"),
                            ("Delete / Backspace", "Delete unlocked selection"),
                            ("F", "Fit artboard"),
                            ("Space + drag / middle drag", "Pan"),
                            ("Scroll", "Pan"),
                            ("⌘ / Ctrl + scroll", "Zoom around pointer"),
                        ] {
                            ui.label(RichText::new(keys).color(TEXT_DIM));
                            ui.label(action);
                            ui.end_row();
                        }
                    });
            });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let strip = ui.max_rect();
        ui.painter().hline(
            strip.x_range(),
            strip.bottom() - 0.5,
            Stroke::new(1.0, BORDER),
        );
        let background = ui.interact(strip, egui::Id::new("tab_strip"), Sense::click_and_drag());
        if background.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if background.double_clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_XS;
            ui.add_space(TAB_INSET);
            if tab(
                ui,
                icon::SQUARES_FOUR,
                "Dashboard",
                self.view == View::Dashboard,
                None,
                260.0,
            )
            .0
            .clicked()
            {
                self.flush_properties();
                self.view = View::Dashboard;
                self.cards.invalidate();
            }
            if self.document_open {
                let (rect, _) = ui.allocate_exact_size(vec2(SPACE_S, SPACE_L), Sense::hover());
                ui.painter()
                    .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
                // Tabs share the space left of the new-file button.
                let count = self.tabs.len() as f32;
                let room = ui.available_width() - 40.0 - SPACE_XS * count;
                let max_width = (room / count).clamp(96.0, 260.0);
                let mut select = None;
                let mut close_tab = None;
                egui::ScrollArea::horizontal()
                    .id_salt("document_tabs")
                    .max_width((ui.available_width() - 40.0).max(0.0))
                    .max_height(28.0)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for index in 0..self.tabs.len() {
                                let name = self.tab_document(index).name.clone();
                                let selected = self.view == View::Editor && index == self.active;
                                let (response, close) =
                                    tab(ui, icon::FILE, &name, selected, Some(index), max_width);
                                let dirty = self.tab_dirty(index);
                                let response =
                                    response.on_hover_text(match (self.tab_path(index), dirty) {
                                        (Some(path), true) => {
                                            format!("{} · unsaved changes", path.display())
                                        }
                                        (Some(path), false) => path.display().to_string(),
                                        (None, _) => "Not saved yet".into(),
                                    });
                                if close.is_some_and(|c| c.clicked()) {
                                    close_tab = Some(index);
                                } else if response.clicked() {
                                    select = Some(index);
                                }
                            }
                        })
                    });
                if let Some(index) = close_tab {
                    self.activate_tab(index);
                    self.request(Intent::CloseDocument, ctx);
                } else if let Some(index) = select {
                    self.activate_tab(index);
                }
            }
            ui.add_space(SPACE_XS);
            if ui::icon_button(
                ui,
                icon::PLUS,
                vec2(ROW, ROW),
                ICON,
                false,
                true,
                "New file",
            )
            .clicked()
            {
                self.request(Intent::New, ctx);
            }
        });
    }

    fn editor_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let side = egui::Frame::NONE.fill(SHELL);
        if self.left_open {
            egui::Panel::left("layers")
                .default_size(ui::SIDEBAR)
                .size_range(ui::SIDEBAR..=320.0)
                .resizable(true)
                .frame(side)
                .show(ui, |ui| self.left_panel(ui, ctx));
        }
        egui::Panel::left("tools")
            .exact_size(ui::RAIL)
            .resizable(false)
            .frame(side)
            .show(ui, |ui| self.tool_rail(ui));
        egui::Panel::right("inspector")
            .default_size(ui::INSPECTOR)
            .size_range(ui::INSPECTOR..=320.0)
            .resizable(true)
            .frame(side)
            .show(ui, |ui| self.right_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(color(self.editor.document.canvas_color())))
            .show(ui, |ui| {
                self.canvas(ui);
                if !self.left_open {
                    self.floating_file_header(ui, ctx);
                }
            });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.set_min_width(220.0);
        let command = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl+"
        };
        let item = |ui: &mut egui::Ui, enabled: bool, label: &str, shortcut: &str| {
            ui.add_enabled(
                enabled,
                egui::Button::new(label)
                    .frame_when_inactive(false)
                    .shortcut_text(RichText::new(shortcut).color(TEXT_DIM)),
            )
            .clicked()
        };
        if item(ui, true, "New file", &format!("{command}N")) {
            ui.close();
            self.request(Intent::New, ctx);
        }
        if item(ui, true, "Open…", &format!("{command}O")) {
            ui.close();
            self.request(Intent::Open, ctx);
        }
        ui.separator();
        if item(ui, true, "Save", &format!("{command}S")) {
            ui.close();
            self.save_document(false);
        }
        if item(ui, true, "Save as…", "") {
            ui.close();
            self.save_document(true);
        }
        self.export_menu(ui);
        ui.separator();
        let can_undo = self.editor.can_undo() || self.property_before.is_some();
        if item(ui, can_undo, "Undo", &format!("{command}Z")) {
            ui.close();
            self.undo();
        }
        let shift = if cfg!(target_os = "macos") {
            "⇧⌘Z"
        } else {
            "Ctrl+Shift+Z"
        };
        if item(ui, self.editor.can_redo(), "Redo", shift) {
            ui.close();
            self.redo();
        }
        ui.separator();
        if item(ui, true, "Keyboard shortcuts", "") {
            ui.close();
            self.help = true;
        }
        if item(ui, true, "Close file", "") {
            ui.close();
            self.request(Intent::CloseDocument, ctx);
        }
    }

    /// File name with the file menu, as shown at the top of the layer panel.
    fn file_name_button(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, width: f32) {
        let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "File menu"));
        if response.hovered() || egui::Popup::is_id_open(ctx, egui::Id::new("file_menu")) {
            ui.painter().rect_filled(rect, ui::RADIUS, ui::HOVER);
        }
        ui.painter().text(
            pos2(rect.left() + SPACE_S + ICON_SMALL / 2.0, rect.center().y),
            Align2::CENTER_CENTER,
            icon::COOKIE,
            ui::icon_font(ICON_SMALL),
            TEXT_DIM,
        );
        let text_x = SPACE_S + ICON_SMALL + SPACE_S;
        let galley = ui::truncated(
            ui,
            &self.editor.document.name,
            BODY,
            TEXT,
            rect.width() - text_x - SPACE_S,
        );
        ui.painter().galley(
            pos2(
                rect.left() + text_x,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            TEXT,
        );
        let response = response.on_hover_text("File menu");
        egui::Popup::menu(&response)
            .id(egui::Id::new("file_menu"))
            .show(|ui| self.file_menu(ui, ctx));
    }

    fn left_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        // File header.
        ui.allocate_ui_with_layout(
            vec2(width, ui::SPACE_2XL + SPACE_S),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(SPACE_S);
                self.file_name_button(ui, ctx, width - 3.0 * SPACE_S - ROW);
                ui.add_space(SPACE_XS);
                if ui::icon_button(
                    ui,
                    icon::SIDEBAR_SIMPLE,
                    vec2(ROW, ROW),
                    ICON,
                    false,
                    true,
                    "Hide sidebar",
                )
                .clicked()
                {
                    self.left_open = false;
                }
            },
        );
        ui::hline(ui);
        ui.add_space(SPACE_M);
        if segmented(ui, self.theme) {
            self.flush_properties();
            self.theme = !self.theme;
        }
        ui.add_space(SPACE_S);
        if self.theme {
            self.status_footer(ui);
            self.theme_panel(ui);
            return;
        }

        section_toggle(
            ui,
            "Pages",
            &mut self.pages_open,
            Some((icon::PLUS, "PaperCraft documents have one page", false)),
        );
        if self.pages_open
            && Row::new(icon::FILE, "Page 1")
                .right(Some(icon::CHECK))
                .show(ui)
                .on_hover_text("Current page · select the artboard")
                .clicked()
        {
            self.flush_properties();
            self.editor.select(None);
        }
        ui.add_space(8.0);
        ui::hline(ui);

        self.status_footer(ui);

        ui.add_space(8.0);
        section_toggle(ui, "Layers", &mut self.layers_open, None);
        if !self.layers_open {
            return;
        }
        let total = self.editor.document.nodes.len();
        // Only visible rows are built, so long layer lists cost the same as short ones.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, ROW, total, |ui, rows| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let nodes: Vec<_> = self
                    .editor
                    .document
                    .nodes
                    .iter()
                    .rev()
                    .skip(rows.start)
                    .take(rows.len())
                    .map(|n| (n.id, n.name.clone(), n.kind, n.hidden, n.locked))
                    .collect();
                for (id, name, kind, hidden, locked) in nodes {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
                    let selected = self.editor.is_selected(id);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name)
                    });
                    let glyph = if kind == NodeKind::Text {
                        icon::TEXT_AA
                    } else {
                        icon::SQUARE
                    };
                    if self.rename_row(ui, id, rect, &response, glyph, selected) {
                        continue;
                    }
                    let hovered = ui.rect_contains_pointer(rect);
                    Row::new(glyph, &name)
                        .selected(selected)
                        .dim(hidden)
                        .reserve(if hovered || hidden || locked {
                            48.0
                        } else {
                            0.0
                        })
                        .paint(ui, rect, &response, SPACE_S);
                    let mut toggle = |offset: f32, glyph: &str, label: &str| {
                        let button = Rect::from_center_size(
                            pos2(rect.right() - offset, rect.center().y),
                            vec2(20.0, 20.0),
                        );
                        ui::icon_button_at(
                            ui,
                            button,
                            egui::Id::new(("layer_toggle", id, offset as i32)),
                            glyph,
                            ICON_SMALL,
                            false,
                            true,
                            label,
                        )
                        .clicked()
                    };
                    let mut changed = None;
                    if (hovered || hidden)
                        && toggle(
                            40.0,
                            if hidden { icon::EYE_SLASH } else { icon::EYE },
                            if hidden { "Show layer" } else { "Hide layer" },
                        )
                    {
                        changed = Some((true, !hidden));
                    }
                    if (hovered || locked)
                        && toggle(
                            SPACE_L + SPACE_XS,
                            if locked { icon::LOCK } else { icon::LOCK_OPEN },
                            if locked { "Unlock layer" } else { "Lock layer" },
                        )
                    {
                        changed = Some((false, !locked));
                    }
                    if let Some((visibility, value)) = changed {
                        self.flush_properties();
                        let before = self.editor.document.clone();
                        if let Some(node) =
                            self.editor.document.nodes.iter_mut().find(|n| n.id == id)
                        {
                            if visibility {
                                node.hidden = value;
                            } else {
                                node.locked = value;
                            }
                        }
                        let result = self.editor.commit(before);
                        self.result(result);
                    } else if response.double_clicked() {
                        self.flush_properties();
                        self.renaming = Some((id, name.clone()));
                        ui.memory_mut(|m| {
                            m.request_focus(egui::Id::new(("rename_layer", self.document_key, id)))
                        });
                    } else if response.clicked() {
                        self.flush_properties();
                        // Shift or ⌘/Ctrl adds or removes the row; locked and hidden
                        // layers can be selected here but are not arranged.
                        if ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                            self.editor.toggle_selection(id);
                        } else {
                            self.editor.select(Some(id));
                        }
                    }
                }
                if self.editor.document.nodes.is_empty() {
                    ui.add_space(SPACE_S);
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(16, 0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(
                                    "No layers. Use the rectangle or text tool to add one.",
                                )
                                .color(TEXT_DIM),
                            );
                        });
                }
            });
    }

    /// Draws the name editor over a layer row while that layer is renamed.
    /// Enter or clicking elsewhere commits one history step; Escape cancels.
    /// Returns `true` when the row was drawn here.
    fn rename_row(
        &mut self,
        ui: &mut egui::Ui,
        id: u64,
        rect: Rect,
        response: &egui::Response,
        glyph: &str,
        selected: bool,
    ) -> bool {
        let Some((_, text)) = self.renaming.as_mut().filter(|(rid, _)| *rid == id) else {
            return false;
        };
        Row::new(glyph, "")
            .indent(SPACE_M)
            .selected(selected)
            .paint(ui, rect, response, 8.0);
        let edit_id = egui::Id::new(("rename_layer", self.document_key, id));
        let field = Rect::from_min_max(
            pos2(
                rect.left() + SPACE_M + ICON_SMALL + SPACE_S,
                rect.top() + SPACE_XS,
            ),
            pos2(rect.right() - SPACE_M, rect.bottom() - SPACE_XS),
        );
        let edit = ui.put(
            field,
            egui::TextEdit::singleline(text)
                .id(edit_id)
                .frame(
                    egui::Frame::NONE
                        .fill(SHELL)
                        .stroke(Stroke::new(1.0, ACCENT))
                        .corner_radius(4),
                )
                .margin(vec2(SPACE_XS, 0.0))
                .font(ui::font(BODY))
                .desired_width(f32::INFINITY),
        );
        if !edit.lost_focus() {
            return true;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.renaming = None;
        } else {
            self.flush_properties();
        }
        true
    }

    fn status_footer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("left_status")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 12,
            }))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.label(RichText::new(&self.status).size(CAPTION).color(TEXT_DIM));
            });
    }

    /// Theme tab: the document's color tokens. Names and values edit in place as one
    /// history step per completed edit; add, delete and binding changes are one step each.
    fn theme_panel(&mut self, ui: &mut egui::Ui) {
        let snapshot = Snapshot::take(&mut self.editor.document, None);
        let mut add = false;
        let section = section_toggle(
            ui,
            "Colors",
            &mut self.colors_open,
            Some((icon::PLUS, "Add color token", true)),
        );
        if section.is_some_and(|r| r.clicked()) {
            add = true;
        }
        if self
            .selected_token
            .is_some_and(|id| self.editor.document.token(id).is_none())
        {
            self.selected_token = None;
        }
        let mut delete = None;
        let mut finish = false;
        let mut rename: Option<(u64, String)> = None;
        if self.colors_open {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let tokens: Vec<ColorToken> = self.editor.document.tokens.clone();
                    for token in &tokens {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
                        let selected = self.selected_token == Some(token.id);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                true,
                                selected,
                                &token.name,
                            )
                        });
                        Row::new("", &token.name)
                            .indent(SPACE_L + ICON_SMALL + SPACE_S)
                            .selected(selected)
                            .paint(ui, rect, &response, SPACE_S);
                        swatch(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(rect.left() + SPACE_L + ICON_SMALL / 2.0, rect.center().y),
                                vec2(ICON_SMALL, ICON_SMALL),
                            ),
                            token.value,
                        );
                        if response.clicked() {
                            self.flush_properties();
                            self.selected_token = (!selected).then_some(token.id);
                        }
                        if !selected {
                            continue;
                        }
                        let usage = self.editor.document.token_usage(token.id).len();
                        egui::Frame::NONE
                            .inner_margin(egui::Margin {
                                left: 16,
                                right: 12,
                                top: 8,
                                bottom: 12,
                            })
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = vec2(SPACE_S, SPACE_S);
                                let name_id =
                                    egui::Id::new(("token_name", self.document_key, token.id));
                                let mut name = ui
                                    .data(|d| d.get_temp::<String>(name_id))
                                    .unwrap_or_else(|| token.name.clone());
                                let edit = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut name)
                                            .id(name_id)
                                            .frame(
                                                egui::Frame::NONE
                                                    .fill(CONTROL)
                                                    .corner_radius(ui::RADIUS)
                                                    .inner_margin(egui::Margin::symmetric(8, 4)),
                                            )
                                            .margin(egui::Margin::symmetric(8, 4))
                                            .char_limit(64)
                                            .desired_width(f32::INFINITY),
                                    )
                                    .on_hover_text("Token name: letters, digits, - and _");
                                if edit.has_focus() {
                                    ui.data_mut(|d| d.insert_temp(name_id, name.clone()));
                                }
                                if edit.lost_focus() {
                                    ui.data_mut(|d| d.remove::<String>(name_id));
                                    if name != token.name {
                                        rename = Some((token.id, name));
                                    }
                                }
                                if let Some(live) = self
                                    .editor
                                    .document
                                    .tokens
                                    .iter_mut()
                                    .find(|t| t.id == token.id)
                                {
                                    finish |= ui::color_field(
                                        ui,
                                        egui::Id::new(("token_value", self.document_key, token.id)),
                                        &mut live.value,
                                    );
                                }
                                ui.label(
                                    RichText::new(match usage {
                                        0 => "Not used".to_string(),
                                        1 => "Used by 1 property".to_string(),
                                        n => format!("Used by {n} properties"),
                                    })
                                    .size(CAPTION)
                                    .color(TEXT_DIM),
                                );
                                if ui
                                    .button(format!("{}  Delete token", icon::TRASH))
                                    .on_hover_text(
                                        "Bound properties keep this color as a plain value",
                                    )
                                    .clicked()
                                {
                                    delete = Some(token.id);
                                }
                            });
                    }
                    if tokens.is_empty() {
                        ui.add_space(8.0);
                        egui::Frame::NONE
                            .inner_margin(egui::Margin::symmetric(16, 0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(
                                        "No color tokens. Add one here, or create one from a \
                                         color in the inspector.",
                                    )
                                    .color(TEXT_DIM),
                                );
                            });
                    }
                });
        }
        let before = snapshot.before(&mut self.editor.document);
        self.track_properties(before, finish, ui.ctx());
        if let Some((id, name)) = rename {
            self.flush_properties();
            let result = self.editor.rename_token(id, &name);
            self.result(result);
        }
        if let Some(id) = delete {
            self.flush_properties();
            let result = self.editor.delete_token(id);
            self.result(result);
            self.selected_token = None;
        }
        if add {
            self.flush_properties();
            match self.editor.create_token([124, 155, 214, 255]) {
                Ok(id) => {
                    self.selected_token = Some(id);
                    self.colors_open = true;
                }
                Err(error) => self.status = error,
            }
        }
    }

    /// Opens a property transaction with `before`, the document before this frame's
    /// edits (`None` when unchanged), and commits it when an edit finished or no field is
    /// being edited.
    fn track_properties(&mut self, before: Option<Document>, finish: bool, ctx: &egui::Context) {
        if let Some(before) = before
            && self.property_before.is_none()
        {
            self.property_before = Some(before);
        }
        // An in-place text edit takes focus later in the frame and ends itself.
        let idle = !ctx.egui_wants_keyboard_input() && !ctx.egui_is_using_pointer();
        if self.renaming.is_none() && (finish || (idle && self.text_edit.is_none())) {
            self.flush_properties();
        }
    }

    fn floating_file_header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let anchor = ui.max_rect().left_top() + vec2(SPACE_S, SPACE_S);
        egui::Area::new(egui::Id::new("floating_file"))
            .fixed_pos(anchor)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(SHELL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(ui::RADIUS_LARGE)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_XS;
                            self.file_name_button(ui, ctx, ui::SIDEBAR - 2.0 * SPACE_S - ROW);
                            if ui::icon_button(
                                ui,
                                icon::SIDEBAR_SIMPLE,
                                vec2(ROW, ROW),
                                ICON,
                                false,
                                true,
                                "Show sidebar",
                            )
                            .clicked()
                            {
                                self.left_open = true;
                            }
                        });
                    });
            });
    }

    fn tool_rail(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, SPACE_XS);
        ui.add_space(SPACE_XS);
        ui.vertical_centered(|ui| {
            for (index, spec) in TOOLS.iter().enumerate() {
                if index > 0 && TOOLS[index - 1].group != spec.group {
                    rail_separator(ui);
                }
                let label = format!("{} · {}", spec.label, spec.key.name());
                if ui::icon_button(
                    ui,
                    spec.glyph,
                    vec2(ui::SPACE_2XL, ui::SPACE_2XL),
                    ICON,
                    self.tool == spec.tool,
                    true,
                    &label,
                )
                .clicked()
                {
                    self.set_tool(spec.tool);
                }
            }
            rail_separator(ui);
            let label = if self.snapping {
                "Snapping on · hold ⌘/Ctrl while dragging to bypass"
            } else {
                "Snapping off"
            };
            if ui::icon_button(
                ui,
                icon::MAGNET,
                vec2(ui::SPACE_2XL, ui::SPACE_2XL),
                ICON,
                self.snapping,
                true,
                label,
            )
            .clicked()
            {
                self.snapping = !self.snapping;
            }
        });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(width, ui::SPACE_2XL + SPACE_S),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_XS;
                ui.add_space(SPACE_S);
                let export = ui
                    .add(
                        egui::Button::new(RichText::new("Export").size(BODY).color(TEXT))
                            .fill(CONTROL)
                            .min_size(vec2(0.0, ROW)),
                    )
                    .on_hover_text("Export artboard as HTML, SVG or PNG");
                egui::Popup::menu(&export).show(|ui| self.export_menu(ui));
                let zoom = ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{:.0}%  {}",
                                self.zoom * 100.0,
                                icon::CARET_DOWN
                            ))
                            .size(BODY)
                            .color(TEXT_DIM),
                        )
                        .frame_when_inactive(false)
                        .min_size(vec2(0.0, ROW)),
                    )
                    .on_hover_text("Zoom");
                egui::Popup::menu(&zoom).show(|ui| {
                    ui.set_min_width(168.0);
                    let item = |ui: &mut egui::Ui, label: &str, key: &str| {
                        ui.add(
                            egui::Button::new(label)
                                .frame_when_inactive(false)
                                .shortcut_text(RichText::new(key).color(TEXT_DIM)),
                        )
                        .clicked()
                    };
                    if item(ui, "Zoom in", "") {
                        self.zoom_by(1.25);
                    }
                    if item(ui, "Zoom out", "") {
                        self.zoom_by(0.8);
                    }
                    if item(ui, "Zoom to 100%", "") {
                        self.zoom_by(1.0 / self.zoom);
                    }
                    if item(ui, "Zoom to fit", "F") {
                        self.fit = true;
                    }
                });
            },
        );
        ui::hline(ui);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.inspector(ui);
            });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        // The inspector edits document fields and, with one selected layer, that layer.
        let edited = self
            .editor
            .selected()
            .and_then(|id| self.editor.document.nodes.iter().position(|n| n.id == id));
        let snapshot = Snapshot::take(&mut self.editor.document, edited);
        let mut finish = false;
        let mut choice = None;
        let mut toggle_hidden = false;
        let tokens = self.editor.document.tokens.clone();
        // TextEdit sizes its text from `margin`, so the frame margin must match it.
        let text_margin = egui::Margin::symmetric(8, 4);
        let text_frame = egui::Frame::NONE
            .fill(CONTROL)
            .corner_radius(ui::RADIUS)
            .inner_margin(text_margin);
        let count = self.editor.selection().len();
        if count > 1 {
            let nodes = &self.editor.document.nodes;
            let excluded = nodes
                .iter()
                .filter(|n| self.editor.is_selected(n.id) && (n.hidden || n.locked))
                .count();
            let bounds = self.editor.selection_bounds();
            inspector_heading(ui, &format!("{count} layers selected"), true, false);
            inspector_body(ui, |ui| {
                if let Some([left, top, right, bottom]) = bounds {
                    ui.label(
                        RichText::new(format!(
                            "X {left:.0}   Y {top:.0}   W {:.0}   H {:.0}",
                            right - left,
                            bottom - top
                        ))
                        .color(TEXT),
                    )
                    .on_hover_text("Shared bounds of the visible, unlocked selected layers");
                }
                if excluded > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{excluded} hidden or locked {} not moved or arranged.",
                            if excluded == 1 {
                                "layer is"
                            } else {
                                "layers are"
                            }
                        ))
                        .size(CAPTION)
                        .color(TEXT_DIM),
                    );
                }
                ui.label(
                    RichText::new("Select one layer to edit its properties.")
                        .size(CAPTION)
                        .color(TEXT_DIM),
                );
            });
        } else if let Some(index) = self
            .editor
            .document
            .nodes
            .iter()
            .position(|n| Some(n.id) == self.editor.selected())
        {
            let node = &mut self.editor.document.nodes[index];
            let locked = node.locked;
            let hidden = node.hidden;
            let kind = node.kind;
            let id = node.id;
            ui.add_enabled_ui(!locked, |ui| {
                inspector_heading(ui, "Layout", true, true)
                    .on_hover_text("Layout modes are not available yet");
                inspector_body(ui, |ui| {
                    let third = ((ui.available_width() - 2.0 * SPACE_S) / 3.0 / SPACE_XS).floor()
                        * SPACE_XS;
                    let range = -100_000.0..=100_000.0;
                    ui.horizontal(|ui| {
                        let r = ui::number_field(ui, "X", &mut node.x, range.clone(), third);
                        finish |= r.drag_stopped() || r.lost_focus();
                        let r = ui::number_field(ui, "Y", &mut node.y, range.clone(), third);
                        finish |= r.drag_stopped() || r.lost_focus();
                        ui::inert_field(
                            ui,
                            icon::ANGLE,
                            "0°",
                            None,
                            third,
                            "Rotation is not available yet",
                        );
                    });
                    ui.horizontal(|ui| {
                        let r = ui::number_field(ui, "W", &mut node.width, 1.0..=100_000.0, third);
                        finish |= r.drag_stopped() || r.lost_focus();
                        let r = ui::number_field(ui, "H", &mut node.height, 1.0..=100_000.0, third);
                        finish |= r.drag_stopped() || r.lost_focus();
                        ui::segmented(
                            ui,
                            third,
                            &[
                                (
                                    icon::ARROW_COUNTER_CLOCKWISE,
                                    "Rotate 90° · not available yet",
                                ),
                                (icon::FLIP_HORIZONTAL, "Flip horizontal · not available yet"),
                                (icon::FLIP_VERTICAL, "Flip vertical · not available yet"),
                            ],
                            false,
                        );
                    });
                    ui::wide_button(ui, "Add flex", &format!("{} A", icon::ARROW_FAT_UP), false)
                        .on_hover_text("Flex layout is not available yet");
                    ui::check_row(
                        ui,
                        &mut false,
                        "Clip content",
                        &format!("{} C", icon::OPTION),
                        false,
                    )
                    .on_hover_text("Clipping is not available yet");
                });
                if kind == NodeKind::Text {
                    inspector_heading(ui, "Typography", true, false);
                    inspector_body(ui, |ui| {
                        let r = ui::number_field(
                            ui,
                            "Size",
                            &mut node.font_size,
                            1.0..=512.0,
                            ui.available_width(),
                        );
                        finish |= r.drag_stopped() || r.lost_focus();
                        finish |= ui
                            .add(
                                egui::TextEdit::multiline(&mut node.text)
                                    .frame(text_frame)
                                    .margin(text_margin)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY),
                            )
                            .on_hover_text("Text content")
                            .lost_focus();
                    });
                } else {
                    let heading = inspector_heading(ui, "Radius", true, false);
                    ui::icon_button_at(
                        ui,
                        heading_slot(heading.rect, 0),
                        egui::Id::new("radius_corners"),
                        icon::FRAME_CORNERS,
                        16.0,
                        false,
                        false,
                        "Independent corners are not available yet",
                    );
                    inspector_body(ui, |ui| {
                        let third = ((ui.available_width() - 2.0 * SPACE_S) / 3.0 / SPACE_XS)
                            .floor()
                            * SPACE_XS;
                        let max = (node.width.min(node.height) / 2.0).floor().max(0.0);
                        ui.horizontal(|ui| {
                            let r = ui::slider(
                                ui,
                                &mut node.radius,
                                max,
                                third * 2.0 + 8.0,
                                "Corner radius",
                            );
                            finish |= r.drag_stopped() || r.clicked();
                            let r =
                                ui::number_field(ui, "", &mut node.radius, 0.0..=100_000.0, third);
                            finish |= r.drag_stopped() || r.lost_focus();
                        });
                    });
                }
            });
            // Visibility stays available on locked layers, as in the layer list.
            let heading = inspector_heading(ui, "Blending", true, false);
            if ui::icon_button_at(
                ui,
                heading_slot(heading.rect, 0),
                egui::Id::new("inspector_visibility"),
                if hidden { icon::EYE_SLASH } else { icon::EYE },
                16.0,
                false,
                true,
                if hidden { "Show layer" } else { "Hide layer" },
            )
            .clicked()
            {
                toggle_hidden = true;
            }
            inspector_body(ui, |ui| {
                let half = (ui.available_width() - 8.0) / 2.0;
                ui.horizontal(|ui| {
                    ui::inert_field(
                        ui,
                        icon::CHECKERBOARD,
                        "100%",
                        None,
                        half,
                        "Layer opacity is not available yet; Fill sets color opacity",
                    );
                    ui::inert_field(
                        ui,
                        icon::DROP,
                        "Normal",
                        Some(icon::CARET_DOWN),
                        half,
                        "Blend modes are not available yet",
                    );
                });
            });
            let node = &mut self.editor.document.nodes[index];
            ui.add_enabled_ui(!locked, |ui| {
                let heading = inspector_heading(ui, "Fill", true, false);
                ui::icon_button_at(
                    ui,
                    heading_slot(heading.rect, 0),
                    egui::Id::new("fill_add"),
                    icon::PLUS,
                    16.0,
                    false,
                    false,
                    "Multiple fills are not available yet",
                );
                inspector_body(ui, |ui| {
                    ui.horizontal(|ui| {
                        fill_kinds(ui);
                        let right = ui.max_rect().right();
                        let y = ui.min_rect().center().y;
                        for (index, glyph, label) in [
                            (1, icon::EYE, "Fill visibility is not available yet"),
                            (0, icon::MINUS, "Removing the fill is not available yet"),
                        ] {
                            ui::icon_button_at(
                                ui,
                                trailing_slot(right, y, index),
                                egui::Id::new(("fill_control", index)),
                                glyph,
                                16.0,
                                false,
                                false,
                                label,
                            );
                        }
                    });
                    finish |= color_property(
                        ui,
                        egui::Id::new("node_fill"),
                        &tokens,
                        &mut node.fill,
                        node.fill_token,
                        ColorTarget::Node(id),
                        &mut choice,
                    );
                });
            });
            for title in [
                "Outline",
                "Border",
                "Shadow",
                "Inner shadow",
                "Filters",
                "Guides",
                "Video",
                "Export",
            ] {
                let heading = inspector_heading(ui, title, false, false);
                let rect = heading.rect;
                ui::hline(ui);
                if title == "Export" {
                    ui::icon_button_at(
                        ui,
                        heading_slot(rect, 1),
                        egui::Id::new("layer_export_format"),
                        icon::COPY,
                        16.0,
                        false,
                        false,
                        "Layer export is not available yet; the Export button exports the artboard",
                    );
                }
                ui::icon_button_at(
                    ui,
                    heading_slot(rect, 0),
                    egui::Id::new(("inspector_add", title)),
                    icon::PLUS,
                    16.0,
                    false,
                    false,
                    &format!("{title} is not available yet"),
                );
            }
        } else {
            let document = &mut self.editor.document;
            inspector_heading(ui, "Page", true, false);
            inspector_body(ui, |ui| {
                finish |= color_property(
                    ui,
                    egui::Id::new("page_color"),
                    &tokens,
                    &mut document.canvas,
                    document.canvas_token,
                    ColorTarget::Canvas,
                    &mut choice,
                );
            });
            inspector_heading(ui, "Artboard", true, false);
            inspector_body(ui, |ui| {
                finish |= ui
                    .add(
                        egui::TextEdit::singleline(&mut document.name)
                            .frame(text_frame)
                            .margin(text_margin)
                            .desired_width(f32::INFINITY),
                    )
                    .on_hover_text("Document name")
                    .lost_focus();
                let half = (ui.available_width() - SPACE_S) / 2.0;
                ui.horizontal(|ui| {
                    let r = ui::number_field(ui, "W", &mut document.width, 1.0..=100_000.0, half);
                    finish |= r.drag_stopped() || r.lost_focus();
                    let r = ui::number_field(ui, "H", &mut document.height, 1.0..=100_000.0, half);
                    finish |= r.drag_stopped() || r.lost_focus();
                });
                finish |= color_property(
                    ui,
                    egui::Id::new("artboard_fill"),
                    &tokens,
                    &mut document.background,
                    document.background_token,
                    ColorTarget::Background,
                    &mut choice,
                );
            });
            egui::Frame::NONE
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "Select a layer on the canvas or in the layer list to edit it. \
                             Shift-click adds layers to the selection. \
                             Double-click a layer in the list to rename it.",
                        )
                        .color(TEXT_DIM),
                    );
                });
        }
        let before = snapshot.before(&mut self.editor.document);
        self.track_properties(before, finish, ui.ctx());
        if let Some(choice) = choice {
            self.flush_properties();
            let result = match choice {
                TokenChoice::Bind(target, token) => self.editor.bind(target, token),
                TokenChoice::CreateFrom(target) => self
                    .editor
                    .create_token_from(target)
                    .map(|id| self.selected_token = id.or(self.selected_token)),
            };
            self.result(result);
        }
        if toggle_hidden {
            self.flush_properties();
            let result = self.editor.toggle_hidden();
            self.result(result);
        }
        if count == 0 {
            return;
        }
        self.arrange_section(ui);
        self.layer_actions(ui);
    }

    /// Align and distribute controls. Disabled controls state the requirement.
    fn arrange_section(&mut self, ui: &mut egui::Ui) {
        let arrangeable = self.editor.arrangeable().len();
        inspector_heading(ui, "Arrange", true, false);
        let mut align = None;
        let mut distribute = None;
        inspector_body(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(SPACE_XS, SPACE_S);
            let reason = match arrangeable {
                0 => "Hidden and locked layers are not arranged",
                1 => "One layer aligns to the artboard",
                _ => "Layers align to their shared bounds",
            };
            ui.horizontal(|ui| {
                for (glyph, label, value) in [
                    (icon::ALIGN_LEFT, "Align left · ⌥A", Align::Left),
                    (
                        icon::ALIGN_CENTER_HORIZONTAL,
                        "Align horizontal centers · ⌥H",
                        Align::CenterX,
                    ),
                    (icon::ALIGN_RIGHT, "Align right · ⌥D", Align::Right),
                    (icon::ALIGN_TOP, "Align top · ⌥W", Align::Top),
                    (
                        icon::ALIGN_CENTER_VERTICAL,
                        "Align vertical centers · ⌥V",
                        Align::CenterY,
                    ),
                    (icon::ALIGN_BOTTOM, "Align bottom · ⌥S", Align::Bottom),
                ] {
                    let label = format!("{label}\n{reason}");
                    if ui::icon_button(
                        ui,
                        glyph,
                        vec2(ROW, ROW),
                        ICON,
                        false,
                        arrangeable >= 1,
                        &label,
                    )
                    .clicked()
                    {
                        align = Some(value);
                    }
                }
            });
            ui.horizontal(|ui| {
                for (glyph, label, axis) in [
                    (
                        icon::COLUMNS,
                        "Distribute horizontally · ⌃⌥H",
                        Axis::Horizontal,
                    ),
                    (icon::ROWS, "Distribute vertically · ⌃⌥V", Axis::Vertical),
                ] {
                    let label = if arrangeable >= 3 {
                        format!("{label}\nEqual gaps; the outer layers stay in place")
                    } else {
                        format!("{label}\nNeeds three or more visible, unlocked layers")
                    };
                    if ui::icon_button(
                        ui,
                        glyph,
                        vec2(ROW, ROW),
                        ICON,
                        false,
                        arrangeable >= 3,
                        &label,
                    )
                    .clicked()
                    {
                        distribute = Some(axis);
                    }
                }
            });
        });
        if align.is_some() || distribute.is_some() {
            self.arrange(align, distribute);
        }
    }

    /// Order, duplicate, delete, visibility and lock for the selection.
    fn layer_actions(&mut self, ui: &mut egui::Ui) {
        let selected: Vec<_> = self
            .editor
            .document
            .nodes
            .iter()
            .filter(|n| self.editor.is_selected(n.id))
            .map(|n| (n.locked, n.hidden))
            .collect();
        if selected.is_empty() {
            return;
        }
        let single = selected.len() == 1;
        let any_unlocked = selected.iter().any(|(locked, _)| !locked);
        let all_locked = !any_unlocked;
        let any_hidden = selected.iter().any(|(_, hidden)| *hidden);
        inspector_heading(ui, if single { "Layer" } else { "Layers" }, true, false);
        inspector_body(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let action = |ui: &mut egui::Ui, glyph: &str, label: &str, enabled: bool| {
                    ui::icon_button(ui, glyph, vec2(ROW, ROW), ICON, false, enabled, label)
                        .clicked()
                };
                let order = single && any_unlocked;
                let order_label = |label: &str| {
                    if single {
                        label.to_string()
                    } else {
                        format!("{label} · select one layer")
                    }
                };
                if action(ui, icon::ARROW_UP, &order_label("Bring forward"), order) {
                    self.flush_properties();
                    let r = self.editor.reorder(true);
                    self.result(r);
                }
                if action(ui, icon::ARROW_DOWN, &order_label("Send backward"), order) {
                    self.flush_properties();
                    let r = self.editor.reorder(false);
                    self.result(r);
                }
                if action(ui, icon::COPY, "Duplicate unlocked layers", any_unlocked) {
                    self.flush_properties();
                    let r = self.editor.duplicate();
                    self.result(r);
                }
                if action(ui, icon::TRASH, "Delete unlocked layers", any_unlocked) {
                    self.flush_properties();
                    let r = self.editor.delete();
                    self.result(r);
                }
                let toggle = |ui: &mut egui::Ui, glyph, label, on| {
                    ui::icon_button(ui, glyph, vec2(ROW, ROW), ICON, on, true, label).clicked()
                };
                let hide_label = if any_hidden { "Show" } else { "Hide" };
                let hide_glyph = if any_hidden {
                    icon::EYE_SLASH
                } else {
                    icon::EYE
                };
                let lock_label = if all_locked { "Unlock" } else { "Lock" };
                let lock_glyph = if all_locked {
                    icon::LOCK
                } else {
                    icon::LOCK_OPEN
                };
                if toggle(ui, hide_glyph, hide_label, any_hidden) {
                    self.flush_properties();
                    let r = self.editor.toggle_hidden();
                    self.result(r);
                }
                if toggle(ui, lock_glyph, lock_label, all_locked) {
                    self.flush_properties();
                    let r = self.editor.toggle_locked();
                    self.result(r);
                }
            })
        });
    }

    /// Bounds and layers that resize handles act on: the visible, unlocked selected
    /// layers. A single layer resizes alone; several scale together.
    fn resize_target(&self) -> Option<(Frame, IdSet)> {
        let frame = self.editor.selection_bounds()?;
        Some((frame, self.editor.arrangeable().into_iter().collect()))
    }

    fn start_drag(&mut self, start: Pos2, nodes: IdSet, handle: Option<Handle>, frame: Frame) {
        let document = &self.editor.document;
        let indices = (document.nodes.iter().enumerate())
            .filter(|(_, n)| nodes.contains(&n.id))
            .map(|(index, _)| index)
            .collect();
        let others = (document.nodes.iter())
            .filter(|n| !n.hidden && !nodes.contains(&n.id))
            .map(node_frame);
        let snapper = Snapper::new([0.0, 0.0, document.width, document.height], others);
        self.drag = Some(Drag {
            before: document.clone(),
            start,
            nodes,
            indices,
            handle,
            frame,
            snapper,
        });
    }

    /// Moves or resizes the gesture's layers for the pointer at `pointer` and returns
    /// the guides of any snapped edges or centers.
    fn apply_drag(
        &mut self,
        drag: &Drag,
        pointer: Pos2,
        snapping: bool,
        keep_aspect: bool,
    ) -> Vec<Guide> {
        let delta = (pointer - drag.start) / self.zoom;
        let threshold = SNAP_DISTANCE / self.zoom;
        let snapper = snapping.then_some(&drag.snapper);
        let nodes = &mut self.editor.document.nodes;
        let frame = if let Some(handle) = drag.handle {
            let to = snap::resize_frame(
                drag.frame,
                handle,
                [delta.x, delta.y],
                keep_aspect,
                snapper.map(|s| (s, threshold)),
            );
            for &index in &drag.indices {
                let original = &drag.before.nodes[index];
                let Some(node) = nodes.get_mut(index).filter(|n| n.id == original.id) else {
                    continue;
                };
                let [left, top, right, bottom] =
                    snap::map_rect(node_frame(original), drag.frame, to);
                node.x = left.clamp(-100_000.0, 100_000.0);
                node.y = top.clamp(-100_000.0, 100_000.0);
                node.width = (right - left).clamp(1.0, 100_000.0);
                node.height = (bottom - top).clamp(1.0, 100_000.0);
            }
            to
        } else {
            let mut offset = [delta.x.round(), delta.y.round()];
            if let Some(snapper) = snapper {
                let snapped = snapper.snap_move(drag.frame, [delta.x, delta.y], threshold);
                for (offset, snapped) in offset.iter_mut().zip(snapped) {
                    *offset = snapped.unwrap_or(*offset);
                }
            }
            let [dx, dy] = drag.before.clamp_translation(&drag.nodes, offset);
            for &index in &drag.indices {
                let original = &drag.before.nodes[index];
                let Some(node) = nodes.get_mut(index).filter(|n| n.id == original.id) else {
                    continue;
                };
                node.x = original.x + dx;
                node.y = original.y + dy;
            }
            let [l, t, r, b] = drag.frame;
            [l + dx, t + dy, r + dx, b + dy]
        };
        snapper.map_or_else(Vec::new, |s| s.guides(frame))
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let canvas = response.rect;
        if self.fit {
            // Leave a 64 pt margin around the fitted artboard.
            let margin = 2.0 * ui::SPACE_3XL;
            self.zoom = ((canvas.width() - margin) / self.editor.document.width)
                .min((canvas.height() - margin) / self.editor.document.height)
                .clamp(0.05, 2.0);
            self.pan = Vec2::ZERO;
            self.fit = false;
        }
        let size = vec2(self.editor.document.width, self.editor.document.height);
        if response.hovered() {
            let (scroll, zoom_delta, pointer, modifiers) = ui.input(|i| {
                (
                    i.smooth_scroll_delta,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                    i.modifiers,
                )
            });
            if (modifiers.command || modifiers.ctrl) && scroll.y != 0.0
                || (zoom_delta - 1.0).abs() > 0.001
            {
                let old_zoom = self.zoom;
                self.zoom = (self.zoom
                    * if (zoom_delta - 1.0).abs() > 0.001 {
                        zoom_delta
                    } else {
                        (scroll.y * 0.005).exp()
                    })
                .clamp(0.05, 4.0);
                if let Some(pointer) = pointer {
                    self.pan = self.pan * (self.zoom / old_zoom)
                        + (pointer - canvas.center()) * (1.0 - self.zoom / old_zoom);
                }
            } else {
                self.pan += scroll;
            }
        }
        let panning = self.tool == Tool::Hand
            || ui.input(|i| {
                i.key_down(Key::Space) || i.pointer.button_down(egui::PointerButton::Middle)
            });
        if response.dragged() && panning {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(if panning {
                if response.dragged() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                }
            } else if self.tool.creates().is_some() {
                egui::CursorIcon::Crosshair
            } else {
                egui::CursorIcon::Default
            });
        }
        let origin = canvas.center() - size * self.zoom / 2.0 + self.pan;
        let artboard = Rect::from_min_size(origin, size * self.zoom);
        painter.text(
            artboard.min - vec2(0.0, SPACE_M),
            Align2::LEFT_CENTER,
            format!(
                "{}   {} × {}",
                self.editor.document.name, self.editor.document.width, self.editor.document.height
            ),
            FontId::proportional(CAPTION),
            TEXT_DIM,
        );
        if let Some(style) = self.editor.selected_node().and_then(TextStyle::of) {
            self.text_style = style;
        }
        self.sync_text_style();
        paint_document(
            &painter,
            &self.editor.document,
            origin,
            self.zoom,
            canvas,
            false,
            self.text_edit.as_ref().map(|e| e.id),
        );
        let screen = |n: &crate::document::Node| {
            Rect::from_min_size(
                origin + vec2(n.x, n.y) * self.zoom,
                vec2(n.width, n.height) * self.zoom,
            )
        };
        let multiple = self.editor.selection().len() > 1;
        let outlines = self
            .editor
            .document
            .nodes
            .iter()
            .filter(|n| self.editor.is_selected(n.id) && !n.hidden)
            .map(screen)
            .filter(|r| r.intersects(canvas));
        painter.add(outline_mesh(
            outlines,
            if multiple { 1.0 } else { 1.5 },
            ACCENT,
            ui.ctx().pixels_per_point(),
        ));
        if multiple && let Some([left, top, right, bottom]) = self.editor.selection_bounds() {
            let bounds = Rect::from_min_max(
                origin + vec2(left, top) * self.zoom,
                origin + vec2(right, bottom) * self.zoom,
            );
            // On the bounds themselves, where the group's resize handles sit.
            painter.rect_stroke(
                bounds,
                0.0,
                Stroke::new(1.0, ACCENT.gamma_multiply(0.7)),
                StrokeKind::Outside,
            );
            painter.text(
                bounds.center_bottom() + vec2(0.0, SPACE_L),
                Align2::CENTER_CENTER,
                format!("{:.0} × {:.0}", right - left, bottom - top),
                FontId::proportional(CAPTION),
                ACCENT,
            );
        }
        let zoom = self.zoom;
        let to_screen = move |frame: Frame| {
            Rect::from_min_max(
                origin + vec2(frame[0], frame[1]) * zoom,
                origin + vec2(frame[2], frame[3]) * zoom,
            )
        };
        let target = self.resize_target();
        if let Some((frame, _)) = &target {
            let rect = to_screen(*frame);
            for handle in Handle::ALL.iter().filter(|h| h.is_corner()) {
                let [x, y] = handle.point(*frame);
                let center = origin + vec2(x, y) * self.zoom;
                let square = Rect::from_center_size(center, vec2(SPACE_S, SPACE_S));
                painter.rect_filled(square, 1.0, SHELL);
                painter.rect_stroke(square, 1.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
            if self.tool == Tool::Select
                && response.hovered()
                && self.drag.is_none()
                && let Some(handle) = ui
                    .input(|i| i.pointer.hover_pos())
                    .and_then(|p| handle_at(rect, p))
            {
                ui.ctx().set_cursor_icon(resize_cursor(handle));
            }
            if let Some(node) = self.editor.selected_node() {
                painter.text(
                    rect.center_bottom() + vec2(0.0, SPACE_L),
                    Align2::CENTER_CENTER,
                    format!("{:.0} × {:.0}", node.width, node.height),
                    FontId::proportional(CAPTION),
                    ACCENT,
                );
            }
        }
        let to_document = move |p: Pos2| ((p - origin) / zoom).to_pos2();
        if !panning
            && self.pending.is_none()
            && let Some(kind) = self.tool.creates()
        {
            let pointer = response.interact_pointer_pos();
            if response.drag_started()
                && let Some(press) = ui.input(|i| i.pointer.press_origin())
            {
                if artboard.contains(press) {
                    self.flush_properties();
                    self.creation = Some(Creation {
                        kind,
                        start: to_document(press),
                        current: to_document(press),
                    });
                } else {
                    self.status = "Start inside the artboard to add a layer.".into();
                }
            }
            if let (Some(creation), Some(pointer)) = (&mut self.creation, pointer) {
                creation.current = to_document(pointer);
            }
            let modifiers = ui.input(|i| i.modifiers);
            // A click, or a drag too small to see, places a layer at the pointer.
            let finished = if response.clicked() {
                pointer.and_then(|p| {
                    if artboard.contains(p) {
                        Some((kind, to_document(p), None))
                    } else {
                        self.status = "Click inside the artboard to add a layer.".into();
                        None
                    }
                })
            } else if response.drag_stopped() {
                self.creation.take().map(|c| {
                    let rect = creation_rect(c.start, c.current, modifiers.shift, modifiers.alt);
                    let tiny = rect.width() * self.zoom < 4.0 && rect.height() * self.zoom < 4.0;
                    (c.kind, c.start, (!tiny).then_some(rect))
                })
            } else {
                None
            };
            if let Some((kind, at, rect)) = finished {
                self.flush_properties();
                self.set_tool(Tool::Select);
                match kind {
                    NodeKind::Rectangle => {
                        let rect = rect.unwrap_or(Rect::from_min_size(at, CLICK_SIZE));
                        let result = self.editor.add_at(
                            kind,
                            rect.min.x,
                            rect.min.y,
                            rect.width().max(1.0),
                            rect.height().max(1.0),
                        );
                        self.result(result);
                    }
                    NodeKind::Text => self.place_text(&painter, at, rect),
                }
            }
            if let Some(creation) = &self.creation {
                let rect = creation_rect(
                    creation.start,
                    creation.current,
                    modifiers.shift,
                    modifiers.alt,
                );
                let preview = Rect::from_min_max(
                    origin + rect.min.to_vec2() * self.zoom,
                    origin + rect.max.to_vec2() * self.zoom,
                );
                painter.rect_filled(preview, 0.0, ACCENT.gamma_multiply(0.12));
                painter.rect_stroke(preview, 0.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
        } else if !panning && self.pending.is_none() {
            let pointer = response.interact_pointer_pos();
            // Shift adds to or removes from the selection. A drag uses the modifiers of
            // its press, so Shift pressed during the drag only constrains the resize.
            if ui.input(|i| i.pointer.primary_pressed()) {
                self.press_shift = ui.input(|i| i.modifiers.shift);
            }
            let extend = if response.drag_started() {
                self.press_shift
            } else {
                ui.input(|i| i.modifiers.shift)
            };
            if (response.clicked() || response.drag_started())
                && let Some(pointer) = pointer
            {
                self.flush_properties();
                let hit_point = if response.drag_started() {
                    ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer)
                } else {
                    pointer
                };
                let handle =
                    (!extend)
                        .then_some(target.as_ref())
                        .flatten()
                        .and_then(|(frame, ids)| {
                            Some((handle_at(to_screen(*frame), hit_point)?, *frame, ids))
                        });
                let hit = || {
                    if !artboard.contains(hit_point) {
                        return None;
                    }
                    let point = to_document(hit_point);
                    self.editor
                        .document
                        .nodes
                        .iter()
                        .rev()
                        .find(|n| {
                            !n.hidden
                                && !n.locked
                                && Rect::from_min_size(pos2(n.x, n.y), vec2(n.width, n.height))
                                    .contains(point)
                        })
                        .map(|n| n.id)
                };
                if let Some((handle, frame, ids)) = handle {
                    // A click on a handle keeps the selection.
                    if response.drag_started() {
                        let ids = ids.clone();
                        self.start_drag(hit_point, ids, Some(handle), frame);
                    }
                } else {
                    match hit() {
                        Some(id) if response.drag_started() => {
                            // Dragging a selected layer moves the whole selection.
                            if !self.editor.is_selected(id) {
                                if extend {
                                    self.editor.toggle_selection(id);
                                } else {
                                    self.editor.select(Some(id));
                                }
                            }
                            let ids: IdSet = self.editor.arrangeable().into_iter().collect();
                            if let Some(frame) = self.editor.selection_bounds() {
                                self.start_drag(hit_point, ids, None, frame);
                            }
                        }
                        Some(id) if extend => self.editor.toggle_selection(id),
                        Some(id) => self.editor.select(Some(id)),
                        None if response.drag_started() => {
                            // Dragging on empty canvas selects with a marquee.
                            let base = if extend {
                                self.editor.selection().to_vec()
                            } else {
                                Vec::new()
                            };
                            self.editor.set_selection(base.iter().copied());
                            self.marquee = Some(Marquee {
                                start: to_document(hit_point),
                                current: to_document(hit_point),
                                base,
                            });
                        }
                        None if !extend => self.editor.select(None),
                        None => {}
                    }
                }
            }
            // Double-clicking a text layer edits it in place.
            if response.double_clicked()
                && let Some(pointer) = pointer
                && artboard.contains(pointer)
            {
                let point = to_document(pointer);
                let hit = self.editor.document.nodes.iter().rev().find(|n| {
                    !n.hidden
                        && !n.locked
                        && Rect::from_min_size(pos2(n.x, n.y), vec2(n.width, n.height))
                            .contains(point)
                });
                if let Some(id) = hit.filter(|n| n.kind == NodeKind::Text).map(|n| n.id) {
                    self.begin_text_edit(id, false);
                }
            }
            if (response.dragged() || response.drag_stopped())
                && let Some(pointer) = pointer
            {
                let snapping = self.snapping && !ui.input(|i| i.modifiers.command);
                let keep_aspect = ui.input(|i| i.modifiers.shift);
                if let Some(drag) = self.drag.take() {
                    let guides = self.apply_drag(&drag, pointer, snapping, keep_aspect);
                    paint_guides(&painter, &guides, origin, self.zoom);
                    self.drag = Some(drag);
                }
                if let Some(marquee) = &mut self.marquee {
                    marquee.current = to_document(pointer);
                    let area = Rect::from_two_pos(marquee.start, marquee.current);
                    let area = [area.min.x, area.min.y, area.max.x, area.max.y];
                    let inside: Vec<u64> = self
                        .editor
                        .document
                        .nodes
                        .iter()
                        .filter(|n| !n.hidden && !n.locked && snap::intersects(node_frame(n), area))
                        .map(|n| n.id)
                        .collect();
                    let ids = marquee.base.iter().copied().chain(inside);
                    self.editor.set_selection(ids);
                }
            }
            if let Some(marquee) = &self.marquee {
                let area = Rect::from_two_pos(
                    origin + marquee.start.to_vec2() * self.zoom,
                    origin + marquee.current.to_vec2() * self.zoom,
                );
                painter.rect_filled(area, 0.0, ACCENT.gamma_multiply(0.08));
                painter.rect_stroke(area, 0.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
        }
        self.text_overlay(ui, origin);
        if response.drag_stopped() {
            self.creation = None;
            self.marquee = None;
            if let Some(drag) = self.drag.take() {
                let result = self.editor.commit(drag.before);
                self.result(result);
            }
        }
    }

    /// Shows the in-place text editor over the edited layer and keeps the layer sized
    /// to its content. Ends the edit when the editor loses focus.
    fn text_overlay(&mut self, ui: &mut egui::Ui, origin: Pos2) {
        let Some(edit) = &mut self.text_edit else {
            return;
        };
        let document = &mut self.editor.document;
        let Some(index) = document.nodes.iter().position(|n| n.id == edit.id) else {
            self.text_edit = None;
            return;
        };
        let fill = color(document.node_fill(&document.nodes[index]));
        let node = &mut document.nodes[index];
        let id = egui::Id::new(("canvas_text", self.document_key, node.id));
        // egui takes focus from a widget that is new on a frame with a click, so focus
        // waits for a frame without one.
        if edit.start && !ui.input(|i| i.pointer.any_click()) {
            edit.start = false;
            // Each inline edit starts with its own widget undo history; earlier edits of
            // this layer are reached through document undo.
            let mut state = egui::text_edit::TextEditState::default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(node.text.chars().count()),
                )));
            state.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }
        let zoom = self.zoom;
        let rect = Rect::from_min_size(
            origin + vec2(node.x, node.y) * zoom,
            vec2(node.width, node.height) * zoom,
        );
        let response = ui.put(
            rect,
            egui::TextEdit::multiline(&mut node.text)
                .id(id)
                .font(FontId::proportional(node.font_size * zoom))
                .text_color(fill)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .desired_width(rect.width())
                .desired_rows(1),
        );
        if response.changed() {
            if node.text.len() > MAX_TEXT_BYTES {
                let mut end = MAX_TEXT_BYTES;
                while !node.text.is_char_boundary(end) {
                    end -= 1;
                }
                node.text.truncate(end);
                self.status = "Text is limited to 100,000 bytes".into();
            }
            let painter = ui.painter();
            if edit.auto_width {
                // Content wider than the largest layer wraps at that width instead.
                let size = text_size(painter, &node.text, node.font_size, None);
                node.width = size.x.clamp(1.0, MAX_COORDINATE);
                node.height = if size.x > MAX_COORDINATE {
                    text_size(painter, &node.text, node.font_size, Some(MAX_COORDINATE)).y
                } else {
                    size.y
                };
            } else {
                let size = text_size(painter, &node.text, node.font_size, Some(node.width));
                node.height = node.height.max(size.y);
            }
            // Taller content overflows the layer at the largest height.
            node.height = node.height.clamp(1.0, MAX_COORDINATE);
        }
        if !edit.start && !response.has_focus() {
            self.flush_properties();
        }
    }
}

/// Size in artboard units of `text` at `font_size`, wrapped at `width` when given.
fn text_size(painter: &Painter, text: &str, font_size: f32, width: Option<f32>) -> Vec2 {
    let font = FontId::proportional(font_size);
    let galley = match width {
        Some(width) => painter.layout(text.into(), font, Color32::WHITE, width),
        None => painter.layout_no_wrap(text.into(), font, Color32::WHITE),
    };
    let size = galley.size().ceil();
    match width {
        Some(_) => size,
        None => size + vec2(AUTO_WIDTH_SLACK, 0.0),
    }
}

fn node_frame(node: &Node) -> Frame {
    [node.x, node.y, node.x + node.width, node.y + node.height]
}

/// Resize handle under `pointer` for the screen bounds `rect`. Corners take a radius
/// that shrinks with small layers so they stay movable; edges respond along their
/// length once long enough to leave room for both corners.
fn handle_at(rect: Rect, pointer: Pos2) -> Option<Handle> {
    let corner = (rect.width().min(rect.height()) / 3.0).clamp(4.0, 12.0);
    let frame = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    let mut handles = Handle::ALL.iter().copied();
    if let Some(handle) = handles.by_ref().take(4).find(|h| {
        let [x, y] = h.point(frame);
        pointer.distance(pos2(x, y)) < corner
    }) {
        return Some(handle);
    }
    handles.find(|h| {
        let [x, y] = h.point(frame);
        if h.x == Side::Fixed {
            rect.width() >= 3.0 * corner
                && (pointer.y - y).abs() < 5.0
                && (rect.min.x + corner..=rect.max.x - corner).contains(&pointer.x)
        } else {
            rect.height() >= 3.0 * corner
                && (pointer.x - x).abs() < 5.0
                && (rect.min.y + corner..=rect.max.y - corner).contains(&pointer.y)
        }
    })
}

fn resize_cursor(handle: Handle) -> egui::CursorIcon {
    match (handle.x, handle.y) {
        (Side::Fixed, _) => egui::CursorIcon::ResizeVertical,
        (_, Side::Fixed) => egui::CursorIcon::ResizeHorizontal,
        (x, y) if x == y => egui::CursorIcon::ResizeNwSe,
        _ => egui::CursorIcon::ResizeNeSw,
    }
}

/// Snapping guides, drawn across the snapped geometry and its targets.
fn paint_guides(painter: &Painter, guides: &[Guide], origin: Pos2, zoom: f32) {
    for guide in guides {
        let [a, b] = if guide.vertical {
            [pos2(guide.value, guide.low), pos2(guide.value, guide.high)]
        } else {
            [pos2(guide.low, guide.value), pos2(guide.high, guide.value)]
        };
        painter.line_segment(
            [origin + a.to_vec2() * zoom, origin + b.to_vec2() * zoom],
            Stroke::new(1.0, GUIDE),
        );
    }
}

/// Outside outlines of axis-aligned rectangles as one mesh. Large selections would
/// otherwise tessellate one anti-aliased stroke per layer every frame. Edges snap to
/// physical pixels, so the outlines stay crisp without feathering.
fn outline_mesh(
    rects: impl Iterator<Item = Rect>,
    width: f32,
    color: Color32,
    pixels_per_point: f32,
) -> egui::Mesh {
    let snap = |v: f32| (v * pixels_per_point).round() / pixels_per_point;
    let mut mesh = egui::Mesh::default();
    let mut quad = |min: Pos2, max: Pos2| {
        mesh.add_colored_rect(Rect::from_min_max(min, max), color);
    };
    for rect in rects {
        let inner = Rect::from_min_max(
            pos2(snap(rect.min.x), snap(rect.min.y)),
            pos2(snap(rect.max.x), snap(rect.max.y)),
        );
        let outer = inner.expand(width);
        quad(outer.min, pos2(outer.max.x, inner.min.y));
        quad(pos2(outer.min.x, inner.max.y), outer.max);
        quad(
            pos2(outer.min.x, inner.min.y),
            pos2(inner.min.x, inner.max.y),
        );
        quad(
            pos2(inner.max.x, inner.min.y),
            pos2(outer.max.x, inner.max.y),
        );
    }
    mesh
}

/// Existing files use their canonical path so aliases cannot open competing tabs.
fn document_path(path: PathBuf) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| std::path::absolute(&path).unwrap_or(path))
}

/// A window tab at most `max_width` wide. Document tabs pass their index in `closable`
/// and get a close button. Returns the tab response and the close button.
fn tab(
    ui: &mut egui::Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    closable: Option<usize>,
    max_width: f32,
) -> (egui::Response, Option<egui::Response>) {
    let text_x = SPACE_S + ICON_SMALL + SPACE_S;
    let close_space = if closable.is_some() { ROW } else { 0.0 };
    let text = ui::truncated(
        ui,
        label,
        BODY,
        TEXT,
        (max_width - text_x - SPACE_M - close_space).max(0.0),
    );
    let width = (text_x + text.size().x + SPACE_M + close_space).max(if closable.is_some() {
        120.0_f32.min(max_width)
    } else {
        0.0
    });
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    let hovered = ui.rect_contains_pointer(rect);
    if selected {
        ui.painter()
            .rect_filled(rect, ui::RADIUS, Color32::from_gray(57));
    } else if hovered {
        ui.painter().rect_filled(rect, ui::RADIUS, ui::HOVER);
    }
    let color = if selected || hovered { TEXT } else { TEXT_DIM };
    ui.painter().text(
        pos2(rect.left() + SPACE_S + ICON_SMALL / 2.0, rect.center().y),
        Align2::CENTER_CENTER,
        glyph,
        ui::icon_font(ICON_SMALL),
        color,
    );
    ui.painter().galley(
        pos2(rect.left() + text_x, rect.center().y - text.size().y / 2.0),
        text,
        color,
    );
    // The close control appears while its document tab is hovered.
    let close = closable.filter(|_| hovered).map(|index| {
        let button = Rect::from_center_size(
            pos2(rect.right() - SPACE_M, rect.center().y),
            vec2(SPACE_L, SPACE_L),
        );
        ui::icon_button_at(
            ui,
            button,
            egui::Id::new(("close_file_tab", index)),
            icon::X,
            ICON_SMALL,
            false,
            true,
            "Close file",
        )
    });
    (response, close)
}

/// Design/Theme switch. Returns `true` when the inactive segment was clicked.
fn segmented(ui: &mut egui::Ui, theme: bool) -> bool {
    let width = ui.available_width() - 2.0 * SPACE_S;
    ui.horizontal(|ui| {
        ui.add_space(SPACE_S);
        let (rect, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
        ui.painter()
            .rect_filled(rect, ui::RADIUS, Color32::from_gray(53));
        let (left, right) = rect.split_left_right_at_fraction(0.5);
        let selected = if theme { right } else { left }.shrink(1.0);
        ui.painter()
            .rect_filled(selected, ui::RADIUS, Color32::from_gray(63));
        ui.painter().rect_stroke(
            selected,
            ui::RADIUS,
            Stroke::new(1.0, Color32::from_gray(72)),
            StrokeKind::Inside,
        );
        let mut switch = false;
        for (half, label, active) in [(left, "Design", !theme), (right, "Theme", theme)] {
            let response = ui.interact(half, egui::Id::new(("segment", label)), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
            });
            let color = if active || response.hovered() {
                TEXT
            } else {
                TEXT_DIM
            };
            ui.painter().text(
                half.center(),
                Align2::CENTER_CENTER,
                label,
                ui::font(BODY),
                color,
            );
            switch |= response.clicked() && !active;
        }
        switch
    })
    .inner
}

/// Color control with token binding. Unbound: the literal color field plus a token
/// menu. Bound: the token's swatch and name with a detach button; the color is edited
/// through the token in the Theme tab. Returns `true` when a literal edit finished;
/// binding changes are reported through `choice`.
fn color_property(
    ui: &mut egui::Ui,
    id: egui::Id,
    tokens: &[ColorToken],
    literal: &mut [u8; 4],
    binding: Option<u64>,
    target: ColorTarget,
    choice: &mut Option<TokenChoice>,
) -> bool {
    let mut finished = false;
    let enabled = ui.is_enabled();
    let width = ui.available_width();
    let bound = binding.and_then(|id| tokens.iter().find(|t| t.id == id));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let field_width = width - 48.0;
        ui.allocate_ui_with_layout(
            vec2(field_width, ROW),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(field_width);
                match bound {
                    Some(token) => {
                        ui::field_frame().show(ui, |ui| {
                            ui.set_width(field_width - SPACE_L);
                            ui.set_height(ROW);
                            ui.horizontal_centered(|ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_S;
                                let (rect, _) = ui.allocate_exact_size(
                                    vec2(ICON_SMALL, ICON_SMALL),
                                    Sense::hover(),
                                );
                                swatch(ui.painter(), rect, token.value);
                                // Leave room for the spacing and the 16 pt detach button.
                                let name_width =
                                    (ui.available_width() - SPACE_L - SPACE_S).max(0.0);
                                let galley = ui::truncated(ui, &token.name, BODY, TEXT, name_width);
                                let (rect, _) = ui.allocate_exact_size(
                                    vec2(name_width, ROW - SPACE_S),
                                    Sense::hover(),
                                );
                                ui.painter().galley(
                                    pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
                                    galley,
                                    TEXT,
                                );
                                if ui::icon_button(
                                    ui,
                                    icon::LINK_BREAK,
                                    vec2(SPACE_L, SPACE_L),
                                    ICON_SMALL,
                                    false,
                                    enabled,
                                    "Detach token · keeps the current color",
                                )
                                .clicked()
                                {
                                    *choice = Some(TokenChoice::Bind(target, None));
                                }
                            });
                        });
                    }
                    None => finished = ui::color_field(ui, id, literal),
                }
            },
        );
        let right = ui.max_rect().right();
        let y = ui.min_rect().center().y;
        ui::icon_button_at(
            ui,
            trailing_slot(right, y, 0),
            id.with("eyedropper"),
            icon::EYEDROPPER,
            16.0,
            false,
            false,
            "Picking a color from the screen is not available yet",
        );
        let menu = ui::icon_button_at(
            ui,
            trailing_slot(right, y, 1),
            id.with("token_menu"),
            icon::CIRCLES_FOUR,
            16.0,
            bound.is_some(),
            enabled,
            "Color tokens",
        );
        egui::Popup::menu(&menu).id(id.with("tokens")).show(|ui| {
            ui.set_min_width(208.0);
            if tokens.is_empty() {
                ui.label(RichText::new("No color tokens yet").color(TEXT_DIM));
            }
            for token in tokens {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ICON_SMALL, ICON_SMALL), Sense::hover());
                    swatch(ui.painter(), rect, token.value);
                    let current = binding == Some(token.id);
                    if ui
                        .add(egui::Button::selectable(current, &token.name))
                        .clicked()
                    {
                        if !current {
                            *choice = Some(TokenChoice::Bind(target, Some(token.id)));
                        }
                        ui.close();
                    }
                });
            }
            ui.separator();
            if ui.button("Create token from this color").clicked() {
                *choice = Some(TokenChoice::CreateFrom(target));
                ui.close();
            }
            if bound.is_some() && ui.button("Detach token").clicked() {
                *choice = Some(TokenChoice::Bind(target, None));
                ui.close();
            }
        });
    });
    finished
}

/// Color swatch with a faint outline so light and transparent colors stay visible.
fn swatch(painter: &Painter, rect: Rect, value: [u8; 4]) {
    painter.rect_filled(rect, 2, color(value));
    painter.rect_stroke(
        rect,
        2,
        Stroke::new(1.0, Color32::from_white_alpha(46)),
        StrokeKind::Inside,
    );
}

/// Horizontal inset of inspector headings and controls.
const INSPECTOR_PAD: f32 = 12.0;

/// Inspector section heading. Open sections leave room above their controls;
/// `active` false draws the dim title of a collapsed, empty section.
fn inspector_heading(ui: &mut egui::Ui, title: &str, active: bool, caret: bool) -> egui::Response {
    let height = if active { 32.0 } else { ROW };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let y = rect.top() + ROW / 2.0;
    let color = if active { TEXT } else { TEXT_DIM };
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), ui::font(BODY), color);
    let width = galley.size().x;
    ui.painter().galley(
        pos2(rect.left() + INSPECTOR_PAD, y - galley.size().y / 2.0),
        galley,
        color,
    );
    if caret {
        ui.painter().text(
            pos2(rect.left() + INSPECTOR_PAD + width + SPACE_S, y),
            Align2::CENTER_CENTER,
            icon::CARET_DOWN,
            ui::icon_font(12.0),
            TEXT_DIM,
        );
    }
    response
}

/// Inspector section controls, followed by the section rule.
fn inspector_body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: INSPECTOR_PAD as i8,
            right: INSPECTOR_PAD as i8,
            top: 0,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            add(ui)
        })
        .inner;
    ui::hline(ui);
    inner
}

/// Icon slot `index` counted from the right end of a heading.
fn heading_slot(heading: Rect, index: usize) -> Rect {
    trailing_slot(
        heading.right() - INSPECTOR_PAD,
        heading.top() + ROW / 2.0,
        index,
    )
}

/// 24 pt icon slot `index` counted leftwards from the content edge `right`.
fn trailing_slot(right: f32, y: f32, index: usize) -> Rect {
    Rect::from_center_size(pos2(right - 8.0 - 24.0 * index as f32, y), vec2(24.0, 24.0))
}

/// Fill type selector. Solid is the only supported type.
fn fill_kinds(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(3.0 * ROW + 2.0 * SPACE_XS, ROW), Sense::hover());
    let slot = |i: usize| {
        Rect::from_min_size(
            pos2(
                rect.left() + SPACE_XS + i as f32 * ROW,
                rect.top() + SPACE_XS,
            ),
            vec2(ROW, ROW - 2.0 * SPACE_XS),
        )
    };
    let painter = ui.painter();
    painter.rect_filled(rect, ui::RADIUS, CONTROL);
    painter.rect_filled(slot(0), 4, Color32::from_gray(74));
    let mark = |i: usize| Rect::from_center_size(slot(i).center(), vec2(ICON, ICON_SMALL));
    painter.rect_filled(mark(0), 2, Color32::from_gray(150));
    let gradient = mark(1);
    let mut mesh = egui::Mesh::default();
    for (corner, gray) in [
        (gradient.left_top(), 110),
        (gradient.left_bottom(), 110),
        (gradient.right_top(), 62),
        (gradient.right_bottom(), 62),
    ] {
        mesh.colored_vertex(corner, Color32::from_gray(gray));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(mesh);
    painter.text(
        slot(2).center(),
        Align2::CENTER_CENTER,
        icon::IMAGE,
        ui::icon_font(16.0),
        ui::TEXT_FAINT,
    );
    for (i, label) in [
        "Solid fill",
        "Gradient fills are not available yet",
        "Image fills are not available yet",
    ]
    .into_iter()
    .enumerate()
    {
        ui.interact(slot(i), egui::Id::new(("fill_kind", i)), Sense::hover())
            .on_hover_text(label);
    }
}

/// Collapsible panel heading with an optional trailing icon control
/// `(glyph, label, enabled)`. Returns the trailing control's response.
fn section_toggle(
    ui: &mut egui::Ui,
    title: &str,
    open: &mut bool,
    trailing: Option<(&str, &str, bool)>,
) -> Option<egui::Response> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, *open, title)
    });
    if response.clicked() {
        *open = !*open;
    }
    let hovered = response.hovered();
    ui.painter().text(
        pos2(rect.left() + SPACE_S + ICON_SMALL / 2.0, rect.center().y),
        Align2::CENTER_CENTER,
        if *open {
            icon::CARET_DOWN
        } else {
            icon::CARET_RIGHT
        },
        ui::icon_font(ICON_SMALL),
        if hovered { TEXT } else { TEXT_DIM },
    );
    ui.painter().text(
        pos2(
            rect.left() + SPACE_S + ICON_SMALL + SPACE_XS,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        title,
        ui::font(BODY),
        TEXT,
    );
    trailing.map(|(glyph, label, enabled)| {
        let slot = Rect::from_center_size(
            pos2(rect.right() - SPACE_S - SPACE_M, rect.center().y),
            vec2(ROW, ROW),
        );
        ui::icon_button_at(
            ui,
            slot,
            egui::Id::new(("section_control", title)),
            glyph,
            ICON_SMALL,
            false,
            enabled,
            label,
        )
    })
}

/// Hairline between tool rail groups.
fn rail_separator(ui: &mut egui::Ui) {
    ui.add_space(SPACE_S);
    let (rect, _) = ui.allocate_exact_size(vec2(SPACE_L, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
    ui.add_space(SPACE_S);
}

impl eframe::App for PaperCraftApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            self.flush_properties();
            if self.first_dirty_tab().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.request(Intent::Close, ctx);
            }
        }
        #[cfg(target_os = "macos")]
        craft_quit::on_close_request(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_switches_preserve_dirty_state_and_document_local_text_styles() {
        let mut app = PaperCraftApp::from_document(Document::example());
        assert!(!app.dirty());
        app.editor.add(NodeKind::Rectangle).unwrap();
        app.text_style = TextStyle {
            font_size: 44.0,
            fill_token: Some(1),
            ..Default::default()
        };
        assert!(app.dirty());
        app.open_tab(Document::default(), None);
        assert!(!app.dirty());
        assert_eq!(app.text_style, TextStyle::default());
        app.editor.add(NodeKind::Rectangle).unwrap();
        app.set_saved(app.editor.document.clone());
        assert!(!app.dirty());
        app.activate_tab(0);
        assert!(
            app.dirty(),
            "Equal revision numbers belong to different documents"
        );
        assert_eq!(app.text_style.font_size, 44.0);
        assert_eq!(app.text_style.fill_token, Some(1));
        app.activate_tab(1);
        assert!(!app.dirty());
        assert_eq!(app.text_style, TextStyle::default());
        assert!(app.close_document());
        assert!(app.dirty());
        assert!(!app.close_document());
    }

    #[test]
    fn live_text_edits_are_dirty_before_their_history_commit() {
        let mut app = PaperCraftApp::from_document(Document::example());
        let id = app
            .editor
            .document
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Text)
            .unwrap()
            .id;
        let original = app.editor.document.clone();
        assert!(!app.dirty());
        app.begin_text_edit(id, false);
        app.editor
            .document
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .unwrap()
            .text = "Unsaved typing".into();
        assert!(app.dirty());
        assert!(!app.close_document());
        app.editor.undo();
        assert_eq!(app.editor.document, original);
        assert!(!app.dirty());
    }

    #[test]
    fn files_open_once_and_other_tabs_cannot_replace_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("open.papercraft");
        let document = Document::example();
        document.save(&path).unwrap();
        let mut app = PaperCraftApp::from_document(Document::default());
        app.open_path(path.clone());
        let before = app.editor.document.clone();
        app.editor.document.name = "Unsaved file".into();
        app.editor.commit(before).unwrap();
        app.activate_tab(0);
        app.open_path(directory.path().join(".").join("open.papercraft"));
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.editor.document.name, "Unsaved file");
        assert!(app.editor.can_undo());

        app.activate_tab(0);
        assert!(!app.write_document(path.clone()));
        assert_eq!(Document::load(&path).unwrap(), document);
        app.activate_tab(1);
        assert!(app.write_document(path.clone()));
        assert_eq!(Document::load(&path).unwrap(), app.editor.document);
    }

    #[test]
    fn renames_survive_tab_switches_and_quit_checks_every_dirty_tab() {
        let ctx = egui::Context::default();
        let mut app = PaperCraftApp::from_document(Document::example());
        let id = app.editor.document.nodes[0].id;
        let original = app.editor.document.clone();
        app.renaming = Some((id, "Renamed before switching".into()));
        app.request(Intent::New, &ctx);
        assert_eq!(app.tab_count(), 2);
        app.activate_tab(0);
        assert_eq!(
            app.editor.document.nodes[0].name,
            "Renamed before switching"
        );
        app.editor.undo();
        assert_eq!(app.editor.document, original);

        app.renaming = Some((id, "Renamed before quit".into()));
        app.request(Intent::Close, &ctx);
        assert_eq!(app.pending, Some(Intent::Close));
        assert_eq!(app.editor.document.nodes[0].name, "Renamed before quit");
        app.pending = None;
        app.activate_tab(1);
        app.editor.add(NodeKind::Rectangle).unwrap();
        app.request(Intent::Close, &ctx);
        assert_eq!(app.active_tab(), 1);
        assert_eq!(app.pending, Some(Intent::Close));
        app.pending = None;
        app.perform(Intent::Close, &ctx);
        assert_eq!(app.active_tab(), 0);
        assert_eq!(app.pending, Some(Intent::Close));
        assert!(!app.allow_close);
        app.pending = None;
        app.perform(Intent::Close, &ctx);
        assert_eq!(app.tab_count(), 0);
        assert!(app.allow_close);
    }
}
