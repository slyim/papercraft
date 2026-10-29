use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Editor, Node, NodeKind},
    mcp::Server,
};
use serde_json::{Value, json};

fn scene() -> Document {
    Document {
        next_id: 4,
        nodes: vec![
            Node {
                id: 1,
                kind: NodeKind::Frame,
                x: 100.0,
                y: 100.0,
                width: 300.0,
                height: 250.0,
                ..Node::default()
            },
            Node {
                id: 2,
                parent: Some(1),
                x: 130.0,
                y: 140.0,
                locked: true,
                ..Node::default()
            },
            Node {
                id: 3,
                parent: Some(1),
                x: 240.0,
                y: 170.0,
                hidden: true,
                ..Node::default()
            },
        ],
        ..Document::default()
    }
}
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
fn keys(k: Key, m: Modifiers) -> Vec<Event> {
    vec![
        Event::ModifiersChanged(m),
        Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: m,
        },
        Event::Key {
            key: k,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: m,
        },
    ]
}
#[test]
fn gui_new_starts_with_an_editable_frame_not_a_fixed_page() {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(Document::default());
    run(&ctx, &mut app, vec![]);
    run(&ctx, &mut app, keys(Key::N, Modifiers::COMMAND));
    assert_eq!(app.editor.document.nodes.len(), 1);
    let n = &app.editor.document.nodes[0];
    assert_eq!(n.kind, NodeKind::Frame);
    assert!(!n.locked);
    assert_eq!(app.editor.document.background[3], 0);
}
#[test]
fn moving_a_frame_moves_hidden_and_locally_locked_children_once() {
    let mut e = Editor::new(scene());
    e.set_selection([1, 2]);
    let before = e.document.clone();
    e.nudge(-30.0, 20.0).unwrap();
    for (a, b) in before.nodes.iter().zip(&e.document.nodes) {
        assert_eq!((b.x, b.y), (a.x - 30.0, a.y + 20.0));
    }
    e.undo();
    assert_eq!(e.document, before);
    e.redo();
    assert_eq!(e.document.nodes[1].x, 100.0);
}
#[test]
fn duplicate_and_delete_are_whole_subtree_operations() {
    let mut e = Editor::new(scene());
    e.select(Some(1));
    e.duplicate().unwrap();
    assert_eq!(e.document.nodes.len(), 6);
    let copied = &e.document.nodes[3..];
    assert_eq!(copied[1].parent, Some(copied[0].id));
    assert_eq!(copied[1].x, 150.0);
    e.undo();
    assert_eq!(e.document, scene());
    e.select(Some(1));
    e.delete().unwrap();
    assert!(e.document.nodes.is_empty());
    e.undo();
    assert_eq!(e.document, scene());
}
fn call(s: &mut Server, id: u64, name: &str, args: Value) -> Value {
    let r=s.handle_line(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string()).unwrap();
    assert!(r.get("error").is_none(), "{r}");
    r
}
#[test]
fn mcp_frame_translation_and_resize_preserve_child_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.papercraft");
    scene().save(&path).unwrap();
    let mut server = Server::default();
    let r = call(&mut server, 1, "document_open", json!({"path":path}));
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    let state: Value = serde_json::from_str(text).unwrap();
    let id = state["document_id"].clone();
    let r = call(
        &mut server,
        2,
        "document_apply",
        json!({"document_id":id,"operations":[{"op":"update_layer","id":1,"x":-200,"y":20,"width":400}]}),
    );
    assert_ne!(r["result"]["isError"], true, "{r}");
    let r = call(
        &mut server,
        3,
        "document_inspect",
        json!({"document_id":id}),
    );
    let v: Value =
        serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(v["layers"][1]["x"], -170.0);
    assert_eq!(v["layers"][1]["y"], 60.0);
    assert_eq!(v["layers"][1]["width"], 100.0);
}
#[test]
fn pointer_frame_move_matches_keyboard_even_with_a_locked_child() {
    let ctx = egui::Context::default();
    PaperCraftApp::configure(&ctx);
    let mut app = PaperCraftApp::from_document(scene());
    let out = run(&ctx, &mut app, vec![]);
    let fill = egui::Color32::from_rgba_unmultiplied(248, 247, 243, 255);
    let board = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) if r.fill == fill => Some(r.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap();
    let scale = board.width() / 960.0;
    let a = board.min + vec2(110.0, 110.0) * scale;
    let b = a + vec2(50.0, 30.0) * scale;
    let button = |p, pressed| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::COMMAND,
    };
    for ev in [
        vec![
            Event::ModifiersChanged(Modifiers::COMMAND),
            Event::PointerMoved(a),
            button(a, true),
        ],
        vec![Event::PointerMoved(b)],
        vec![button(b, false)],
        vec![],
    ] {
        run(&ctx, &mut app, ev);
    }
    assert_eq!(app.editor.document.nodes[0].x, 150.0);
    assert_eq!(app.editor.document.nodes[1].x, 180.0);
    assert_eq!(app.editor.document.nodes[2].x, 290.0);
}
