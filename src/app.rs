use crate::document::{
    Align, Axis, ColorTarget, ColorToken, Document, Editor, IdSet, Node, NodeKind, atomic_write,
};
use crate::recents::Recents;
use crate::ui::{self, ACCENT, BODY, BORDER, CONTROL, ROW, Row, SHELL, TEXT, TEXT_DIM};
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, Painter, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};
use egui_phosphor::regular as icon;
use std::path::PathBuf;

mod dashboard;

/// Left inset of the tab strip. On macOS the strip shares the title bar with the window
/// controls.
const TAB_INSET: f32 = if cfg!(target_os = "macos") { 80.0 } else { 8.0 };
/// Default geometry for a layer placed with a single click.
const CLICK_SIZE: Vec2 = vec2(220.0, 80.0);

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
    resize: bool,
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
    creation: Option<Creation>,
    property_before: Option<Document>,
    pending: Option<Intent>,
    allow_close: bool,
    help: bool,
    pub view: View,
    pub tool: Tool,
    document_open: bool,
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
    /// Format version of the file at `path` when it was opened or last saved.
    file_version: u32,
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
            creation: None,
            property_before: None,
            pending: None,
            allow_close: false,
            help: false,
            view: View::Editor,
            tool: Tool::Select,
            document_open: true,
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
            file_version: 1,
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
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
        ctx.set_visuals(egui::Visuals::dark());
        ctx.global_style_mut(|s| {
            s.spacing.item_spacing = vec2(8.0, 8.0);
            s.spacing.button_padding = vec2(10.0, 6.0);
            s.spacing.interact_size.y = ROW;
            s.spacing.menu_margin = egui::Margin::same(6);
            let v = &mut s.visuals;
            v.panel_fill = SHELL;
            v.window_fill = SHELL;
            v.window_stroke = Stroke::new(1.0, BORDER);
            v.window_corner_radius = egui::CornerRadius::same(10);
            v.menu_corner_radius = egui::CornerRadius::same(8);
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
                .insert(egui::TextStyle::Small, FontId::proportional(12.0));
            s.text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(17.0));
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

    /// Closes the document and shows the dashboard. Refuses (returns `false`) when the
    /// document has unsaved changes; interactive closing asks first instead.
    pub fn close_document(&mut self) -> bool {
        self.flush_properties();
        if self.dirty() {
            return false;
        }
        self.replace_document(Document::default(), None);
        self.document_open = false;
        self.view = View::Dashboard;
        self.cards.invalidate();
        self.status = "Document closed".into();
        true
    }

    fn dirty(&self) -> bool {
        if !self.document_open {
            return false;
        }
        // Uncommitted edits change the document without a new revision.
        if self.property_before.is_some() || self.drag.is_some() {
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

    fn result(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.status = error;
        }
    }

    fn flush_properties(&mut self) {
        if let Some(before) = self.property_before.take() {
            let result = self.editor.commit(before);
            self.result(result);
        }
    }

    fn remember(&mut self, path: &std::path::Path) {
        if let Err(error) = self.recents.touch(path) {
            self.status = format!("{} {error}", self.status);
        }
        self.cards.invalidate();
    }

    fn replace_document(&mut self, document: Document, path: Option<PathBuf>) {
        self.file_version = document.version;
        self.selected_token = None;
        self.upgrade_prompt = false;
        self.set_saved(document.clone());
        self.editor = Editor::new(document);
        self.path = path;
        self.fit = true;
        self.property_before = None;
        self.drag = None;
        self.creation = None;
        self.tool = Tool::Select;
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
        let upgrading = self.path.as_ref() == Some(&path)
            && self.file_version < self.editor.document.required_version();
        if upgrading {
            self.upgrade_prompt = true;
            return false;
        }
        self.write_document(path)
    }

    fn write_document(&mut self, path: PathBuf) -> bool {
        self.upgrade_prompt = false;
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
        match Document::load(&path) {
            Ok(document) => {
                self.remember(&path);
                self.replace_document(document, Some(path));
                self.document_open = true;
                self.view = View::Editor;
                self.status = "Document opened".into();
            }
            Err(error) => {
                self.status = if self.document_open {
                    format!("{error} Current document was kept.")
                } else {
                    error
                }
            }
        }
    }

    fn request(&mut self, intent: Intent, ctx: &egui::Context) {
        self.flush_properties();
        if let Intent::OpenPath(path) = &intent
            && self.document_open
            && self.path.as_ref() == Some(path)
        {
            self.view = View::Editor;
            return;
        }
        if self.dirty() {
            self.pending = Some(intent);
        } else {
            self.perform(intent, ctx);
        }
    }

    fn perform(&mut self, intent: Intent, ctx: &egui::Context) {
        match intent {
            Intent::New => {
                self.replace_document(Document::default(), None);
                self.document_open = true;
                self.view = View::Editor;
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
            Intent::CloseDocument => {
                // Unsaved changes were confirmed or discarded before reaching here.
                self.set_saved(self.editor.document.clone());
                self.close_document();
            }
            Intent::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn export(&mut self, format: ExportFormat) {
        self.flush_properties();
        let (label, extension) = match format {
            ExportFormat::Html => ("HTML", "html"),
            ExportFormat::Svg => ("SVG", "svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[extension])
            .set_file_name(format!("{}.{extension}", self.editor.document.name))
            .save_file()
        else {
            return;
        };
        let data = match format {
            ExportFormat::Html => self.editor.document.export_html(),
            ExportFormat::Svg => self.editor.document.export_svg(),
        };
        match data.and_then(|data| atomic_write(&path, data.as_bytes())) {
            Ok(()) => self.status = format!("Exported {}", path.display()),
            Err(error) => self.status = error,
        }
    }

    fn export_menu(&mut self, ui: &mut egui::Ui) {
        for (label, format) in [
            ("Export HTML…", ExportFormat::Html),
            ("Export SVG…", ExportFormat::Svg),
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
        self.tool = tool;
        self.creation = None;
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
            if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            } else {
                self.editor.select(None);
            }
        }
        if !ctx.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt) {
            for (key, tool) in [
                (Key::V, Tool::Select),
                (Key::H, Tool::Hand),
                (Key::R, Tool::Rectangle),
                (Key::T, Tool::Text),
            ] {
                if ctx.input(|i| i.key_pressed(key)) {
                    self.set_tool(tool);
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
        let finish_gesture_first = self.drag.is_some() || self.creation.is_some();
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
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(TAB_INSET);
            if tab(
                ui,
                icon::SQUARES_FOUR,
                "Dashboard",
                self.view == View::Dashboard,
                false,
            )
            .0
            .clicked()
            {
                self.flush_properties();
                self.view = View::Dashboard;
                self.cards.invalidate();
            }
            if self.document_open {
                let (rect, _) = ui.allocate_exact_size(vec2(9.0, 18.0), Sense::hover());
                ui.painter()
                    .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
                let name = self.editor.document.name.clone();
                let (response, close) = tab(ui, icon::FILE, &name, self.view == View::Editor, true);
                let dirty = self.dirty();
                if dirty {
                    // Unsaved marker before the close control.
                    ui.painter().circle_filled(
                        pos2(response.rect.right() - 34.0, response.rect.center().y),
                        3.0,
                        TEXT_DIM,
                    );
                }
                let response = response.on_hover_text(match (&self.path, dirty) {
                    (Some(path), true) => format!("{} · unsaved changes", path.display()),
                    (Some(path), false) => path.display().to_string(),
                    (None, _) => "Not saved yet".into(),
                });
                if close.is_some_and(|c| c.clicked()) {
                    self.request(Intent::CloseDocument, ctx);
                } else if response.clicked() {
                    self.view = View::Editor;
                }
            }
            ui.add_space(6.0);
            if ui::icon_button(
                ui,
                icon::PLUS,
                vec2(26.0, 26.0),
                17.0,
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
                .default_size(180.0)
                .size_range(180.0..=320.0)
                .resizable(true)
                .frame(side)
                .show(ui, |ui| self.left_panel(ui, ctx));
        }
        egui::Panel::left("tools")
            .exact_size(40.0)
            .resizable(false)
            .frame(side)
            .show(ui, |ui| self.tool_rail(ui));
        egui::Panel::right("inspector")
            .default_size(280.0)
            .size_range(260.0..=360.0)
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
        let (rect, response) = ui.allocate_exact_size(vec2(width, ROW + 4.0), Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "File menu"));
        if response.hovered() || egui::Popup::is_id_open(ctx, egui::Id::new("file_menu")) {
            ui.painter().rect_filled(rect, ui::RADIUS, ui::HOVER);
        }
        ui.painter().text(
            pos2(rect.left() + 12.0, rect.center().y),
            Align2::CENTER_CENTER,
            icon::COOKIE,
            ui::icon_font(17.0),
            TEXT_DIM,
        );
        let galley = ui::truncated(
            ui,
            &self.editor.document.name,
            15.0,
            TEXT,
            rect.width() - 36.0,
        );
        ui.painter().galley(
            pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0),
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
            vec2(width, 42.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                self.file_name_button(ui, ctx, width - 52.0);
                ui.add_space(4.0);
                if ui::icon_button(
                    ui,
                    icon::SIDEBAR_SIMPLE,
                    vec2(28.0, 28.0),
                    18.0,
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
        ui.add_space(14.0);
        if segmented(ui, self.theme) {
            self.flush_properties();
            self.theme = !self.theme;
        }
        ui.add_space(10.0);
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
                .indent(17.0)
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
                    let hovered = ui.rect_contains_pointer(rect);
                    let glyph = if kind == NodeKind::Text {
                        icon::TEXT_AA
                    } else {
                        icon::SQUARE
                    };
                    Row::new(glyph, &name)
                        .indent(17.0)
                        .selected(selected)
                        .dim(hidden)
                        .reserve(if hovered || hidden || locked {
                            48.0
                        } else {
                            0.0
                        })
                        .paint(ui, rect, &response, 8.0);
                    let mut toggle = |offset: f32, glyph: &str, label: &str| {
                        let button = Rect::from_center_size(
                            pos2(rect.right() - offset, rect.center().y),
                            vec2(22.0, 22.0),
                        );
                        ui::icon_button_at(
                            ui,
                            button,
                            egui::Id::new(("layer_toggle", id, offset as i32)),
                            glyph,
                            15.0,
                            false,
                            true,
                            label,
                        )
                        .clicked()
                    };
                    let mut changed = None;
                    if (hovered || hidden)
                        && toggle(
                            42.0,
                            if hidden { icon::EYE_SLASH } else { icon::EYE },
                            if hidden { "Show layer" } else { "Hide layer" },
                        )
                    {
                        changed = Some((true, !hidden));
                    }
                    if (hovered || locked)
                        && toggle(
                            20.0,
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
                    ui.add_space(8.0);
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(17, 0))
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
                ui.label(RichText::new(&self.status).size(12.0).color(TEXT_DIM));
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
                            .indent(42.0)
                            .selected(selected)
                            .paint(ui, rect, &response, 8.0);
                        swatch(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(rect.left() + 28.0, rect.center().y),
                                vec2(14.0, 14.0),
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
                                top: 6,
                                bottom: 10,
                            })
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                let name_id = egui::Id::new(("token_name", token.id));
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
                                                    .inner_margin(egui::Margin::symmetric(8, 6)),
                                            )
                                            .margin(egui::Margin::symmetric(8, 6))
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
                                        egui::Id::new(("token_value", token.id)),
                                        &mut live.value,
                                    );
                                }
                                ui.label(
                                    RichText::new(match usage {
                                        0 => "Not used".to_string(),
                                        1 => "Used by 1 property".to_string(),
                                        n => format!("Used by {n} properties"),
                                    })
                                    .size(12.0)
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
                            .inner_margin(egui::Margin::symmetric(17, 0))
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
        if finish || (!ctx.egui_wants_keyboard_input() && !ctx.egui_is_using_pointer()) {
            self.flush_properties();
        }
    }

    fn floating_file_header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let anchor = ui.max_rect().left_top() + vec2(10.0, 10.0);
        egui::Area::new(egui::Id::new("floating_file"))
            .fixed_pos(anchor)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(SHELL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            self.file_name_button(ui, ctx, 168.0);
                            if ui::icon_button(
                                ui,
                                icon::SIDEBAR_SIMPLE,
                                vec2(28.0, 28.0),
                                18.0,
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
        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            let groups: [&[(Tool, &str, &str)]; 2] = [
                &[
                    (Tool::Select, icon::CURSOR, "Select · V"),
                    (Tool::Hand, icon::HAND, "Hand · H"),
                ],
                &[
                    (Tool::Rectangle, icon::SQUARE, "Rectangle · R"),
                    (Tool::Text, icon::TEXT_AA, "Text · T"),
                ],
            ];
            for (index, group) in groups.into_iter().enumerate() {
                if index > 0 {
                    ui.add_space(10.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 1.0), Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, BORDER);
                    ui.add_space(10.0);
                }
                for &(tool, glyph, label) in group {
                    if ui::icon_button(
                        ui,
                        glyph,
                        vec2(32.0, 32.0),
                        20.0,
                        self.tool == tool,
                        true,
                        label,
                    )
                    .clicked()
                    {
                        self.set_tool(tool);
                    }
                }
            }
        });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(width, 49.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(12.0);
                let export = ui
                    .add(
                        egui::Button::new(RichText::new("Export").size(BODY).color(TEXT))
                            .fill(CONTROL)
                            .min_size(vec2(52.0, ROW)),
                    )
                    .on_hover_text("Export artboard as HTML or SVG");
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
                    ui.set_min_width(170.0);
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
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
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
        let tokens = self.editor.document.tokens.clone();
        let section = |ui: &mut egui::Ui, title: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
            ui::section_title(ui, title);
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: 0,
                    bottom: 14,
                })
                .show(ui, |ui| add(ui));
            ui::hline(ui);
        };
        // TextEdit sizes its text from `margin`, so the frame margin must match it.
        let text_margin = egui::Margin::symmetric(8, 6);
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
            section(ui, &format!("{count} layers selected"), &mut |ui| {
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
                        .size(12.0)
                        .color(TEXT_DIM),
                    );
                }
                ui.label(
                    RichText::new("Select one layer to edit its properties.")
                        .size(12.0)
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
            let kind = node.kind;
            let id = node.id;
            section(
                ui,
                if kind == NodeKind::Text {
                    "Text layer"
                } else {
                    "Rectangle"
                },
                &mut |ui| {
                    finish |= ui
                        .add(
                            egui::TextEdit::singleline(&mut node.name)
                                .frame(text_frame)
                                .margin(text_margin)
                                .desired_width(f32::INFINITY),
                        )
                        .on_hover_text("Layer name")
                        .lost_focus();
                },
            );
            ui.add_enabled_ui(!locked, |ui| {
                section(ui, "Position and size", &mut |ui| {
                    let half = (ui.available_width() - 8.0) / 2.0;
                    let range = -100_000.0..=100_000.0;
                    ui.horizontal(|ui| {
                        let r = ui::number_field(ui, "X", &mut node.x, range.clone(), half);
                        finish |= r.drag_stopped() || r.lost_focus();
                        let r = ui::number_field(ui, "Y", &mut node.y, range.clone(), half);
                        finish |= r.drag_stopped() || r.lost_focus();
                    });
                    ui.horizontal(|ui| {
                        let r = ui::number_field(ui, "W", &mut node.width, 1.0..=100_000.0, half);
                        finish |= r.drag_stopped() || r.lost_focus();
                        let r = ui::number_field(ui, "H", &mut node.height, 1.0..=100_000.0, half);
                        finish |= r.drag_stopped() || r.lost_focus();
                    });
                });
                section(
                    ui,
                    if kind == NodeKind::Text {
                        "Text color"
                    } else {
                        "Fill"
                    },
                    &mut |ui| {
                        finish |= color_property(
                            ui,
                            egui::Id::new("node_fill"),
                            &tokens,
                            &mut node.fill,
                            node.fill_token,
                            ColorTarget::Node(id),
                            &mut choice,
                        );
                        if kind == NodeKind::Rectangle {
                            let r = ui::number_field(
                                ui,
                                "Radius",
                                &mut node.radius,
                                0.0..=100_000.0,
                                ui.available_width(),
                            );
                            finish |= r.drag_stopped() || r.lost_focus();
                        }
                    },
                );
                if kind == NodeKind::Text {
                    section(ui, "Typography", &mut |ui| {
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
                }
            });
        } else {
            let document = &mut self.editor.document;
            section(ui, "Page", &mut |ui| {
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
            section(ui, "Artboard", &mut |ui| {
                finish |= ui
                    .add(
                        egui::TextEdit::singleline(&mut document.name)
                            .frame(text_frame)
                            .margin(text_margin)
                            .desired_width(f32::INFINITY),
                    )
                    .on_hover_text("Document name")
                    .lost_focus();
                let half = (ui.available_width() - 8.0) / 2.0;
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
                .inner_margin(egui::Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "Select a layer on the canvas or in the layer list to edit it. \
                             Shift-click adds layers to the selection.",
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
        if count == 0 {
            return;
        }
        self.arrange_section(ui);
        self.layer_actions(ui);
    }

    /// Align and distribute controls. Disabled controls state the requirement.
    fn arrange_section(&mut self, ui: &mut egui::Ui) {
        let arrangeable = self.editor.arrangeable().len();
        ui::section_title(ui, "Arrange");
        let mut align = None;
        let mut distribute = None;
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 10,
                right: 12,
                top: 0,
                bottom: 14,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 8.0);
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
                            vec2(30.0, 30.0),
                            18.0,
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
                            vec2(30.0, 30.0),
                            18.0,
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
        ui::hline(ui);
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
        ui::section_title(ui, if single { "Layer" } else { "Layers" });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(10.0);
            let action = |ui: &mut egui::Ui, glyph: &str, label: &str, enabled: bool| {
                ui::icon_button(ui, glyph, vec2(32.0, 30.0), 18.0, false, enabled, label).clicked()
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
                ui::icon_button(ui, glyph, vec2(32.0, 30.0), 18.0, on, true, label).clicked()
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
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let canvas = response.rect;
        if self.fit {
            self.zoom = ((canvas.width() - 110.0) / self.editor.document.width)
                .min((canvas.height() - 130.0) / self.editor.document.height)
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
            artboard.min - vec2(0.0, 14.0),
            Align2::LEFT_CENTER,
            format!(
                "{}   {} × {}",
                self.editor.document.name, self.editor.document.width, self.editor.document.height
            ),
            FontId::proportional(12.0),
            TEXT_DIM,
        );
        paint_document(
            &painter,
            &self.editor.document,
            origin,
            self.zoom,
            canvas,
            false,
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
            painter.rect_stroke(
                bounds.expand(3.0),
                0.0,
                Stroke::new(1.0, ACCENT.gamma_multiply(0.7)),
                StrokeKind::Outside,
            );
            painter.text(
                bounds.center_bottom() + vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                format!("{:.0} × {:.0}", right - left, bottom - top),
                FontId::proportional(11.0),
                ACCENT,
            );
        }
        if let Some(node) = self.editor.selected_node().filter(|n| !n.hidden) {
            let rect = screen(node);
            if !node.locked {
                let handle = Rect::from_center_size(rect.right_bottom(), vec2(7.0, 7.0));
                painter.rect_filled(handle, 1.0, SHELL);
                painter.rect_stroke(handle, 1.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                if response.hovered()
                    && self.tool == Tool::Select
                    && ui
                        .input(|i| i.pointer.hover_pos())
                        .is_some_and(|p| p.distance(rect.right_bottom()) < 12.0)
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
                painter.text(
                    rect.center_bottom() + vec2(0.0, 17.0),
                    Align2::CENTER_CENTER,
                    format!("{:.0} × {:.0}", node.width, node.height),
                    FontId::proportional(11.0),
                    ACCENT,
                );
            }
        }
        let zoom = self.zoom;
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
            let finished = if response.clicked() {
                pointer.and_then(|p| {
                    if artboard.contains(p) {
                        let start = to_document(p);
                        Some((kind, Rect::from_min_size(start, CLICK_SIZE)))
                    } else {
                        self.status = "Click inside the artboard to add a layer.".into();
                        None
                    }
                })
            } else if response.drag_stopped() {
                self.creation.take().map(|c| {
                    let rect = Rect::from_two_pos(c.start, c.current);
                    // A tiny drag is treated as a click.
                    if rect.width() * self.zoom < 4.0 && rect.height() * self.zoom < 4.0 {
                        (c.kind, Rect::from_min_size(c.start, CLICK_SIZE))
                    } else {
                        (c.kind, rect)
                    }
                })
            } else {
                None
            };
            if let Some((kind, rect)) = finished {
                self.flush_properties();
                let result = self.editor.add_at(
                    kind,
                    rect.min.x,
                    rect.min.y,
                    rect.width().max(1.0),
                    rect.height().max(1.0),
                );
                self.result(result);
                self.set_tool(Tool::Select);
            }
            if let Some(creation) = &self.creation {
                let preview = Rect::from_two_pos(
                    origin + creation.start.to_vec2() * self.zoom,
                    origin + creation.current.to_vec2() * self.zoom,
                );
                painter.rect_filled(preview, 0.0, ACCENT.gamma_multiply(0.12));
                painter.rect_stroke(preview, 0.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            }
        } else if !panning && self.pending.is_none() {
            let pointer = response.interact_pointer_pos();
            // Shift adds to or removes from the selection.
            let extend = ui.input(|i| i.modifiers.shift);
            if (response.clicked() || response.drag_started())
                && let Some(pointer) = pointer
            {
                self.flush_properties();
                let hit_point = if response.drag_started() {
                    ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer)
                } else {
                    pointer
                };
                let resize_id = (!extend)
                    .then(|| self.editor.selected_node())
                    .flatten()
                    .filter(|n| !n.locked && !n.hidden)
                    .and_then(|n| {
                        let bottom = origin + vec2(n.x + n.width, n.y + n.height) * self.zoom;
                        (bottom.distance(hit_point) < 12.0).then_some(n.id)
                    });
                let hit = resize_id.or_else(|| {
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
                });
                match hit {
                    Some(id) if response.drag_started() => {
                        // Dragging a selected layer moves the whole selection.
                        if !self.editor.is_selected(id) {
                            if extend {
                                self.editor.toggle_selection(id);
                            } else {
                                self.editor.select(Some(id));
                            }
                        }
                        let nodes: IdSet = if resize_id.is_some() {
                            IdSet::from_iter([id])
                        } else {
                            self.editor.arrangeable().into_iter().collect()
                        };
                        let indices = (self.editor.document.nodes.iter().enumerate())
                            .filter(|(_, n)| nodes.contains(&n.id))
                            .map(|(index, _)| index)
                            .collect();
                        self.drag = Some(Drag {
                            before: self.editor.document.clone(),
                            start: hit_point,
                            nodes,
                            indices,
                            resize: resize_id.is_some(),
                        });
                    }
                    Some(id) if extend => self.editor.toggle_selection(id),
                    Some(id) => self.editor.select(Some(id)),
                    None if !extend => self.editor.select(None),
                    None => {}
                }
            }
            if (response.dragged() || response.drag_stopped())
                && let (Some(drag), Some(pointer)) = (&self.drag, pointer)
            {
                let delta = (pointer - drag.start) / self.zoom;
                let translation = drag
                    .before
                    .clamp_translation(&drag.nodes, [delta.x.round(), delta.y.round()]);
                for &index in &drag.indices {
                    let original = &drag.before.nodes[index];
                    let Some(node) = self
                        .editor
                        .document
                        .nodes
                        .get_mut(index)
                        .filter(|n| n.id == original.id)
                    else {
                        continue;
                    };
                    if drag.resize {
                        node.width = (original.width + delta.x).round().clamp(1.0, 100_000.0);
                        node.height = (original.height + delta.y).round().clamp(1.0, 100_000.0);
                    } else {
                        node.x = original.x + translation[0];
                        node.y = original.y + translation[1];
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.creation = None;
            if let Some(drag) = self.drag.take() {
                let result = self.editor.commit(drag.before);
                self.result(result);
            }
        }
    }
}

/// Paints the artboard and its visible layers. `preview` rounds font sizes so small
/// thumbnails do not rasterize a separate glyph set for every scale.
fn paint_document(
    painter: &Painter,
    document: &Document,
    origin: Pos2,
    zoom: f32,
    clip: Rect,
    preview: bool,
) {
    let artboard = Rect::from_min_size(origin, vec2(document.width, document.height) * zoom);
    let painter = painter.with_clip_rect(clip);
    painter.rect_filled(artboard, 0.0, color(document.background_color()));
    let content = painter.with_clip_rect(clip.intersect(artboard));
    for node in document.nodes.iter().filter(|n| !n.hidden) {
        let rect = Rect::from_min_size(
            origin + vec2(node.x, node.y) * zoom,
            vec2(node.width, node.height) * zoom,
        );
        if !rect.intersects(content.clip_rect()) {
            continue;
        }
        match node.kind {
            NodeKind::Rectangle => {
                content.rect_filled(
                    rect,
                    (node.radius.min(node.width.min(node.height) / 2.0) * zoom).min(255.0) as u8,
                    color(document.node_fill(node)),
                );
            }
            NodeKind::Text => {
                let mut size = node.font_size * zoom;
                if preview {
                    if size < 2.0 {
                        continue;
                    }
                    size = (size * 2.0).round() / 2.0;
                }
                let fill = color(document.node_fill(node));
                let galley = content.layout(
                    node.text.clone(),
                    FontId::proportional(size),
                    fill,
                    rect.width(),
                );
                content
                    .with_clip_rect(rect.intersect(artboard).intersect(clip))
                    .galley(rect.min, galley, fill);
            }
        }
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

fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// A window tab. Returns the tab response and, for closable tabs, the close button.
fn tab(
    ui: &mut egui::Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    closable: bool,
) -> (egui::Response, Option<egui::Response>) {
    let text = ui::truncated(ui, label, BODY, TEXT, 180.0);
    let close_space = if closable { 44.0 } else { 0.0 };
    let width = (34.0 + text.size().x + 12.0 + close_space).max(if closable { 150.0 } else { 0.0 });
    let (rect, response) = ui.allocate_exact_size(vec2(width, 27.0), Sense::click());
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
        pos2(rect.left() + 17.0, rect.center().y),
        Align2::CENTER_CENTER,
        glyph,
        ui::icon_font(17.0),
        color,
    );
    ui.painter().galley(
        pos2(rect.left() + 34.0, rect.center().y - text.size().y / 2.0),
        text,
        color,
    );
    let close = closable.then(|| {
        let button =
            Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(20.0, 20.0));
        ui::icon_button_at(
            ui,
            button,
            egui::Id::new("close_file_tab"),
            icon::X,
            14.0,
            false,
            true,
            "Close file",
        )
    });
    (response, close)
}

/// Design/Theme switch. Returns `true` when the inactive segment was clicked.
fn segmented(ui: &mut egui::Ui, theme: bool) -> bool {
    let width = ui.available_width() - 24.0;
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
        ui.painter().rect_filled(rect, 7, Color32::from_gray(53));
        let (left, right) = rect.split_left_right_at_fraction(0.5);
        let selected = if theme { right } else { left }.shrink(1.0);
        ui.painter()
            .rect_filled(selected, 6, Color32::from_gray(63));
        ui.painter().rect_stroke(
            selected,
            6,
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
        let field_width = width - 32.0;
        ui.allocate_ui_with_layout(
            vec2(field_width, ROW),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(field_width);
                match bound {
                    Some(token) => {
                        ui::field_frame().show(ui, |ui| {
                            ui.set_width(field_width - 16.0);
                            ui.set_height(ROW);
                            ui.horizontal_centered(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                                swatch(ui.painter(), rect, token.value);
                                // Leave room for the spacing and the 22 pt detach button.
                                let name_width = (ui.available_width() - 30.0).max(0.0);
                                let galley = ui::truncated(ui, &token.name, BODY, TEXT, name_width);
                                let (rect, _) = ui.allocate_exact_size(
                                    vec2(name_width, ROW - 6.0),
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
                                    vec2(22.0, 22.0),
                                    15.0,
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
        let menu = ui::icon_button(
            ui,
            icon::CIRCLES_FOUR,
            vec2(28.0, 28.0),
            17.0,
            bound.is_some(),
            enabled,
            "Color tokens",
        );
        egui::Popup::menu(&menu).id(id.with("tokens")).show(|ui| {
            ui.set_min_width(210.0);
            if tokens.is_empty() {
                ui.label(RichText::new("No color tokens yet").color(TEXT_DIM));
            }
            for token in tokens {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
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
    painter.rect_filled(rect, 3, color(value));
    painter.rect_stroke(
        rect,
        3,
        Stroke::new(1.0, Color32::from_white_alpha(46)),
        StrokeKind::Inside,
    );
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
        pos2(rect.left() + 9.0, rect.center().y),
        Align2::CENTER_CENTER,
        if *open {
            icon::CARET_DOWN
        } else {
            icon::CARET_RIGHT
        },
        ui::icon_font(12.0),
        if hovered { TEXT } else { TEXT_DIM },
    );
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        ui::font(BODY),
        TEXT,
    );
    trailing.map(|(glyph, label, enabled)| {
        let slot =
            Rect::from_center_size(pos2(rect.right() - 19.0, rect.center().y), vec2(24.0, 24.0));
        ui::icon_button_at(
            ui,
            slot,
            egui::Id::new(("section_control", title)),
            glyph,
            16.0,
            false,
            enabled,
            label,
        )
    })
}

impl eframe::App for PaperCraftApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.request(Intent::Close, ctx);
        }
        #[cfg(target_os = "macos")]
        craft_quit::on_close_request(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
