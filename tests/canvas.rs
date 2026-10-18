use eframe::egui;
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Editor, NodeKind},
    snap::{self, Handle, Side, Snapper},
};

const SIZES: [[f32; 2]; 2] = [[1380.0, 860.0], [960.0, 640.0]];
/// Snapping guide color used by the canvas.
const GUIDE: egui::Color32 = egui::Color32::from_rgb(242, 78, 120);

/// A at (100, 100), B at (300, 100), hidden C at (100, 300), locked D at (300, 300);
/// each 100 × 100.
fn layout() -> Document {
    let mut editor = Editor::new(Document::default());
    for (x, y) in [
        (100.0, 100.0),
        (300.0, 100.0),
        (100.0, 300.0),
        (300.0, 300.0),
    ] {
        editor
            .add_at(NodeKind::Rectangle, x, y, 100.0, 100.0)
            .unwrap();
    }
    editor.document.nodes[2].hidden = true;
    editor.document.nodes[3].locked = true;
    editor.document
}

fn ids(app: &PaperCraftApp) -> Vec<u64> {
    app.editor.document.nodes.iter().map(|n| n.id).collect()
}

fn geometry(app: &PaperCraftApp, index: usize) -> [f32; 4] {
    let n = &app.editor.document.nodes[index];
    [n.x, n.y, n.width, n.height]
}

fn frame(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    modifiers: egui::Modifiers,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut all = vec![egui::Event::ModifiersChanged(modifiers)];
    all.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0], size[1]),
        )),
        events: all,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    output
}

fn button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}

/// Presses at `start` with `press` modifiers, drags to `end` holding `held`, and
/// releases. Returns the output of the last frame before release.
fn gesture(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    press: egui::Modifiers,
    held: egui::Modifiers,
    start: egui::Pos2,
    end: egui::Pos2,
) -> egui::FullOutput {
    frame(
        ctx,
        app,
        size,
        press,
        vec![egui::Event::PointerMoved(start), button(start, true, press)],
    );
    frame(
        ctx,
        app,
        size,
        held,
        vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
    );
    let last = frame(ctx, app, size, held, vec![egui::Event::PointerMoved(end)]);
    frame(ctx, app, size, held, vec![button(end, false, held)]);
    frame(ctx, app, size, egui::Modifiers::NONE, vec![]);
    last
}

fn artboard(ctx: &egui::Context, app: &mut PaperCraftApp, size: [f32; 2]) -> egui::Rect {
    let fill = egui::Color32::from_rgb(248, 247, 243);
    frame(ctx, app, size, egui::Modifiers::NONE, Vec::new())
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap()
}

fn guides(output: &egui::FullOutput) -> usize {
    output
        .shapes
        .iter()
        .filter(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == GUIDE))
        .count()
}

struct Canvas {
    ctx: egui::Context,
    app: PaperCraftApp,
    size: [f32; 2],
    board: egui::Rect,
    scale: f32,
}

impl Canvas {
    fn new(size: [f32; 2]) -> Self {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut app = PaperCraftApp::from_document(layout());
        let board = artboard(&ctx, &mut app, size);
        let scale = board.width() / app.editor.document.width;
        Self {
            ctx,
            app,
            size,
            board,
            scale,
        }
    }

    fn at(&self, x: f32, y: f32) -> egui::Pos2 {
        self.board.min + egui::vec2(x, y) * self.scale
    }

    fn drag(
        &mut self,
        press: egui::Modifiers,
        held: egui::Modifiers,
        from: [f32; 2],
        to: [f32; 2],
    ) -> egui::FullOutput {
        let (start, end) = (self.at(from[0], from[1]), self.at(to[0], to[1]));
        gesture(&self.ctx, &mut self.app, self.size, press, held, start, end)
    }
}

const NONE: egui::Modifiers = egui::Modifiers::NONE;
const SHIFT: egui::Modifiers = egui::Modifiers::SHIFT;
const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;

#[test]
fn marquee_selects_visible_unlocked_layers_and_shift_adds() {
    for size in SIZES {
        let mut c = Canvas::new(size);
        let all = ids(&c.app);
        let history = c.app.editor.can_undo();
        c.drag(NONE, NONE, [50.0, 50.0], [250.0, 450.0]);
        assert_eq!(
            c.app.editor.selection(),
            &all[..1],
            "{size:?}: hidden C is skipped"
        );
        c.drag(SHIFT, SHIFT, [280.0, 50.0], [450.0, 450.0]);
        assert_eq!(
            c.app.editor.selection(),
            &all[..2],
            "{size:?}: Shift adds B; locked D is skipped"
        );
        c.drag(NONE, NONE, [450.0, 450.0], [470.0, 470.0]);
        assert!(c.app.editor.selection().is_empty(), "{size:?}");
        assert_eq!(c.app.editor.can_undo(), history, "selection is not history");
    }
}

#[test]
fn corner_and_edge_handles_resize_one_layer_as_one_step() {
    for size in SIZES {
        let mut c = Canvas::new(size);
        c.app.snapping = false;
        let a = ids(&c.app)[0];
        c.app.editor.select(Some(a));
        let original = c.app.editor.document.clone();

        c.drag(NONE, NONE, [100.0, 100.0], [80.0, 90.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [80.0, 90.0, 120.0, 110.0],
            "{size:?} top-left"
        );
        c.app.editor.undo();
        assert_eq!(c.app.editor.document, original);

        c.drag(NONE, NONE, [200.0, 150.0], [230.0, 170.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [100.0, 100.0, 130.0, 100.0],
            "{size:?} right edge"
        );
        c.app.editor.undo();

        c.drag(NONE, NONE, [150.0, 100.0], [150.0, 160.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [100.0, 160.0, 100.0, 40.0],
            "{size:?} top edge"
        );
        c.app.editor.undo();

        // Dragging an edge past the opposite one stops at a width of 1.
        c.drag(NONE, NONE, [100.0, 150.0], [260.0, 150.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [199.0, 100.0, 1.0, 100.0],
            "{size:?} minimum"
        );
        c.app.editor.undo();

        // Shift after the press keeps the proportions of a corner resize.
        c.drag(NONE, SHIFT, [200.0, 200.0], [250.0, 210.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [100.0, 100.0, 150.0, 150.0],
            "{size:?} aspect"
        );
        c.app.editor.undo();
        assert_eq!(c.app.editor.document, original);
        assert_eq!(c.app.editor.selection(), &[a]);
    }
}

#[test]
fn group_handles_scale_the_arrangeable_selection_together() {
    for size in SIZES {
        let mut c = Canvas::new(size);
        c.app.snapping = false;
        let all = ids(&c.app);
        c.app.editor.set_selection(all.clone());
        let original = c.app.editor.document.clone();
        // Bounds of A and B are 100..400 × 100..200; hidden C and locked D are excluded.
        c.drag(NONE, NONE, [400.0, 200.0], [700.0, 300.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [100.0, 100.0, 200.0, 200.0],
            "{size:?}"
        );
        assert_eq!(
            geometry(&c.app, 1),
            [500.0, 100.0, 200.0, 200.0],
            "{size:?}"
        );
        assert_eq!(geometry(&c.app, 2), [100.0, 300.0, 100.0, 100.0]);
        assert_eq!(geometry(&c.app, 3), [300.0, 300.0, 100.0, 100.0]);
        c.app.editor.undo();
        assert_eq!(
            c.app.editor.document, original,
            "one undo restores the group"
        );
    }
}

#[test]
fn moves_and_resizes_snap_to_edges_with_guides_unless_bypassed() {
    for size in SIZES {
        let mut c = Canvas::new(size);
        let b = ids(&c.app)[1];
        // B's left edge lands 3 units from A's right edge and snaps onto it.
        let output = c.drag(NONE, NONE, [350.0, 150.0], [253.0, 150.0]);
        assert_eq!(
            geometry(&c.app, 1)[..2],
            [200.0, 100.0],
            "{size:?} snapped move"
        );
        assert!(guides(&output) > 0, "{size:?}: a guide marks the snap");
        assert_eq!(c.app.editor.selection(), &[b]);
        c.app.editor.undo();

        // ⌘/Ctrl bypasses snapping.
        let output = c.drag(NONE, COMMAND, [350.0, 150.0], [253.0, 150.0]);
        assert_eq!(geometry(&c.app, 1)[..2], [203.0, 100.0], "{size:?} bypass");
        assert_eq!(guides(&output), 0);
        c.app.editor.undo();

        // A's right edge resizes onto B's left edge.
        c.app.editor.select(Some(ids(&c.app)[0]));
        c.drag(NONE, NONE, [200.0, 150.0], [297.0, 150.0]);
        assert_eq!(
            geometry(&c.app, 0),
            [100.0, 100.0, 200.0, 100.0],
            "{size:?} resize"
        );
        c.app.editor.undo();

        // The toggle turns snapping off.
        c.app.snapping = false;
        c.app.editor.select(Some(b));
        c.drag(NONE, NONE, [350.0, 150.0], [253.0, 150.0]);
        assert_eq!(geometry(&c.app, 1)[..2], [203.0, 100.0], "{size:?} toggle");
    }
}

#[test]
fn snapping_targets_edges_and_centers_but_not_hidden_or_moving_layers() {
    let snapper = Snapper::new([0.0, 0.0, 960.0, 640.0], [[100.0, 100.0, 200.0, 200.0]]);
    // The left edge would land 3 units from 200, so the delta puts it exactly there.
    assert_eq!(
        snapper.snap_move([300.0, 500.0, 400.0, 600.0], [-97.0, 0.0], 5.0),
        [Some(-100.0), None]
    );
    assert_eq!(
        snapper.snap_move([300.0, 500.0, 400.0, 600.0], [-90.0, 0.0], 5.0),
        [None, None]
    );
    // A center snaps to the artboard center.
    assert_eq!(
        snapper.snap_move([429.0, 0.0, 529.0, 50.0], [0.0, 0.0], 5.0)[0],
        Some(1.0)
    );
    let guides = snapper.guides([200.0, 100.0, 300.0, 200.0]);
    assert!(guides.iter().any(|g| g.vertical && g.value == 200.0));
    assert!(
        guides
            .iter()
            .any(|g| { !g.vertical && g.value == 100.0 && g.low == 100.0 && g.high == 300.0 })
    );

    // In the editor, D moves near hidden C's center and bottom without snapping;
    // nothing else is within reach. Once C is visible, the same move snaps.
    for hidden in [true, false] {
        let mut c = Canvas::new(SIZES[0]);
        c.app.editor.document.nodes[2].hidden = hidden;
        c.app.editor.document.nodes[3].locked = false;
        c.drag(NONE, NONE, [350.0, 350.0], [367.0, 397.0]);
        let expected = if hidden { 347.0 } else { 350.0 };
        assert_eq!(
            geometry(&c.app, 3)[..2],
            [317.0, expected],
            "hidden C: {hidden}"
        );
    }
}

#[test]
fn resize_and_map_geometry() {
    let start = [100.0, 100.0, 200.0, 200.0];
    let corner = Handle::new(Side::Start, Side::Start);
    assert_eq!(
        snap::resize_frame(start, corner, [-20.4, 30.6], false, None),
        [80.0, 131.0, 200.0, 200.0]
    );
    assert_eq!(
        snap::resize_frame(start, corner, [500.0, 500.0], false, None),
        [199.0, 199.0, 200.0, 200.0]
    );
    let aspect = snap::resize_frame(start, corner, [-50.0, -10.0], true, None);
    assert_eq!(aspect, [50.0, 50.0, 200.0, 200.0]);
    // A rect equal to the source bounds maps exactly; others scale and round.
    let to = [0.0, 0.0, 50.5, 300.0];
    assert_eq!(snap::map_rect(start, start, to), to);
    assert_eq!(
        snap::map_rect(
            [150.0, 150.0, 200.0, 200.0],
            start,
            [100.0, 100.0, 300.0, 300.0]
        ),
        [200.0, 200.0, 300.0, 300.0]
    );
    assert!(snap::intersects(start, [200.0, 200.0, 300.0, 300.0]));
    assert!(!snap::intersects(start, [201.0, 0.0, 300.0, 300.0]));
}
