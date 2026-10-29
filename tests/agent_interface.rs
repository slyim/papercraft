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
