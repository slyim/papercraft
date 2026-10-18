use crate::document::{Document, Editor, NodeKind, atomic_write};
use crate::recents::Recents;
use crate::ui::{self, ACCENT, BODY, BORDER, CONTROL, ROW, Row, SHELL, TEXT, TEXT_DIM, TEXT_FAINT};
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
    node: u64,
    resize: bool,
}

/// A layer being drawn with the rectangle or text tool, in artboard coordinates.
struct Creation {
    kind: NodeKind,
    start: Pos2,
    current: Pos2,
}

pub struct CrumbsApp {
    pub editor: Editor,
    saved: Document,
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
}

impl CrumbsApp {
    pub fn from_document(document: Document) -> Self {
        Self {
            saved: document.clone(),
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
        self.document_open && self.editor.document != self.saved
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
        self.saved = document.clone();
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
                .add_filter("Crumbs document", &["crumbs"])
                .set_file_name(format!("{}.crumbs", self.editor.document.name))
                .save_file()
        };
        let Some(path) = path else {
            return false;
        };
        match self.editor.document.save(&path) {
            Ok(()) => {
                self.saved = self.editor.document.clone();
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
                    .add_filter("Crumbs document", &["crumbs"])
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Intent::OpenPath(path) => self.open_path(path),
            Intent::CloseDocument => {
                // Unsaved changes were confirmed or discarded before reaching here.
                self.saved = self.editor.document.clone();
                self.close_document();
            }
            Intent::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn export(&mut self) {
        self.flush_properties();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("HTML", &["html"])
            .set_file_name(format!("{}.html", self.editor.document.name))
            .save_file()
        else {
            return;
        };
        match self
            .editor
            .document
            .export_html()
            .and_then(|html| atomic_write(&path, html.as_bytes()))
        {
            Ok(()) => self.status = format!("Exported {}", path.display()),
            Err(error) => self.status = error,
        }
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
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.flush_properties();
            let result = self.editor.delete();
            self.result(result);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            } else {
                self.editor.selected = None;
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
        if delta != Vec2::ZERO
            && let Some(id) = self.editor.selected
        {
            self.flush_properties();
            let before = self.editor.document.clone();
            if let Some(node) = self
                .editor
                .document
                .nodes
                .iter_mut()
                .find(|n| n.id == id && !n.locked)
            {
                node.x = (node.x + delta.x).clamp(-100_000.0, 100_000.0);
                node.y = (node.y + delta.y).clamp(-100_000.0, 100_000.0);
            }
            let result = self.editor.commit(before);
            self.result(result);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Intent::Close, &ctx);
        }
        if self.pending.is_none() && !ctx.input(|i| i.pointer.any_down()) {
            self.shortcuts(&ctx);
        }
        let title = if self.document_open {
            format!(
                "{}{} — Crumbs",
                self.editor.document.name,
                if self.dirty() { " •" } else { "" }
            )
        } else {
            "Crumbs".into()
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
                            ("Arrow keys", "Move selection (Shift: 10 px)"),
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
            .frame(egui::Frame::NONE.fill(color(self.editor.document.canvas)))
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
        if item(ui, true, "Export HTML…", "") {
            ui.close();
            self.export();
        }
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
        segmented(ui);
        ui.add_space(10.0);

        section_toggle(
            ui,
            "Pages",
            &mut self.pages_open,
            Some((icon::PLUS, "Crumbs documents have one page", false)),
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
            self.editor.selected = None;
        }
        ui.add_space(8.0);
        ui::hline(ui);

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

        ui.add_space(8.0);
        section_toggle(ui, "Layers", &mut self.layers_open, None);
        if !self.layers_open {
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let nodes: Vec<_> = self
                    .editor
                    .document
                    .nodes
                    .iter()
                    .rev()
                    .map(|n| (n.id, n.name.clone(), n.kind, n.hidden, n.locked))
                    .collect();
                for (id, name, kind, hidden, locked) in nodes {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
                    let selected = self.editor.selected == Some(id);
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
                        self.editor.selected = Some(id);
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
                if ui
                    .add(
                        egui::Button::new(RichText::new("Export").size(BODY).color(TEXT))
                            .fill(CONTROL)
                            .min_size(vec2(52.0, ROW)),
                    )
                    .on_hover_text("Export HTML")
                    .clicked()
                {
                    self.export();
                }
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
        let before = self.editor.document.clone();
        let mut finish = false;
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
        if let Some(index) = self
            .editor
            .document
            .nodes
            .iter()
            .position(|n| Some(n.id) == self.editor.selected)
        {
            let node = &mut self.editor.document.nodes[index];
            let locked = node.locked;
            let kind = node.kind;
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
                        finish |= ui::color_field(ui, egui::Id::new("node_fill"), &mut node.fill);
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
                finish |= ui::color_field(ui, egui::Id::new("page_color"), &mut document.canvas);
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
                finish |=
                    ui::color_field(ui, egui::Id::new("artboard_fill"), &mut document.background);
            });
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "Select a layer on the canvas or in the layer list to edit it.",
                        )
                        .color(TEXT_DIM),
                    );
                });
        }
        if before != self.editor.document && self.property_before.is_none() {
            self.property_before = Some(before);
        }
        if finish || (!ui.ctx().egui_wants_keyboard_input() && !ui.ctx().egui_is_using_pointer()) {
            self.flush_properties();
        }
        if self.editor.selected.is_some() {
            let Some((locked, hidden)) = self.editor.selected_node().map(|n| (n.locked, n.hidden))
            else {
                return;
            };
            ui::section_title(ui, "Layer");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(10.0);
                let action = |ui: &mut egui::Ui, glyph, label, enabled| {
                    ui::icon_button(ui, glyph, vec2(32.0, 30.0), 18.0, false, enabled, label)
                        .clicked()
                };
                if action(ui, icon::ARROW_UP, "Bring forward", !locked) {
                    self.flush_properties();
                    let r = self.editor.reorder(true);
                    self.result(r);
                }
                if action(ui, icon::ARROW_DOWN, "Send backward", !locked) {
                    self.flush_properties();
                    let r = self.editor.reorder(false);
                    self.result(r);
                }
                if action(ui, icon::COPY, "Duplicate", !locked) {
                    self.flush_properties();
                    let r = self.editor.duplicate();
                    self.result(r);
                }
                if action(ui, icon::TRASH, "Delete layer", !locked) {
                    self.flush_properties();
                    let r = self.editor.delete();
                    self.result(r);
                }
                let toggle = |ui: &mut egui::Ui, glyph, label, on| {
                    ui::icon_button(ui, glyph, vec2(32.0, 30.0), 18.0, on, true, label).clicked()
                };
                let hide_label = if hidden { "Show layer" } else { "Hide layer" };
                let hide_glyph = if hidden { icon::EYE_SLASH } else { icon::EYE };
                let lock_label = if locked { "Unlock layer" } else { "Lock layer" };
                let lock_glyph = if locked { icon::LOCK } else { icon::LOCK_OPEN };
                let hide = toggle(ui, hide_glyph, hide_label, hidden);
                let lock = toggle(ui, lock_glyph, lock_label, locked);
                if hide || lock {
                    self.flush_properties();
                    let before = self.editor.document.clone();
                    if let Some(id) = self.editor.selected
                        && let Some(node) =
                            self.editor.document.nodes.iter_mut().find(|n| n.id == id)
                    {
                        if hide {
                            node.hidden = !node.hidden;
                        } else {
                            node.locked = !node.locked;
                        }
                    }
                    let result = self.editor.commit(before);
                    self.result(result);
                }
            });
        }
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
        if let Some(node) = self.editor.selected_node().filter(|n| !n.hidden) {
            let rect = Rect::from_min_size(
                origin + vec2(node.x, node.y) * self.zoom,
                vec2(node.width, node.height) * self.zoom,
            );
            painter.rect_stroke(rect, 0.0, Stroke::new(1.5, ACCENT), StrokeKind::Outside);
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
            if (response.clicked() || response.drag_started())
                && let Some(pointer) = pointer
            {
                self.flush_properties();
                let hit_point = if response.drag_started() {
                    ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer)
                } else {
                    pointer
                };
                let resize_id = self
                    .editor
                    .selected_node()
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
                self.editor.selected = hit;
                if response.drag_started()
                    && let Some(id) = hit
                {
                    self.drag = Some(Drag {
                        before: self.editor.document.clone(),
                        start: hit_point,
                        node: id,
                        resize: resize_id.is_some(),
                    });
                }
            }
            if response.dragged()
                && let (Some(drag), Some(pointer)) = (&self.drag, pointer)
            {
                let delta = (pointer - drag.start) / self.zoom;
                let original = drag
                    .before
                    .nodes
                    .iter()
                    .find(|n| n.id == drag.node)
                    .unwrap();
                let node = self
                    .editor
                    .document
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == drag.node)
                    .unwrap();
                if drag.resize {
                    node.width = (original.width + delta.x).round().clamp(1.0, 100_000.0);
                    node.height = (original.height + delta.y).round().clamp(1.0, 100_000.0);
                } else {
                    node.x = (original.x + delta.x).round().clamp(-100_000.0, 100_000.0);
                    node.y = (original.y + delta.y).round().clamp(-100_000.0, 100_000.0);
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
    painter.rect_filled(artboard, 0.0, color(document.background));
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
                    color(node.fill),
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
                let galley = content.layout(
                    node.text.clone(),
                    FontId::proportional(size),
                    color(node.fill),
                    rect.width(),
                );
                content
                    .with_clip_rect(rect.intersect(artboard).intersect(clip))
                    .galley(rect.min, galley, color(node.fill));
            }
        }
    }
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

/// Design/Theme switch. Theme tokens are not part of the Crumbs model yet, so that
/// segment is shown disabled.
fn segmented(ui: &mut egui::Ui) {
    let width = ui.available_width() - 24.0;
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
        ui.painter().rect_filled(rect, 7, Color32::from_gray(53));
        let (left, right) = rect.split_left_right_at_fraction(0.5);
        let selected = left.shrink(1.0);
        ui.painter()
            .rect_filled(selected, 6, Color32::from_gray(63));
        ui.painter().rect_stroke(
            selected,
            6,
            Stroke::new(1.0, Color32::from_gray(72)),
            StrokeKind::Inside,
        );
        ui.painter().text(
            left.center(),
            Align2::CENTER_CENTER,
            "Design",
            ui::font(BODY),
            TEXT,
        );
        ui.painter().text(
            right.center(),
            Align2::CENTER_CENTER,
            "Theme",
            ui::font(BODY),
            TEXT_FAINT,
        );
        let design = ui.interact(left, egui::Id::new("segment_design"), Sense::hover());
        design.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, true, "Design")
        });
        let theme = ui.interact(right, egui::Id::new("segment_theme"), Sense::hover());
        theme.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, false, false, "Theme")
        });
        theme.on_hover_text("Theme tokens are not available yet");
    });
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

impl eframe::App for CrumbsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
