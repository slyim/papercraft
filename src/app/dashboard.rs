//! Dashboard: recent local files, search, and file creation.

use super::{CrumbsApp, Intent, View, color, paint_document};
use crate::document::Document;
use crate::recents::edited_label;
use crate::ui::{self, BODY, BORDER, MAIN, ROW, Row, SHELL, TEXT, TEXT_DIM, TEXT_FAINT};
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use egui_phosphor::regular as icon;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

const CARD_MIN_WIDTH: f32 = 280.0;
const CARD_GAP: f32 = 24.0;
const CARD_HEADER: f32 = 58.0;

pub(super) fn search_id() -> egui::Id {
    egui::Id::new("dashboard_search")
}

struct Loaded {
    modified: Option<SystemTime>,
    document: Result<Document, String>,
}

/// Documents read from recent paths, reloaded when their modification time changes.
#[derive(Default)]
pub(super) struct CardCache {
    loaded: HashMap<PathBuf, Loaded>,
    fresh: bool,
}

impl CardCache {
    pub(super) fn invalidate(&mut self) {
        self.fresh = false;
    }

    fn refresh(&mut self, paths: &[PathBuf]) {
        if self.fresh {
            return;
        }
        self.loaded.retain(|path, _| paths.contains(path));
        for path in paths {
            let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            let stale = self
                .loaded
                .get(path)
                .is_none_or(|l| l.modified != modified || modified.is_none());
            if stale {
                self.loaded.insert(
                    path.clone(),
                    Loaded {
                        modified,
                        document: Document::load(path),
                    },
                );
            }
        }
        self.fresh = true;
    }
}

/// One dashboard entry.
struct Card<'a> {
    path: Option<&'a Path>,
    title: String,
    subtitle: String,
    document: Option<&'a Document>,
    current: bool,
}

enum Action {
    Open(PathBuf),
    ShowCurrent,
    Remove(PathBuf),
}

impl CrumbsApp {
    pub(super) fn dashboard(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.cards.refresh(self.recents.files());
        egui::Panel::left("dashboard_sidebar")
            .exact_size(240.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(SHELL))
            .show(ui, |ui| self.dashboard_sidebar(ui, ctx));
        let action = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(MAIN))
            .show(ui, |ui| self.dashboard_main(ui, ctx))
            .inner;
        match action {
            Some(Action::Open(path)) => self.request(Intent::OpenPath(path), ctx),
            Some(Action::ShowCurrent) => self.view = View::Editor,
            Some(Action::Remove(path)) => {
                if let Err(error) = self.recents.remove(&path) {
                    self.status = error;
                } else {
                    self.status = format!("Removed {} from recents", path.display());
                }
                self.cards.invalidate();
            }
            None => {}
        }
    }

    fn dashboard_sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
        ui.add_space(11.0);
        // Application header.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
        let mark = pos2(rect.left() + 26.0, rect.center().y);
        ui.painter().circle_filled(mark, 11.0, ui::CONTROL);
        ui.painter().text(
            mark,
            Align2::CENTER_CENTER,
            icon::COOKIE,
            ui::icon_font(14.0),
            TEXT,
        );
        ui.painter().text(
            pos2(rect.left() + 46.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Crumbs",
            ui::font(BODY),
            TEXT,
        );
        ui.add_space(12.0);
        // Search.
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                ui::field_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ROW);
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(icon::MAGNIFYING_GLASS)
                                .size(16.0)
                                .color(TEXT_DIM),
                        );
                        let hint = if cfg!(target_os = "macos") {
                            "⌘F"
                        } else {
                            "Ctrl F"
                        };
                        let edit_width = ui.available_width() - 40.0;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .id(search_id())
                                .hint_text(RichText::new("Search").color(TEXT_DIM))
                                .frame(egui::Frame::NONE)
                                .margin(egui::Vec2::ZERO)
                                .desired_width(edit_width)
                                .text_color(TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new(hint).size(12.5).color(TEXT_DIM));
                        });
                    });
                });
            });
        ui.add_space(12.0);
        if Row::new(icon::CLOCK, "Recents")
            .indent(19.0)
            .selected(true)
            .show(ui)
            .on_hover_text("Recently opened and saved files")
            .clicked()
        {
            self.search.clear();
        }
        if Row::new(icon::FOLDER_OPEN, "Open file…")
            .indent(19.0)
            .show(ui)
            .on_hover_text("Open a .crumbs document")
            .clicked()
        {
            self.request(Intent::Open, ctx);
        }
        ui.add_space(12.0);
        ui::hline(ui);

        egui::Panel::bottom("dashboard_footer")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 12,
            }))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                if !self.status.is_empty() && !self.document_open {
                    ui.label(RichText::new(&self.status).size(12.0).color(TEXT_DIM));
                }
                let shortcuts = ui
                    .add(
                        egui::Label::new(
                            RichText::new("Keyboard shortcuts")
                                .size(12.5)
                                .color(TEXT_DIM),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if shortcuts.hovered() {
                    ui.painter().hline(
                        shortcuts.rect.x_range(),
                        shortcuts.rect.bottom(),
                        Stroke::new(1.0, TEXT_DIM),
                    );
                }
                if shortcuts.clicked() {
                    self.help = true;
                }
            });
    }

    fn collect_cards(&self) -> Vec<Card<'_>> {
        let now = SystemTime::now();
        let mut cards = Vec::new();
        let current_path = self.path.as_deref().filter(|_| self.document_open);
        if self.document_open
            && current_path.is_none_or(|p| !self.recents.files().iter().any(|r| r == p))
        {
            cards.push(Card {
                path: current_path,
                title: self.editor.document.name.clone(),
                subtitle: if self.dirty() {
                    "Open · unsaved changes".into()
                } else {
                    "Open · not saved yet".into()
                },
                document: Some(&self.editor.document),
                current: true,
            });
        }
        for path in self.recents.files() {
            let current = current_path == Some(path.as_path());
            let loaded = self.cards.loaded.get(path);
            let (document, subtitle) = if current {
                (
                    Some(&self.editor.document),
                    if self.dirty() {
                        "Open · unsaved changes".to_string()
                    } else {
                        loaded
                            .and_then(|l| l.modified)
                            .map(|m| edited_label(m, now))
                            .unwrap_or_else(|| "Open".into())
                    },
                )
            } else {
                match loaded.map(|l| (&l.document, l.modified)) {
                    Some((Ok(document), modified)) => (
                        Some(document),
                        modified.map(|m| edited_label(m, now)).unwrap_or_default(),
                    ),
                    Some((Err(_), None)) | None => (None, "File not found".into()),
                    Some((Err(_), Some(_))) => (None, "Cannot be opened".into()),
                }
            };
            let title = document.map(|d| d.name.clone()).unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            });
            cards.push(Card {
                path: Some(path),
                title,
                subtitle,
                document,
                current,
            });
        }
        let query = self.search.trim().to_lowercase();
        if !query.is_empty() {
            cards.retain(|c| {
                c.title.to_lowercase().contains(&query)
                    || c.path.is_some_and(|p| {
                        p.file_name()
                            .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&query))
                    })
            });
        }
        cards
    }

    fn dashboard_main(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> Option<Action> {
        let full = ui.max_rect();
        let padding = (full.width() * 0.065).clamp(24.0, 80.0);
        let content = full.shrink2(vec2(padding, 0.0));
        // Header: title, new file, layout switch.
        let header = Rect::from_min_size(
            pos2(content.left(), full.top() + 30.0),
            vec2(content.width(), 34.0),
        );
        ui.painter().text(
            header.left_center(),
            Align2::LEFT_CENTER,
            "Recents",
            ui::font(26.0),
            TEXT,
        );
        let switch = Rect::from_min_size(
            pos2(header.right() - 58.0, header.center().y - 15.0),
            vec2(58.0, 30.0),
        );
        ui.painter()
            .rect_filled(switch, ui::RADIUS, Color32::from_gray(45));
        for (index, (glyph, label, list)) in [
            (icon::SQUARES_FOUR, "Grid view", false),
            (icon::LIST, "List view", true),
        ]
        .into_iter()
        .enumerate()
        {
            let slot = Rect::from_min_size(
                switch.min + vec2(1.0 + index as f32 * 28.0, 1.0),
                vec2(28.0, 28.0),
            );
            let selected = self.list_view == list;
            if ui::icon_button_at(
                ui,
                slot,
                egui::Id::new(("layout_switch", index)),
                glyph,
                17.0,
                selected,
                true,
                label,
            )
            .clicked()
            {
                self.list_view = list;
            }
        }
        let new_file = Rect::from_min_size(
            pos2(switch.left() - 12.0 - 108.0, header.center().y - 15.0),
            vec2(108.0, 30.0),
        );
        if ui
            .put(
                new_file,
                egui::Button::new(
                    RichText::new(format!("{}  New file", icon::PLUS))
                        .size(BODY)
                        .color(Color32::from_gray(24)),
                )
                .fill(Color32::from_gray(242)),
            )
            .on_hover_text("Create a new document")
            .clicked()
        {
            self.request(Intent::New, ctx);
            return None;
        }

        let body = Rect::from_min_max(pos2(full.left(), full.top() + 108.0), full.max);
        let mut action = None;
        let cards = self.collect_cards();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        if cards.is_empty() {
            let (title, detail) = if self.search.trim().is_empty() {
                (
                    "No recent files",
                    "Files you open or save appear here. Create a new file or open a .crumbs document.",
                )
            } else {
                ("No matching files", "Try another name or clear the search.")
            };
            child.add_space(40.0);
            child.vertical_centered(|ui| {
                ui.label(
                    RichText::new(icon::FILE_DASHED)
                        .size(32.0)
                        .color(TEXT_FAINT),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(title).size(15.0).color(TEXT));
                ui.add_space(4.0);
                ui.label(RichText::new(detail).color(TEXT_DIM));
            });
            return None;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                let left = content.left();
                let width = content.width();
                if self.list_view {
                    for (index, card) in cards.iter().enumerate() {
                        let (rect, _) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::hover());
                        let row = Rect::from_min_size(pos2(left, rect.top()), vec2(width, 56.0));
                        if let Some(a) = list_row(ui, index, row, card) {
                            action = Some(a);
                        }
                    }
                } else {
                    let columns = (((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor()
                        as usize)
                        .max(1);
                    let card_width = (width - CARD_GAP * (columns as f32 - 1.0)) / columns as f32;
                    let card_height = CARD_HEADER + (card_width - 8.0) / 1.53 + 4.0;
                    for (row_index, row) in cards.chunks(columns).enumerate() {
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), card_height + CARD_GAP),
                            Sense::hover(),
                        );
                        for (column, card) in row.iter().enumerate() {
                            let card_rect = Rect::from_min_size(
                                pos2(left + column as f32 * (card_width + CARD_GAP), rect.top()),
                                vec2(card_width, card_height),
                            );
                            if let Some(a) =
                                grid_card(ui, row_index * columns + column, card_rect, card)
                            {
                                action = Some(a);
                            }
                        }
                    }
                }
            });
        action
    }
}

fn card_response(ui: &mut egui::Ui, index: usize, rect: Rect, card: &Card) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new(("recent_card", index)), Sense::click());
    let title = card.title.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &title));
    match card.path {
        Some(path) => response.on_hover_text(path.display().to_string()),
        None => response.on_hover_text("Open document · not saved yet"),
    }
}

fn card_action(response: &egui::Response, card: &Card) -> Option<Action> {
    let mut action = None;
    if let Some(path) = card.path {
        egui::Popup::context_menu(response).show(|ui| {
            if ui
                .add(egui::Button::new("Remove from recents").frame_when_inactive(false))
                .clicked()
            {
                action = Some(Action::Remove(path.to_path_buf()));
            }
        });
    }
    if response.clicked() {
        action = if card.current {
            Some(Action::ShowCurrent)
        } else {
            card.path.map(|p| Action::Open(p.to_path_buf()))
        };
    }
    action
}

fn grid_card(ui: &mut egui::Ui, index: usize, rect: Rect, card: &Card) -> Option<Action> {
    let response = card_response(ui, index, rect, card);
    let hovered = response.hovered();
    let painter = ui.painter();
    painter.rect_filled(rect, 8, SHELL);
    painter.rect_stroke(
        rect,
        8,
        Stroke::new(
            1.0,
            if hovered {
                Color32::from_gray(78)
            } else {
                BORDER
            },
        ),
        StrokeKind::Inside,
    );
    let title = ui::truncated(ui, &card.title, BODY, TEXT, rect.width() - 32.0);
    ui.painter().galley(
        pos2(rect.left() + 16.0, rect.top() + 21.0 - title.size().y / 2.0),
        title,
        TEXT,
    );
    let subtitle = ui::truncated(ui, &card.subtitle, BODY, TEXT_DIM, rect.width() - 32.0);
    ui.painter().galley(
        pos2(
            rect.left() + 16.0,
            rect.top() + 40.0 - subtitle.size().y / 2.0,
        ),
        subtitle,
        TEXT_DIM,
    );
    let thumb = Rect::from_min_max(
        pos2(rect.left() + 4.0, rect.top() + CARD_HEADER),
        rect.max - vec2(4.0, 4.0),
    );
    thumbnail(ui, thumb, card.document, 5);
    card_action(&response, card)
}

fn list_row(ui: &mut egui::Ui, index: usize, rect: Rect, card: &Card) -> Option<Action> {
    let response = card_response(ui, index, rect, card);
    if response.hovered() {
        ui.painter().rect_filled(rect, 8, ui::HOVER);
    }
    let thumb = Rect::from_min_size(rect.left_top() + vec2(8.0, 8.0), vec2(64.0, 40.0));
    thumbnail(ui, thumb, card.document, 4);
    let text_left = thumb.right() + 14.0;
    let path_width = (rect.width() * 0.4).min(420.0);
    let text_width = rect.right() - path_width - text_left - 16.0;
    let title = ui::truncated(ui, &card.title, BODY, TEXT, text_width);
    ui.painter()
        .galley(pos2(text_left, rect.top() + 10.0), title, TEXT);
    let subtitle = ui::truncated(ui, &card.subtitle, 12.5, TEXT_DIM, text_width);
    ui.painter()
        .galley(pos2(text_left, rect.top() + 30.0), subtitle, TEXT_DIM);
    if let Some(path) = card.path {
        let location = ui::truncated(
            ui,
            &path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            12.5,
            TEXT_FAINT,
            path_width,
        );
        ui.painter().galley(
            pos2(
                rect.right() - 16.0 - location.size().x,
                rect.center().y - location.size().y / 2.0,
            ),
            location,
            TEXT_FAINT,
        );
    }
    card_action(&response, card)
}

/// Scaled document preview, or a neutral placeholder when the file cannot be read.
fn thumbnail(ui: &egui::Ui, rect: Rect, document: Option<&Document>, radius: u8) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let Some(document) = document else {
        painter.rect_filled(rect, radius, Color32::from_gray(48));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon::WARNING,
            ui::icon_font(20.0),
            TEXT_FAINT,
        );
        return;
    };
    painter.rect_filled(rect, radius, color(document.canvas));
    let inner = rect.shrink(rect.width().min(rect.height()) * 0.08);
    let zoom = (inner.width() / document.width).min(inner.height() / document.height);
    let origin = rect.center() - vec2(document.width, document.height) * zoom / 2.0;
    paint_document(&painter, document, origin, zoom, painter.clip_rect(), true);
}
