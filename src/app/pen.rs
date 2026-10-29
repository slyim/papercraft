//! Pen drafts and point-edit previews never mutate the authoritative document.
use super::*;
use crate::vector::{self, Anchor, PointMode};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct PenState {
    pub draft: Option<Draft>,
    pub edit: Option<u64>,
    pub selected: BTreeSet<usize>,
    pub drag: Option<PointDrag>,
    pub preview: Option<Document>,
}
pub(super) struct Draft {
    anchors: Vec<Anchor>,
    parent: Option<u64>,
    press: Option<(usize, Pos2)>,
}
#[derive(Clone, Copy)]
enum Hit {
    Anchor(usize),
    Handle(usize, bool),
}
pub(super) struct PointDrag {
    node: Node,
    anchors: Vec<Anchor>,
    start: Pos2,
    target: Hit,
}
impl PenState {
    pub fn busy(&self) -> bool {
        self.draft.is_some() || self.drag.is_some()
    }
    pub fn cancel(&mut self) {
        self.draft = None;
        self.drag = None;
        self.preview = None;
    }
}
fn constrained(origin: Pos2, p: Pos2, shift: bool) -> Pos2 {
    if !shift {
        return p;
    }
    let d = p - origin;
    let angle = (d.angle() / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
    origin + Vec2::angled(angle) * d.length()
}
impl PaperCraftApp {
    pub(super) fn begin_path_edit(&mut self, id: u64) {
        self.flush_properties();
        if self.editor.document.nodes.iter().any(|n| {
            n.id == id && n.kind.is_path() && !self.editor.document.is_effectively_locked(n)
        }) {
            self.pen = PenState {
                edit: Some(id),
                ..Default::default()
            };
            self.tool = Tool::Select;
            self.editor.select(Some(id));
            self.status =
                "Edit points · drag anchors/handles · click a segment to insert · Esc exits".into();
        }
    }
    pub(super) fn finish_pen(&mut self, closed: bool) {
        let Some(draft) = self.pen.draft.take() else {
            return;
        };
        if draft.anchors.len() < 2 {
            self.status = "Path discarded: place at least two anchors.".into();
            return;
        }
        let mut node = Node {
            id: self.editor.document.next_id,
            name: "Path".into(),
            kind: NodeKind::Path,
            closed,
            fill: [0; 4],
            stroke: Some([255, 107, 43, 255]),
            stroke_width: 3.0,
            parent: draft.parent,
            ..Node::default()
        };
        if let Err(error) = vector::set_world_anchors(&mut node, &draft.anchors) {
            self.pen.draft = Some(draft);
            self.status = error;
            return;
        }
        let id = node.id;
        let result = self.editor.apply(|doc| {
            doc.next_id = doc
                .next_id
                .checked_add(1)
                .ok_or("Layer identifiers exhausted.")?;
            doc.nodes.push(node);
            Ok(())
        });
        if result.is_ok() {
            self.editor.select(Some(id));
            self.tool = Tool::Select;
            self.status = "Path created · Enter to edit points".into();
        } else {
            self.pen.draft = Some(draft);
        }
        self.result(result);
    }
    fn replace_path_anchors(&mut self, id: u64, anchors: &[Anchor]) -> Result<(), String> {
        self.editor.apply(|doc| {
            let old = doc
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or("Path no longer exists.")?;
            if doc.is_effectively_locked(old) {
                return Err("Unlock the path and its ancestors to edit points.".into());
            }
            let node = doc
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .expect("existing path");
            vector::set_world_anchors(node, anchors)
        })
    }
    pub(super) fn set_point_mode(&mut self, mode: PointMode) {
        if self.pen.selected.is_empty() {
            return;
        }
        let Some(id) = self.pen.edit else {
            return;
        };
        let Some(node) = self.editor.document.nodes.iter().find(|n| n.id == id) else {
            return;
        };
        let mut anchors = vector::world_anchors(node);
        let count = anchors.len();
        for &index in &self.pen.selected {
            if index >= count {
                continue;
            }
            let center = anchors[index].pos();
            let previous = if index == 0 && !node.closed {
                center
            } else {
                anchors[(index + count - 1) % count].pos()
            };
            let next = if index + 1 == count && !node.closed {
                center
            } else {
                anchors[(index + 1) % count].pos()
            };
            let mut direction = (next - previous) * 0.15;
            if direction.length_sq() < 1e-6 {
                direction = (anchors[(index + 1) % count].pos() - center) * 0.15;
            }
            let a = &mut anchors[index];
            a.mode = mode;
            if mode != PointMode::Corner {
                let p = a.outgoing.map_or(center + direction, |p| pos2(p[0], p[1]));
                if a.incoming.is_none() {
                    a.incoming = Some([center.x - direction.x, center.y - direction.y]);
                }
                a.set_handle(false, p, false);
            }
        }
        let r = self.replace_path_anchors(id, &anchors);
        self.result(r);
    }
    /// True when a Pen-specific key was consumed, so layer shortcuts cannot also run.
    pub(super) fn pen_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if ctx.egui_wants_keyboard_input() {
            return false;
        }
        let escape = (self.pen.busy() || self.pen.edit.is_some())
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if escape && (self.pen.busy() || self.pen.edit.is_some()) {
            self.pen = PenState::default();
            self.tool = Tool::Select;
            self.status = "Point editing ended; uncommitted path discarded.".into();
            return true;
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
        if self.pen.draft.is_some() {
            if delta != Vec2::ZERO {
                if let Some(a) = self.pen.draft.as_mut().and_then(|d| d.anchors.last_mut()) {
                    a.translate(delta);
                }
                return true;
            }
            if ctx.input(|i| {
                (i.modifiers.command || i.modifiers.ctrl)
                    && (i.key_pressed(Key::A) || i.key_pressed(Key::D))
            }) {
                return true;
            }
            if ctx.input(|i| {
                i.key_pressed(Key::Backspace)
                    || i.key_pressed(Key::Delete)
                    || ((i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Z))
            }) {
                if let Some(d) = &mut self.pen.draft {
                    d.anchors.pop();
                }
                return true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                self.finish_pen(false);
                return true;
            }
        }
        let Some(id) = self.pen.edit else {
            return false;
        };
        if self.editor.selected() != Some(id) {
            self.pen = PenState::default();
            return false;
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::A)) {
            if let Some(node) = self.editor.selected_node() {
                self.pen.selected = (0..vector::world_anchors(node).len()).collect();
            }
            return true;
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            self.pen = PenState::default();
            return true;
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            if self.pen.selected.is_empty() {
                return true;
            }
            let Some(node) = self.editor.selected_node() else {
                return true;
            };
            let mut anchors = vector::world_anchors(node);
            let mut i = 0;
            anchors.retain(|_| {
                let keep = !self.pen.selected.contains(&i);
                i += 1;
                keep
            });
            let r = if anchors.len() < 2 {
                self.editor.delete()
            } else {
                self.replace_path_anchors(id, &anchors)
            };
            self.pen.selected.clear();
            self.result(r);
            return true;
        }
        if delta != Vec2::ZERO {
            if self.pen.selected.is_empty() {
                return true;
            }
            if let Some(node) = self.editor.selected_node() {
                let mut a = vector::world_anchors(node);
                for &i in &self.pen.selected {
                    if let Some(a) = a.get_mut(i) {
                        a.translate(delta);
                    }
                }
                let r = self.replace_path_anchors(id, &a);
                self.result(r);
            }
            return true;
        }
        false
    }
    pub(super) fn pen_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &Painter,
        origin: Pos2,
        panning: bool,
    ) -> bool {
        let drawing = self.tool == Tool::Pen;
        if !drawing && self.pen.edit.is_none() {
            return false;
        }
        if let Some(id) = self.pen.edit
            && !self.editor.document.nodes.iter().any(|n| {
                n.id == id
                    && self.editor.selected() == Some(id)
                    && !self.editor.document.is_effectively_locked(n)
            })
        {
            self.pen = PenState::default();
            return false;
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let modifiers = ui.input(|i| i.modifiers);
        let zoom = self.zoom;
        let to_world = |p: Pos2| ((p - origin) / zoom).to_pos2();
        let active = !panning && !self.ignore_canvas_pointer && self.pending.is_none();
        if active
            && ui.input(|i| i.pointer.primary_pressed())
            && let Some(p) = pointer.filter(|p| response.rect.contains(*p))
        {
            let mut world = to_world(p);
            if drawing {
                if self.snapping && !modifiers.command && !modifiers.ctrl {
                    let [x, y] = self
                        .creation_snapper()
                        .snap_point([world.x, world.y], SNAP_DISTANCE / self.zoom);
                    world = pos2(x, y);
                }
                if let Some(d) = &self.pen.draft
                    && d.anchors.len() >= 2
                    && d.anchors[0].pos().distance(to_world(p)) * self.zoom <= 7.0
                {
                    self.finish_pen(true);
                    self.ignore_canvas_pointer = true;
                    return true;
                }
                if let Some(d) = &self.pen.draft
                    && let Some(last) = d.anchors.last()
                {
                    world = constrained(last.pos(), world, modifiers.shift);
                }
                let parent = self
                    .editor
                    .document
                    .nodes
                    .iter()
                    .rev()
                    .find(|n| {
                        n.kind.is_frame()
                            && !self.editor.document.is_effectively_hidden(n)
                            && !self.editor.document.is_effectively_locked(n)
                            && Rect::from_min_size(pos2(n.x, n.y), vec2(n.width, n.height))
                                .contains(world)
                    })
                    .map(|n| n.id);
                let draft = self.pen.draft.get_or_insert(Draft {
                    anchors: Vec::new(),
                    parent,
                    press: None,
                });
                if draft.anchors.len() < vector::MAX_ANCHORS
                    && !draft
                        .anchors
                        .last()
                        .is_some_and(|a| a.pos().distance(world) * self.zoom < 3.0)
                {
                    draft.anchors.push(Anchor::corner(world));
                    draft.press = Some((draft.anchors.len() - 1, to_world(p)));
                }
            } else if let Some(node) = self.editor.selected_node().cloned() {
                let anchors = vector::world_anchors(&node);
                let hit = anchors.iter().enumerate().find_map(|(i, a)| {
                    if a.pos().distance(world) * self.zoom <= 7.0 {
                        return Some(Hit::Anchor(i));
                    }
                    if self.pen.selected.contains(&i) {
                        for (incoming, p) in [(true, a.incoming), (false, a.outgoing)] {
                            if p.is_some_and(|p| {
                                pos2(p[0], p[1]).distance(world) * self.zoom <= 7.0
                            }) {
                                return Some(Hit::Handle(i, incoming));
                            }
                        }
                    }
                    None
                });
                if let Some(target) = hit {
                    if let Hit::Anchor(i) = target {
                        if modifiers.shift {
                            if !self.pen.selected.insert(i) {
                                self.pen.selected.remove(&i);
                            }
                        } else if !self.pen.selected.contains(&i) {
                            self.pen.selected.clear();
                            self.pen.selected.insert(i);
                        }
                        if modifiers.alt {
                            let mut a = anchors.clone();
                            a[i].incoming = None;
                            a[i].outgoing = None;
                            a[i].mode = PointMode::Corner;
                            let r = self.replace_path_anchors(node.id, &a);
                            self.result(r);
                            return true;
                        }
                    }
                    self.pen.drag = Some(PointDrag {
                        node,
                        anchors,
                        start: world,
                        target,
                    });
                } else if let Some((index, t, distance)) =
                    vector::nearest_segment(&anchors, node.closed, world)
                        .filter(|v| v.2 * self.zoom <= 7.0)
                {
                    let _ = distance;
                    let mut a = anchors;
                    let r = vector::split_segment(&mut a, index, t, node.closed).and_then(|i| {
                        self.pen.selected.clear();
                        self.pen.selected.insert(i);
                        self.replace_path_anchors(node.id, &a)
                    });
                    self.result(r);
                } else {
                    self.pen = PenState::default();
                    return true;
                }
            }
        }
        if active && let Some(p) = pointer {
            let world = to_world(p);
            if let Some(d) = &mut self.pen.draft
                && let Some((i, start)) = d.press
            {
                let center = d.anchors[i].pos();
                if start.distance(world) * self.zoom > 3.0 {
                    let p = constrained(center, center + (world - start), modifiers.shift);
                    d.anchors[i].mode = PointMode::Symmetric;
                    d.anchors[i].set_handle(false, p, false);
                }
                if ui.input(|i| i.pointer.primary_released()) {
                    d.press = None;
                }
            }
            if let Some(drag) = &self.pen.drag {
                let mut anchors = drag.anchors.clone();
                match drag.target {
                    Hit::Anchor(_) => {
                        let delta = constrained(drag.start, world, modifiers.shift) - drag.start;
                        for &i in &self.pen.selected {
                            if let Some(a) = anchors.get_mut(i) {
                                a.translate(delta);
                            }
                        }
                    }
                    Hit::Handle(i, incoming) => {
                        if drag.start.distance(world) * self.zoom > 3.0 {
                            let old = if incoming {
                                anchors[i].incoming
                            } else {
                                anchors[i].outgoing
                            }
                            .expect("hit handle");
                            let p = constrained(
                                anchors[i].pos(),
                                pos2(old[0], old[1]) + (world - drag.start),
                                modifiers.shift,
                            );
                            anchors[i].set_handle(incoming, p, modifiers.alt);
                        }
                    }
                }
                let mut preview = self.editor.document.clone();
                let node = preview
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == drag.node.id)
                    .expect("editing node");
                if anchors != drag.anchors {
                    let result =
                        vector::set_world_anchors(node, &anchors).and_then(|()| preview.validate());
                    match result {
                        Ok(()) => self.pen.preview = Some(preview),
                        Err(error) => {
                            self.pen.preview = None;
                            self.status = error;
                        }
                    }
                } else {
                    self.pen.preview = None;
                }
                if ui.input(|i| i.pointer.primary_released()) {
                    self.pen.drag = None;
                    if let Some(preview) = self.pen.preview.take() {
                        let r = self.editor.apply(|doc| {
                            *doc = preview;
                            Ok(())
                        });
                        self.result(r);
                    }
                }
            }
            if drawing
                && response.double_clicked()
                && self
                    .pen
                    .draft
                    .as_ref()
                    .is_some_and(|d| d.anchors.len() >= 2)
            {
                self.finish_pen(false);
            }
        }
        let displayed = self.pen.preview.as_ref().unwrap_or(&self.editor.document);
        let anchors = if let Some(d) = &self.pen.draft {
            d.anchors.clone()
        } else {
            self.pen
                .edit
                .and_then(|id| displayed.nodes.iter().find(|n| n.id == id))
                .map_or(Vec::new(), vector::world_anchors)
        };
        let screen = |p: Pos2| origin + p.to_vec2() * self.zoom;
        if drawing {
            let mut preview = Node {
                kind: NodeKind::Path,
                ..Node::default()
            };
            let mut a = anchors.clone();
            if active
                && let Some(p) = pointer
                && self.pen.draft.as_ref().is_some_and(|d| d.press.is_none())
            {
                a.push(Anchor::corner(constrained(
                    a.last().map_or(to_world(p), Anchor::pos),
                    to_world(p),
                    modifiers.shift,
                )));
            }
            if a.len() >= 2
                && vector::set_world_anchors(&mut preview, &a).is_ok()
                && let Ok(points) = vector::flatten(&preview)
            {
                painter.add(egui::Shape::line(
                    points.into_iter().map(screen).collect(),
                    Stroke::new(1.5, ACCENT),
                ));
            }
        }
        for (i, a) in anchors.iter().enumerate() {
            let selected = drawing || self.pen.selected.contains(&i);
            if selected {
                for p in [a.incoming, a.outgoing].into_iter().flatten() {
                    let handle = screen(pos2(p[0], p[1]));
                    painter.line_segment([screen(a.pos()), handle], Stroke::new(1.0, ACCENT));
                    painter.circle_filled(handle, 3.5, Color32::WHITE);
                    painter.circle_stroke(handle, 3.5, Stroke::new(1.0, ACCENT));
                }
            }
            let r = Rect::from_center_size(screen(a.pos()), vec2(7.0, 7.0));
            painter.rect_filled(r, 1.0, if selected { ACCENT } else { Color32::WHITE });
            painter.rect_stroke(r, 1.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
        }
        let hint = if drawing {
            "Pen · click corners / drag curves · Enter finishes · Esc cancels"
        } else {
            "Edit points · click segment to insert · Alt breaks handles · Esc exits"
        };
        painter.text(
            response.rect.left_bottom() + vec2(16.0, -16.0),
            Align2::LEFT_BOTTOM,
            hint,
            FontId::proportional(CAPTION),
            TEXT,
        );
        if response.hovered() && !panning {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if self
            .pan_button
            .is_some_and(|b| !ui.input(|i| i.pointer.button_down(b)))
        {
            self.pan_button = None;
        }
        true
    }
}
