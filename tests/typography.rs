use eframe::egui;
use papercraft::app::PaperCraftApp;
use papercraft::document::{Document, Editor, Node, NodeKind, TextAlign};
use papercraft::render::rasterize;

fn text_node(id: u64, text: &str, line_height: Option<f32>, align: TextAlign) -> Node {
    Node {
        id,
        name: format!("T{id}"),
        kind: NodeKind::Text,
        x: 20.0,
        y: 20.0,
        width: 400.0,
        height: 160.0,
        fill: [20, 20, 20, 255],
        radius: 0.0,
        text: text.into(),
        font_size: 24.0,
        hidden: false,
        locked: false,
        fill_token: None,
        line_height,
        text_align: align,
    }
}

fn document(nodes: Vec<Node>) -> Document {
    let next_id = nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    let mut doc = Document {
        width: 440.0,
        height: 200.0,
        background: [255, 255, 255, 255],
        nodes,
        next_id,
        ..Document::default()
    };
    // Sets the format version the fields require, as loading and committing do.
    doc.normalize();
    doc
}

#[test]
fn format_version_follows_typography_fields_and_returns_to_legacy() {
    let mut doc = document(vec![text_node(1, "A", None, TextAlign::Left)]);
    assert_eq!(doc.required_version(), 1, "defaults keep version 1");
    doc.nodes[0].line_height = Some(32.0);
    assert_eq!(doc.required_version(), 3);
    doc.nodes[0].line_height = None;
    doc.nodes[0].text_align = TextAlign::Right;
    assert_eq!(doc.required_version(), 3);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("t.papercraft");
    doc.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"version\": 3") && saved.contains("\"text_align\": \"right\""));
    assert!(!saved.contains("line_height"), "defaults are omitted");
    assert_eq!(
        Document::load(&path).unwrap().nodes[0].text_align,
        TextAlign::Right
    );

    doc.nodes[0].text_align = TextAlign::Left;
    doc.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"version\": 1") && !saved.contains("text_align"));
}

#[test]
fn legacy_files_load_unchanged_and_old_versions_cannot_carry_new_fields() {
    for fixture in [
        "tests/fixtures/legacy.crumbs",
        "tests/fixtures/tokens-v2.papercraft",
        "tests/fixtures/svg-exchange.papercraft",
    ] {
        let doc = Document::load(std::path::Path::new(fixture)).unwrap();
        assert!(
            doc.nodes
                .iter()
                .all(|n| n.line_height.is_none() && n.text_align == TextAlign::Left)
        );
        assert!(doc.required_version() <= 2, "{fixture}");
    }
    let directory = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string("tests/fixtures/tokens-v2.papercraft").unwrap();
    let tampered = source.replacen("\"font_size\":", "\"line_height\": 30, \"font_size\":", 1);
    assert_ne!(tampered, source);
    let path = directory.path().join("v2-with-line-height.papercraft");
    std::fs::write(&path, &tampered).unwrap();
    let error = Document::load(&path).unwrap_err();
    assert!(error.contains("Version 1 and 2"), "{error}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        tampered,
        "rejected file is untouched"
    );
}

#[test]
fn invalid_typography_is_rejected_atomically() {
    let mut editor = Editor::new(document(vec![text_node(1, "A", None, TextAlign::Left)]));
    let before = editor.document.clone();
    for bad in [
        Some(0.0),
        Some(-4.0),
        Some(f32::NAN),
        Some(f32::INFINITY),
        Some(5000.0),
    ] {
        let result = editor.apply(|doc| {
            doc.nodes[0].line_height = bad;
            Ok(())
        });
        assert!(result.is_err(), "{bad:?}");
        assert_eq!(editor.document, before);
    }
    let mut rectangle = text_node(2, "", None, TextAlign::Center);
    rectangle.kind = NodeKind::Rectangle;
    let result = editor.apply(|doc| {
        doc.next_id = 3;
        doc.nodes.push(rectangle);
        Ok(())
    });
    assert!(result.unwrap_err().contains("Only text layers"));
    assert_eq!(editor.document, before);
    assert!(!editor.can_undo());
}

#[test]
fn html_and_svg_express_authored_typography_and_keep_legacy_output() {
    let legacy = document(vec![text_node(1, "One\nTwo", None, TextAlign::Left)]);
    let html = legacy.export_html().unwrap();
    assert!(html.contains("line-height:1.2;") && !html.contains("text-align"));
    let svg = legacy.export_svg().unwrap();
    assert!(svg.contains("<tspan x=\"20\" y=\"44\">One</tspan>"));
    assert!(svg.contains("y=\"72.8\">Two</tspan>"), "{svg}");
    assert!(!svg.contains("text-anchor"));

    let styled = document(vec![text_node(
        1,
        "One\nTwo",
        Some(40.0),
        TextAlign::Center,
    )]);
    let html = styled.export_html().unwrap();
    assert!(html.contains("line-height:40px;") && html.contains("text-align:center"));
    let svg = styled.export_svg().unwrap();
    assert!(svg.contains("text-anchor=\"middle\""));
    assert!(svg.contains("<tspan x=\"220\" y=\"44\">One</tspan>"));
    assert!(svg.contains("<tspan x=\"220\" y=\"84\">Two</tspan>"));
    let right = document(vec![text_node(1, "R", None, TextAlign::Right)]);
    assert!(right.export_svg().unwrap().contains("text-anchor=\"end\""));
}

/// Columns and rows that contain dark (text) pixels.
fn ink(doc: &Document) -> (Vec<u32>, Vec<u32>) {
    let raster = rasterize(doc, 1.0).unwrap();
    let dark = |x: u32, y: u32| raster.pixels[((y * raster.width + x) * 4) as usize] < 128;
    let columns = (0..raster.width)
        .filter(|&x| (0..raster.height).any(|y| dark(x, y)))
        .collect();
    let rows = (0..raster.height)
        .filter(|&y| (0..raster.width).any(|x| dark(x, y)))
        .collect();
    (columns, rows)
}

#[test]
fn rendering_honors_alignment_and_line_height() {
    let center = |columns: &[u32]| (columns[0] + columns[columns.len() - 1]) as f32 / 2.0;
    let (left, _) = ink(&document(vec![text_node(
        1,
        "Kitap",
        None,
        TextAlign::Left,
    )]));
    let (middle, _) = ink(&document(vec![text_node(
        1,
        "Kitap",
        None,
        TextAlign::Center,
    )]));
    let (right, _) = ink(&document(vec![text_node(
        1,
        "Kitap",
        None,
        TextAlign::Right,
    )]));
    assert!(left[0] < 30, "left text starts at the layer edge");
    assert!(
        (center(&middle) - 220.0).abs() < 3.0,
        "centered on the layer: {}",
        center(&middle)
    );
    assert!(
        *right.last().unwrap() > 410,
        "right text ends at the layer edge"
    );

    // Rows of ink for the second line move down by the added line height.
    let lines = |h| ink(&document(vec![text_node(1, "Ab\nAb", h, TextAlign::Left)])).1;
    let bottom = |rows: Vec<u32>| *rows.last().unwrap() as f32;
    let (tight, loose) = (bottom(lines(Some(30.0))), bottom(lines(Some(70.0))));
    assert!((loose - tight - 40.0).abs() <= 2.0, "{tight} → {loose}");
}

fn frame(
    ctx: &egui::Context,
    app: &mut PaperCraftApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1380.0, 860.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output
}

/// Painted rectangle of `text` nearest to `near`; the inspector shows the same text
/// in its own field once the layer is selected.
fn painted(output: &egui::FullOutput, text: &str, near: egui::Pos2) -> Option<egui::Rect> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            _ => None,
        })
        .min_by(|a, b| (a.min - near).length().total_cmp(&(b.min - near).length()))
}

#[test]
fn text_does_not_move_when_inline_editing_starts() {
    let text = "Merhaba\ndünya, uzun satır";
    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        for line_height in [None, Some(44.0)] {
            let ctx = egui::Context::default();
            PaperCraftApp::configure(&ctx);
            let mut app = PaperCraftApp::from_document(document(vec![text_node(
                1,
                text,
                line_height,
                align,
            )]));
            for _ in 0..3 {
                frame(&ctx, &mut app, Vec::new());
            }
            let still = painted(&frame(&ctx, &mut app, Vec::new()), text, egui::Pos2::ZERO)
                .expect("static text");
            app.editor.select(Some(1));
            let key = |pressed| egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            frame(&ctx, &mut app, vec![key(true), key(false)]);
            let mut editing = None;
            for _ in 0..4 {
                editing = painted(&frame(&ctx, &mut app, Vec::new()), text, still.min).or(editing);
            }
            let editing = editing.expect("edited text");
            assert!(
                (still.min - editing.min).length() < 0.6
                    && (still.size() - editing.size()).length() < 0.6,
                "{align:?} {line_height:?}: {still:?} vs {editing:?}"
            );
        }
    }
}
