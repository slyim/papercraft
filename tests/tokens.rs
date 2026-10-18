//! Color tokens: propagation, references, deletion, history, persistence and export.

use eframe::egui;
use papercraft::{
    app::PaperCraftApp,
    document::{ColorTarget, Document, Editor, NodeKind},
    recents::Recents,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/tokens-v2.papercraft");

fn fixture(directory: &std::path::Path) -> std::path::PathBuf {
    let path = directory.join("tokens.papercraft");
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

fn editor_with_rectangles(count: usize) -> Editor {
    let mut editor = Editor::new(Document::default());
    for index in 0..count {
        editor
            .add_at(
                NodeKind::Rectangle,
                40.0 + index as f32 * 100.0,
                40.0,
                80.0,
                60.0,
            )
            .unwrap();
    }
    editor
}

#[test]
fn token_edits_update_every_bound_property_and_keep_literals_working() {
    let mut editor = editor_with_rectangles(3);
    let [a, b, c] = [0, 1, 2].map(|i| editor.document.nodes[i].id);
    let token = editor.create_token([10, 20, 30, 255]).unwrap();
    for target in [
        ColorTarget::Node(a),
        ColorTarget::Node(b),
        ColorTarget::Background,
        ColorTarget::Canvas,
    ] {
        editor.bind(target, Some(token)).unwrap();
    }
    let literal = editor.document.nodes[2].fill;
    editor.set_token_value(token, [200, 100, 50, 128]).unwrap();

    let doc = &editor.document;
    for node in &doc.nodes[..2] {
        assert_eq!(doc.node_fill(node), [200, 100, 50, 128]);
        assert_eq!(
            node.fill,
            [200, 100, 50, 128],
            "Bound literal is synchronized"
        );
    }
    assert_eq!(doc.background_color(), [200, 100, 50, 128]);
    assert_eq!(doc.canvas_color(), [200, 100, 50, 128]);
    assert_eq!(doc.nodes[2].fill, literal, "Unbound literal is unchanged");
    assert_eq!(doc.node_fill(&doc.nodes[2]), literal);
    assert_eq!(doc.token_usage(token).len(), 4);
    assert_eq!(doc.binding(ColorTarget::Node(c)), None);

    // Unbound literal edits still work alongside tokens.
    let before = editor.document.clone();
    editor.document.nodes[2].fill = [1, 2, 3, 4];
    editor.commit(before).unwrap();
    assert_eq!(
        editor.document.node_fill(&editor.document.nodes[2]),
        [1, 2, 3, 4]
    );
}

#[test]
fn renaming_and_names_are_validated() {
    let mut editor = Editor::new(Document::default());
    let first = editor.create_token([0, 0, 0, 255]).unwrap();
    let second = editor.create_token([255, 255, 255, 255]).unwrap();
    assert_eq!(editor.document.token(first).unwrap().name, "color-1");
    assert_eq!(editor.document.token(second).unwrap().name, "color-2");
    editor.rename_token(first, " surface-muted ").unwrap();
    assert_eq!(editor.document.token(first).unwrap().name, "surface-muted");
    editor.rename_token(second, "Überschrift_2").unwrap();
    let before = editor.document.clone();
    for invalid in [
        "",
        "SURFACE-muted",
        "has space",
        "semi;colon",
        "brace}",
        &"x".repeat(65),
    ] {
        assert!(editor.rename_token(second, invalid).is_err(), "{invalid:?}");
    }
    assert_eq!(editor.document, before, "Rejected names change nothing");
    // A new default name skips names already taken.
    editor.rename_token(second, "color-3").unwrap();
    let third = editor.create_token([1, 1, 1, 255]).unwrap();
    assert_eq!(editor.document.token(third).unwrap().name, "color-1");
}

#[test]
fn invalid_references_are_rejected_without_replacing_work() {
    let mut editor = editor_with_rectangles(1);
    let id = editor.document.nodes[0].id;
    let token = editor.create_token([5, 5, 5, 255]).unwrap();
    let valid = editor.document.clone();

    assert!(
        editor
            .bind(ColorTarget::Node(id), Some(token + 10))
            .is_err()
    );
    assert_eq!(editor.document, valid);

    // A direct edit that leaves a dangling reference is rolled back by commit.
    let before = editor.document.clone();
    editor.document.nodes[0].fill_token = Some(99);
    assert!(editor.commit(before).is_err());
    assert_eq!(editor.document, valid);

    let directory = tempfile::tempdir().unwrap();
    let cases = [
        ("dangling", serde_json::json!({"fill_token": 99}), "nodes"),
        ("v1 with tokens", serde_json::json!({"version": 1}), "root"),
        ("newer", serde_json::json!({"version": 3}), "root"),
        (
            "duplicate token",
            serde_json::json!({"tokens": [
                {"id": 1, "name": "a", "value": [0, 0, 0, 255]},
                {"id": 1, "name": "b", "value": [0, 0, 0, 255]}]}),
            "root",
        ),
        (
            "case duplicate",
            serde_json::json!({"tokens": [
                {"id": 1, "name": "Accent", "value": [0, 0, 0, 255]},
                {"id": 2, "name": "accent", "value": [0, 0, 0, 255]}]}),
            "root",
        ),
        (
            "token id beyond next",
            serde_json::json!({"next_token_id": 2}),
            "root",
        ),
    ];
    let original: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    for (label, patch, place) in cases {
        let mut value = original.clone();
        let target = if place == "nodes" {
            &mut value["nodes"][0]
        } else {
            &mut value
        };
        for (key, field) in patch.as_object().unwrap() {
            target[key] = field.clone();
        }
        let path = directory.path().join(format!("{label}.papercraft"));
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = Document::load(&path).expect_err(label);
        if label == "newer" {
            assert!(error.contains("newer PaperCraft"), "{error}");
        }
    }
}

#[test]
fn deleting_a_token_detaches_bindings_and_preserves_their_colors() {
    let directory = tempfile::tempdir().unwrap();
    let mut editor = Editor::new(Document::load(&fixture(directory.path())).unwrap());
    let accent = editor
        .document
        .tokens
        .iter()
        .find(|t| t.name == "accent")
        .unwrap()
        .id;
    let before = editor.document.clone();
    let shown: Vec<_> = editor
        .document
        .nodes
        .iter()
        .map(|n| editor.document.node_fill(n))
        .collect();
    editor.delete_token(accent).unwrap();

    let doc = &editor.document;
    assert!(doc.token(accent).is_none());
    assert!(doc.nodes.iter().all(|n| n.fill_token.is_none()));
    // Locked "Label" is detached too, keeping its color.
    assert!(doc.nodes[1].locked);
    let after: Vec<_> = doc.nodes.iter().map(|n| doc.node_fill(n)).collect();
    assert_eq!(after, shown, "Every effective color is preserved");
    assert_eq!(doc.nodes[0].fill, [39, 65, 48, 204]);
    doc.validate().unwrap();

    editor.undo();
    assert_eq!(
        editor.document, before,
        "One undo restores the token and bindings"
    );
    editor.redo();
    assert!(editor.document.token(accent).is_none());
}

#[test]
fn token_operations_are_single_history_steps() {
    let mut editor = editor_with_rectangles(1);
    let id = editor.document.nodes[0].id;
    let original = fill(&editor.document, id);
    let start = editor.document.clone();

    // Creating from a property creates and binds in one step.
    let token = editor
        .create_token_from(ColorTarget::Node(id))
        .unwrap()
        .unwrap();
    assert_eq!(editor.document.binding(ColorTarget::Node(id)), Some(token));
    assert_eq!(editor.document.token(token).unwrap().value, original);
    editor.undo();
    assert_eq!(editor.document, start);
    editor.redo();

    let steps: [(&str, Step); 4] = [
        (
            "rename",
            Box::new(move |e| e.rename_token(token, "brand").unwrap()),
        ),
        (
            "value",
            Box::new(move |e| e.set_token_value(token, [9, 9, 9, 255]).unwrap()),
        ),
        (
            "detach",
            Box::new(move |e| e.bind(ColorTarget::Node(id), None).unwrap()),
        ),
        (
            "rebind",
            Box::new(move |e| e.bind(ColorTarget::Node(id), Some(token)).unwrap()),
        ),
    ];
    for (label, step) in steps {
        let before = editor.document.clone();
        step(&mut editor);
        assert_ne!(editor.document, before, "{label} changed the document");
        editor.undo();
        assert_eq!(editor.document, before, "One undo reverts {label}");
        editor.redo();
    }
    // Detaching keeps the current token color as the literal.
    editor.set_token_value(token, [7, 8, 9, 255]).unwrap();
    editor.bind(ColorTarget::Node(id), None).unwrap();
    assert_eq!(editor.document.nodes[0].fill, [7, 8, 9, 255]);
    editor.set_token_value(token, [1, 1, 1, 255]).unwrap();
    assert_eq!(
        fill(&editor.document, id),
        [7, 8, 9, 255],
        "Detached stays put"
    );

    // Locked layers keep their binding unchanged.
    editor.document.nodes[0].locked = true;
    let locked = editor.document.clone();
    editor.bind(ColorTarget::Node(id), Some(token)).unwrap();
    assert_eq!(
        editor.create_token_from(ColorTarget::Node(id)).unwrap(),
        None
    );
    assert_eq!(editor.document, locked);
}

#[test]
fn persistence_writes_the_lowest_version_and_migrates_without_touching_originals() {
    let directory = tempfile::tempdir().unwrap();

    // Version 2 fixture: loads, resolves stale literals to token values, round trips.
    let path = fixture(directory.path());
    let document = Document::load(&path).unwrap();
    assert_eq!(document.version, 2);
    assert_eq!(
        document.nodes[0].fill,
        [39, 65, 48, 204],
        "Token wins over literal"
    );
    assert_eq!(document.background, [246, 241, 232, 255]);
    let copy = directory.path().join("copy.papercraft");
    document.save(&copy).unwrap();
    assert_eq!(Document::load(&copy).unwrap(), document);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        FIXTURE,
        "Loading never rewrites"
    );

    // Version 1 inputs (legacy .crumbs and SVG fixture) keep the version 1 shape.
    for (name, bytes) in [
        (
            "legacy.crumbs",
            &include_bytes!("fixtures/legacy.crumbs")[..],
        ),
        (
            "exchange.papercraft",
            &include_bytes!("fixtures/svg-exchange.papercraft")[..],
        ),
    ] {
        let original = directory.path().join(name);
        std::fs::write(&original, bytes).unwrap();
        let document = Document::load(&original).unwrap();
        assert_eq!(document.version, 1);
        let resaved = directory.path().join(format!("resaved-{name}"));
        document.save(&resaved).unwrap();
        let keys = |bytes: &[u8]| {
            let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
            for node in value["nodes"].as_array().unwrap() {
                keys.extend(
                    node.as_object()
                        .unwrap()
                        .keys()
                        .map(|k| format!("node.{k}")),
                );
            }
            keys.sort();
            keys.dedup();
            keys
        };
        let written = std::fs::read(&resaved).unwrap();
        let mut expected = keys(bytes);
        if !expected.contains(&"canvas".to_string()) {
            // Earlier version 1 files omit the optional page color.
            expected.push("canvas".into());
            expected.sort();
        }
        assert_eq!(keys(&written), expected, "{name}: no version 2 keys");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&written).unwrap()["version"],
            1
        );

        // Migration: adding a token writes version 2 to the new path only.
        let mut editor = Editor::new(document);
        let token = editor.create_token([1, 2, 3, 255]).unwrap();
        editor.bind(ColorTarget::Background, Some(token)).unwrap();
        let migrated = directory.path().join(format!("migrated-{name}.papercraft"));
        editor.document.save(&migrated).unwrap();
        assert_eq!(Document::load(&migrated).unwrap().version, 2);
        assert_eq!(
            std::fs::read(&original).unwrap(),
            bytes,
            "{name}: original intact"
        );

        // Rollback: deleting every token returns to version 1 with the same colors.
        let shown = editor.document.background_color();
        editor.delete_token(token).unwrap();
        let rolled = directory.path().join(format!("rolled-{name}.papercraft"));
        editor.document.save(&rolled).unwrap();
        let rolled = Document::load(&rolled).unwrap();
        assert_eq!(rolled.version, 1);
        assert_eq!(rolled.background, shown);
    }
}

#[test]
fn html_and_svg_exports_resolve_the_same_colors() {
    let directory = tempfile::tempdir().unwrap();
    let document = Document::load(&fixture(directory.path())).unwrap();
    let html = document.export_html().unwrap();
    assert!(html.contains(
        "<style>:root{--color-surface:rgba(246,241,232,1.0000);\
         --color-accent:rgba(39,65,48,0.8000);--color-page:rgba(30,30,34,1.0000);}</style>"
    ));
    assert!(html.contains("<body style=\"margin:0;background:var(--color-page)\">"));
    assert!(html.contains("background:var(--color-surface)\">"));
    assert!(html.contains("background:var(--color-accent);border-radius:12px"));
    assert!(html.contains("color:var(--color-accent)\">Tokens &amp; &lt;values&gt;</div>"));
    assert!(
        html.contains("background:rgba(200,30,30,1.0000)"),
        "Unbound literal"
    );

    let svg = document.export_svg().unwrap();
    assert!(
        svg.contains(
            "<rect width=\"400\" height=\"300\" fill=\"#f6f1e8\" fill-opacity=\"1.0000\"/>"
        )
    );
    assert_eq!(
        svg.matches("fill=\"#274130\" fill-opacity=\"0.8000\"")
            .count(),
        2
    );
    assert!(svg.contains("fill=\"#c81e1e\""));
    assert!(!svg.contains("#010203"), "Stale literal is never exported");
}

fn run(ctx: &egui::Context, app: &mut PaperCraftApp, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1380.0, 860.0),
        )),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    output
}

fn text_rect(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(t) if t.galley.text() == text => {
            Some(t.galley.rect.translate(t.pos.to_vec2()))
        }
        _ => None,
    })
}

fn leftmost(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            _ => None,
        })
        .min_by(|l, r| l.left().total_cmp(&r.left()))
}

fn has_fill(output: &egui::FullOutput, value: [u8; 4]) -> bool {
    let fill = egui::Color32::from_rgba_unmultiplied(value[0], value[1], value[2], value[3]);
    output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Rect(rect) => rect.fill == fill,
        _ => false,
    })
}

fn click(ctx: &egui::Context, app: &mut PaperCraftApp, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    run(ctx, app, vec![egui::Event::PointerMoved(at), button(true)]);
    run(ctx, app, vec![button(false)]);
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn inspector_and_theme_tab_bind_edit_and_detach_through_the_interface() {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut editor = editor_with_rectangles(2);
    let [a, b] = [0, 1].map(|i| editor.document.nodes[i].id);
    editor.select(Some(a));
    let mut app = PaperCraftApp::from_document(editor.document.clone());
    app.editor.select(Some(a));

    // Inspector: token menu → create a token from the fill (one step).
    let output = run(&ctx, &mut app, Vec::new());
    let menu = text_rect(&output, egui_phosphor::regular::CIRCLES_FOUR).expect("Token button");
    click(&ctx, &mut app, menu.center());
    let output = run(&ctx, &mut app, Vec::new());
    let create = text_rect(&output, "Create token from this color").expect("Token menu");
    click(&ctx, &mut app, create.center());
    let token = app
        .editor
        .document
        .binding(ColorTarget::Node(a))
        .expect("Bound");
    assert_eq!(app.editor.document.token(token).unwrap().name, "color-1");

    // Bind the second layer from the same menu.
    app.editor.select(Some(b));
    let output = run(&ctx, &mut app, Vec::new());
    let menu = text_rect(&output, egui_phosphor::regular::CIRCLES_FOUR).unwrap();
    click(&ctx, &mut app, menu.center());
    let output = run(&ctx, &mut app, Vec::new());
    let item = text_rect(&output, "color-1").expect("Token listed in menu");
    click(&ctx, &mut app, item.center());
    assert_eq!(
        app.editor.document.binding(ColorTarget::Node(b)),
        Some(token)
    );
    let output = run(&ctx, &mut app, Vec::new());
    assert!(
        text_rect(&output, "color-1").is_some(),
        "Bound field shows the token name"
    );

    // Theme tab: select the token and edit its value as hexadecimal text.
    let theme = text_rect(&output, "Theme").expect("Theme segment");
    click(&ctx, &mut app, theme.center());
    assert!(app.theme);
    let output = run(&ctx, &mut app, Vec::new());
    let row = leftmost(&output, "color-1").expect("Token row in the left panel");
    click(&ctx, &mut app, row.center());
    let output = run(&ctx, &mut app, Vec::new());
    let hex = leftmost(&output, "7C9BD6").expect("Token value field in the left panel");
    let before_value = app.editor.document.clone();
    click(&ctx, &mut app, hex.center());
    let mut events = vec![key(egui::Key::End)];
    events.extend((0..6).map(|_| key(egui::Key::Backspace)));
    run(&ctx, &mut app, events);
    run(&ctx, &mut app, vec![egui::Event::Text("204060".into())]);
    // Live propagation: the canvas paints both layers with the edited value.
    let output = run(&ctx, &mut app, Vec::new());
    assert!(has_fill(&output, [32, 64, 96, 255]));
    run(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    run(&ctx, &mut app, Vec::new());
    let doc = &app.editor.document;
    assert_eq!(fill(doc, a), [32, 64, 96, 255]);
    assert_eq!(fill(doc, b), [32, 64, 96, 255]);
    assert_eq!(doc.nodes.len(), 2, "Typing did not delete layers");
    app.editor.undo();
    assert_eq!(
        app.editor.document, before_value,
        "One undo reverts the value edit"
    );
    app.editor.redo();

    // Detach from the inspector keeps the color.
    click(&ctx, &mut app, theme.center());
    let output = run(&ctx, &mut app, Vec::new());
    let design = text_rect(&output, "Design").unwrap();
    click(&ctx, &mut app, design.center());
    assert!(!app.theme);
    app.editor.select(Some(a));
    let output = run(&ctx, &mut app, Vec::new());
    let detach = text_rect(&output, egui_phosphor::regular::LINK_BREAK).expect("Detach");
    click(&ctx, &mut app, detach.center());
    assert_eq!(app.editor.document.binding(ColorTarget::Node(a)), None);
    assert_eq!(fill(&app.editor.document, a), [32, 64, 96, 255]);
}

#[test]
fn dashboard_previews_paint_resolved_token_colors() {
    let directory = tempfile::tempdir().unwrap();
    let path = fixture(directory.path());
    let mut recents = Recents::load(directory.path().join("recents.json"));
    recents.touch(&path).unwrap();
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::default()).with_recents(recents);
    assert!(app.close_document());
    run(&ctx, &mut app, Vec::new());
    let output = run(&ctx, &mut app, Vec::new());
    assert!(text_rect(&output, "Token fixture").is_some(), "Card shown");
    assert!(
        has_fill(&output, [246, 241, 232, 255]),
        "Artboard token color"
    );
    assert!(has_fill(&output, [30, 30, 34, 255]), "Page token color");
    assert!(
        !has_fill(&output, [1, 2, 3, 255]),
        "Stale literal never painted"
    );
}

type Step = Box<dyn Fn(&mut Editor)>;

fn fill(document: &Document, id: u64) -> [u8; 4] {
    document.target_color(ColorTarget::Node(id)).unwrap()
}
