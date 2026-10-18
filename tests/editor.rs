use eframe::egui;
use papercraft::{
    app::{PaperCraftApp, Tool, View},
    document::{DEFAULT_CANVAS, Document, Editor, NodeKind},
    recents::Recents,
    ui,
};

#[test]
fn legacy_crumbs_documents_survive_the_papercraft_rename() {
    let directory = tempfile::tempdir().unwrap();
    let old_path = directory.path().join("legacy.crumbs");
    let original = include_bytes!("fixtures/legacy.crumbs");
    std::fs::write(&old_path, original).unwrap();
    let document = Document::load(&old_path).unwrap();
    assert_eq!(document.version, 1);
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.text.contains("Crumbs"))
    );
    let new_path = directory.path().join("design.papercraft");
    document.save(&new_path).unwrap();
    assert_eq!(Document::load(&new_path).unwrap(), document);
    assert_eq!(std::fs::read(old_path).unwrap(), original);
}

#[test]
fn document_edit_history_save_and_export_round_trip() {
    let mut editor = Editor::new(Document::default());
    editor.add(NodeKind::Text).unwrap();
    let id = editor.selected().unwrap();
    let before = editor.document.clone();
    let node = &mut editor.document.nodes[0];
    node.x = 175.0;
    node.width = 310.0;
    node.text = "<script>alert(1)</script> & 'Text'\n第二行".into();
    editor.commit(before.clone()).unwrap();
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_eq!(editor.selected(), Some(id));
    assert_eq!(editor.document.nodes[0].x, 175.0);
    editor.duplicate().unwrap();
    assert_ne!(editor.document.nodes[0].id, editor.document.nodes[1].id);
    editor.reorder(false).unwrap();
    assert_eq!(editor.document.nodes[0].id, editor.selected().unwrap());
    editor.delete().unwrap();
    assert_eq!(editor.document.nodes.len(), 1);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.crumbs");
    editor.document.save(&path).unwrap();
    let reopened = Document::load(&path).unwrap();
    assert_eq!(reopened, editor.document);
    let html = reopened.export_html().unwrap();
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &#39;Text&#39;"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("left:175px"));
    assert!(html.contains("第二行"));
    editor.document.nodes[0].hidden = true;
    assert!(
        !editor
            .document
            .export_html()
            .unwrap()
            .contains("&lt;script&gt;")
    );
}

#[test]
fn invalid_documents_and_edits_cannot_replace_valid_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("document.crumbs");
    let valid = Document::example();
    valid.save(&path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let mut invalid = valid.clone();
    invalid.nodes[0].width = f32::NAN;
    assert!(invalid.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    invalid = valid.clone();
    invalid.nodes[1].id = invalid.nodes[0].id;
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.version = 3;
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.nodes[0].height = -1.0;
    assert!(invalid.validate().is_err());
    let mut editor = Editor::new(valid.clone());
    editor.document.width = f32::INFINITY;
    assert!(editor.commit(valid.clone()).is_err());
    assert_eq!(editor.document, valid);
    let bad_path = directory.path().join("invalid.crumbs");
    std::fs::write(&bad_path, b"{not a document}").unwrap();
    assert!(Document::load(&bad_path).is_err());
    assert_eq!(Document::load(&path).unwrap(), valid);
    editor.document.nodes[0].locked = true;
    editor.select(Some(editor.document.nodes[0].id));
    let count = editor.document.nodes.len();
    editor.delete().unwrap();
    editor.duplicate().unwrap();
    assert_eq!(editor.document.nodes.len(), count);
}

#[test]
fn native_editor_renders_at_supported_sizes_and_keyboard_actions_use_history() {
    for size in [[1380.0, 860.0], [960.0, 640.0]] {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut app = PaperCraftApp::from_document(Document::example());
        // This checks exact pointer alignment; snapping is covered in tests/canvas.rs.
        app.snapping = false;
        let frame = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(frame(Vec::new()), |ui| app.show(ui));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
        let board = artboard(&ctx, &mut app, size);
        let count = app.editor.document.nodes.len();
        // R selects the rectangle tool; a click inside the artboard places a layer there.
        let mut output = ctx.run_ui(frame(vec![key(egui::Key::R, true)]), |ui| app.show(ui));
        output.textures_delta.clear();
        assert_eq!(app.tool, Tool::Rectangle);
        assert_eq!(app.editor.document.nodes.len(), count);
        let scale = board.width() / app.editor.document.width;
        let click = board.min + egui::vec2(480.0, 320.0) * scale;
        pointer_drag(&ctx, &mut app, size, click, click);
        assert_eq!(app.editor.document.nodes.len(), count + 1);
        assert_eq!(
            app.tool,
            Tool::Select,
            "Tools return to selection after placing"
        );
        let placed = app.editor.selected_node().unwrap();
        assert_eq!((placed.x, placed.y), (480.0, 320.0));
        app.editor.undo();
        assert_eq!(app.editor.document.nodes.len(), count);
        app.editor.redo();
        assert_eq!(app.editor.document.nodes.len(), count + 1);

        let original = app.editor.document.nodes.last().unwrap().clone();
        let start = board.min
            + egui::vec2(
                original.x + original.width / 2.0,
                original.y + original.height / 2.0,
            ) * scale;
        let movement = egui::vec2(30.0, 20.0) * scale;
        pointer_drag(&ctx, &mut app, size, start, start + movement);
        let moved = app.editor.selected_node().unwrap().clone();
        assert_eq!((moved.x, moved.y), (original.x + 30.0, original.y + 20.0));
        app.editor.undo();
        assert_eq!(
            app.editor.selected_node().unwrap(),
            &original,
            "One undo must revert the whole drag"
        );
        app.editor.redo();
        let corner = board.min + egui::vec2(moved.x + moved.width, moved.y + moved.height) * scale;
        pointer_drag(
            &ctx,
            &mut app,
            size,
            corner,
            corner + egui::vec2(40.0, 20.0) * scale,
        );
        let resized = app.editor.selected_node().unwrap();
        assert_eq!(
            (resized.width, resized.height),
            (moved.width + 40.0, moved.height + 20.0)
        );
        app.editor.undo();
        assert_eq!(
            app.editor.selected_node().unwrap(),
            &moved,
            "One undo must revert the whole resize"
        );
    }
}

fn key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn input(size: [f32; 2], events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0], size[1]),
        )),
        events,
        ..Default::default()
    }
}

fn run(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input(size, events), |ui| app.show(ui));
    output.textures_delta.clear();
    output
}

/// Screen rectangle of the painted artboard.
fn artboard(ctx: &egui::Context, app: &mut PaperCraftApp, size: [f32; 2]) -> egui::Rect {
    let background = app.editor.document.background;
    let fill = egui::Color32::from_rgba_unmultiplied(
        background[0],
        background[1],
        background[2],
        background[3],
    );
    run(ctx, app, size, Vec::new())
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("Artboard was not painted")
}

/// Screen rectangles of painted texts exactly equal to `text`.
fn text_rects(output: &egui::FullOutput, text: &str) -> Vec<egui::Rect> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            _ => None,
        })
        .collect()
}

fn text_rect(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    text_rects(output, text).into_iter().next()
}

fn click(ctx: &egui::Context, app: &mut PaperCraftApp, size: [f32; 2], at: egui::Pos2) {
    pointer_drag(ctx, app, size, at, at);
}

fn pointer_drag(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    start: egui::Pos2,
    end: egui::Pos2,
) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    for events in [
        vec![],
        vec![egui::Event::PointerMoved(start), button(start, true)],
        vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        vec![
            button(end, false),
            egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    ] {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
    }
}

#[test]
fn page_color_is_compatible_with_earlier_files_and_exported() {
    let mut document = Document::example();
    let mut legacy = serde_json::to_value(&document).unwrap();
    legacy.as_object_mut().unwrap().remove("canvas");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.crumbs");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(Document::load(&path).unwrap().canvas, DEFAULT_CANVAS);

    document.canvas = [10, 20, 30, 128];
    document.save(&path).unwrap();
    assert_eq!(Document::load(&path).unwrap(), document);
    assert!(
        document
            .export_html()
            .unwrap()
            .contains("<body style=\"margin:0;background:rgba(10,20,30,0.5020)\">")
    );
}

#[test]
fn positioned_layers_are_rounded_clamped_and_single_history_steps() {
    let mut editor = Editor::new(Document::default());
    editor
        .add_at(NodeKind::Rectangle, 10.4, -2.6, 0.2, 300_000.0)
        .unwrap();
    let node = editor.selected_node().unwrap();
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (10.0, -3.0, 1.0, 100_000.0)
    );
    assert!(
        editor
            .add_at(NodeKind::Text, f32::NAN, 0.0, 1.0, 1.0)
            .is_err()
    );
    assert_eq!(editor.document.nodes.len(), 1);
    editor.undo();
    assert!(editor.document.nodes.is_empty());
    assert_eq!(editor.selected(), None);
}

#[test]
fn drawing_with_the_text_tool_creates_the_dragged_geometry() {
    let size = [1380.0, 860.0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::default());
    let board = artboard(&ctx, &mut app, size);
    run(&ctx, &mut app, size, vec![key(egui::Key::T, true)]);
    assert_eq!(app.tool, Tool::Text);
    let scale = board.width() / app.editor.document.width;
    let start = board.min + egui::vec2(100.0, 50.0) * scale;
    let end = board.min + egui::vec2(400.0, 150.0) * scale;
    pointer_drag(&ctx, &mut app, size, start, end);
    let node = app
        .editor
        .selected_node()
        .expect("Text layer was not created");
    assert_eq!(node.kind, NodeKind::Text);
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (100.0, 50.0, 300.0, 100.0)
    );
    assert_eq!(app.tool, Tool::Select);
    app.editor.undo();
    assert!(app.editor.document.nodes.is_empty());

    // Escape leaves a creation tool without changing the document; starting outside
    // the artboard creates nothing.
    run(&ctx, &mut app, size, vec![key(egui::Key::R, true)]);
    run(&ctx, &mut app, size, vec![key(egui::Key::Escape, true)]);
    assert_eq!(app.tool, Tool::Select);
    run(&ctx, &mut app, size, vec![key(egui::Key::R, true)]);
    let outside = board.min - egui::vec2(20.0, 20.0);
    pointer_drag(
        &ctx,
        &mut app,
        size,
        outside,
        outside + egui::vec2(60.0, 60.0),
    );
    assert!(app.editor.document.nodes.is_empty());
}

#[test]
fn side_panels_keep_their_width_across_frames() {
    for selected in [None, Some(4)] {
        for size in [[1380.0, 860.0], [960.0, 640.0]] {
            let ctx = egui::Context::default();
            PaperCraftApp::configure(&ctx);
            let mut app = PaperCraftApp::from_document(Document::example());
            app.editor.select(selected);
            let width = |ctx: &egui::Context, id: &str| {
                egui::PanelState::load(ctx, egui::Id::new(id)).map(|s| s.size().x)
            };
            run(&ctx, &mut app, size, Vec::new());
            let first = (width(&ctx, "layers"), width(&ctx, "inspector"));
            for _ in 0..8 {
                run(&ctx, &mut app, size, Vec::new());
            }
            assert_eq!(first, (Some(ui::SIDEBAR), Some(ui::INSPECTOR)));
            assert_eq!((width(&ctx, "layers"), width(&ctx, "inspector")), first);
        }
    }
}

#[test]
fn dashboard_opens_recents_and_tabs_protect_unsaved_work() {
    let size = [1380.0, 860.0];
    let directory = tempfile::tempdir().unwrap();
    let other = Document {
        name: "Second document".into(),
        ..Default::default()
    };
    let other_path = directory.path().join("second.crumbs");
    other.save(&other_path).unwrap();
    let mut recents = Recents::load(directory.path().join("recents.json"));
    recents.touch(&other_path).unwrap();

    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::example()).with_recents(recents);
    // Unsaved edit to the open starter document.
    let before = app.editor.document.clone();
    app.editor.document.nodes[0].x += 5.0;
    app.editor.commit(before).unwrap();

    let output = run(&ctx, &mut app, size, Vec::new());
    let dashboard_tab = text_rect(&output, "Dashboard").expect("Dashboard tab");
    click(&ctx, &mut app, size, dashboard_tab.center());
    assert_eq!(app.view, View::Dashboard);

    let output = run(&ctx, &mut app, size, Vec::new());
    assert!(text_rect(&output, "Recents").is_some());
    // Tab, plus the card for the open document.
    assert_eq!(text_rects(&output, "A place to begin").len(), 2);
    let card = text_rect(&output, "Second document").expect("Recent file card");
    click(&ctx, &mut app, size, card.center());
    // The modal asks before replacing unsaved work; cancel keeps everything.
    let output = run(&ctx, &mut app, size, Vec::new());
    let cancel = text_rect(&output, "Cancel").expect("Unsaved changes prompt");
    click(&ctx, &mut app, size, cancel.center());
    assert_eq!(app.editor.document.name, "A place to begin");
    assert_eq!(app.view, View::Dashboard);

    // Search filters the cards.
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("dashboard_search")));
    run(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::Text("second".into())],
    );
    let output = run(&ctx, &mut app, size, Vec::new());
    assert_eq!(
        text_rects(&output, "A place to begin").len(),
        1,
        "Only the tab remains"
    );
    let card = text_rect(&output, "Second document").expect("Search result");
    click(&ctx, &mut app, size, card.center());
    let output = run(&ctx, &mut app, size, Vec::new());
    let discard = text_rect(&output, "Discard changes").expect("Unsaved changes prompt");
    click(&ctx, &mut app, size, discard.center());
    assert_eq!(app.editor.document, other);
    assert_eq!(app.view, View::Editor);

    // Closing a clean document returns to the dashboard; the file stays in recents.
    // The close control appears while the tab is hovered.
    let output = run(&ctx, &mut app, size, Vec::new());
    let tab = text_rect(&output, "Second document").expect("Document tab");
    assert!(
        text_rect(&output, egui_phosphor::regular::X).is_none(),
        "Close appears only on hover"
    );
    run(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::PointerMoved(tab.center())],
    );
    let output = run(&ctx, &mut app, size, Vec::new());
    let close = text_rect(&output, egui_phosphor::regular::X).expect("Close tab button");
    click(&ctx, &mut app, size, close.center());
    assert!(!app.document_open());
    assert_eq!(app.view, View::Dashboard);
    let output = run(&ctx, &mut app, size, Vec::new());
    assert!(text_rect(&output, "Second document").is_some());
    assert!(
        text_rect(&output, "Untitled").is_none(),
        "No tab without a document"
    );
}

#[test]
fn page_color_field_edits_hex_and_records_one_history_step() {
    let size = [1380.0, 860.0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::example());
    let output = run(&ctx, &mut app, size, Vec::new());
    let field = text_rect(&output, "282828").expect("Page color field");
    click(&ctx, &mut app, size, field.center());
    let mut events = vec![key(egui::Key::End, true)];
    events.extend((0..6).map(|_| key(egui::Key::Backspace, true)));
    run(&ctx, &mut app, size, events);
    // Values apply while typing once they parse (three-digit shorthand included).
    run(&ctx, &mut app, size, vec![egui::Event::Text("1A2".into())]);
    assert_eq!(app.editor.document.canvas, [17, 170, 34, 255]);
    run(&ctx, &mut app, size, vec![egui::Event::Text("B3C".into())]);
    assert_eq!(app.editor.document.canvas, [26, 43, 60, 255]);
    run(&ctx, &mut app, size, vec![key(egui::Key::Enter, true)]);
    run(&ctx, &mut app, size, Vec::new());
    assert_eq!(
        app.editor.document.nodes.len(),
        6,
        "Backspace did not delete layers"
    );
    app.editor.undo();
    assert_eq!(
        app.editor.document.canvas, DEFAULT_CANVAS,
        "One undo restores the page color"
    );
    assert!(!app.editor.can_undo());
}
