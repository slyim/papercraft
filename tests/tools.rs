use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use papercraft::{
    app::{PaperCraftApp, TOOLS, Tool, creation_rect},
    document::{ColorTarget, Document, NEW_TEXT, NodeKind},
};

const SIZE: [f32; 2] = [1380.0, 860.0];

fn run(ctx: &egui::Context, app: &mut PaperCraftApp, size: [f32; 2], events: Vec<Event>) {
    run_with(ctx, app, size, Modifiers::NONE, events);
}

fn run_with(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    modifiers: Modifiers,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut events = events;
    events.insert(0, Event::ModifiersChanged(modifiers));
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]))),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    output
}

fn key(key: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .into()
}

/// Presses at `start`, moves through the midpoint to `end` and releases, holding
/// `modifiers` throughout.
fn drag(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    modifiers: Modifiers,
    start: Pos2,
    end: Pos2,
) {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    for events in [
        vec![],
        vec![Event::PointerMoved(start), button(start, true)],
        vec![Event::PointerMoved(start + (end - start) * 0.5)],
        vec![Event::PointerMoved(end)],
        vec![button(end, false)],
        vec![],
    ] {
        run_with(ctx, app, SIZE, modifiers, events);
    }
}

fn setup(document: Document) -> (egui::Context, PaperCraftApp, Rect, f32) {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(document);
    let board = artboard(&ctx, &mut app, SIZE);
    let scale = board.width() / app.editor.document.width;
    (ctx, app, board, scale)
}

fn artboard(ctx: &egui::Context, app: &mut PaperCraftApp, size: [f32; 2]) -> Rect {
    let b = app.editor.document.background;
    let fill = egui::Color32::from_rgba_unmultiplied(b[0], b[1], b[2], b[3]);
    run_with(ctx, app, size, Modifiers::NONE, Vec::new())
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("Artboard was not painted")
}

#[test]
fn creation_rect_constrains_and_centers() {
    let start = pos2(100.0, 100.0);
    let end = pos2(160.0, 80.0);
    assert_eq!(
        creation_rect(start, end, false, false),
        Rect::from_min_max(pos2(100.0, 80.0), pos2(160.0, 100.0))
    );
    assert_eq!(
        creation_rect(start, end, true, false),
        Rect::from_min_max(pos2(100.0, 40.0), pos2(160.0, 100.0))
    );
    assert_eq!(
        creation_rect(start, end, false, true),
        Rect::from_min_max(pos2(40.0, 80.0), pos2(160.0, 120.0))
    );
    assert_eq!(
        creation_rect(start, end, true, true),
        Rect::from_min_max(pos2(40.0, 40.0), pos2(160.0, 160.0))
    );
}

#[test]
fn every_rail_tool_has_a_working_shortcut() {
    let (ctx, mut app, _, _) = setup(Document::default());
    let mut keys: Vec<_> = TOOLS.iter().map(|spec| spec.key).collect();
    keys.dedup();
    assert_eq!(keys.len(), TOOLS.len(), "Shortcuts must be unique");
    for spec in TOOLS.iter().rev() {
        run(&ctx, &mut app, SIZE, key(spec.key));
        assert_eq!(app.tool, spec.tool, "{} shortcut", spec.label);
    }
}

#[test]
fn shift_draws_squares_and_alt_draws_from_the_center() {
    let (ctx, mut app, board, scale) = setup(Document::default());
    let at = |x: f32, y: f32| board.min + vec2(x, y) * scale;
    for (modifiers, expected) in [
        (Modifiers::SHIFT, (200.0, 200.0, 120.0, 120.0)),
        (Modifiers::ALT, (80.0, 160.0, 240.0, 80.0)),
        (
            Modifiers::SHIFT | Modifiers::ALT,
            (80.0, 80.0, 240.0, 240.0),
        ),
    ] {
        run(&ctx, &mut app, SIZE, key(Key::R));
        drag(
            &ctx,
            &mut app,
            modifiers,
            at(200.0, 200.0),
            at(320.0, 240.0),
        );
        let node = app.editor.selected_node().expect("Rectangle was not drawn");
        assert_eq!((node.x, node.y, node.width, node.height), expected);
        assert_eq!(app.tool, Tool::Select);
    }
    assert_eq!(app.editor.document.nodes.len(), 3);
}

#[test]
fn clicked_text_follows_its_content_and_edits_in_one_step() {
    let (ctx, mut app, board, scale) = setup(Document::default());
    run(&ctx, &mut app, SIZE, key(Key::T));
    let at = board.min + vec2(100.0, 100.0) * scale;
    drag(&ctx, &mut app, Modifiers::NONE, at, at);
    let placed = app.editor.selected_node().unwrap().clone();
    assert_eq!(placed.text, NEW_TEXT);
    assert_eq!((placed.x, placed.y), (100.0, 100.0));
    assert!(placed.width < 220.0, "Clicked text is sized to its content");
    assert!(
        ctx.egui_wants_keyboard_input(),
        "Placing text starts editing"
    );

    // Typing replaces the selected placeholder and the width follows the content.
    run(
        &ctx,
        &mut app,
        SIZE,
        vec![Event::Text("A much longer heading".into())],
    );
    run(&ctx, &mut app, SIZE, vec![]);
    let typed = app.editor.selected_node().unwrap().clone();
    assert_eq!(typed.text, "A much longer heading");
    assert!(typed.width > placed.width);
    run(&ctx, &mut app, SIZE, key(Key::Escape));
    run(&ctx, &mut app, SIZE, vec![]);
    assert!(!ctx.egui_wants_keyboard_input(), "Escape ends editing");
    assert_eq!(
        app.editor.selected(),
        Some(typed.id),
        "Escape keeps the layer"
    );

    app.editor.undo();
    assert_eq!(app.editor.selected_node().unwrap(), &placed);
    app.editor.undo();
    assert!(app.editor.document.nodes.is_empty());
}

#[test]
fn dragged_text_keeps_its_width_and_grows_in_height() {
    let (ctx, mut app, board, scale) = setup(Document::default());
    run(&ctx, &mut app, SIZE, key(Key::T));
    let at = |x: f32, y: f32| board.min + vec2(x, y) * scale;
    drag(
        &ctx,
        &mut app,
        Modifiers::NONE,
        at(100.0, 100.0),
        at(260.0, 140.0),
    );
    run(
        &ctx,
        &mut app,
        SIZE,
        vec![Event::Text(
            "one two three four five six seven eight".into(),
        )],
    );
    run(&ctx, &mut app, SIZE, key(Key::Escape));
    let node = app.editor.selected_node().unwrap();
    assert_eq!(node.width, 160.0);
    assert!(node.height > 40.0, "Wrapped text grows the layer");
}

#[test]
fn enter_edits_the_selected_text_without_inserting_a_newline() {
    let (ctx, mut app, _, _) = setup(Document::example());
    let id = app
        .editor
        .document
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Text && !n.locked && !n.hidden)
        .unwrap()
        .id;
    app.editor.select(Some(id));
    let before = app.editor.document.clone();
    run(&ctx, &mut app, SIZE, key(Key::Enter));
    run(&ctx, &mut app, SIZE, vec![]);
    assert!(ctx.egui_wants_keyboard_input());
    assert_eq!(app.editor.document, before);
    run(&ctx, &mut app, SIZE, vec![Event::Text("Edited".into())]);
    run(&ctx, &mut app, SIZE, key(Key::Escape));
    assert_eq!(app.editor.selected_node().unwrap().text, "Edited");
    app.editor.undo();
    assert_eq!(app.editor.document, before);
}

#[test]
fn new_text_uses_the_last_selected_text_style() {
    let document = Document {
        width: 800.0,
        ..Default::default()
    };
    let (ctx, mut app, board, scale) = setup(document);
    app.editor
        .add_at(NodeKind::Text, 10.0, 10.0, 200.0, 60.0)
        .unwrap();
    let before = app.editor.document.clone();
    app.editor.document.nodes[0].font_size = 44.0;
    app.editor.document.nodes[0].fill = [200, 10, 20, 255];
    app.editor.commit(before).unwrap();
    run(&ctx, &mut app, SIZE, vec![]);
    app.editor.select(None);
    run(&ctx, &mut app, SIZE, key(Key::T));
    let at = board.min + vec2(300.0, 300.0) * scale;
    drag(&ctx, &mut app, Modifiers::NONE, at, at);
    let node = app.editor.selected_node().unwrap();
    assert_eq!((node.font_size, node.fill), (44.0, [200, 10, 20, 255]));
}

#[test]
fn marquee_selects_visible_unlocked_layers_without_history() {
    let document = Document {
        width: 800.0,
        height: 600.0,
        ..Default::default()
    };
    let (ctx, mut app, board, scale) = setup(document);
    for x in [50.0, 250.0, 450.0, 650.0] {
        app.editor
            .add_at(NodeKind::Rectangle, x, 50.0, 100.0, 100.0)
            .unwrap();
    }
    let ids: Vec<_> = app.editor.document.nodes.iter().map(|n| n.id).collect();
    let before = app.editor.document.clone();
    app.editor.document.nodes[1].locked = true;
    app.editor.commit(before).unwrap();
    let can_undo = app.editor.can_undo();
    app.editor.select(None);
    let at = |x: f32, y: f32| board.min + vec2(x, y) * scale;

    // From empty space across the first three layers; the locked one is skipped.
    drag(
        &ctx,
        &mut app,
        Modifiers::NONE,
        at(20.0, 300.0),
        at(500.0, 120.0),
    );
    assert_eq!(app.editor.selection(), &[ids[0], ids[2]]);
    // Shift keeps the selection and adds the fourth layer.
    drag(
        &ctx,
        &mut app,
        Modifiers::SHIFT,
        at(780.0, 300.0),
        at(700.0, 100.0),
    );
    assert_eq!(app.editor.selection(), &[ids[0], ids[2], ids[3]]);
    // Without Shift a new marquee replaces the selection.
    drag(
        &ctx,
        &mut app,
        Modifiers::NONE,
        at(780.0, 300.0),
        at(700.0, 100.0),
    );
    assert_eq!(app.editor.selection(), &[ids[3]]);
    assert_eq!(app.editor.can_undo(), can_undo);
    assert_eq!(app.editor.document.nodes.len(), 4);
}

#[test]
fn auto_width_text_stays_on_one_line_at_other_zoom_levels() {
    for size in [[700.0, 480.0], [960.0, 640.0], SIZE, [2400.0, 1600.0]] {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut app = PaperCraftApp::from_document(Document::default());
        let board = artboard(&ctx, &mut app, size);
        let scale = board.width() / app.editor.document.width;
        run(&ctx, &mut app, size, key(Key::T));
        let at = board.min + vec2(40.0, 40.0) * scale;
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(at), button(true)],
        );
        run(&ctx, &mut app, size, vec![button(false)]);
        run(&ctx, &mut app, size, key(Key::Escape));
        let output = run_with(&ctx, &mut app, size, Modifiers::NONE, vec![]);
        let rows: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == NEW_TEXT => Some(t.galley.rows.len()),
                _ => None,
            })
            .collect();
        // The canvas and the inspector's text field both show the text.
        assert!(!rows.is_empty(), "window {size:?}");
        assert!(rows.iter().all(|r| *r == 1), "window {size:?}: {rows:?}");
    }
}

fn command_key(key: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        })
        .into()
}

/// Adds a committed text layer reading `text` and selects it.
fn add_text(app: &mut PaperCraftApp, text: &str) -> u64 {
    app.editor
        .add_at(NodeKind::Text, 10.0, 10.0, 200.0, 60.0)
        .unwrap();
    let before = app.editor.document.clone();
    app.editor.document.nodes[0].text = text.into();
    app.editor.commit(before).unwrap();
    app.editor.selected().unwrap()
}

/// Enters inline editing of the selected text, types `text` and leaves with Escape.
fn edit_inline(ctx: &egui::Context, app: &mut PaperCraftApp, text: &str) {
    run(ctx, app, SIZE, key(Key::Enter));
    run(ctx, app, SIZE, vec![]);
    assert!(ctx.egui_wants_keyboard_input(), "Enter starts editing");
    run(ctx, app, SIZE, vec![Event::Text(text.into())]);
    run(ctx, app, SIZE, key(Key::Escape));
    run(ctx, app, SIZE, vec![]);
}

#[test]
fn inline_text_undo_stays_within_its_document_tab() {
    let (ctx, mut app, _, _) = setup(Document::default());
    let a = add_text(&mut app, "A-original");
    edit_inline(&ctx, &mut app, "A-secret");
    assert_eq!(app.editor.selected_node().unwrap().text, "A-secret");

    run_with(
        &ctx,
        &mut app,
        SIZE,
        Modifiers::COMMAND,
        command_key(Key::N),
    );
    run(&ctx, &mut app, SIZE, vec![]);
    assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
    let b = add_text(&mut app, "B-original");
    assert_eq!(a, b, "Both documents use the same layer ID");

    // Undo inside a fresh inline edit has nothing of its own to undo.
    run(&ctx, &mut app, SIZE, key(Key::Enter));
    run(&ctx, &mut app, SIZE, vec![]);
    assert!(ctx.egui_wants_keyboard_input());
    run_with(
        &ctx,
        &mut app,
        SIZE,
        Modifiers::COMMAND,
        command_key(Key::Z),
    );
    run(&ctx, &mut app, SIZE, key(Key::Escape));
    run(&ctx, &mut app, SIZE, vec![]);
    assert_eq!(app.editor.selected_node().unwrap().text, "B-original");

    // Undo within one inline edit still reverts that edit's typing.
    run(&ctx, &mut app, SIZE, key(Key::Enter));
    run(&ctx, &mut app, SIZE, vec![]);
    run(&ctx, &mut app, SIZE, vec![Event::Text("B-typed".into())]);
    run_with(
        &ctx,
        &mut app,
        SIZE,
        Modifiers::COMMAND,
        command_key(Key::Z),
    );
    run(&ctx, &mut app, SIZE, key(Key::Escape));
    run(&ctx, &mut app, SIZE, vec![]);
    assert_eq!(app.editor.selected_node().unwrap().text, "B-original");

    app.activate_tab(0);
    run(&ctx, &mut app, SIZE, vec![]);
    assert_eq!(app.editor.document.nodes[0].text, "A-secret");
}

/// Selects a red-token-bound text layer, deselects it and returns the token ID.
fn inherit_bound_style(ctx: &egui::Context, app: &mut PaperCraftApp) -> u64 {
    let text = add_text(app, "Bound");
    let token = app.editor.create_token([200, 10, 20, 255]).unwrap();
    app.editor
        .bind(ColorTarget::Node(text), Some(token))
        .unwrap();
    app.editor.select(Some(text));
    run(ctx, app, SIZE, vec![]);
    app.editor.select(None);
    run(ctx, app, SIZE, vec![]);
    token
}

fn place_text_at(ctx: &egui::Context, app: &mut PaperCraftApp, board: Rect, scale: f32) {
    run(ctx, app, SIZE, key(Key::T));
    let at = board.min + vec2(300.0, 300.0) * scale;
    drag(ctx, app, Modifiers::NONE, at, at);
    run(ctx, app, SIZE, key(Key::Escape));
    run(ctx, app, SIZE, vec![]);
}

#[test]
fn inherited_text_style_detaches_when_its_token_is_deleted() {
    let (ctx, mut app, board, scale) = setup(Document::default());
    let red = inherit_bound_style(&ctx, &mut app);
    app.editor.delete_token(red).unwrap();
    run(&ctx, &mut app, SIZE, vec![]);
    let blue = app.editor.create_token([10, 20, 200, 255]).unwrap();
    assert_eq!(blue, red, "The replacement token reuses the deleted ID");
    place_text_at(&ctx, &mut app, board, scale);
    let node = app.editor.selected_node().unwrap();
    assert_eq!((node.fill, node.fill_token), ([200, 10, 20, 255], None));

    // Undoing the deletion does not rebind the detached style.
    app.editor.undo();
    app.editor.undo();
    app.editor.undo();
    run(&ctx, &mut app, SIZE, vec![]);
    assert!(app.editor.document.token(red).is_some());
    place_text_at(&ctx, &mut app, board, scale);
    let node = app.editor.selected_node().unwrap();
    assert_eq!((node.fill, node.fill_token), ([200, 10, 20, 255], None));
}

#[test]
fn inherited_text_style_follows_a_token_that_still_exists() {
    let (ctx, mut app, board, scale) = setup(Document::default());
    let token = inherit_bound_style(&ctx, &mut app);
    app.editor.set_token_value(token, [0, 128, 0, 255]).unwrap();
    run(&ctx, &mut app, SIZE, vec![]);
    place_text_at(&ctx, &mut app, board, scale);
    let placed = app.editor.selected().unwrap();
    let node = app.editor.selected_node().unwrap();
    assert_eq!(
        (node.fill, node.fill_token),
        ([0, 128, 0, 255], Some(token))
    );
    app.editor.set_token_value(token, [0, 0, 0, 255]).unwrap();
    let node = app
        .editor
        .document
        .nodes
        .iter()
        .find(|n| n.id == placed)
        .unwrap();
    assert_eq!(node.fill, [0, 0, 0, 255], "New text stays bound");
}
