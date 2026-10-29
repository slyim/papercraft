use papercraft::{
    app::PaperCraftApp,
    document::{Document, NodeKind},
};
use serde_json::{Value, json};

fn call(app: &mut PaperCraftApp, name: &str, args: Value) -> (bool, Value) {
    let result = app.handle_live_request(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string()).unwrap();
    let result = &result["result"];
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
    )
}

#[test]
fn manual_and_mcp_edits_share_history_and_tabs() {
    let mut app = PaperCraftApp::from_document(Document::default());
    let (_, list) = call(&mut app, "document_list", json!({}));
    let id = list[0]["document_id"].clone();
    app.editor.add(NodeKind::Rectangle).unwrap();
    let (_, inspected) = call(&mut app, "document_inspect", json!({"document_id":id}));
    assert_eq!(inspected["layers"].as_array().unwrap().len(), 1);
    let manual = app.editor.document.clone();
    assert!(!call(&mut app,"document_apply",json!({"document_id":id,"expected_revision":1,"operations":[{"op":"create_text","text":"Live"}]})).0);
    assert_eq!(app.editor.document.nodes.len(), 2);
    app.editor.undo();
    assert_eq!(app.editor.document, manual);
    call(&mut app, "document_undo", json!({"document_id":id}));
    assert!(app.editor.document.nodes.is_empty());
    let (_, before) = call(&mut app, "document_inspect", json!({"document_id":id}));
    assert!(call(&mut app,"document_apply",json!({"document_id":id,"operations":[{"op":"create_rectangle"},{"op":"set_artboard","width":-1}]})).0);
    assert_eq!(
        call(&mut app, "document_inspect", json!({"document_id":id})).1,
        before
    );
    call(&mut app, "document_redo", json!({"document_id":id}));
    assert_eq!(app.editor.document, manual);
    call(&mut app, "document_new", json!({"name":"Second"}));
    assert_eq!(app.tab_count(), 2);
    assert!(
        !call(
            &mut app,
            "document_apply",
            json!({"document_id":id,"operations":[{"op":"set_artboard","name":"First updated"}]})
        )
        .0
    );
    assert_eq!(app.active_tab(), 0);
    assert_eq!(app.editor.document.name, "First updated");
    app.activate_tab(1);
    assert_eq!(app.editor.document.name, "Second");
}

#[test]
fn live_saves_protect_other_tabs_and_reject_silent_upgrades() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.papercraft");
    let second = directory.path().join("second.papercraft");
    Document::default().save(&first).unwrap();
    Document::default().save(&second).unwrap();
    let bytes = std::fs::read(&first).unwrap();
    let mut app = PaperCraftApp::from_document(Document::default());
    let (_, one) = call(&mut app, "document_open", json!({"path":first}));
    let (_, two) = call(&mut app, "document_open", json!({"path":second}));
    assert!(
        call(
            &mut app,
            "document_save",
            json!({"document_id":two["document_id"],"path":first})
        )
        .0
    );
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let id = one["document_id"].clone();
    assert!(
        !call(
            &mut app,
            "document_apply",
            json!({"document_id":id,"operations":[{"op":"create_text","line_height":40}]})
        )
        .0
    );
    assert!(call(&mut app, "document_save", json!({"document_id":id})).0);
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let saved = directory.path().join("new-v3.papercraft");
    assert!(
        !call(
            &mut app,
            "document_save",
            json!({"document_id":id,"path":saved})
        )
        .0
    );
    assert_eq!(Document::load(&saved).unwrap().required_version(), 3);
}

#[test]
fn live_mode_never_silently_falls_back_to_headless() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_papercraft"))
        .args(["mcp", "--live"])
        .env("PAPERCRAFT_LIVE_DIR", dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No live editor"));
    let config = dir.path().join("external.json");
    std::fs::write(&config, br#"{"address":"192.0.2.1:1","token":"test"}"#).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_papercraft"))
        .args(["mcp", "--live"])
        .arg(config)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid live session"));
}

#[test]
fn discovery_selects_one_editor_and_requires_a_choice_for_several() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = eframe::egui::Context::default();
    let first = papercraft::live::Host::start(ctx.clone(), dir.path()).unwrap();
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_papercraft"))
            .args(["mcp", "--live"])
            .env("PAPERCRAFT_LIVE_DIR", dir.path())
            .output()
            .unwrap()
    };
    let one = run();
    assert!(
        one.status.success(),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    assert!(one.stdout.is_empty());
    let second = papercraft::live::Host::start(ctx, dir.path()).unwrap();
    let multiple = run();
    assert_eq!(multiple.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&multiple.stderr).contains("Several editors"));
    drop(second);
    assert!(run().status.success());
    drop(first);
    assert_eq!(run().status.code(), Some(1));
}

#[test]
fn stdio_proxy_reaches_the_gui_event_loop() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    let ctx = eframe::egui::Context::default();
    let mut app = PaperCraftApp::from_document(Document::default());
    app.start_live(&ctx, directory.path()).unwrap();
    let path = app.live_session_path().unwrap().to_owned();
    let child = Command::new(env!("CARGO_BIN_EXE_papercraft"))
        .args(["mcp", "--live"])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let (send, receive) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut child = child;
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut rpc = |request: Value| {
            writeln!(input, "{request}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        };
        let init = rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}));
        assert_eq!(init["result"]["serverInfo"]["name"], "papercraft");
        let list = rpc(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"document_list","arguments":{}}}),
        );
        let docs: Value =
            serde_json::from_str(list["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let edit = rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"document_apply","arguments":{"document_id":docs[0]["document_id"],"operations":[{"op":"create_text","text":"Through live stdio"}]}}}),
        );
        assert_eq!(edit["result"]["isError"], false);
        drop(rpc);
        drop(input);
        assert!(child.wait().unwrap().success());
        send.send(()).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    while receive.try_recv().is_err() {
        assert!(
            Instant::now() < deadline,
            "Live stdio client did not finish"
        );
        app.poll_live(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    client.join().unwrap();
    assert_eq!(app.editor.document.nodes[0].text, "Through live stdio");
    drop(app);
    assert!(!path.exists(), "Closing the host removes its session file");
}

#[test]
fn default_mcp_command_connects_to_open_editor_and_headless_isolates() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    let ctx = eframe::egui::Context::default();
    let mut app = PaperCraftApp::from_document(Document::default());
    app.start_live(&ctx, directory.path()).unwrap();

    // Bare `papercraft mcp` discovers the live session automatically.
    let child = Command::new(env!("CARGO_BIN_EXE_papercraft"))
        .arg("mcp")
        .env("PAPERCRAFT_LIVE_DIR", directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let (send, receive) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut child = child;
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut rpc = |request: Value| {
            writeln!(input, "{request}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        };
        let list = rpc(
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"document_list","arguments":{}}}),
        );
        let docs: Value =
            serde_json::from_str(list["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(docs.as_array().unwrap().len(), 1);
        let edit = rpc(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"document_apply","arguments":{"document_id":docs[0]["document_id"],"operations":[{"op":"create_text","text":"Default mcp reaches live editor"}]}}}),
        );
        assert_eq!(edit["result"]["isError"], false);
        drop(rpc);
        drop(input);
        assert!(child.wait().unwrap().success());
        send.send(()).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    while receive.try_recv().is_err() {
        assert!(Instant::now() < deadline, "Default mcp client timed out");
        app.poll_live(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    client.join().unwrap();
    assert_eq!(
        app.editor.document.nodes[0].text,
        "Default mcp reaches live editor"
    );

    // `papercraft mcp --headless` runs an isolated session with zero open documents.
    let mut headless = Command::new(env!("CARGO_BIN_EXE_papercraft"))
        .args(["mcp", "--headless"])
        .env("PAPERCRAFT_LIVE_DIR", directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    {
        let mut input = headless.stdin.take().unwrap();
        let mut output = BufReader::new(headless.stdout.take().unwrap());
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"document_list","arguments":{}}})
        )
        .unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        let docs: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert!(docs.as_array().unwrap().is_empty());
    }
    assert!(headless.wait().unwrap().success());
}

#[test]
fn start_default_succeeds_and_cleans_up_session() {
    let ctx = eframe::egui::Context::default();
    let host = papercraft::live::Host::start_default(ctx).unwrap();
    let path = host.path().to_owned();
    assert!(path.exists());
    drop(host);
    assert!(!path.exists());
}
