//! Shared colors, metrics and small painted controls for the PaperCraft shell.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2, WidgetInfo, WidgetType, vec2,
};

const fn gray(v: u8) -> Color32 {
    Color32::from_rgb(v, v, v)
}

/// Panels, tab bar and sidebars.
pub const SHELL: Color32 = gray(42);
/// Dashboard content area.
pub const MAIN: Color32 = gray(34);
/// Fields, selected rows and secondary buttons.
pub const CONTROL: Color32 = gray(55);
pub const HOVER: Color32 = gray(49);
pub const PRESSED: Color32 = gray(63);
pub const BORDER: Color32 = gray(55);
pub const TEXT: Color32 = gray(236);
pub const TEXT_DIM: Color32 = gray(161);
pub const TEXT_FAINT: Color32 = gray(112);
/// Selection outlines on the canvas and focus.
pub const ACCENT: Color32 = Color32::from_rgb(134, 178, 255);

pub const BODY: f32 = 13.5;
pub const ROW: f32 = 28.0;
/// Height of inspector fields, buttons and sliders.
pub const FIELD: f32 = 24.0;
pub const RADIUS: u8 = 6;
/// Height of the window tab strip.
pub const TAB_BAR: f32 = 41.0;

pub fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/papercraft-icon.png"))
        .expect("Bundled PaperCraft icon must be valid PNG")
}

/// Icon handed to the OS for the window and Dock. macOS does not mask
/// application icons, so it gets the rounded tile with a transparent margin.
pub fn window_icon() -> egui::IconData {
    if cfg!(target_os = "macos") {
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/papercraft-macos.png"))
            .expect("Bundled PaperCraft macOS icon must be valid PNG")
    } else {
        app_icon()
    }
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn icon_font(size: f32) -> FontId {
    FontId::proportional(size)
}

/// One-pixel horizontal rule across the current width, without spacing.
pub fn hline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
}

fn background(response: &Response, selected: bool, enabled: bool) -> Option<Color32> {
    if !enabled {
        return selected.then_some(CONTROL);
    }
    if response.is_pointer_button_down_on() {
        Some(PRESSED)
    } else if selected {
        Some(CONTROL)
    } else if response.hovered() {
        Some(HOVER)
    } else {
        None
    }
}

/// Square icon button with hover, pressed, selected and disabled states.
pub fn icon_button(
    ui: &mut Ui,
    glyph: &str,
    size: Vec2,
    icon_size: f32,
    selected: bool,
    enabled: bool,
    label: &str,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    paint_icon_button(
        ui, rect, response, glyph, icon_size, selected, enabled, label,
    )
}

/// Icon button placed at `rect` without moving the layout cursor (for controls
/// overlaid on rows and tabs). `id` must be unique among overlaid controls.
#[allow(clippy::too_many_arguments)]
pub fn icon_button_at(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    glyph: &str,
    icon_size: f32,
    selected: bool,
    enabled: bool,
    label: &str,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    paint_icon_button(
        ui, rect, response, glyph, icon_size, selected, enabled, label,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_icon_button(
    ui: &mut Ui,
    rect: Rect,
    response: Response,
    glyph: &str,
    icon_size: f32,
    selected: bool,
    enabled: bool,
    label: &str,
) -> Response {
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, label));
    if ui.is_rect_visible(rect) {
        if let Some(fill) = background(&response, selected, enabled) {
            ui.painter().rect_filled(rect, RADIUS, fill);
        }
        if enabled && response.has_focus() {
            ui.painter()
                .rect_stroke(rect, RADIUS, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
        }
        let color = if !enabled {
            TEXT_FAINT
        } else if selected || response.hovered() {
            TEXT
        } else {
            TEXT_DIM
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon_font(icon_size),
            color,
        );
    }
    response.on_hover_text(label)
}

/// Full-width row with an icon, a label and an optional right-side glyph.
pub struct Row<'a> {
    pub glyph: &'a str,
    pub label: &'a str,
    pub selected: bool,
    pub dim: bool,
    pub indent: f32,
    pub right: Option<&'a str>,
    /// Width kept free at the right edge for controls painted by the caller.
    pub reserve: f32,
}

impl<'a> Row<'a> {
    pub fn new(glyph: &'a str, label: &'a str) -> Self {
        Self {
            glyph,
            label,
            selected: false,
            dim: false,
            indent: 12.0,
            right: None,
            reserve: 0.0,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    pub fn reserve(mut self, reserve: f32) -> Self {
        self.reserve = reserve;
        self
    }

    pub fn right(mut self, right: Option<&'a str>) -> Self {
        self.right = right;
        self
    }

    /// Paints the row inside `rect` (already allocated) and returns its interaction.
    pub fn paint(self, ui: &mut Ui, rect: Rect, response: &Response, inset: f32) {
        let shape = rect.shrink2(vec2(inset, 0.0));
        if let Some(fill) = background(response, self.selected, true) {
            ui.painter().rect_filled(shape, RADIUS, fill);
        }
        let color = if self.dim { TEXT_FAINT } else { TEXT };
        let glyph_color = if self.dim { TEXT_FAINT } else { TEXT_DIM };
        let x = rect.left() + self.indent;
        if !self.glyph.is_empty() {
            ui.painter().text(
                egui::pos2(x + 8.0, rect.center().y),
                Align2::CENTER_CENTER,
                self.glyph,
                icon_font(16.0),
                if self.selected { TEXT } else { glyph_color },
            );
        }
        let text_x = if self.glyph.is_empty() { x } else { x + 24.0 };
        let right_space = if self.right.is_some() { 30.0 } else { 12.0 } + self.reserve;
        let max_width = (rect.right() - right_space - text_x).max(8.0);
        let galley = ui
            .painter()
            .layout(self.label.to_owned(), font(BODY), color, max_width);
        let galley = if galley.rows.len() > 1 {
            truncated(ui, self.label, BODY, color, max_width)
        } else {
            galley
        };
        ui.painter().galley(
            egui::pos2(text_x, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if let Some(right) = self.right {
            ui.painter().text(
                egui::pos2(rect.right() - inset - 14.0, rect.center().y),
                Align2::CENTER_CENTER,
                right,
                icon_font(15.0),
                TEXT,
            );
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
        let label = self.label.to_owned();
        let selected = self.selected;
        response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, &label));
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, &response, 8.0);
        }
        response
    }
}

/// Single-line text truncated with an ellipsis to `width`.
pub fn truncated(
    ui: &Ui,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font(size), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}

/// Section title used in the editor side panels.
pub fn section_title(ui: &mut Ui, title: &str) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(egui::RichText::new(title).size(BODY).color(TEXT));
    });
    ui.add_space(8.0);
}

/// Filled field container matching the inspector controls.
pub fn field_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(CONTROL)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::symmetric(8, 0))
}

/// Formats an sRGB color as six uppercase hexadecimal digits without `#`.
pub fn hex(color: [u8; 4]) -> String {
    format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

/// Parses `RGB`, `RRGGBB` or `RRGGBBAA`, with or without `#`. Alpha defaults to `alpha`.
pub fn parse_hex(text: &str, alpha: u8) -> Option<[u8; 4]> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    match digits.len() {
        3 => {
            let mut out = [0, 0, 0, alpha];
            for (i, c) in digits.chars().enumerate() {
                let v = c.to_digit(16)? as u8;
                out[i] = v * 17;
            }
            Some(out)
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, alpha]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

/// Color control: swatch (opens a picker), editable hexadecimal value and opacity.
/// Returns `true` when an edit gesture finished, so the caller can record history.
pub fn color_field(ui: &mut Ui, id: egui::Id, color: &mut [u8; 4]) -> bool {
    let mut finished = false;
    let width = ui.available_width();
    field_frame().show(ui, |ui| {
        ui.set_width(width - 16.0);
        ui.set_height(FIELD);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (swatch, response) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::ColorButton, true, "Color"));
            let shown = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
            ui.painter().rect_filled(swatch, 3, shown);
            ui.painter().rect_stroke(
                swatch,
                3,
                Stroke::new(1.0, Color32::from_white_alpha(46)),
                StrokeKind::Inside,
            );
            let response = response.on_hover_text("Choose color");
            let popup = egui::Popup::menu(&response)
                .id(id.with("picker"))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            let was_open = popup.is_open();
            let shown_popup = popup.show(|ui| {
                let mut value = shown;
                if egui::widgets::color_picker::color_picker_color32(
                    ui,
                    &mut value,
                    egui::widgets::color_picker::Alpha::OnlyBlend,
                ) {
                    *color = value.to_srgba_unmultiplied();
                }
            });
            if was_open && shown_popup.is_none() {
                finished = true;
            }

            let edit_id = id.with("hex");
            let focused = ui.memory(|m| m.has_focus(edit_id));
            let mut text = if focused {
                ui.data(|d| d.get_temp::<String>(edit_id))
                    .unwrap_or_else(|| hex(*color))
            } else {
                hex(*color)
            };
            let opacity_width = 44.0;
            let hex_width = (ui.available_width() - opacity_width - 8.0).max(48.0);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(edit_id)
                    .frame(egui::Frame::NONE)
                    .margin(Vec2::ZERO)
                    .char_limit(9)
                    .font(font(BODY))
                    .text_color(TEXT)
                    .desired_width(hex_width),
            );
            if edit.changed()
                && let Some(parsed) = parse_hex(&text, color[3])
            {
                *color = parsed;
            }
            if edit.has_focus() {
                ui.data_mut(|d| d.insert_temp(edit_id, text));
            } else {
                ui.data_mut(|d| d.remove::<String>(edit_id));
            }
            finished |= edit.lost_focus();

            let mut percent = (color[3] as f32 / 255.0 * 100.0).round();
            let drag = ui.scope(|ui| {
                plain_drag_style(ui);
                ui.spacing_mut().button_padding = vec2(2.0, 0.0);
                ui.add_sized(
                    [opacity_width, FIELD - 6.0],
                    egui::DragValue::new(&mut percent)
                        .range(0.0..=100.0)
                        .max_decimals(0)
                        .speed(0.5)
                        .suffix("%"),
                )
                .on_hover_text("Opacity")
            });
            if drag.inner.changed() {
                color[3] = (percent / 100.0 * 255.0).round() as u8;
            }
            finished |= drag.inner.drag_stopped() || drag.inner.lost_focus();
        });
    });
    finished
}

/// Read-only field for a property the document model does not store yet.
pub fn inert_field(
    ui: &mut Ui,
    lead: &str,
    text: &str,
    trailing: Option<&str>,
    width: f32,
    hint: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, FIELD), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, false, hint));
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS, CONTROL);
    let mut x = rect.left() + 8.0;
    if !lead.is_empty() {
        let galley = painter.layout_no_wrap(lead.to_owned(), font(BODY), TEXT_FAINT);
        let lead_width = galley.size().x;
        painter.galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            TEXT_FAINT,
        );
        x += lead_width + 8.0;
    }
    painter.text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font(BODY),
        TEXT_DIM,
    );
    if let Some(glyph) = trailing {
        painter.text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            Align2::CENTER_CENTER,
            glyph,
            icon_font(12.0),
            TEXT_FAINT,
        );
    }
    response.on_hover_text(hint)
}

/// Icon buttons joined in one field, separated by thin rules.
/// Returns the index of the clicked item.
pub fn segmented(ui: &mut Ui, width: f32, items: &[(&str, &str)], enabled: bool) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(vec2(width, FIELD), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS, CONTROL);
    let cell = width / items.len().max(1) as f32;
    let mut clicked = None;
    for (index, (glyph, label)) in items.iter().enumerate() {
        let slot = Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * cell, rect.top()),
            vec2(cell, FIELD),
        );
        if index > 0 {
            ui.painter()
                .vline(slot.left(), rect.y_range(), Stroke::new(1.0, SHELL));
        }
        let id = ui.id().with(("segment", *label));
        if icon_button_at(ui, slot, id, glyph, 16.0, false, enabled, label).clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// Full-width filled button with a centered label and dim shortcut.
pub fn wide_button(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), FIELD), sense);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    let fill = if enabled && response.hovered() {
        PRESSED
    } else {
        CONTROL
    };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS, fill);
    let color = if enabled { TEXT } else { TEXT_DIM };
    let text = painter.layout_no_wrap(label.to_owned(), font(BODY), color);
    let keys = painter.layout_no_wrap(shortcut.to_owned(), font(BODY), TEXT_FAINT);
    let gap = if shortcut.is_empty() { 0.0 } else { 8.0 };
    let total = text.size().x + gap + keys.size().x;
    let x = rect.center().x - total / 2.0;
    let text_width = text.size().x;
    painter.galley(
        egui::pos2(x, rect.center().y - text.size().y / 2.0),
        text,
        color,
    );
    painter.galley(
        egui::pos2(x + text_width + gap, rect.center().y - keys.size().y / 2.0),
        keys,
        TEXT_FAINT,
    );
    response
}

/// Checkbox with a label and dim shortcut.
pub fn check_row(
    ui: &mut Ui,
    checked: &mut bool,
    label: &str,
    shortcut: &str,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(vec2(ui.available_width(), FIELD), sense);
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let value = *checked;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, value, label));
    let painter = ui.painter();
    let mark = Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - 8.0),
        vec2(16.0, 16.0),
    );
    painter.rect_filled(mark, 4, if response.hovered() { PRESSED } else { CONTROL });
    let color = if enabled { TEXT } else { TEXT_DIM };
    if value {
        painter.text(
            mark.center(),
            Align2::CENTER_CENTER,
            egui_phosphor::regular::CHECK,
            icon_font(12.0),
            color,
        );
    }
    let text = painter.layout_no_wrap(label.to_owned(), font(BODY), color);
    let text_width = text.size().x;
    let x = mark.right() + 10.0;
    painter.galley(
        egui::pos2(x, rect.center().y - text.size().y / 2.0),
        text,
        color,
    );
    painter.text(
        egui::pos2(x + text_width + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        shortcut,
        font(BODY),
        TEXT_FAINT,
    );
    response
}

/// Horizontal slider from 0 to `max` with a pill-shaped knob. Values are whole numbers.
pub fn slider(ui: &mut Ui, value: &mut f32, max: f32, width: f32, label: &str) -> Response {
    const TRACK: f32 = 11.0;
    const KNOB: f32 = 20.0;
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, FIELD), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2(width, TRACK));
    let travel = (track.width() - KNOB).max(1.0);
    if response.is_pointer_button_down_on()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let t = ((pointer.x - track.left() - KNOB / 2.0) / travel).clamp(0.0, 1.0);
        let next = (t * max).round();
        if next != *value {
            *value = next;
            response.mark_changed();
        }
    }
    let shown = *value;
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), shown as f64, label));
    let t = if max > 0.0 {
        (shown / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let knob = Rect::from_min_size(
        egui::pos2(track.left() + t * travel, track.top()),
        vec2(KNOB, TRACK),
    );
    let painter = ui.painter();
    painter.rect_filled(track, TRACK / 2.0, CONTROL);
    let knob_color = if !ui.is_enabled() {
        TEXT_FAINT
    } else if response.hovered() || response.dragged() {
        Color32::WHITE
    } else {
        TEXT
    };
    painter.rect_filled(knob, TRACK / 2.0, knob_color);
    response.on_hover_text(label)
}

/// Removes the frame from drag values inside an existing field container.
pub fn plain_drag_style(ui: &mut Ui) {
    // Keep the value centered inside the 18 px slot of a 24 px field.
    ui.spacing_mut().interact_size.y = FIELD - 6.0;
    let visuals = &mut ui.visuals_mut().widgets;
    for w in [
        &mut visuals.inactive,
        &mut visuals.hovered,
        &mut visuals.active,
    ] {
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::NONE;
    }
    visuals.inactive.fg_stroke.color = TEXT_DIM;
    visuals.hovered.fg_stroke.color = TEXT;
}

/// Labeled numeric field: dim label on the left, value on the right.
/// Returns the value response.
pub fn number_field(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
) -> Response {
    field_frame()
        .show(ui, |ui| {
            ui.set_width(width - 16.0);
            ui.set_height(FIELD);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if !label.is_empty() {
                    let galley =
                        ui.painter()
                            .layout_no_wrap(label.to_owned(), font(BODY), TEXT_DIM);
                    let (slot, _) =
                        ui.allocate_exact_size(vec2(galley.size().x, FIELD - 6.0), Sense::hover());
                    ui.painter().galley(
                        egui::pos2(slot.left(), slot.center().y - galley.size().y / 2.0),
                        galley,
                        TEXT_DIM,
                    );
                }
                plain_drag_style(ui);
                // Keep the value left-aligned rather than centered in a minimum-width button.
                ui.spacing_mut().interact_size.x = 0.0;
                ui.visuals_mut().widgets.inactive.fg_stroke.color = TEXT;
                ui.spacing_mut().button_padding = vec2(2.0, 0.0);
                let value_width = ui.available_width();
                ui.allocate_ui_with_layout(
                    vec2(value_width, FIELD - 6.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let response = ui.add(egui::DragValue::new(value).speed(1.0).range(range));
                        if label.is_empty() {
                            response
                        } else {
                            response.on_hover_text(label)
                        }
                    },
                )
                .inner
            })
            .inner
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_icon_decodes_for_the_window_and_dashboard() {
        let icon = app_icon();
        assert_eq!([icon.width, icon.height], [256, 256]);
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn macos_window_icon_has_transparent_corners() {
        let icon =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/papercraft-macos.png"))
                .unwrap();
        let alpha = |x: u32, y: u32| icon.rgba[((y * icon.width + x) * 4 + 3) as usize];
        assert_eq!([icon.width, icon.height], [512, 512]);
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(60, 60), 0);
        assert_eq!(alpha(256, 256), 255);
    }

    #[test]
    fn hexadecimal_colors_round_trip_and_reject_invalid_input() {
        assert_eq!(hex([40, 40, 40, 255]), "282828");
        assert_eq!(parse_hex("282828", 255), Some([40, 40, 40, 255]));
        assert_eq!(parse_hex("#fff", 128), Some([255, 255, 255, 128]));
        assert_eq!(parse_hex(" 0a0B0c80 ", 255), Some([10, 11, 12, 128]));
        for invalid in ["", "12", "12345", "GGGGGG", "#12345678a", "€€€"] {
            assert_eq!(parse_hex(invalid, 255), None, "{invalid}");
        }
    }
}
