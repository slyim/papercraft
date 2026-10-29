use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Node, NodeKind},
};

fn run(ctx: &egui::Context, app: &mut PaperCraftApp, events: Vec<Event>) -> egui::FullOutput {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1380.0, 860.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    out
}
fn key(k: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .into()
}
fn click(ctx: &egui::Context, app: &mut PaperCraftApp, p: Pos2) {
    for pressed in [true, false] {
        run(
            ctx,
            app,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    run(ctx, app, vec![]);
}
fn setup() -> (egui::Context, PaperCraftApp, Rect) {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::default());
    let out = run(&ctx, &mut app, vec![]);
    let board = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgb(248, 247, 243) => {
                Some(r.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap();
    (ctx, app, board)
}
#[test]
fn pen_authors_multiple_clicks_as_one_transaction_and_escape_preserves_redo() {
    let (ctx, mut app, board) = setup();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    for p in [at(90.0, 90.0), at(230.0, 110.0), at(180.0, 260.0)] {
        click(&ctx, &mut app, p);
    }
    assert!(
        app.editor.document.nodes.is_empty(),
        "draft must not leak into authoritative state"
    );
    run(&ctx, &mut app, key(Key::Enter));
    assert_eq!(app.editor.document.nodes.len(), 1);
    let n = &app.editor.document.nodes[0];
    assert_eq!(n.kind, NodeKind::Path);
    assert_eq!(n.points.len(), 3);
    assert!((n.x - 90.0).abs() < 0.1);
    assert!((n.width - 140.0).abs() < 0.1);
    app.editor.undo();
    assert!(app.editor.document.nodes.is_empty());
    assert!(app.editor.can_redo());
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(120.0, 120.0));
    run(&ctx, &mut app, key(Key::Escape));
    assert!(app.editor.document.nodes.is_empty());
    assert!(app.editor.can_redo());
}
#[test]
fn closed_concave_fill_is_pickable_only_inside_the_outline() {
    let d = Document {
        next_id: 2,
        nodes: vec![Node {
            id: 1,
            kind: NodeKind::Path,
            width: 100.0,
            height: 100.0,
            points: vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 0.3],
                [0.3, 0.3],
                [0.3, 1.0],
                [0.0, 1.0],
            ],
            closed: true,
            fill: [255; 4],
            ..Node::default()
        }],
        ..Document::default()
    };
    assert_eq!(d.hit_test([10.0, 60.0]), Some(1));
    assert_eq!(d.hit_test([60.0, 60.0]), None);
}
#[test]
fn dragging_during_placement_persists_bezier_controls_in_v4() {
    let (ctx, mut app, board) = setup();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    let a = at(150.0, 170.0);
    let b = at(210.0, 110.0);
    run(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(a),
            Event::PointerButton {
                pos: a,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    run(&ctx, &mut app, vec![Event::PointerMoved(b)]);
    run(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    click(&ctx, &mut app, at(370.0, 240.0));
    click(&ctx, &mut app, a);
    let n = &app.editor.document.nodes[0];
    assert!(
        n.closed,
        "Two-anchor curves can close without a duplicate anchor"
    );
    assert_eq!(n.anchors.len(), 2);
    let json = serde_json::to_value(n).unwrap();
    assert!(json["anchors"][0]["outgoing"].is_array(), "{json}");
    assert_eq!(app.editor.document.required_version(), 4);
    let svg = app.editor.document.export_svg().unwrap();
    assert!(svg.contains(" C "), "{svg}");
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("curve.papercraft");
    app.editor.document.save(&p).unwrap();
    assert_eq!(Document::load(&p).unwrap(), app.editor.document);
}
fn button(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
#[test]
fn point_previews_cancel_then_move_insert_delete_and_undo_without_layer_shortcut_leaks() {
    let (ctx, mut app, board) = setup();
    app.snapping = false;
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    for p in [at(90.0, 90.0), at(230.0, 110.0), at(180.0, 260.0)] {
        click(&ctx, &mut app, p);
    }
    run(&ctx, &mut app, key(Key::Enter));
    run(&ctx, &mut app, key(Key::Enter));
    let before = app.editor.document.clone();
    let revision = app.editor.revision();
    let a = at(90.0, 90.0);
    let b = at(100.0, 130.0);
    run(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(a), button(a, true)],
    );
    run(&ctx, &mut app, vec![Event::PointerMoved(b)]);
    assert_eq!(app.editor.document, before);
    run(&ctx, &mut app, key(Key::Escape));
    run(&ctx, &mut app, vec![button(b, false)]);
    assert_eq!(app.editor.document, before);
    assert_eq!(app.editor.revision(), revision);
    run(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(a), button(a, true)],
    );
    run(&ctx, &mut app, vec![Event::PointerMoved(b)]);
    run(&ctx, &mut app, vec![button(b, false)]);
    let points = papercraft::vector::world_anchors(&app.editor.document.nodes[0]);
    assert!(points[0].pos().distance(egui::pos2(100.0, 130.0)) < 0.1);
    assert!(points[1].pos().distance(egui::pos2(230.0, 110.0)) < 0.1);
    app.editor.undo();
    assert_eq!(app.editor.document, before);
    app.editor.redo();
    app.editor.select(Some(1));
    click(&ctx, &mut app, at(205.0, 185.0));
    assert_eq!(app.editor.document.nodes[0].points.len(), 4);
    run(&ctx, &mut app, key(Key::Delete));
    assert_eq!(app.editor.document.nodes.len(), 1);
    assert_eq!(app.editor.document.nodes[0].points.len(), 3);
    app.editor.undo();
    assert_eq!(app.editor.document.nodes[0].points.len(), 4);
}
#[test]
fn pen_draft_anchor_removal_and_focus_loss_do_not_touch_redo() {
    let (ctx, mut app, board) = setup();
    app.editor.add(NodeKind::Rectangle).unwrap();
    app.editor.undo();
    let before = app.editor.document.clone();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(100.0, 100.0));
    click(&ctx, &mut app, at(230.0, 140.0));
    run(&ctx, &mut app, key(Key::Backspace));
    run(&ctx, &mut app, key(Key::Enter));
    assert_eq!(app.editor.document, before);
    assert!(app.editor.can_redo());
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(100.0, 100.0));
    run(&ctx, &mut app, vec![Event::WindowFocused(false)]);
    run(&ctx, &mut app, key(Key::Enter));
    assert_eq!(app.editor.document, before);
    assert!(app.editor.can_redo());
}
#[test]
fn snapped_clicks_are_corners_not_accidental_dragged_handles() {
    let (ctx, mut app, board) = setup();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(5.0, 100.0));
    click(&ctx, &mut app, at(220.0, 200.0));
    run(&ctx, &mut app, key(Key::Enter));
    assert_eq!(
        app.editor.document.required_version(),
        3,
        "snapping must not synthesize click-drag handles"
    );
    assert!(app.editor.document.nodes[0].anchors.is_empty());
    assert_eq!(app.editor.document.nodes[0].x, 0.0);
}
#[test]
fn authored_point_gestures_and_nudges_are_world_stable_across_zooms() {
    let (ctx, mut app, mut board) = setup();
    app.snapping = false;
    for factor in [0.5, 2.0, 2.0] {
        let out = run(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(board.center()), Event::Zoom(factor)],
        );
        board = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgb(248, 247, 243) => {
                    Some(r.rect)
                }
                _ => None,
            })
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap();
        let scale = board.width() / 960.0;
        let at = |x, y| board.min + vec2(x, y) * scale;
        run(&ctx, &mut app, key(Key::P));
        for p in [at(420.0, 270.0), at(540.0, 290.0), at(500.0, 410.0)] {
            click(&ctx, &mut app, p);
        }
        run(&ctx, &mut app, key(Key::Enter));
        run(&ctx, &mut app, key(Key::Enter));
        let a = at(420.0, 270.0);
        let b = at(435.0, 285.0);
        run(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(a), button(a, true)],
        );
        run(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        run(&ctx, &mut app, vec![button(b, false)]);
        run(&ctx, &mut app, key(Key::ArrowRight));
        let n = app.editor.selected_node().unwrap();
        let a = papercraft::vector::world_anchors(n);
        assert!(a[0].pos().distance(egui::pos2(436.0, 285.0)) < 0.1);
        assert!(a[1].pos().distance(egui::pos2(540.0, 290.0)) < 0.1);
        run(&ctx, &mut app, key(Key::Escape));
    }
    assert_eq!(app.editor.document.nodes.len(), 3);
}
#[test]
fn draft_nudging_does_not_move_the_previously_selected_frame() {
    let (ctx, mut app, board) = setup();
    app.editor.add(NodeKind::Frame).unwrap();
    let before = app.editor.document.clone();
    let revision = app.editor.revision();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(450.0, 300.0));
    click(&ctx, &mut app, at(570.0, 330.0));
    run(&ctx, &mut app, key(Key::ArrowRight));
    assert_eq!(app.editor.document, before);
    assert_eq!(app.editor.revision(), revision);
    run(&ctx, &mut app, key(Key::Enter));
    let anchors = papercraft::vector::world_anchors(app.editor.selected_node().unwrap());
    assert!(anchors[1].pos().distance(egui::pos2(571.0, 330.0)) < 0.1);
    app.editor.undo();
    assert_eq!(app.editor.document, before);
}
#[test]
fn smooth_endpoint_conversion_exposes_draggable_handles_on_a_straight_line() {
    let (ctx, mut app, board) = setup();
    let scale = board.width() / 960.0;
    let at = |x, y| board.min + vec2(x, y) * scale;
    run(&ctx, &mut app, key(Key::P));
    click(&ctx, &mut app, at(450.0, 300.0));
    click(&ctx, &mut app, at(570.0, 330.0));
    run(&ctx, &mut app, key(Key::Enter));
    run(&ctx, &mut app, key(Key::Enter));
    click(&ctx, &mut app, at(450.0, 300.0));
    let out = run(&ctx, &mut app, vec![]);
    let button = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Smooth" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Smooth point control is visible");
    let before = app.editor.document.clone();
    click(&ctx, &mut app, button);
    let a = &papercraft::vector::world_anchors(app.editor.selected_node().unwrap())[0];
    let h = a.outgoing.unwrap();
    assert!(
        a.pos().distance(egui::pos2(h[0], h[1])) > 10.0,
        "Endpoint handles must not collapse onto the anchor"
    );
    assert_eq!(app.editor.document.required_version(), 4);
    app.editor.undo();
    assert_eq!(app.editor.document, before);
}
