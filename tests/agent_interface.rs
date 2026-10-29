//! CLI and MCP behaviour. MCP requests go through the same handler the stdio server
//! uses; the executable itself is exercised in the CLI tests.

use papercraft::document::Document;
use papercraft::mcp::Server;
use serde_json::{Value, json};
use std::process::Command;

fn papercraft() -> Command {
    Command::new(env!("CARGO_BIN_EXE_papercraft"))
}

fn call(server: &mut Server, tool: &str, arguments: Value) -> (bool, Value) {
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let response = server.handle_line(&request.to_string()).unwrap();
    let result = &response["result"];
    let text = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|c| c["type"] == "text")
        .map(|c| c["text"].as_str().unwrap().to_owned())
        .unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (result["isError"] == true, body)
}

fn new_document(server: &mut Server) -> String {
    let (error, state) = call(
        server,
        "document_new",
        json!({ "width": 200, "height": 100 }),
    );
    assert!(!error);
    state["document_id"].as_str().unwrap().to_owned()
}

#[test]
fn inspect_prints_deterministic_json_without_touching_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tokens.papercraft");
    std::fs::copy("tests/fixtures/tokens-v2.papercraft", &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let run = || papercraft().arg("inspect").arg(&path).output().unwrap();
    let (first, second) = (run(), run());
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    let document = Document::load(&path).unwrap();
    assert_eq!(value["format"], "papercraft");
    assert_eq!(value["version"], 2);
    assert_eq!(
        value["layers"].as_array().unwrap().len(),
        document.nodes.len()
    );
    assert_eq!(
        value["tokens"].as_array().unwrap().len(),
        document.tokens.len()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn export_matches_the_document_api_and_preserves_files_on_failure() {
    let directory = tempfile::tempdir().unwrap();
    let source = "tests/fixtures/svg-exchange.papercraft";
    let document = Document::load(std::path::Path::new(source)).unwrap();
    for (format, expected) in [
        ("svg", document.export_svg().unwrap()),
        ("html", document.export_html().unwrap()),
    ] {
        let output = directory.path().join(format!("out.{format}"));
        let status = papercraft()
            .args(["export", source, "--format", format, "--output"])
            .arg(&output)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&output).unwrap(), expected);
    }
    let png = directory.path().join("out.png");
    assert!(
        papercraft()
            .args([
                "export", source, "--format", "png", "--scale", "2", "--output"
            ])
            .arg(&png)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(&std::fs::read(&png).unwrap()[..8], b"\x89PNG\r\n\x1a\n");

    let existing = directory.path().join("keep.png");
    std::fs::write(&existing, b"keep").unwrap();
    let newer = directory.path().join("newer.papercraft");
    std::fs::write(&newer, br#"{"version": 99}"#).unwrap();
    let failure = papercraft()
        .arg("export")
        .arg(&newer)
        .args(["--format", "png", "--output"])
        .arg(&existing)
        .output()
        .unwrap();
    assert_eq!(failure.status.code(), Some(1));
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("newer PaperCraft"));
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
}

#[test]
fn usage_errors_exit_with_status_two() {
    for args in [
        vec!["inspect"],
        vec!["export", "a.papercraft", "--format", "gif", "--output", "x"],
        vec!["export", "a.papercraft", "--format", "png"],
        vec![
            "export",
            "a.papercraft",
            "--format",
            "svg",
            "--output",
            "x",
            "--scale",
            "2",
        ],
        vec!["export", "a.papercraft", "--frobnicate"],
        vec![
            "export",
            "a.papercraft",
            "--format",
            "svg",
            "--output",
            "x",
            "--scale",
            "1",
        ],
        vec!["mcp", "extra"],
    ] {
        let output = papercraft().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn mcp_batches_are_atomic_single_undo_steps() {
    let mut server = Server::default();
    let id = new_document(&mut server);
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
        "document_id": id, "expected_revision": 0, "operations": [
            { "op": "create_token", "name": "ink", "value": "#291D35" },
            { "op": "create_rectangle", "x": 10, "y": 10, "width": 50, "height": 20, "fill_token": 1 },
            { "op": "create_text", "x": 10, "y": 40, "width": 100, "height": 30, "text": "Merhaba" },
        ]}),
    );
    assert!(!error, "{state}");
    assert_eq!(state["revision"], 1);
    assert_eq!(state["created"].as_array().unwrap().len(), 3);

    let (error, message) = call(
        &mut server,
        "document_apply",
        json!({
        "document_id": id, "operations": [
            { "op": "create_rectangle", "x": 0, "y": 0, "width": 5, "height": 5 },
            { "op": "update_layer", "id": 99, "x": 1 },
        ]}),
    );
    assert!(error);
    assert!(message.as_str().unwrap().contains("Operation 1 failed"));
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert_eq!(inspected["state"]["revision"], 1);
    assert_eq!(inspected["layers"].as_array().unwrap().len(), 2);
    assert_eq!(inspected["layers"][0]["fill"], "#291d35ff");

    let (_, state) = call(&mut server, "document_undo", json!({ "document_id": id }));
    assert_eq!(state["revision"], 0);
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert!(inspected["layers"].as_array().unwrap().is_empty());
    assert!(inspected["tokens"].as_array().unwrap().is_empty());
}

#[test]
fn mcp_rejects_stale_revisions_unknown_fields_and_invalid_geometry() {
    let mut server = Server::default();
    let id = new_document(&mut server);
    let create = json!({ "document_id": id, "operations": [
        { "op": "create_rectangle", "x": 0, "y": 0, "width": 10, "height": 10 }] });
    assert!(!call(&mut server, "document_apply", create).0);
    for (operations, expected, revision) in [
        (
            json!([{ "op": "update_layer", "id": 1, "x": 5 }]),
            "Revision mismatch",
            Some(0),
        ),
        (
            json!([{ "op": "update_layer", "id": 1, "colour": "#000" }]),
            "Unknown field colour",
            None,
        ),
        (
            json!([{ "op": "update_layer", "id": 1, "width": 1e9 }]),
            "out-of-range",
            None,
        ),
        (
            json!([{ "op": "update_layer", "id": 1, "text": "x" }]),
            "rectangle",
            None,
        ),
        (
            json!([{ "op": "create_token", "name": "bad name!", "value": "#fff" }]),
            "",
            None,
        ),
        (json!([{ "op": "teleport" }]), "Unknown op", None),
    ] {
        let mut arguments = json!({ "document_id": id, "operations": operations });
        if let Some(revision) = revision {
            arguments["expected_revision"] = json!(revision);
        }
        let (error, message) = call(&mut server, "document_apply", arguments);
        assert!(error, "{operations}");
        assert!(message.as_str().unwrap().contains(expected), "{message}");
    }
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert_eq!(inspected["state"]["revision"], 1);
}

#[test]
fn mcp_save_state_render_and_close() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("card.papercraft");
    let mut server = Server::default();
    let id = new_document(&mut server);
    let (error, message) = call(&mut server, "document_save", json!({ "document_id": id }));
    assert!(error && message.as_str().unwrap().contains("pass path"));
    let (error, state) = call(
        &mut server,
        "document_save",
        json!({ "document_id": id, "path": path.to_str().unwrap() }),
    );
    assert!(!error);
    assert_eq!(state["dirty"], false);
    assert_eq!(Document::load(&path).unwrap().width, 200.0);

    let edit = json!({ "document_id": id, "operations": [{ "op": "set_artboard", "background": "#f6f6fe" }] });
    let (_, state) = call(&mut server, "document_apply", edit);
    assert_eq!(state["dirty"], true);
    let (error, _) = call(&mut server, "document_close", json!({ "document_id": id }));
    assert!(error, "unsaved changes block close");
    let (_, state) = call(&mut server, "document_undo", json!({ "document_id": id }));
    assert_eq!(state["dirty"], false, "undo back to the saved revision");

    let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": "document_render", "arguments": { "document_id": id, "scale": 2 } } });
    let response = server.handle_line(&request.to_string()).unwrap();
    let content = response["result"]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["mimeType"], "image/png");
    assert!(
        content[0]["data"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );

    let (_, reopened) = call(
        &mut server,
        "document_open",
        json!({ "path": path.to_str().unwrap() }),
    );
    assert_eq!(
        reopened["document_id"],
        id.as_str(),
        "an open file is not opened twice"
    );
    assert!(!call(&mut server, "document_close", json!({ "document_id": id })).0);
}

#[test]
fn mcp_protocol_handshake_and_notifications() {
    let mut server = Server::default();
    let init = server
        .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "papercraft");
    assert!(
        server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none()
    );
    let tools = server
        .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
        .unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 11);
    let unknown = server
        .handle_line(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#)
        .unwrap();
    assert_eq!(unknown["error"]["code"], -32601);
    let garbage = server.handle_line("{not json").unwrap();
    assert_eq!(garbage["error"]["code"], -32700);
}

#[test]
fn rejected_batches_preserve_the_document_and_redo_history() {
    let mut server = Server::default();
    let id = new_document(&mut server);
    let operations = json!([{ "op": "create_rectangle", "width": 10, "height": 10 }]);
    assert!(
        !call(
            &mut server,
            "document_apply",
            json!({
                "document_id": id, "operations": operations,
            })
        )
        .0
    );
    call(&mut server, "document_undo", json!({ "document_id": id }));
    let (_, before) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert_eq!(before["state"]["can_redo"], true);
    for failing in [
        json!({ "op": "update_layer", "id": 999, "x": 1 }),
        json!({ "op": "set_artboard", "width": -1 }),
    ] {
        assert!(
            call(
                &mut server,
                "document_apply",
                json!({
                    "document_id": id,
                    "operations": [{ "op": "create_rectangle" }, failing],
                })
            )
            .0
        );
        let (_, after) = call(
            &mut server,
            "document_inspect",
            json!({ "document_id": id }),
        );
        assert_eq!(after, before);
    }
    let (_, restored) = call(&mut server, "document_redo", json!({ "document_id": id }));
    assert_eq!(restored["revision"], 1);
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert_eq!(inspected["layers"].as_array().unwrap().len(), 1);
    assert_eq!(inspected["layers"][0]["width"].as_f64(), Some(10.0));
}

#[test]
fn mcp_typography_fields_and_upgrade_report() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.papercraft");
    std::fs::copy("tests/fixtures/svg-exchange.papercraft", &path).unwrap();
    let mut server = Server::default();
    let (_, state) = call(
        &mut server,
        "document_open",
        json!({ "path": path.to_str().unwrap() }),
    );
    let id = state["document_id"].as_str().unwrap().to_owned();
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    let text = inspected["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "text")
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let rectangle = inspected["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "rectangle")
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let (error, message) = call(
        &mut server,
        "document_apply",
        json!({ "document_id": id,
        "operations": [{ "op": "update_layer", "id": rectangle, "text_align": "center" }] }),
    );
    assert!(error && message.as_str().unwrap().contains("rectangle"));
    let (error, message) = call(
        &mut server,
        "document_apply",
        json!({ "document_id": id,
        "operations": [{ "op": "update_layer", "id": text, "text_align": "justify" }] }),
    );
    assert!(error && message.as_str().unwrap().contains("left, center or right"));
    let (error, _) = call(
        &mut server,
        "document_apply",
        json!({ "document_id": id,
        "operations": [{ "op": "update_layer", "id": text, "line_height": 0 }] }),
    );
    assert!(error, "line height out of range");

    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({ "document_id": id,
        "operations": [{ "op": "update_layer", "id": text, "line_height": 36, "text_align": "right" }] }),
    );
    assert!(!error, "{state}");
    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    let layer = inspected["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == text)
        .unwrap()
        .clone();
    assert_eq!(layer["line_height"], 36.0);
    assert_eq!(layer["text_align"], "right");
    assert_eq!(inspected["version"], 3);

    let (error, state) = call(&mut server, "document_save", json!({ "document_id": id }));
    assert!(!error);
    assert_eq!(state["version"], 3);
    assert!(state["upgraded_from"].as_u64().unwrap() < 3, "{state}");
    assert_eq!(Document::load(&path).unwrap().required_version(), 3);

    let (_, state) = call(
        &mut server,
        "document_apply",
        json!({ "document_id": id,
        "operations": [{ "op": "update_layer", "id": text, "line_height": null, "text_align": "left" }] }),
    );
    assert_eq!(state["dirty"], true);
    let (_, state) = call(&mut server, "document_save", json!({ "document_id": id }));
    assert_eq!(
        state["version"], 1,
        "removing the fields returns the fixture to its original version 1"
    );
    assert!(state.get("upgraded_from").is_none());
}

#[test]
fn mcp_token_names_shapes_fit_text_and_arrangement() {
    let mut server = Server::default();
    let id = new_document(&mut server);

    // 1. Bind tokens by name in the same batch as create_token, and create shapes + text.
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [
                { "op": "create_token", "name": "paper", "value": "#f8f7f3" },
                { "op": "create_token", "name": "brand", "value": "#d94f30" },
                { "op": "set_artboard", "background_token": "paper" },
                { "op": "create_ellipse", "name": "Badge", "x": 10, "y": 15, "width": 20, "height": 20, "fill_token": "brand" },
                { "op": "create_triangle", "name": "Peak", "x": 40, "y": 30, "width": 20, "height": 20, "fill_token": "brand" },
                { "op": "create_shape", "name": "Card", "shape": "rectangle", "x": 130, "y": 5, "width": 20, "height": 20, "radius": 6 },
                { "op": "create_text", "name": "Copy", "x": 10, "y": 60, "width": 80, "height": 10, "font_size": 24, "text": "Multi line wrapped text block" }
            ]
        }),
    );
    assert!(!error, "{state}");

    // Radius is rejected on non-rectangle shapes.
    let (error, message) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [{ "op": "create_ellipse", "radius": 4 }]
        }),
    );
    assert!(error && message.as_str().unwrap().contains("radius"));

    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert_eq!(inspected["artboard"]["background_token_name"], "paper");
    let layers = inspected["layers"].as_array().unwrap();
    assert_eq!(layers[0]["kind"], "ellipse");
    assert_eq!(layers[0]["fill_token_name"], "brand");
    assert_eq!(layers[1]["kind"], "triangle");
    assert_eq!(layers[3]["kind"], "text");
    assert_eq!(layers[3]["clipped"], true);
    assert!(layers[3]["measured_height"].as_f64().unwrap() > 10.0);

    // 2. Swap shape, auto-fit text height, align & distribute shapes, duplicate, and select.
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [
                { "op": "update_layer", "id": 3, "shape": "ellipse" },
                { "op": "update_layer", "id": 4, "fit_text": true },
                { "op": "align_layers", "ids": [1, 2, 3], "align": "top" },
                { "op": "distribute_layers", "ids": [1, 2, 3], "axis": "horizontal" },
                { "op": "duplicate_layers", "ids": [1], "offset_x": 15, "offset_y": 25 },
                { "op": "update_token", "id": "brand", "value": "#1e66f5" },
                { "op": "delete_token", "name": "paper" }
            ]
        }),
    );
    assert!(!error, "{state}");
    assert_eq!(state["selected_ids"], json!([5]));

    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert!(inspected["artboard"]["background_token"].is_null());
    assert_eq!(inspected["artboard"]["background"], "#f8f7f3ff");
    let layers = inspected["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 5);
    assert_eq!(layers[2]["kind"], "ellipse");
    // Align top -> y == 5.0 for ids 1, 2, 3; distribute horizontal -> x == 10, 70, 130.
    assert_eq!(layers[0]["y"], 5.0);
    assert_eq!(layers[1]["y"], 5.0);
    assert_eq!(layers[2]["y"], 5.0);
    assert_eq!(layers[0]["x"], 10.0);
    assert_eq!(layers[1]["x"], 70.0);
    assert_eq!(layers[2]["x"], 130.0);
    // Text layer 4 is now auto-sized and not clipped.
    assert_eq!(layers[3]["clipped"], false);
    assert!(
        layers[3]["height"].as_f64().unwrap() >= layers[3]["measured_height"].as_f64().unwrap()
    );
    // Duplicated layer 5 is offset by (15, 25) from layer 1 (10, 5) -> (25, 30) and bound to updated brand token.
    assert_eq!(layers[4]["id"], 5);
    assert_eq!(layers[4]["x"], 25.0);
    assert_eq!(layers[4]["y"], 30.0);
    assert_eq!(layers[4]["fill"], "#1e66f5ff");

    // 3. Explicit select_layers and clearing selection.
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [{ "op": "select_layers", "ids": [1, 3] }]
        }),
    );
    assert!(!error);
    assert_eq!(state["selected_ids"], json!([1, 3]));

    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [{ "op": "select_layers", "ids": [] }]
        }),
    );
    assert!(!error);
    assert_eq!(state["selected_ids"], json!([]));
}

#[test]
fn mcp_frames_paths_strokes_shaders_and_directives() {
    let mut server = Server::default();
    let id = new_document(&mut server);

    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [
                { "op": "create_token", "name": "accent", "value": "#ff6b2b" },
                {
                    "op": "create_rectangle",
                    "name": "CardBg",
                    "x": 20,
                    "y": 15,
                    "width": 80,
                    "height": 50,
                    "radius": 8,
                    "fill": "#1e293b",
                    "shader": {
                        "kind": "mesh",
                        "secondary": "#38bdf8",
                        "angle": 45.0,
                        "intensity": 0.75
                    }
                },
                {
                    "op": "create_ellipse",
                    "name": "Orb",
                    "x": 35,
                    "y": 25,
                    "width": 24,
                    "height": 24,
                    "fill": "#f8fafc",
                    "stroke": "#94a3b8",
                    "stroke_width": 2.0
                },
                { "op": "wrap_in_frame", "ids": [1, 2], "name": "Hero Card" },
                {
                    "op": "create_path",
                    "name": "Wave",
                    "parent": 3,
                    "x": 25,
                    "y": 30,
                    "width": 60,
                    "height": 30,
                    "points": [[0.0, 0.8], [0.5, 0.2], [1.0, 0.8]],
                    "stroke_token": "accent",
                    "stroke_width": 3.0
                },
                {
                    "op": "create_path",
                    "name": "Badge",
                    "x": 120,
                    "y": 20,
                    "width": 50,
                    "height": 50,
                    "svg_path": "M 0 0 L 50 0 L 25 50 Z",
                    "fill": "#22c55e80",
                    "stroke_token": "accent"
                },
                {
                    "op": "create_directive",
                    "layer_id": 3,
                    "message": "Polish hero card contrast"
                }
            ]
        }),
    );
    assert!(!error, "{state}");
    assert_eq!(state["selected_ids"], json!([3]));

    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    let layers = inspected["layers"].as_array().unwrap();
    let find_layer = |layer_id: u64| {
        layers
            .iter()
            .find(|l| l["id"] == layer_id)
            .expect("layer exists")
    };

    let frame = find_layer(3);
    assert_eq!(frame["kind"], "frame");
    assert_eq!(frame["name"], "Hero Card");
    assert_eq!(frame["clip"], true);

    let card = find_layer(1);
    assert_eq!(card["parent"], 3);
    assert_eq!(card["shader"]["kind"], "mesh");
    assert_eq!(card["shader"]["secondary"], "#38bdf8ff");
    assert_eq!(card["shader"]["angle"], 45.0);
    assert_eq!(card["shader"]["intensity"], 0.75);

    let orb = find_layer(2);
    assert_eq!(orb["parent"], 3);
    assert_eq!(orb["stroke"], "#94a3b8ff");
    assert_eq!(orb["stroke_width"], 2.0);

    let wave = find_layer(4);
    assert_eq!(wave["kind"], "path");
    assert_eq!(wave["parent"], 3);
    assert_eq!(wave["closed"], false);
    assert_eq!(wave["points"].as_array().unwrap().len(), 3);
    assert_eq!(wave["stroke_token_name"], "accent");
    assert_eq!(wave["stroke"], "#ff6b2bff");
    assert_eq!(wave["stroke_width"], 3.0);

    let badge = find_layer(5);
    assert_eq!(badge["kind"], "path");
    assert_eq!(badge["closed"], true);
    assert_eq!(badge["points"].as_array().unwrap().len(), 3);
    assert_eq!(badge["stroke_token_name"], "accent");

    let directives = inspected["directives"].as_array().unwrap();
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0]["id"], 1);
    assert_eq!(directives[0]["layer_id"], 3);
    assert_eq!(directives[0]["message"], "Polish hero card contrast");
    assert_eq!(directives[0]["resolved"], false);

    // Attach a linear_gradient shader to the Frame and resolve the directive.
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [
                {
                    "op": "update_layer",
                    "id": 3,
                    "clip": false,
                    "shader": {
                        "kind": "linear_gradient",
                        "secondary": "#e2e8f0",
                        "angle": 120.0,
                        "intensity": 0.6
                    }
                },
                { "op": "resolve_directive", "id": 1, "resolved": true }
            ]
        }),
    );
    assert!(!error, "{state}");

    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    let frame = inspected["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == 3)
        .unwrap();
    assert_eq!(frame["clip"], false);
    assert_eq!(frame["shader"]["kind"], "linear_gradient");
    assert_eq!(inspected["directives"][0]["resolved"], true);

    // Render the entire composition to PNG.
    let request = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": {
            "name": "document_render",
            "arguments": { "document_id": id, "scale": 1 }
        }
    });
    let response = server.handle_line(&request.to_string()).unwrap();
    let content = response["result"]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["mimeType"], "image/png");
    assert!(
        content[0]["data"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );

    // Delete the directive and clear the card shader.
    let (error, state) = call(
        &mut server,
        "document_apply",
        json!({
            "document_id": id,
            "operations": [
                { "op": "delete_directive", "id": 1 },
                { "op": "update_layer", "id": 1, "shader": null }
            ]
        }),
    );
    assert!(!error, "{state}");

    let (_, inspected) = call(
        &mut server,
        "document_inspect",
        json!({ "document_id": id }),
    );
    assert!(inspected["directives"].as_array().unwrap().is_empty());
    let card = inspected["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == 1)
        .unwrap();
    assert!(card.get("shader").is_none());
}
