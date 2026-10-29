use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use papercraft::{
    app::{PaperCraftApp, Tool},
    document::{Document, Node, NodeKind},
    mcp::Server,
};
use serde_json::{Value, json};

const SIZE: [f32; 2] = [1380.0, 860.0];

fn frame(ctx: &egui::Context, app: &mut PaperCraftApp, events: Vec<Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(SIZE[0], SIZE[1]))),
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
            egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgb(248, 247, 243) => {
                Some(r.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap()
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    vec![
        Event::ModifiersChanged(modifiers),
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
        Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        },
    ]
}

fn assert_live_busy(app: &mut PaperCraftApp) {
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"document_new","arguments":{}}});
    let response = app.handle_live_request(&request.to_string()).unwrap();
    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Editor busy")
    );
}

fn setup(document: Document) -> (egui::Context, PaperCraftApp, Rect) {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(document);
    frame(&ctx, &mut app, vec![]);
    let rect = board(&frame(&ctx, &mut app, vec![]));
    (ctx, app, rect)
}

#[test]
fn offboard_human_creation_paints_selects_and_roundtrips() {
    let (ctx, mut app, b) = setup(Document::default());
    let zoom = b.width() / 960.0;
    let at = |x, y| b.min + vec2(x, y) * zoom;
    app.tool = Tool::Rectangle;
    let (start, end) = (at(-60.0, 100.0), at(-20.0, 160.0));
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(start), button(start, true)],
    );
    frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert!(
        app.editor.document.nodes.is_empty(),
        "creation is only a preview"
    );
    frame(&ctx, &mut app, vec![button(end, false)]);
    assert_eq!(app.editor.document.nodes.len(), 1);
    let node = app.editor.document.nodes[0].clone();
    assert_eq!(
        [node.x, node.y, node.width, node.height],
        [-60.0, 100.0, 40.0, 60.0]
    );
    let output = frame(&ctx, &mut app, vec![]);
    assert!(
        output.shapes.iter().any(|s| match &s.shape {
            egui::Shape::Rect(r) =>
                r.rect.contains(at(-40.0, 130.0))
                    && s.clip_rect.contains(at(-40.0, 130.0))
                    && r.fill == papercraft::render::color(node.fill),
            _ => false,
        }),
        "offboard geometry is actually painted, not merely stored"
    );
    app.editor.select(None);
    let click = at(-40.0, 130.0);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(click), button(click, true)],
    );
    frame(&ctx, &mut app, vec![button(click, false)]);
    assert_eq!(app.editor.selection(), &[node.id]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("offboard.crumbs");
    app.editor.document.save(&path).unwrap();
    assert_eq!(Document::load(&path).unwrap(), app.editor.document);
    // The same authored geometry/style reaches the same native document through
    // GUI input, headless MCP and live MCP; no toolbar-specific model is involved.
    for mut client in [
        Client::Headless(Server::default()),
        Client::Live(Box::new(PaperCraftApp::from_document(Document::default()))),
    ] {
        let (_, state, _) = client.call("document_new", json!({}));
        let id = &state["document_id"];
        let (error, result, _) = client.call("document_apply", json!({"document_id":id,"expected_revision":0,"operations":[
            {"op":"create_rectangle","ref":"shape","x":node.x,"y":node.y,"width":node.width,"height":node.height,"fill":node.fill},
            {"op":"select_layers","ids":[{"ref":"shape"}]}
        ]}));
        assert!(!error, "{result}");
        assert_eq!(result["revision"], app.editor.revision());
        let agent_path = dir.path().join("agent.papercraft");
        let (error, result, _) =
            client.call("document_save", json!({"document_id":id,"path":agent_path}));
        assert!(!error, "{result}");
        assert_eq!(Document::load(&agent_path).unwrap(), app.editor.document);
    }
    let crop = papercraft::render::rasterize_region(
        &app.editor.document,
        [-60.0, 100.0, -20.0, 160.0],
        1.0,
    )
    .unwrap();
    assert_eq!(&crop.pixels[4 * (20 + 30 * 40)..][..4], &node.fill);
    assert!(
        app.editor
            .document
            .export_svg()
            .unwrap()
            .contains("overflow=\"hidden\"")
    );
    assert!(
        app.editor
            .document
            .export_html()
            .unwrap()
            .contains("overflow:hidden")
    );
    app.editor.undo();
    assert!(
        app.editor.document.nodes.is_empty(),
        "creation was one undo step"
    );
    app.editor.redo();
    assert_eq!(app.editor.document.nodes, vec![node]);
}

#[test]
fn gesture_drafts_cancel_without_touching_document_history_or_redo() {
    for cancel in [Key::Escape, Key::F] {
        let (ctx, mut app, b) = setup(Document::default());
        app.snapping = false;
        app.editor
            .add_at(NodeKind::Rectangle, 100.0, 100.0, 100.0, 100.0)
            .unwrap();
        app.editor.add(NodeKind::Text).unwrap();
        app.editor.undo();
        app.editor.select(Some(1));
        let original = app.editor.document.clone();
        let revision = app.editor.revision();
        let scale = b.width() / 960.0;
        let start = b.min + vec2(150.0, 150.0) * scale;
        let end = start + vec2(40.0, 20.0) * scale;
        frame(&ctx, &mut app, vec![]);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(start + (end - start) * 0.5)],
        );
        frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        let response = app.handle_live_request(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"document_new","arguments":{}}}).to_string()).unwrap();
        assert_eq!(
            response["result"]["isError"], true,
            "gesture did not start: board={b:?}, start={start:?}"
        );
        assert_eq!(
            app.editor.document, original,
            "a drag must not mutate committed state"
        );
        let cancellation = if cancel == Key::Escape {
            key(cancel, Modifiers::NONE)
        } else {
            vec![Event::WindowFocused(false)]
        };
        frame(&ctx, &mut app, cancellation);
        frame(
            &ctx,
            &mut app,
            vec![button(end, false), Event::WindowFocused(true)],
        );
        assert_eq!(app.editor.document, original, "cancel={cancel:?}");
        assert_eq!(app.editor.revision(), revision);
        assert!(app.editor.can_redo());
        app.editor.redo();
        assert_eq!(
            app.editor.document.nodes.len(),
            2,
            "cancel did not replace redo"
        );
    }
}

#[test]
fn fit_recovers_distant_content_and_zoom_stays_pointer_anchored() {
    let mut document = Document::default();
    document.nodes.push(Node {
        id: 1,
        x: 10_000.0,
        y: -2_000.0,
        width: 240.0,
        height: 160.0,
        fill: [10, 200, 70, 255],
        ..Node::default()
    });
    document.next_id = 2;
    let (ctx, mut app, _) = setup(document);
    let original = app.editor.document.clone();
    let output = frame(&ctx, &mut app, key(Key::Num1, Modifiers::SHIFT));
    let rect = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgb(10, 200, 70) => {
                Some((r.rect, s.clip_rect))
            }
            _ => None,
        })
        .expect("fit content reveals distant geometry");
    assert!(rect.1.contains_rect(rect.0));
    app.editor.select(Some(1));
    let output = frame(&ctx, &mut app, key(Key::Num2, Modifiers::SHIFT));
    let before = board(&output);
    let zoom = before.width() / 960.0;
    let pointer = pos2(700.0, 450.0);
    let world = (pointer - before.min) / zoom;
    frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(pointer),
        ],
    );
    let after = board(&frame(&ctx, &mut app, vec![Event::Zoom(1.5)]));
    let anchored = after.min + world * (after.width() / 960.0);
    assert!(
        anchored.distance(pointer) < 0.05,
        "{anchored:?} != {pointer:?}"
    );
    frame(&ctx, &mut app, key(Key::Num0, Modifiers::SHIFT));
    assert_eq!(app.editor.document, original);
    assert!(!app.editor.can_undo(), "navigation is not document history");
}

enum Client {
    Headless(Server),
    Live(Box<PaperCraftApp>),
}
impl Client {
    fn call(&mut self, name: &str, arguments: Value) -> (bool, Value, Value) {
        let line = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}}).to_string();
        let response = match self {
            Self::Headless(s) => s.handle_line(&line),
            Self::Live(a) => a.handle_live_request(&line),
        }
        .unwrap();
        let result = &response["result"];
        let text = result["content"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|c| c["type"] == "text")
            .unwrap()["text"]
            .as_str()
            .unwrap();
        (
            result["isError"] == true,
            serde_json::from_str(text).unwrap_or(json!(text)),
            result.clone(),
        )
    }
}

#[test]
fn live_and_headless_build_measure_render_refine_save_with_typed_local_refs() {
    for mut client in [
        Client::Headless(Server::default()),
        Client::Live(Box::new(PaperCraftApp::from_document(Document::default()))),
    ] {
        let (_, state, _) = client.call("document_new", json!({"width":40,"height":20}));
        let id = state["document_id"].clone();
        let (error, result, _) = client.call("document_apply", json!({"document_id":id,"expected_revision":0,"operations":[
            {"op":"create_token","ref":"ink","name":"ink","value":"#f00"},
            {"op":"create_frame","ref":"screen","x":-40,"y":0,"width":20,"height":20,"radius":0,"fill":"#fff"},
            {"op":"create_rectangle","ref":"button","parent":{"ref":"screen"},"x":-36,"y":4,"width":8,"height":8,"fill_token":{"ref":"ink"}},
            {"op":"create_frame","ref":"second","x":80,"y":0,"width":20,"height":20,"radius":0},
            {"op":"create_directive","ref":"note","x":-30,"y":10,"message":"Check button","layer_id":{"ref":"button"}},
            {"op":"resolve_directive","id":{"ref":"note"},"resolved":true},
            {"op":"select_layers","ids":[{"ref":"button"}]}
        ]}));
        assert!(!error, "{result}");
        assert_eq!(result["revision"], 1);
        assert_eq!(result["refs"]["screen"], 1);
        assert_eq!(
            result["created"][2]["bounds"],
            json!([-36.0, 4.0, -28.0, 12.0])
        );
        let (_, inspected, _) = client.call("document_inspect", json!({"document_id":id}));
        assert_eq!(inspected["coordinate_space"], "world");
        assert_eq!(inspected["directives"][0]["layer_id"], 2);
        assert_eq!(inspected["directives"][0]["resolved"], true);
        assert_eq!(
            inspected["content_bounds"],
            json!([-40.0, 0.0, 100.0, 20.0])
        );
        let (error, rendered, content) = client.call(
            "document_render",
            json!({"document_id":id,"frame_id":1,"scale":2}),
        );
        assert!(!error, "{rendered}");
        assert_eq!(rendered["bounds"], json!([-40.0, 0.0, -20.0, 20.0]));
        assert_eq!(
            (rendered["width"].as_u64(), rendered["height"].as_u64()),
            (Some(40), Some(40))
        );
        assert_eq!(content["content"][0]["mimeType"], "image/png");
        let (error, rendered, _) = client.call("document_render", json!({"document_id":id,"region":{"x":-100000,"y":0,"width":100000,"height":1},"scale":0.1}));
        assert!(!error, "{rendered}");
        assert_eq!(rendered["width"], 10000);
        assert_eq!(rendered["height"], 1);
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("frame.png");
        let (error, value, _) = client.call("document_export", json!({"document_id":id,"format":"png","path":png,"region":{"x":-40,"y":0,"width":20,"height":20}}));
        assert!(!error, "{value}");
        let image = image::open(&png).unwrap().to_rgba8();
        assert_eq!(image.get_pixel(8, 8).0, [255, 0, 0, 255]);
        let (error, value, _) = client.call("document_apply", json!({"document_id":id,"expected_revision":1,"operations":[{"op":"update_layer","id":2,"width":10}]}));
        assert!(!error, "{value}");
        let path = dir.path().join("screens.papercraft");
        let (error, value, _) = client.call("document_save", json!({"document_id":id,"path":path}));
        assert!(!error, "{value}");
        assert_eq!(value["dirty"], false);
        let reopened = Document::load(&path).unwrap();
        assert_eq!(reopened.nodes[1].x, -36.0);
        assert_eq!(reopened.nodes[1].width, 10.0);
        client.call("document_undo", json!({"document_id":id}));
        let (_, before, _) = client.call("document_inspect", json!({"document_id":id}));
        assert_eq!(before["state"]["can_redo"], true);
        for operations in [
            json!([{"op":"create_rectangle","ref":"a"},{"op":"update_layer","id":{"ref":"missing"},"x":1}]),
            json!([{"op":"create_rectangle","ref":"a"},{"op":"create_text","ref":"a"}]),
            json!([{"op":"create_token","ref":"t","name":"other","value":"#fff"},{"op":"update_layer","id":{"ref":"t"},"x":1}]),
            json!([{"op":"create_rectangle","ref":"a","width":0}]),
            json!([{"op":"create_rectangle","ref":"a"},{"op":"select_layers","ids":[{"ref":"a"}]},{"op":"create_rectangle","width":0}]),
            json!([{"op":"create_rectangle","ref":"bad label"}]),
            json!([{"op":"create_rectangle","ref":"a"},{"op":"resolve_directive","id":{"ref":"a"},"resolved":true}]),
        ] {
            assert!(
                client
                    .call(
                        "document_apply",
                        json!({"document_id":id,"operations":operations})
                    )
                    .0
            );
            assert_eq!(
                client.call("document_inspect", json!({"document_id":id})).1,
                before
            );
        }
        let keep = b"existing export";
        std::fs::write(&png, keep).unwrap();
        for arguments in [
            json!({"document_id":id,"format":"png","path":png,"region":{"x":0,"y":0,"width":20000,"height":1}}),
            json!({"document_id":id,"format":"svg","path":png,"region":{"x":0,"y":0,"width":20,"height":20}}),
            json!({"document_id":id,"format":"png","path":png,"frame_id":1,"region":{"x":0,"y":0,"width":20,"height":20}}),
            json!({"document_id":id,"format":"png","path":png,"region":{"x":0,"y":0,"width":9000,"height":9000}}),
            json!({"document_id":id,"format":"png","path":png,"region":{"x":0,"y":0,"width":0,"height":20}}),
            json!({"document_id":id,"format":"png","path":png,"frame_id":2}),
            json!({"document_id":id,"format":"png","path":png,"frame_id":999}),
            json!({"document_id":id,"format":"png","path":png,"scale":0}),
        ] {
            assert!(client.call("document_export", arguments).0);
            assert_eq!(std::fs::read(&png).unwrap(), keep);
        }
        assert_eq!(
            client.call("document_inspect", json!({"document_id":id})).1,
            before
        );
        client.call("document_redo", json!({"document_id":id}));
        assert_eq!(
            client.call("document_inspect", json!({"document_id":id})).1["state"]["dirty"],
            false
        );
    }
}

#[test]
fn creation_resize_and_marquee_cancel_preserve_redo_and_selection() {
    for kind in ["creation", "resize", "marquee", "press"] {
        let (ctx, mut app, b) = setup(Document::default());
        app.snapping = false;
        app.editor
            .add_at(NodeKind::Rectangle, 100.0, 100.0, 100.0, 100.0)
            .unwrap();
        app.editor.add(NodeKind::Text).unwrap();
        app.editor.undo();
        app.editor.select(Some(1));
        let original = app.editor.document.clone();
        let revision = app.editor.revision();
        let scale = b.width() / 960.0;
        let at = |x, y| b.min + vec2(x, y) * scale;
        let (start, end) = match kind {
            "creation" | "press" => {
                app.tool = Tool::Rectangle;
                (at(-60.0, 100.0), at(-20.0, 160.0))
            }
            "resize" => (at(200.0, 200.0), at(250.0, 240.0)),
            _ => (at(450.0, 450.0), at(50.0, 50.0)),
        };
        frame(&ctx, &mut app, vec![]);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(start), button(start, true)],
        );
        if kind != "press" {
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(start + (end - start) * 0.5)],
            );
            frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        }
        assert_eq!(app.editor.document, original, "{kind} is a preview only");
        assert_live_busy(&mut app);
        frame(&ctx, &mut app, key(Key::Escape, Modifiers::NONE));
        assert_live_busy(&mut app);
        let release = if kind == "press" { start } else { end };
        frame(&ctx, &mut app, vec![button(release, false)]);
        assert_eq!(
            app.editor.document, original,
            "cancelled {kind} changed document"
        );
        assert_eq!(
            app.editor.selection(),
            &[1],
            "cancelled {kind} changed selection"
        );
        assert_eq!(app.editor.revision(), revision);
        assert!(app.editor.can_redo());
        app.editor.redo();
        assert_eq!(app.editor.document.nodes.len(), 2);
    }
}

#[test]
fn released_v03_file_keeps_absolute_geometry_and_lowest_version() {
    let source = std::path::Path::new("tests/fixtures/released-v0.3.papercraft");
    let original = std::fs::read(source).unwrap();
    let doc = Document::load(source).unwrap();
    assert_eq!(doc.required_version(), 3);
    assert_eq!(doc.nodes[1].parent, Some(1));
    assert_eq!(doc.nodes[1].x, -110.0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("release.crumbs");
    doc.save(&path).unwrap();
    assert_eq!(Document::load(&path).unwrap(), doc);
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn clipped_world_bounds_selection_and_region_pixels_agree() {
    let mut doc = Document {
        nodes: vec![
            Node {
                id: 1,
                kind: NodeKind::Frame,
                x: -40.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
                clip: true,
                fill: [255, 255, 255, 255],
                ..Node::default()
            },
            Node {
                id: 2,
                parent: Some(1),
                x: -35.0,
                y: 5.0,
                width: 30.0,
                height: 10.0,
                fill: [255, 0, 0, 255],
                ..Node::default()
            },
        ],
        next_id: 3,
        ..Document::default()
    };
    doc.normalize();
    doc.validate().unwrap();
    assert_eq!(
        doc.visible_bounds(&doc.nodes[1]),
        Some([-35.0, 5.0, -20.0, 15.0])
    );
    assert_eq!(doc.hit_test([-25.0, 10.0]), Some(2));
    assert_eq!(
        doc.hit_test([-15.0, 10.0]),
        None,
        "clipped child cannot be selected"
    );
    let bounds = [-50.0, 0.0, 0.0, 20.0];
    let raster = papercraft::render::rasterize_region(&doc, bounds, 1.0).unwrap();
    let pixel = |x: usize, y: usize| &raster.pixels[(y * 50 + x) * 4..(y * 50 + x + 1) * 4];
    assert_eq!(pixel(25, 10), [255, 0, 0, 255]);
    assert_eq!(
        pixel(35, 10),
        [0, 0, 0, 0],
        "outside artboard and clipped frame is transparent"
    );
    for export in [doc.export_html(), doc.export_svg()] {
        assert!(export.unwrap_err().contains("ancestor clipping"));
    }
    doc.nodes[0].locked = true;
    assert_eq!(doc.hit_test([-25.0, 10.0]), None);
    let mut editor = papercraft::document::Editor::new(doc.clone());
    editor.select_all();
    assert!(editor.selection().is_empty());
    doc.nodes[0].hidden = true;
    assert!(doc.content_bounds().is_none());
    assert_eq!(doc.hit_test([-25.0, 10.0]), None);
}

#[test]
fn old_schemas_reject_explicit_new_defaults_and_unsupported_exports_preserve_destinations() {
    let dir = tempfile::tempdir().unwrap();
    for version in [1, 2] {
        for (key, value) in [
            ("line_height", Value::Null),
            ("text_align", json!("left")),
            ("parent", Value::Null),
            ("clip", json!(false)),
            ("stroke", Value::Null),
            ("stroke_width", json!(0)),
            ("points", json!([])),
            ("closed", json!(false)),
            ("shader", Value::Null),
        ] {
            let mut data = serde_json::to_value(Document::default()).unwrap();
            data["version"] = json!(version);
            data["next_id"] = json!(2);
            data["nodes"] = json!([Node::default()]);
            data["nodes"][0][key] = value;
            let path = dir.path().join("invalid.crumbs");
            let bytes = serde_json::to_vec(&data).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            assert!(Document::load(&path).is_err(), "v{version} accepted {key}");
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
    let mut doc = Document::default();
    doc.nodes.push(Node {
        shader: Some(papercraft::document::ShaderFill::default()),
        ..Node::default()
    });
    doc.next_id = 2;
    doc.normalize();
    let source = dir.path().join("shader.papercraft");
    doc.save(&source).unwrap();
    let destination = dir.path().join("keep.svg");
    for format in ["svg", "html"] {
        std::fs::write(&destination, b"keep").unwrap();
        assert!(
            papercraft::cli::export_file(&source, format, &destination, 1.0)
                .unwrap_err()
                .contains("shader")
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"keep");
    }
}

#[test]
fn mcp_protects_other_open_sources_and_rejects_curve_approximation() {
    for mut client in [
        Client::Headless(Server::default()),
        Client::Live(Box::new(PaperCraftApp::from_document(Document::default()))),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.papercraft");
        let second = dir.path().join("second.papercraft");
        Document::default().save(&first).unwrap();
        Document::default().save(&second).unwrap();
        let bytes = std::fs::read(&first).unwrap();
        let (_, _, _) = client.call("document_open", json!({"path":first}));
        let (_, two, _) = client.call("document_open", json!({"path":second}));
        let id = two["document_id"].clone();
        for (name, args) in [
            ("document_save", json!({"document_id":id,"path":first})),
            (
                "document_export",
                json!({"document_id":id,"format":"svg","path":first}),
            ),
        ] {
            assert!(client.call(name, args).0);
            assert_eq!(std::fs::read(&first).unwrap(), bytes);
        }
        let (_, before, _) = client.call("document_inspect", json!({"document_id":id}));
        for d in [
            "M 0 0 C 10 100 90 100 100 0",
            "M 0 0 Q 50 100 100 0",
            "m 0 0 l 10 10",
            "M 0 0 L 1 1 M 2 2 L 3 3",
            "M 0 0 L 1 1 Z L 2 2",
            "M 0 L 0 1 1",
        ] {
            let (error, message, _) = client.call(
                "document_apply",
                json!({"document_id":id,"operations":[{"op":"create_path","svg_path":d}]}),
            );
            assert!(error, "{d}: {message}");
            assert_eq!(
                client.call("document_inspect", json!({"document_id":id})).1,
                before
            );
        }
    }
}
