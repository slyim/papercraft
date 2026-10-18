use eframe::egui;
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Editor, NodeKind},
};

#[test]
fn duplicating_a_valid_unicode_name_keeps_a_valid_name_and_one_history_step() {
    let mut editor = Editor::new(Document::default());
    editor.add(NodeKind::Text).unwrap();
    editor.document.nodes[0].name = "界".repeat(85);
    editor.document.validate().unwrap();
    let before = editor.document.clone();
    editor.duplicate().unwrap();
    editor.document.validate().unwrap();
    assert_eq!(editor.document.nodes.len(), 2);
    assert!(editor.selected_node().unwrap().name.ends_with(" copy"));
    editor.undo();
    assert_eq!(editor.document, before);
}

#[test]
fn dragging_and_resizing_include_the_release_position_at_multiple_zoom_levels() {
    for size in [[1380.0, 860.0], [960.0, 640.0]] {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut editor = Editor::new(Document::default());
        editor
            .add_at(NodeKind::Rectangle, 160.0, 150.0, 220.0, 80.0)
            .unwrap();
        let mut app = PaperCraftApp::from_document(editor.document);
        let output = frame(&ctx, &mut app, size, Vec::new());
        let board = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_rgb(248, 247, 243) => {
                    Some(rect.rect)
                }
                _ => None,
            })
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap();
        let scale = board.width() / app.editor.document.width;
        let original = app.editor.document.nodes[0].clone();
        let start = board.min + egui::vec2(270.0, 190.0) * scale;
        release_at_final_position(
            &ctx,
            &mut app,
            size,
            start,
            start + egui::vec2(40.0, 20.0) * scale,
        );
        let moved = app.editor.selected_node().unwrap().clone();
        assert_eq!((moved.x, moved.y), (200.0, 170.0));
        app.editor.undo();
        assert_eq!(app.editor.document.nodes[0], original);
        app.editor.redo();
        assert_eq!(app.editor.document.nodes[0], moved);

        let corner = board.min + egui::vec2(moved.x + moved.width, moved.y + moved.height) * scale;
        release_at_final_position(
            &ctx,
            &mut app,
            size,
            corner,
            corner + egui::vec2(60.0, 40.0) * scale,
        );
        let resized = app.editor.selected_node().unwrap().clone();
        assert_eq!((resized.width, resized.height), (280.0, 120.0));
        app.editor.undo();
        assert_eq!(app.editor.document.nodes[0], moved);
        app.editor.redo();
        assert_eq!(app.editor.document.nodes[0], resized);
    }
}

#[test]
fn release_frame_undo_runs_after_the_completed_gesture() {
    let size = [1380.0, 860.0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut editor = Editor::new(Document::default());
    editor
        .add_at(NodeKind::Rectangle, 160.0, 150.0, 220.0, 80.0)
        .unwrap();
    let original = editor.document.clone();
    let mut app = PaperCraftApp::from_document(original.clone());
    let output = frame(&ctx, &mut app, size, Vec::new());
    let board = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_rgb(248, 247, 243) => {
                Some(rect.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap();
    let scale = board.width() / app.editor.document.width;
    let start = board.min + egui::vec2(270.0, 190.0) * scale;
    let end = start + egui::vec2(40.0, 20.0) * scale;
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&ctx, &mut app, size, vec![egui::Event::PointerMoved(end)]);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events: vec![
                button(end, false),
                egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    assert_eq!(app.editor.document, original);
    assert!(app.editor.can_redo());
}

#[test]
fn release_frame_delete_follows_the_gesture_without_panicking_or_merging_history() {
    let size = [1380.0, 860.0];
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut editor = Editor::new(Document::default());
    editor
        .add_at(NodeKind::Rectangle, 160.0, 150.0, 220.0, 80.0)
        .unwrap();
    let original = editor.document.clone();
    let mut moved = original.clone();
    moved.nodes[0].x += 40.0;
    moved.nodes[0].y += 20.0;
    let mut app = PaperCraftApp::from_document(original.clone());
    let output = frame(&ctx, &mut app, size, Vec::new());
    let board = artboard(&output);
    let scale = board.width() / app.editor.document.width;
    let start = board.min + egui::vec2(270.0, 190.0) * scale;
    let end = start + egui::vec2(40.0, 20.0) * scale;
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            egui::Event::PointerMoved(end),
            button(end, false),
            egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(app.editor.document.nodes.is_empty());
    assert_eq!(app.editor.selected(), None);
    app.editor.undo();
    assert_eq!(app.editor.document, moved);
    app.editor.undo();
    assert_eq!(app.editor.document, original);
    app.editor.redo();
    assert_eq!(app.editor.document, moved);
    app.editor.redo();
    assert!(app.editor.document.nodes.is_empty());
}

#[test]
fn export_button_opens_both_format_choices_without_changing_the_document() {
    for size in [[1380.0, 860.0], [960.0, 640.0]] {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let document = Document::example();
        let mut app = PaperCraftApp::from_document(document.clone());
        let output = frame(&ctx, &mut app, size, Vec::new());
        let at = text_rect(&output, "Export")
            .expect("Export button")
            .center();
        release_at_final_position(&ctx, &mut app, size, at, at);
        let output = frame(&ctx, &mut app, size, Vec::new());
        assert!(text_rect(&output, "Export HTML…").is_some());
        assert!(text_rect(&output, "Export SVG…").is_some());
        assert!(text_rect(&output, "SVG keeps explicit text lines; fonts may differ.").is_some());
        assert_eq!(app.editor.document, document);
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn minimized_window_close_preserves_unsaved_work_before_rendering_ui() {
    let ctx = egui::Context::default();
    let mut app = PaperCraftApp::from_document(Document::default());
    app.editor.add(NodeKind::Rectangle).unwrap();
    let unsaved = app.editor.document.clone();
    let mut input = egui::RawInput::default();
    let root = input.viewports.entry(egui::ViewportId::ROOT).or_default();
    root.minimized = Some(true);
    root.events.push(egui::ViewportEvent::Close);
    let output = ctx.run_logic(&input, |ctx| {
        eframe::App::logic(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    let commands = output
        .viewport_commands
        .get(&egui::ViewportId::ROOT)
        .map(Vec::as_slice)
        .unwrap_or_default();
    assert!(
        commands.contains(&egui::ViewportCommand::CancelClose),
        "The hidden-window logic callback must cancel close while changes are unsaved"
    );
    assert_eq!(app.editor.document, unsaved);
}

fn frame(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    size: [f32; 2],
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output
}

fn artboard(output: &egui::FullOutput) -> egui::Rect {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_rgb(248, 247, 243) => {
                Some(rect.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("Artboard was painted")
}

fn text_rect(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text_shape) if text_shape.galley.text() == text => {
            Some(text_shape.galley.rect.translate(text_shape.pos.to_vec2()))
        }
        _ => None,
    })
}

fn release_at_final_position(
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
        vec![egui::Event::PointerMoved(start), button(start, true)],
        vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
        vec![egui::Event::PointerMoved(end), button(end, false)],
    ] {
        frame(ctx, app, size, events);
    }
}
