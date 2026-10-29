use eframe::egui;
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Editor, Node, NodeKind},
};

fn large(layers: usize) -> Document {
    let mut doc = Document::default();
    for i in 0..layers {
        doc.nodes.push(Node {
            id: i as u64 + 1,
            name: format!("Layer {i}"),
            kind: NodeKind::Rectangle,
            x: (i % 50) as f32 * 18.0,
            y: (i / 50 % 30) as f32 * 18.0,
            width: 16.0,
            height: 16.0,
            fill: [124, 155, 214, 255],
            radius: 0.0,
            text: String::new(),
            font_size: 12.0,
            hidden: false,
            locked: false,
            fill_token: None,
            line_height: None,
            text_align: papercraft::document::TextAlign::Left,
            ..Node::default()
        });
    }
    doc.next_id = layers as u64 + 1;
    doc
}

fn run(ctx: &egui::Context, app: &mut PaperCraftApp) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1380.0, 860.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    output
}

#[test]
fn revisions_identify_committed_states_through_undo_and_redo() {
    let mut editor = Editor::new(Document::default());
    let initial = editor.revision();
    editor.add(NodeKind::Rectangle).unwrap();
    let added = editor.revision();
    assert_ne!(added, initial);

    // A commit that changes nothing keeps the revision.
    let before = editor.document.clone();
    editor.commit(before).unwrap();
    assert_eq!(editor.revision(), added);

    editor.undo();
    assert_eq!(editor.revision(), initial);
    editor.redo();
    assert_eq!(editor.revision(), added);

    // A new edit after undo gets a revision distinct from every earlier state.
    editor.undo();
    editor.add(NodeKind::Text).unwrap();
    assert_ne!(editor.revision(), initial);
    assert_ne!(editor.revision(), added);
}

#[test]
fn undoing_back_to_the_saved_state_leaves_no_unsaved_changes() {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::example());
    run(&ctx, &mut app);
    let before = app.editor.document.clone();
    app.editor.document.nodes[0].x += 5.0;
    app.editor.commit(before).unwrap();
    run(&ctx, &mut app);
    assert!(!app.close_document(), "an unsaved edit blocks closing");
    app.editor.undo();
    run(&ctx, &mut app);
    assert!(app.close_document(), "the saved state needs no prompt");
}

#[test]
fn large_selections_and_layer_lists_stay_bounded() {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(large(10_000));
    app.editor.select_all();
    assert_eq!(app.editor.selection().len(), 10_000);
    let output = run(&ctx, &mut app);
    // Only rows in view are built: the list shows the top of the paint order.
    let rows: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with("Layer ") => {
                Some(t.galley.text().to_owned())
            }
            _ => None,
        })
        .collect();
    assert!(!rows.is_empty() && rows.len() < 100, "{} rows", rows.len());
    assert!(rows.contains(&"Layer 9999".to_owned()));

    app.editor.toggle_selection(5_000);
    assert_eq!(app.editor.selection().len(), 9_999);
    assert!(!app.editor.is_selected(5_000));
    app.editor.delete().unwrap();
    assert_eq!(app.editor.document.nodes.len(), 1);
    assert_eq!(app.editor.selection(), &[] as &[u64]);
    app.editor.undo();
    assert_eq!(app.editor.document.nodes.len(), 10_000);
    assert!(app.editor.selection().is_empty());
}
