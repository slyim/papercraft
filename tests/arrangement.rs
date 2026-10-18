//! Multi-selection, shared moves, alignment, distribution and format-upgrade prompts.

use eframe::egui;
use papercraft::{
    app::{PaperCraftApp, View},
    document::{Align, Axis, ColorTarget, Document, Editor, NodeKind},
    recents::Recents,
    ui,
};

/// Three 40×40 rectangles at x = 0, 100, 300 (y = 10, 50, 30) plus one hidden and one
/// locked rectangle.
fn layout() -> Editor {
    let mut editor = Editor::new(Document::default());
    for (x, y) in [
        (0.0, 10.0),
        (100.0, 50.0),
        (300.0, 30.0),
        (500.0, 0.0),
        (600.0, 0.0),
    ] {
        editor
            .add_at(NodeKind::Rectangle, x, y, 40.0, 40.0)
            .unwrap();
    }
    let before = editor.document.clone();
    editor.document.nodes[3].hidden = true;
    editor.document.nodes[4].locked = true;
    editor.commit(before).unwrap();
    editor
}

fn ids(editor: &Editor) -> Vec<u64> {
    editor.document.nodes.iter().map(|n| n.id).collect()
}

fn positions(editor: &Editor) -> Vec<(f32, f32)> {
    editor.document.nodes.iter().map(|n| (n.x, n.y)).collect()
}

#[test]
fn selection_invariants_hold_through_edits_and_history() {
    let mut editor = layout();
    let all = ids(&editor);
    editor.set_selection([all[0], all[0], 999, all[1]]);
    assert_eq!(
        editor.selection(),
        &all[..2],
        "Unknown and repeated ids are ignored"
    );
    assert_eq!(
        editor.selected(),
        None,
        "Inspector single selection needs one layer"
    );
    editor.toggle_selection(all[0]);
    assert_eq!(editor.selection(), &all[1..2]);
    assert_eq!(editor.selected(), Some(all[1]));

    editor.select_all();
    assert_eq!(
        editor.selection(),
        &all[..3],
        "Select all skips hidden and locked layers"
    );

    // Delete removes unlocked selected layers (hidden included) in one step and
    // drops their ids from the selection.
    editor.set_selection(all.clone());
    let before = editor.document.clone();
    editor.delete().unwrap();
    assert_eq!(ids(&editor), vec![all[4]], "Locked layer survives");
    assert_eq!(editor.selection(), &[all[4]]);
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    editor.undo();

    // Redo after a deletion clears stale selection.
    editor.set_selection([all[0], all[1]]);
    let created = editor.document.clone();
    editor.delete().unwrap();
    editor.undo();
    editor.set_selection([all[0]]);
    editor.redo();
    assert!(editor.selection().is_empty());
    editor.undo();
    assert_eq!(editor.document, created);

    // Duplicate copies unlocked layers above, in order, and selects the copies.
    editor.set_selection([all[2], all[0], all[4]]);
    editor.duplicate().unwrap();
    let copies = editor.selection().to_vec();
    assert_eq!(copies.len(), 2);
    let names: Vec<_> = editor.document.nodes[5..]
        .iter()
        .map(|n| (n.name.clone(), n.x))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Rectangle copy".into(), 20.0),
            ("Rectangle copy".into(), 320.0)
        ]
    );
    editor.undo();
    assert_eq!(editor.document, created, "One undo removes every copy");
}

#[test]
fn alignment_uses_selection_bounds_or_the_artboard() {
    let mut editor = layout();
    let all = ids(&editor);
    // Hidden and locked layers in the selection are neither moved nor measured.
    editor.set_selection(all.clone());
    let expectations = [
        (Align::Left, [(0.0, 10.0), (0.0, 50.0), (0.0, 30.0)]),
        (Align::Right, [(300.0, 10.0), (300.0, 50.0), (300.0, 30.0)]),
        (
            Align::CenterX,
            [(150.0, 10.0), (150.0, 50.0), (150.0, 30.0)],
        ),
        (Align::Top, [(0.0, 10.0), (100.0, 10.0), (300.0, 10.0)]),
        (Align::Bottom, [(0.0, 50.0), (100.0, 50.0), (300.0, 50.0)]),
        (Align::CenterY, [(0.0, 30.0), (100.0, 30.0), (300.0, 30.0)]),
    ];
    let original = editor.document.clone();
    for (align, expected) in expectations {
        editor.align(align).unwrap();
        assert_eq!(positions(&editor)[..3], expected, "{align:?}");
        assert_eq!(editor.document.nodes[3..], original.nodes[3..], "{align:?}");
        editor.undo();
        assert_eq!(editor.document, original, "One undo reverts {align:?}");
    }

    // A single layer aligns to the artboard (960×640).
    editor.set_selection([all[1]]);
    editor.align(Align::Right).unwrap();
    editor.align(Align::Bottom).unwrap();
    assert_eq!(positions(&editor)[1], (920.0, 600.0));
    editor.align(Align::CenterX).unwrap();
    editor.align(Align::CenterY).unwrap();
    assert_eq!(positions(&editor)[1], (460.0, 300.0));

    // Only hidden/locked layers: nothing to arrange, no history step.
    editor.set_selection([all[3], all[4]]);
    let before = editor.document.clone();
    let undo = editor.can_undo();
    editor.align(Align::Left).unwrap();
    editor.distribute(Axis::Horizontal).unwrap();
    editor.nudge(5.0, 5.0).unwrap();
    assert_eq!(editor.document, before);
    assert_eq!(editor.can_undo(), undo);
}

#[test]
fn distribution_gives_equal_gaps_and_keeps_outer_layers() {
    let mut editor = layout();
    let all = ids(&editor);
    // Selection order differs from position order; positions decide the order.
    editor.set_selection([all[2], all[0], all[1], all[3], all[4]]);
    editor.distribute(Axis::Horizontal).unwrap();
    // Span 0..340, sizes 120 → gap 110.
    assert_eq!(
        positions(&editor)[..3],
        [(0.0, 10.0), (150.0, 50.0), (300.0, 30.0)]
    );
    editor.undo();
    editor.distribute(Axis::Vertical).unwrap();
    // Sorted by y: 10, 30, 50; span 10..90 = 80, sizes 120 → overlapping gap of -20,
    // which these positions already satisfy.
    let doc = &editor.document;
    assert_eq!(
        (doc.nodes[0].y, doc.nodes[2].y, doc.nodes[1].y),
        (10.0, 30.0, 50.0)
    );
    // Two layers are not distributed.
    editor.set_selection([all[0], all[1]]);
    let before = editor.document.clone();
    editor.distribute(Axis::Horizontal).unwrap();
    assert_eq!(editor.document, before);

    // Unequal sizes: gaps become equal.
    let mut editor = Editor::new(Document::default());
    for (x, width) in [(0.0, 10.0), (20.0, 50.0), (75.0, 5.0), (200.0, 20.0)] {
        editor
            .add_at(NodeKind::Rectangle, x, 0.0, width, 10.0)
            .unwrap();
    }
    editor.select_all();
    editor.distribute(Axis::Horizontal).unwrap();
    let nodes = &editor.document.nodes;
    let gaps: Vec<f32> = nodes
        .windows(2)
        .map(|pair| pair[1].x - (pair[0].x + pair[0].width))
        .collect();
    assert!(
        gaps.iter().all(|gap| (gap - gaps[0]).abs() < 1e-3),
        "{gaps:?}"
    );
    assert_eq!((nodes[0].x, nodes[3].x), (0.0, 200.0));
}

#[test]
fn nudging_and_visibility_toggles_apply_to_the_selection_as_one_step() {
    let mut editor = layout();
    let all = ids(&editor);
    editor.set_selection(all.clone());
    let original = editor.document.clone();
    editor.nudge(10.0, -5.0).unwrap();
    assert_eq!(
        positions(&editor)[..3],
        [(10.0, 5.0), (110.0, 45.0), (310.0, 25.0)]
    );
    assert_eq!(editor.document.nodes[3..], original.nodes[3..]);
    editor.undo();
    assert_eq!(editor.document, original);

    editor.set_selection([all[0], all[3]]);
    editor.toggle_hidden().unwrap();
    assert!(
        !editor.document.nodes[0].hidden && !editor.document.nodes[3].hidden,
        "Any hidden → show all"
    );
    editor.toggle_hidden().unwrap();
    assert!(editor.document.nodes[0].hidden && editor.document.nodes[3].hidden);
    editor.set_selection([all[0], all[4]]);
    editor.toggle_locked().unwrap();
    assert!(
        editor.document.nodes[0].locked && editor.document.nodes[4].locked,
        "Mixed → lock all"
    );
    editor.toggle_locked().unwrap();
    assert!(!editor.document.nodes[0].locked && !editor.document.nodes[4].locked);
}

// Simulated egui input.

#[test]
fn coordinate_limits_keep_shared_nudges_together() {
    for sign in [-1.0, 1.0] {
        let mut editor = layout();
        editor.document.nodes.truncate(2);
        let before = editor.document.clone();
        for (node, position) in editor.document.nodes.iter_mut().zip([160.25, 99_990.25]) {
            node.x = position * sign;
            node.y = position * sign;
        }
        editor.commit(before).unwrap();
        editor.select_all();
        let original = editor.document.clone();
        editor.nudge(40.0 * sign, 40.0 * sign).unwrap();
        assert_eq!(
            positions(&editor),
            [
                (170.0 * sign, 170.0 * sign),
                (100_000.0 * sign, 100_000.0 * sign)
            ]
        );
        editor.undo();
        assert_eq!(editor.document, original);
    }
}

#[test]
fn coordinate_limits_reject_arrangements_that_cannot_fit() {
    let mut editor = Editor::new(Document::default());
    for width in [100_000.0, 1.0, 1.0] {
        editor
            .add_at(NodeKind::Rectangle, 100_000.0, 0.0, width, 10.0)
            .unwrap();
    }
    editor.select_all();
    let original = editor.document.clone();
    assert!(editor.align(Align::Right).is_err());
    assert_eq!(editor.document, original);
    assert!(editor.distribute(Axis::Horizontal).is_err());
    assert_eq!(editor.document, original);
    editor.undo();
    assert_eq!(
        editor.document.nodes.len(),
        2,
        "Rejected operations add no history"
    );
}

#[test]
fn coordinate_limits_keep_shared_drags_together() {
    let size = SIZES[0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut editor = layout();
    editor.document.nodes.truncate(2);
    editor.document.nodes[0].x = 160.25;
    editor.document.nodes[1].x = 99_990.25;
    let mut app = PaperCraftApp::from_document(editor.document);
    app.editor.select_all();
    let original = app.editor.document.clone();
    let board = artboard(&ctx, &mut app, size);
    let zoom = board.width() / app.editor.document.width;
    let start = board.min + egui::vec2(180.0, 30.0) * zoom;
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        start,
        start + egui::vec2(40.0, 0.0) * zoom,
    );
    assert_eq!(positions(&app.editor), [(170.0, 10.0), (100_000.0, 50.0)]);
    app.editor.undo();
    assert_eq!(app.editor.document, original);
}

const SIZES: [[f32; 2]; 2] = [[1380.0, 860.0], [960.0, 640.0]];

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

fn gesture(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    modifiers: egui::Modifiers,
    start: egui::Pos2,
    end: egui::Pos2,
) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    for events in [
        vec![egui::Event::PointerMoved(start), button(start, true)],
        vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
        vec![egui::Event::PointerMoved(end)],
        vec![button(end, false)],
        vec![],
    ] {
        frame(ctx, app, size, modifiers, events);
    }
}

fn shortcut(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    modifiers: egui::Modifiers,
    key: egui::Key,
) {
    let event = |pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    frame(ctx, app, size, modifiers, vec![event(true)]);
    frame(ctx, app, size, egui::Modifiers::NONE, vec![event(false)]);
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

fn text_rect(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(t) if t.galley.text() == text => {
            Some(t.galley.rect.translate(t.pos.to_vec2()))
        }
        _ => None,
    })
}

#[test]
fn shift_click_and_shared_drag_move_the_selection_at_multiple_zoom_levels() {
    for size in SIZES {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let editor = layout();
        let all = ids(&editor);
        let mut app = PaperCraftApp::from_document(editor.document);
        let board = artboard(&ctx, &mut app, size);
        let scale = board.width() / app.editor.document.width;
        let at = |x: f32, y: f32| board.min + egui::vec2(x, y) * scale;
        let none = egui::Modifiers::NONE;
        let shift = egui::Modifiers::SHIFT;

        gesture(&ctx, &mut app, size, none, at(20.0, 30.0), at(20.0, 30.0));
        assert_eq!(app.editor.selection(), &all[..1]);
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(120.0, 70.0),
            at(120.0, 70.0),
        );
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(320.0, 50.0),
            at(320.0, 50.0),
        );
        assert_eq!(app.editor.selection(), &all[..3]);
        // Shift-click on a selected layer removes it; on empty canvas keeps the rest.
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(320.0, 50.0),
            at(320.0, 50.0),
        );
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(800.0, 400.0),
            at(800.0, 400.0),
        );
        assert_eq!(app.editor.selection(), &all[..2]);
        // Locked and hidden layers cannot be picked on the canvas.
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(520.0, 20.0),
            at(520.0, 20.0),
        );
        gesture(
            &ctx,
            &mut app,
            size,
            shift,
            at(620.0, 20.0),
            at(620.0, 20.0),
        );
        assert_eq!(app.editor.selection(), &all[..2]);

        // Dragging one selected layer moves both by the same document distance.
        let before = app.editor.document.clone();
        gesture(&ctx, &mut app, size, none, at(120.0, 70.0), at(150.0, 90.0));
        assert_eq!(
            positions(&app.editor)[..3],
            [(30.0, 30.0), (130.0, 70.0), (300.0, 30.0)]
        );
        assert_eq!(
            app.editor.selection(),
            &all[..2],
            "Drag keeps the selection"
        );
        app.editor.undo();
        assert_eq!(
            app.editor.document, before,
            "One undo reverts the shared move"
        );
        app.editor.redo();

        // Plain click replaces the selection; empty click clears it.
        gesture(&ctx, &mut app, size, none, at(320.0, 50.0), at(320.0, 50.0));
        assert_eq!(app.editor.selection(), &all[2..3]);
        gesture(
            &ctx,
            &mut app,
            size,
            none,
            at(800.0, 400.0),
            at(800.0, 400.0),
        );
        assert!(app.editor.selection().is_empty());
    }
}

#[test]
fn keyboard_and_inspector_arrangement_record_one_step_each() {
    let size = SIZES[0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let editor = layout();
    let all = ids(&editor);
    let mut app = PaperCraftApp::from_document(editor.document);
    let original = app.editor.document.clone();

    shortcut(&ctx, &mut app, size, egui::Modifiers::COMMAND, egui::Key::A);
    assert_eq!(app.editor.selection(), &all[..3]);
    shortcut(&ctx, &mut app, size, egui::Modifiers::ALT, egui::Key::W);
    assert_eq!(
        positions(&app.editor)[..3],
        [(0.0, 10.0), (100.0, 10.0), (300.0, 10.0)]
    );
    shortcut(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::CTRL | egui::Modifiers::ALT,
        egui::Key::H,
    );
    assert_eq!(
        positions(&app.editor)[..3],
        [(0.0, 10.0), (150.0, 10.0), (300.0, 10.0)],
        "Ctrl+Alt+H distributes; aligning centers would move every layer to x = 150"
    );
    shortcut(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        egui::Key::ArrowRight,
    );
    assert_eq!(positions(&app.editor)[0], (1.0, 10.0));
    for _ in 0..3 {
        app.editor.undo();
    }
    assert_eq!(
        app.editor.document, original,
        "Three operations, three undo steps"
    );

    // Inspector controls for the same selection.
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    assert!(text_rect(&output, "3 layers selected").is_some());
    let button = text_rect(&output, egui_phosphor::regular::ALIGN_LEFT).expect("Align left");
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        button.center(),
        button.center(),
    );
    assert_eq!(
        positions(&app.editor)[..3],
        [(0.0, 10.0), (0.0, 50.0), (0.0, 30.0)]
    );
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    let button = text_rect(&output, egui_phosphor::regular::ROWS).expect("Distribute vertically");
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        button.center(),
        button.center(),
    );
    app.editor.undo();
    app.editor.undo();
    assert_eq!(app.editor.document, original);

    // Layer list: Shift-click adds a row, including a locked layer; it is not moved.
    app.editor.select(None);
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    let rows: Vec<egui::Rect> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "Rectangle" => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 5, "One row per layer, topmost first");
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        rows[0].center(),
        rows[0].center(),
    );
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::SHIFT,
        rows[4].center(),
        rows[4].center(),
    );
    assert_eq!(app.editor.selection(), &[all[4], all[0]]);
    shortcut(&ctx, &mut app, size, egui::Modifiers::ALT, egui::Key::D);
    // One arrangeable layer aligns to the artboard; the locked one stays.
    assert_eq!(positions(&app.editor)[0], (920.0, 10.0));
    assert_eq!(positions(&app.editor)[4], (600.0, 0.0));
}

#[test]
fn saving_a_version_1_file_with_tokens_asks_before_upgrading_it() {
    let size = SIZES[0];
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.crumbs");
    let original = include_bytes!("fixtures/legacy.crumbs");
    std::fs::write(&path, original).unwrap();
    let mut recents = Recents::load(directory.path().join("recents.json"));
    recents.touch(&path).unwrap();
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::default()).with_recents(recents);
    assert!(app.close_document());
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    let card = text_rect(&output, &Document::load(&path).unwrap().name).expect("Recent card");
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        card.center(),
        card.center(),
    );
    assert_eq!(app.view, View::Editor);

    let token = app.editor.create_token([1, 2, 3, 255]).unwrap();
    app.editor
        .bind(ColorTarget::Background, Some(token))
        .unwrap();
    shortcut(&ctx, &mut app, size, egui::Modifiers::COMMAND, egui::Key::S);
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    assert!(text_rect(&output, "Save this file in the new format?").is_some());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "Nothing written before consent"
    );

    let cancel = text_rect(&output, "Cancel").unwrap();
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        cancel.center(),
        cancel.center(),
    );
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    assert!(text_rect(&output, "Save this file in the new format?").is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    shortcut(&ctx, &mut app, size, egui::Modifiers::COMMAND, egui::Key::S);
    let output = frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
    let upgrade = text_rect(&output, "Upgrade this file").unwrap();
    gesture(
        &ctx,
        &mut app,
        size,
        egui::Modifiers::NONE,
        upgrade.center(),
        upgrade.center(),
    );
    let saved = Document::load(&path).unwrap();
    assert_eq!(saved.version, 2);
    assert_eq!(saved, app.editor.document);
    // Later saves of the upgraded file do not ask again.
    let before = app.editor.document.clone();
    app.editor.document.nodes[0].x += 1.0;
    app.editor.commit(before).unwrap();
    shortcut(&ctx, &mut app, size, egui::Modifiers::COMMAND, egui::Key::S);
    assert_eq!(Document::load(&path).unwrap(), app.editor.document);
}

#[test]
fn inspector_width_is_stable_for_bound_and_multiple_selections() {
    for size in SIZES {
        let mut editor = Editor::new(Document::example());
        let all = ids(&editor);
        let token = editor
            .create_token_from(ColorTarget::Background)
            .unwrap()
            .unwrap();
        editor
            .rename_token(token, "a-rather-long-token-name-for-width")
            .unwrap();
        editor.bind(ColorTarget::Node(all[3]), Some(token)).unwrap();
        let selections: [Vec<u64>; 3] = [vec![], vec![all[3]], all[1..4].to_vec()];
        for selection in selections {
            let ctx = egui::Context::default();
            PaperCraftApp::configure(&ctx);
            let mut app = PaperCraftApp::from_document(editor.document.clone());
            app.editor.set_selection(selection.clone());
            for _ in 0..4 {
                frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
            }
            let width =
                egui::PanelState::load(&ctx, egui::Id::new("inspector")).map(|s| s.size().x);
            assert_eq!(
                width,
                Some(ui::INSPECTOR),
                "{size:?} selection {selection:?}"
            );
        }
        // Theme tab with the token editor open keeps the left panel width.
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut app = PaperCraftApp::from_document(editor.document.clone());
        app.theme = true;
        app.selected_token = Some(token);
        for _ in 0..4 {
            frame(&ctx, &mut app, size, egui::Modifiers::NONE, Vec::new());
        }
        let width = egui::PanelState::load(&ctx, egui::Id::new("layers")).map(|s| s.size().x);
        assert_eq!(width, Some(ui::SIDEBAR), "{size:?} theme tab");
    }
}
