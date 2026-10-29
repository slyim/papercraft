//! Slice 1b: semantic geometry, creation snapping and press-owned navigation.
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use papercraft::{
    app::{PaperCraftApp, Tool},
    document::{Document, Node, NodeKind},
};

fn document(nodes: Vec<Node>) -> Document {
    let mut doc = Document {
        next_id: nodes.len() as u64 + 1,
        nodes,
        ..Document::default()
    };
    doc.normalize();
    doc.validate().unwrap();
    doc
}

fn frame(ctx: &egui::Context, app: &mut PaperCraftApp, events: Vec<Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1380.0, 860.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output
}
fn board(output: &egui::FullOutput) -> Rect {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r)
                if r.fill == egui::Color32::from_rgb(248, 247, 243)
                    && s.clip_rect.min.x == papercraft::ui::SIDEBAR + papercraft::ui::RAIL
                    && s.clip_rect.width() > 200.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap()
}
fn setup(doc: Document) -> (egui::Context, PaperCraftApp, Rect) {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(doc);
    frame(&ctx, &mut app, vec![]);
    let b = board(&frame(&ctx, &mut app, vec![]));
    (ctx, app, b)
}
fn at(b: Rect, x: f32, y: f32) -> Pos2 {
    b.min + vec2(x, y) * (b.width() / 960.0)
}
fn button(pos: Pos2, pressed: bool, which: PointerButton) -> Event {
    Event::PointerButton {
        pos,
        pressed,
        button: which,
        modifiers: Modifiers::NONE,
    }
}
fn key(k: Key, modifiers: Modifiers) -> Vec<Event> {
    let mut events = vec![Event::ModifiersChanged(modifiers)];
    for pressed in [true, false] {
        events.push(Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
    events
}
fn click(ctx: &egui::Context, app: &mut PaperCraftApp, p: Pos2) {
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(p),
            button(p, true, PointerButton::Primary),
        ],
    );
    frame(ctx, app, vec![button(p, false, PointerButton::Primary)]);
}
fn drag(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    a: Pos2,
    b: Pos2,
    modifiers: Modifiers,
) -> egui::FullOutput {
    frame(
        ctx,
        app,
        vec![
            Event::ModifiersChanged(modifiers),
            Event::PointerMoved(a),
            button(a, true, PointerButton::Primary),
        ],
    );
    frame(ctx, app, vec![Event::PointerMoved(a + (b - a) * 0.5)]);
    let output = frame(
        ctx,
        app,
        vec![Event::ModifiersChanged(modifiers), Event::PointerMoved(b)],
    );
    frame(ctx, app, vec![button(b, false, PointerButton::Primary)]);
    output
}

#[test]
fn shape_hits_skip_empty_corners_and_resolved_transparent_interiors() {
    for kind in [
        NodeKind::Rectangle,
        NodeKind::Frame,
        NodeKind::Ellipse,
        NodeKind::Triangle,
    ] {
        let mut doc = document(vec![Node {
            kind,
            radius: if kind.supports_radius() { 40.0 } else { 0.0 },
            ..Node::default()
        }]);
        assert_eq!(doc.hit_test([2.0, 2.0]), None, "{kind:?}: empty corner");
        assert_eq!(doc.hit_test([50.0, 50.0]), Some(1));
        doc.nodes[0].fill[3] = 0;
        assert_eq!(
            doc.hit_test([50.0, 50.0]),
            None,
            "{kind:?}: zero-alpha fill"
        );
        let mut editor = papercraft::document::Editor::new(doc);
        let token = editor.create_token([20, 30, 40, 1]).unwrap();
        editor
            .bind(papercraft::document::ColorTarget::Node(1), Some(token))
            .unwrap();
        assert_eq!(
            editor.document.hit_test([50.0, 50.0]),
            Some(1),
            "nonzero resolved alpha"
        );
        editor.set_token_value(token, [20, 30, 40, 0]).unwrap();
        assert_eq!(editor.document.hit_test([50.0, 50.0]), None);
    }
}

#[test]
fn straight_paths_hit_segments_closure_and_convex_fill_not_the_box() {
    let mut doc = document(vec![Node {
        kind: NodeKind::Path,
        points: vec![[0.0, 0.0], [1.0, 1.0]],
        ..Node::default()
    }]);
    assert_eq!(doc.hit_test([20.0, 80.0]), None);
    assert_eq!(
        doc.hit_test([40.0, 40.0]),
        Some(1),
        "fallback 2-unit stroke"
    );
    assert_eq!(doc.hit_test([40.0, 42.0]), None);
    doc.nodes[0].points = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
    assert_eq!(doc.hit_test([70.0, 30.0]), None, "open path has no fill");
    doc.nodes[0].closed = true;
    assert_eq!(doc.hit_test([70.0, 30.0]), Some(1));
    assert_eq!(doc.hit_test([20.0, 80.0]), None);
    doc.nodes[0].fill[3] = 0;
    doc.nodes[0].stroke = Some([0, 0, 0, 255]);
    doc.nodes[0].stroke_width = 4.0;
    assert_eq!(doc.hit_test([70.0, 30.0]), None, "hollow closed path");
    assert_eq!(doc.hit_test([50.0, 50.0]), Some(1), "closing segment");
    // Pen adds even-odd concave fills without rewriting released vertices.
    doc.nodes[0].points = vec![[0.0, 0.0], [1.0, 0.0], [0.3, 0.3], [0.0, 1.0]];
    doc.nodes[0].fill[3] = 255;
    assert_eq!(
        doc.hit_test([20.0, 20.0]),
        Some(1),
        "concave interior follows the even-odd fill"
    );
}

#[test]
fn strokes_have_inside_rect_and_centered_path_bounds_without_interior_hits() {
    let mut doc = document(vec![Node {
        fill: [0, 0, 0, 0],
        stroke: Some([20, 30, 40, 255]),
        stroke_width: 10.0,
        ..Node::default()
    }]);
    assert_eq!(doc.hit_test([50.0, 50.0]), None);
    assert_eq!(doc.hit_test([4.0, 50.0]), Some(1));
    assert_eq!(doc.hit_test([-2.0, 50.0]), None, "rect stroke stays inside");
    doc.nodes[0].kind = NodeKind::Path;
    doc.nodes[0].points = vec![[0.0, 0.0], [1.0, 0.0]];
    assert_eq!(
        doc.hit_test([50.0, -4.0]),
        Some(1),
        "centered stroke extends outside authored box"
    );
    assert_eq!(
        doc.visible_bounds(&doc.nodes[0]),
        Some([-5.0, -5.0, 105.0, 5.0])
    );
    let png = papercraft::render::rasterize_region(&doc, [-10.0, -10.0, 110.0, 10.0], 1.0).unwrap();
    assert!(
        png.pixels[4 * (60 + 6 * 120) + 3] > 0,
        "culling must include the outside stroke"
    );
}

#[test]
fn hover_click_and_double_click_share_shape_lookup_and_screen_stroke_tolerance() {
    let doc = document(vec![
        Node {
            kind: NodeKind::Text,
            x: 100.0,
            y: 100.0,
            text: "Under ellipse".into(),
            width: 160.0,
            ..Node::default()
        },
        Node {
            id: 2,
            kind: NodeKind::Ellipse,
            x: 100.0,
            y: 100.0,
            ..Node::default()
        },
    ]);
    let (ctx, mut app, b) = setup(doc);
    let p = at(b, 103.0, 103.0);
    let out = frame(&ctx, &mut app, vec![Event::PointerMoved(p)]);
    let text_box = Rect::from_min_max(at(b, 100.0, 100.0), at(b, 260.0, 200.0));
    assert!(
        out.shapes.iter().any(|s| match &s.shape {
            egui::Shape::Mesh(m) =>
                m.calc_bounds().expand(1.0).contains_rect(text_box)
                    && m.vertices
                        .iter()
                        .any(|v| v.color == papercraft::ui::ACCENT.gamma_multiply(0.55)),
            _ => false,
        }),
        "hover highlights the underlying text, not ellipse's bounding corner"
    );
    click(&ctx, &mut app, p);
    assert_eq!(app.editor.selection(), &[1]);
    click(&ctx, &mut app, p);
    frame(&ctx, &mut app, vec![]);
    assert!(
        ctx.egui_wants_keyboard_input(),
        "double-click edits the same text hit"
    );
    frame(&ctx, &mut app, key(Key::Escape, Modifiers::NONE));
    for factor in [0.5, 2.0] {
        let doc = document(vec![Node {
            kind: NodeKind::Path,
            x: 100.0,
            y: 100.0,
            points: vec![[0.0, 0.0], [1.0, 0.0]],
            stroke: Some([0, 0, 0, 255]),
            stroke_width: 2.0,
            ..Node::default()
        }]);
        let (ctx, mut app, b) = setup(doc);
        let pointer = at(b, 150.0, 100.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(pointer)]);
        let b = board(&frame(&ctx, &mut app, vec![Event::Zoom(factor)]));
        let edge = at(b, 150.0, 100.0);
        let half = b.width() / 960.0;
        click(&ctx, &mut app, edge - vec2(0.0, half + 4.0));
        assert_eq!(
            app.editor.selection(),
            &[1],
            "5 screen point tolerance at zoom {factor}"
        );
        app.editor.select(None);
        click(&ctx, &mut app, edge - vec2(0.0, half + 7.0));
        assert!(app.editor.selection().is_empty());
    }
}

#[test]
fn marquee_tests_geometry_after_nested_clips_and_inherits_locks_offboard() {
    // A diagonal enters the query box only outside the inner ancestor's clip.
    let doc = document(vec![
        Node {
            kind: NodeKind::Frame,
            x: -60.0,
            y: 100.0,
            width: 90.0,
            height: 200.0,
            clip: true,
            fill: [0; 4],
            ..Node::default()
        },
        Node {
            id: 2,
            kind: NodeKind::Frame,
            parent: Some(1),
            x: -60.0,
            y: 100.0,
            width: 70.0,
            height: 200.0,
            clip: true,
            fill: [0; 4],
            ..Node::default()
        },
        Node {
            id: 3,
            kind: NodeKind::Path,
            parent: Some(2),
            x: -60.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            ..Node::default()
        },
        Node {
            id: 4,
            kind: NodeKind::Ellipse,
            x: 100.0,
            y: 100.0,
            ..Node::default()
        },
        Node {
            id: 5,
            x: 250.0,
            y: 100.0,
            fill: [0; 4],
            stroke: Some([0, 0, 0, 255]),
            stroke_width: 4.0,
            ..Node::default()
        },
    ]);
    let (ctx, mut app, b) = setup(doc);
    for (from, to, expected) in [
        ((-55.0, 220.0), (0.0, 285.0), vec![]),
        ((102.0, 102.0), (110.0, 110.0), vec![]),
        ((270.0, 120.0), (320.0, 170.0), vec![]),
        ((-65.0, 90.0), (-40.0, 120.0), vec![3]),
    ] {
        app.editor.select(None);
        drag(
            &ctx,
            &mut app,
            at(b, from.0, from.1),
            at(b, to.0, to.1),
            Modifiers::NONE,
        );
        assert_eq!(app.editor.selection(), expected, "query {from:?}..{to:?}");
    }
    for flag in ["locked", "hidden"] {
        if flag == "locked" {
            app.editor.document.nodes[0].locked = true;
        } else {
            app.editor.document.nodes[0].locked = false;
            app.editor.document.nodes[0].hidden = true;
        }
        app.editor.select(None);
        drag(
            &ctx,
            &mut app,
            at(b, -65.0, 90.0),
            at(b, -40.0, 120.0),
            Modifiers::NONE,
        );
        assert!(app.editor.selection().is_empty(), "inherited {flag}");
    }
    assert!(!app.editor.can_undo());
}

#[test]
fn creation_snaps_with_guides_modifiers_bypass_release_and_one_step_undo() {
    for (modifiers, expected) in [
        (Modifiers::NONE, [40.0, 30.0, 160.0, 90.0]),
        (Modifiers::SHIFT, [40.0, 30.0, 160.0, 160.0]),
        (Modifiers::ALT, [-120.0, -60.0, 320.0, 180.0]),
        (
            Modifiers::SHIFT | Modifiers::ALT,
            [-120.0, -130.0, 320.0, 320.0],
        ),
        (Modifiers::CTRL, [40.0, 30.0, 157.0, 87.0]),
        (Modifiers::COMMAND, [40.0, 30.0, 157.0, 87.0]),
    ] {
        let doc = document(vec![Node {
            x: 200.0,
            y: 120.0,
            width: 100.0,
            height: 100.0,
            locked: true,
            ..Node::default()
        }]);
        let (ctx, mut app, b) = setup(doc);
        app.tool = Tool::Rectangle;
        let before = app.editor.document.clone();
        let output = drag(
            &ctx,
            &mut app,
            at(b, 40.0, 30.0),
            at(b, 197.0, 117.0),
            modifiers,
        );
        let n = app.editor.document.nodes.last().unwrap();
        assert_eq!([n.x, n.y, n.width, n.height], expected, "{modifiers:?}");
        let bypass = modifiers.ctrl || modifiers.command;
        assert_eq!(output.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::LineSegment {stroke,..} if stroke.color==egui::Color32::from_rgb(242,78,120))),!bypass,"guides {modifiers:?}");
        let after = app.editor.document.clone();
        app.editor.undo();
        assert_eq!(app.editor.document, before);
        app.editor.redo();
        assert_eq!(app.editor.document, after);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapped.papercraft");
        after.save(&path).unwrap();
        assert_eq!(Document::load(&path).unwrap(), after);
    }
    let (ctx, mut app, b) = setup(Document::default());
    app.tool = Tool::Rectangle;
    click(&ctx, &mut app, at(b, 3.0, 3.0));
    assert_eq!(
        [
            app.editor.document.nodes[0].x,
            app.editor.document.nodes[0].y
        ],
        [0.0, 0.0],
        "click origin snaps too"
    );
}

#[test]
fn navigation_cannot_drift_a_press_or_convert_a_pan_to_creation() {
    let (ctx, mut app, b) = setup(Document::default());
    app.tool = Tool::Rectangle;
    let a = at(b, 100.0, 100.0);
    frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(a),
            button(a, true, PointerButton::Primary),
        ],
    );
    assert_eq!(
        board(&frame(&ctx, &mut app, vec![Event::Zoom(1.5)])),
        b,
        "early press freezes navigation before drag threshold"
    );
    frame(
        &ctx,
        &mut app,
        vec![button(a, false, PointerButton::Primary)],
    );
    app.editor.undo();
    let original = app.editor.document.clone();
    for which in [PointerButton::Primary, PointerButton::Middle] {
        let mut press = key(Key::Space, Modifiers::NONE); // Hold Space explicitly for primary pan.
        press.pop();
        if which == PointerButton::Middle {
            press.clear();
        }
        press.extend([Event::PointerMoved(a), button(a, true, which)]);
        frame(&ctx, &mut app, press);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(a + vec2(20.0, 10.0))],
        );
        let events = vec![
            Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::PointerMoved(a + vec2(40.0, 20.0)),
        ];
        frame(&ctx, &mut app, events);
        let after = board(&frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a + vec2(60.0, 30.0)),
                button(a + vec2(60.0, 30.0), false, which),
            ],
        ));
        assert_eq!(
            app.editor.document, original,
            "pan release must not create ({which:?})"
        );
        assert!(app.editor.can_redo());
        let current = board(&frame(&ctx, &mut app, vec![]));
        assert_eq!(current, after);
    }
}

#[test]
fn fit_selection_uses_clipped_envelope_and_tabs_retain_navigation_only() {
    let doc = document(vec![
        Node {
            kind: NodeKind::Frame,
            x: 10000.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            clip: true,
            ..Node::default()
        },
        Node {
            id: 2,
            parent: Some(1),
            x: 10000.0,
            y: 0.0,
            width: 10000.0,
            height: 10000.0,
            ..Node::default()
        },
    ]);
    let (ctx, mut app, _) = setup(doc);
    let original = app.editor.document.clone();
    app.editor.select(Some(2));
    let b = board(&frame(&ctx, &mut app, key(Key::Num2, Modifiers::SHIFT)));
    assert!(
        (b.width() / 960.0 - 2.0).abs() < 0.01,
        "fit clipped 100-unit child, not authored 10k box"
    );
    frame(&ctx, &mut app, key(Key::N, Modifiers::COMMAND));
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.tab_count(), 2);
    app.activate_tab(0);
    let restored = board(&frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
    ));
    assert_eq!(restored, b);
    assert_eq!(app.editor.document, original);
    assert!(!app.editor.can_undo());
}

#[test]
fn marquee_boundaries_and_pointer_tolerance_never_escape_ancestor_clips() {
    let mut doc = document(vec![
        Node {
            kind: NodeKind::Frame,
            x: -100.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            clip: true,
            fill: [0; 4],
            ..Node::default()
        },
        Node {
            id: 2,
            parent: Some(1),
            kind: NodeKind::Path,
            x: -100.0,
            y: 50.0,
            width: 200.0,
            height: 1.0,
            points: vec![[0.0, 0.0], [1.0, 0.0]],
            stroke: Some([0, 0, 0, 255]),
            stroke_width: 10.0,
            ..Node::default()
        },
    ]);
    assert_eq!(
        doc.marquee_hits([-60.0, 45.0, -40.0, 45.0]),
        vec![2],
        "touching stroke is selected"
    );
    assert!(
        doc.marquee_hits([-60.0, 44.9, -40.0, 44.9]).is_empty(),
        "no marquee tolerance"
    );
    assert_eq!(doc.hit_test_with_tolerance([-50.0, 40.1], 5.0), Some(2));
    assert_eq!(
        doc.hit_test_with_tolerance([0.1, 50.0], 5000.0),
        None,
        "strict clip, not tolerance-expanded clip"
    );
    assert!(doc.marquee_hits([0.1, 40.0, 20.0, 60.0]).is_empty());
    for point in [[f32::NAN, 0.0], [0.0, f32::INFINITY]] {
        assert_eq!(doc.hit_test(point), None);
    }
    for tolerance in [f32::NAN, f32::INFINITY, -1.0] {
        assert_eq!(doc.hit_test_with_tolerance([-50.0, 50.0], tolerance), None);
    }
    doc.nodes[0].clip = false;
    assert_eq!(
        doc.hit_test([50.0, 50.0]),
        Some(2),
        "absolute child geometry is not reinterpreted"
    );
    doc.nodes[0].locked = true;
    assert!(doc.marquee_hits([-100.0, 0.0, 100.0, 100.0]).is_empty());
}

#[test]
fn zero_area_ancestor_clip_has_no_hits_even_on_its_touching_boundary() {
    let doc = document(vec![
        Node {
            kind: NodeKind::Frame,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            clip: true,
            fill: [0; 4],
            ..Node::default()
        },
        Node {
            id: 2,
            kind: NodeKind::Frame,
            parent: Some(1),
            x: 100.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            clip: true,
            fill: [0; 4],
            ..Node::default()
        },
        Node {
            id: 3,
            kind: NodeKind::Path,
            parent: Some(2),
            x: 50.0,
            y: 50.0,
            points: vec![[0.0, 0.0], [1.0, 0.0]],
            stroke: Some([0, 0, 0, 255]),
            stroke_width: 10.0,
            fill: [0; 4],
            ..Node::default()
        },
    ]);
    assert_eq!(doc.visible_bounds(&doc.nodes[2]), None);
    assert_eq!(doc.hit_test_with_tolerance([100.0, 50.0], 5.0), None);
    assert!(doc.marquee_hits([90.0, 40.0, 110.0, 60.0]).is_empty());
}

#[test]
fn zoom_limits_wheel_pan_and_empty_fit_leave_history_and_pointer_anchor_intact() {
    let (ctx, mut app, initial) = setup(Document::default());
    let pointer = at(initial, 250.0, 250.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(pointer)]);
    for factor in [1000.0, 0.000001, 2.0] {
        let before = board(&frame(&ctx, &mut app, vec![]));
        let world = (pointer - before.min) / (before.width() / 960.0);
        let after = board(&frame(&ctx, &mut app, vec![Event::Zoom(factor)]));
        let z = after.width() / 960.0;
        assert!((0.00099..=4.0001).contains(&z));
        assert!(
            (after.min + world * z).distance(pointer) < 0.05,
            "anchored at zoom limit {factor}: before={before:?}, after={after:?}, world={world:?}, pointer={pointer:?}, anchored={:?}",
            after.min + world * z
        );
    }
    frame(&ctx, &mut app, key(Key::Num0, Modifiers::SHIFT));
    let fitted = board(&frame(&ctx, &mut app, key(Key::Num1, Modifiers::SHIFT)));
    assert!(
        fitted.min.distance(initial.min) < 0.01,
        "empty content falls back to artboard"
    );
    let selected = board(&frame(&ctx, &mut app, key(Key::Num2, Modifiers::SHIFT)));
    assert_eq!(selected, fitted, "empty selection fit preserves view");
    // Wheel state is smoothed by egui; check its integrated view delta, not one frame's value.
    let delta = vec2(20.0, -30.0);
    frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                modifiers: Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ],
    );
    for _ in 0..100 {
        frame(&ctx, &mut app, vec![]);
    }
    let panned = board(&frame(&ctx, &mut app, vec![]));
    assert!(panned.min.distance(fitted.min + delta) < 0.1);
    assert_eq!(app.editor.document, Document::default());
    assert!(!app.editor.can_undo());
}

#[test]
fn snapped_creation_cancels_with_redo_and_failed_insertion_preserves_state() {
    for cancel in [
        Event::WindowFocused(false),
        key(Key::Escape, Modifiers::NONE)[1].clone(),
    ] {
        let (ctx, mut app, b) = setup(document(vec![Node {
            x: 200.0,
            y: 120.0,
            ..Node::default()
        }]));
        app.editor.add(NodeKind::Text).unwrap();
        app.editor.undo();
        app.editor.select(Some(1));
        let before = app.editor.document.clone();
        let revision = app.editor.revision();
        app.tool = Tool::Rectangle;
        let (a, end) = (at(b, 40.0, 30.0), at(b, 197.0, 117.0));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a),
                button(a, true, PointerButton::Primary),
            ],
        );
        frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        frame(&ctx, &mut app, vec![cancel]);
        frame(
            &ctx,
            &mut app,
            vec![
                button(end, false, PointerButton::Primary),
                Event::WindowFocused(true),
            ],
        );
        assert_eq!(app.editor.document, before);
        assert_eq!(app.editor.revision(), revision);
        assert_eq!(app.editor.selection(), &[1]);
        assert!(app.editor.can_redo());
        assert!(
            app.editor
                .add_configured_at(
                    NodeKind::Rectangle,
                    [f32::NAN, 0.0, 20.0, 20.0],
                    Default::default(),
                    None,
                    None
                )
                .is_err()
        );
        assert_eq!(app.editor.document, before);
        assert_eq!(app.editor.revision(), revision);
        assert!(app.editor.can_redo());
    }
}

#[test]
fn creation_uses_live_modifiers_and_release_position_but_not_hidden_targets() {
    for hidden in [true, false] {
        let (ctx, mut app, b) = setup(document(vec![Node {
            x: 200.0,
            y: 120.0,
            hidden,
            ..Node::default()
        }]));
        app.tool = Tool::Rectangle;
        let (a, end) = (at(b, 40.0, 30.0), at(b, 197.0, 117.0));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a),
                button(a, true, PointerButton::Primary),
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(at(b, 180.0, 110.0))],
        );
        // Shift arrives only with the final release. No stale preview geometry is committed.
        frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(Modifiers::SHIFT),
                Event::PointerMoved(end),
                button(end, false, PointerButton::Primary),
            ],
        );
        let n = app.editor.document.nodes.last().unwrap();
        assert_eq!(
            [n.x, n.y, n.width, n.height],
            if hidden {
                [40.0, 30.0, 157.0, 157.0]
            } else {
                [40.0, 30.0, 160.0, 160.0]
            }
        );
    }
}

#[test]
fn repeated_closed_vertices_do_not_lose_convex_interiors_or_render_finite_geometry() {
    let doc = document(vec![Node {
        kind: NodeKind::Path,
        closed: true,
        points: vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
        ],
        ..Node::default()
    }]);
    assert_eq!(doc.hit_test([50.0, 50.0]), Some(1));
    assert_eq!(doc.marquee_hits([40.0, 40.0, 60.0, 60.0]), vec![1]);
    let png = papercraft::render::rasterize_region(&doc, [0.0, 0.0, 100.0, 100.0], 1.0).unwrap();
    assert_eq!(png.pixels[4 * (50 + 50 * 100) + 3], 255);
}

#[test]
fn pan_ignores_prepress_pointer_displacement_but_includes_release_delta() {
    for tool in [Tool::Hand, Tool::Rectangle] {
        let (ctx, mut app, b) = setup(Document::default());
        app.tool = tool;
        frame(&ctx, &mut app, vec![Event::PointerMoved(at(b, 10.0, 10.0))]);
        let a = at(b, 200.0, 200.0);
        let which = if tool == Tool::Hand {
            PointerButton::Primary
        } else {
            PointerButton::Middle
        };
        let pressed = board(&frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(a), button(a, true, which)],
        ));
        assert_eq!(pressed, b, "press position is not a pan delta");
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(a + vec2(20.0, 10.0))],
        );
        let after = board(&frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a + vec2(50.0, 30.0)),
                button(a + vec2(50.0, 30.0), false, which),
            ],
        ));
        assert!(after.min.distance(b.min + vec2(50.0, 30.0)) < 0.01);
        assert!(app.editor.document.nodes.is_empty());
        assert!(!app.editor.can_undo());
    }
}
