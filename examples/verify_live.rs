//! Native-window proof of the real stdio -> IPC -> GUI -> render/save path.
use eframe::egui;
use papercraft::{app::PaperCraftApp, document::Document};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Check {
    app: PaperCraftApp,
    result: mpsc::Receiver<()>,
    output: PathBuf,
    started: Instant,
    finished: bool,
    screenshot: bool,
    fitted: bool,
    frames: usize,
    draft_ready: Option<mpsc::Sender<()>>,
    busy_result: mpsc::Receiver<()>,
    resume: Option<mpsc::Sender<()>>,
    cancel_pen: bool,
    resumed: bool,
}

impl eframe::App for Check {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.frames == 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let key = |key, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        match self.frames {
            8 => input.events.push(key(egui::Key::P, true)),
            9 => input.events.push(key(egui::Key::P, false)),
            10 | 11 => {
                let p = egui::pos2(500.0, 450.0);
                input.events.extend([
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: self.frames == 10,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            _ => {}
        }
        if self.cancel_pen {
            input
                .events
                .extend([key(egui::Key::Escape, true), key(egui::Key::Escape, false)]);
            self.cancel_pen = false;
            self.resumed = true;
        }
        if self.finished && !self.fitted {
            input
                .events
                .push(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
            for pressed in [true, false] {
                input.events.push(egui::Event::Key {
                    key: egui::Key::Num1,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                });
            }
            self.fitted = true;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
        self.frames += 1;
        if self.frames == 14 {
            assert_eq!(self.app.editor.revision(), 0);
            self.draft_ready.take().unwrap().send(()).unwrap();
        }
        if self.busy_result.try_recv().is_ok() {
            self.cancel_pen = true;
        }
        if self.resumed && self.resume.is_some() {
            assert_eq!(self.app.editor.revision(), 0);
            self.resume.take().unwrap().send(()).unwrap();
        }
        if !self.finished && self.result.try_recv().is_ok() {
            assert_eq!(self.app.editor.document.nodes.len(), 11);
            assert_eq!(self.app.editor.document.nodes[1].text, "Your workspace");
            assert_eq!(self.app.editor.document.nodes[6].text, "Project settings");
            assert!(self.app.editor.can_undo());
            assert_eq!(
                self.app.editor.document.hit_test([-335.0, 375.0]),
                Some(1),
                "rounded button's empty corner passes through to frame"
            );
            assert_eq!(self.app.editor.document.hit_test([-200.0, 418.0]), Some(4));
            self.finished = true;
        } else if self.finished && self.fitted && !self.screenshot {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.screenshot = true;
        }
        let image = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let pixels: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            image::save_buffer(
                &self.output,
                &pixels,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            println!(
                "Real stdio/live GUI/rollback/render/save/undo/redo verified: {}",
                self.output.display()
            );
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        assert!(
            self.started.elapsed() < Duration::from_secs(30),
            "Live native verification timed out"
        );
        ui.ctx().request_repaint_after(Duration::from_millis(10));
    }
}

fn client(
    binary: PathBuf,
    session: PathBuf,
    folder: PathBuf,
    send: mpsc::Sender<()>,
    ready: mpsc::Receiver<()>,
    busy: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
) {
    let mut child = Command::new(binary)
        .args(["mcp", "--live"])
        .arg(session)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut id = 0;
    let mut rpc = |method: &str, params: Value| {
        id += 1;
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["id"], id);
        value["result"].clone()
    };
    assert_eq!(
        rpc("initialize", json!({}))["serverInfo"]["name"],
        "papercraft"
    );
    let mut tool =
        |name: &str, args: Value| rpc("tools/call", json!({"name":name,"arguments":args}));
    let list = tool("document_list", json!({}));
    let docs: Value = serde_json::from_str(list["content"][0]["text"].as_str().unwrap()).unwrap();
    let doc = docs[0]["document_id"].clone();
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    let rejected = tool(
        "document_apply",
        json!({"document_id":doc,"operations":[{"op":"create_rectangle"}]}),
    );
    assert_eq!(rejected["isError"], true);
    assert!(rejected.to_string().contains("Editor busy"));
    busy.send(()).unwrap();
    resume.recv_timeout(Duration::from_secs(10)).unwrap();
    let applied = tool(
        "document_apply",
        json!({"document_id":doc,"expected_revision":0,"operations":[
            {"op":"create_token","ref":"ink","name":"ink","value":"#20392e"},
            {"op":"create_token","ref":"accent","name":"accent","value":"#ff6b2b"},
            {"op":"create_frame","ref":"workspace","name":"Workspace screen","x":-360,"y":70,"width":320,"height":400,"radius":0,"clip":false,"fill":"#f8f7f3"},
            {"op":"create_text","parent":{"ref":"workspace"},"x":-336,"y":102,"width":272,"height":60,"text":"Your workspace","font_size":28,"fill_token":{"ref":"ink"}},
            {"op":"create_text","parent":{"ref":"workspace"},"x":-336,"y":182,"width":272,"height":100,"text":"Two local screens, built through external live MCP. No toolbar simulation.","font_size":18,"fill_token":{"ref":"ink"}},
            {"op":"create_rectangle","parent":{"ref":"workspace"},"x":-336,"y":374,"width":272,"height":48,"radius":8,"fill_token":{"ref":"accent"}},
            {"op":"create_text","parent":{"ref":"workspace"},"x":-320,"y":384,"width":240,"height":28,"text":"Open project","font_size":20,"fill":"#fff","text_align":"center"},
            {"op":"create_frame","ref":"settings","name":"Settings screen","x":1000,"y":70,"width":320,"height":400,"radius":0,"clip":false,"fill":"#f8f7f3"},
            {"op":"create_text","parent":{"ref":"settings"},"x":1024,"y":102,"width":272,"height":60,"text":"Project settings","font_size":28,"fill_token":{"ref":"ink"}},
            {"op":"create_text","parent":{"ref":"settings"},"x":1024,"y":182,"width":272,"height":100,"text":"Local files\nShared editing history\nPortable document state","font_size":18,"line_height":28,"fill_token":{"ref":"ink"}},
            {"op":"create_rectangle","parent":{"ref":"settings"},"x":1024,"y":374,"width":272,"height":48,"radius":8,"fill_token":{"ref":"accent"}},
            {"op":"create_text","parent":{"ref":"settings"},"x":1040,"y":384,"width":240,"height":28,"text":"Save settings","font_size":20,"fill":"#fff","text_align":"center"},
            {"op":"create_path","parent":{"ref":"workspace"},"name":"Authored curve","x":-330,"y":290,"width":260,"height":52,"fill":"#00000000","stroke":"#7564ff","stroke_width":3,"anchors":[{"position":[0,0.5],"outgoing":[0.2,-0.4]},{"position":[1,0.5],"incoming":[0.8,1.4]}]}
        ]}),
    );
    assert_eq!(applied["isError"], false);
    for args in [
        json!({"document_id":doc,"expected_revision":0,"operations":[{"op":"create_rectangle"}]}),
        json!({"document_id":doc,"operations":[{"op":"create_rectangle"},{"op":"set_artboard","width":-1}]}),
    ] {
        assert_eq!(tool("document_apply", args)["isError"], true);
    }
    let inspected = tool("document_inspect", json!({"document_id":doc}));
    let inspected: Value =
        serde_json::from_str(inspected["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(inspected["state"]["revision"], 1);
    assert_eq!(
        inspected["capabilities"]["canvas_interactions"]["creation_snapping"],
        true
    );
    assert_eq!(
        inspected["capabilities"]["selection_rules"]["pointer_stroke_tolerance_screen_points"],
        5.0
    );
    assert_eq!(inspected["layers"].as_array().unwrap().len(), 11);
    assert_eq!(inspected["version"], 4);
    assert_eq!(
        tool(
            "document_apply",
            json!({"document_id":doc,"operations":[{"op":"update_layer","id":1,"x":-380,"y":50}]})
        )["isError"],
        false
    );
    let moved = tool("document_inspect", json!({"document_id":doc}));
    let moved: Value = serde_json::from_str(moved["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(moved["layers"][10]["x"], -350.0);
    assert_eq!(moved["layers"][10]["y"], 270.0);
    assert_eq!(
        tool("document_undo", json!({"document_id":doc}))["isError"],
        false
    );
    assert_eq!(
        inspected["content_bounds"],
        json!([-360.0, 70.0, 1320.0, 470.0])
    );
    assert_eq!(
        tool("document_render", json!({"document_id":doc,"frame_id":1}))["content"][0]["mimeType"],
        "image/png"
    );
    for (frame_id, filename) in [(1, "workspace.png"), (6, "settings.png")] {
        let path = folder.join(filename);
        assert_eq!(
            tool(
                "document_export",
                json!({"document_id":doc,"format":"png","frame_id":frame_id,"path":path})
            )["isError"],
            false
        );
        let image = image::open(path).unwrap();
        assert_eq!((image.width(), image.height()), (320, 400));
    }
    let saved = folder.join("live-native.papercraft");
    assert_eq!(
        tool("document_save", json!({"document_id":doc,"path":saved}))["isError"],
        false
    );
    let reopened = Document::load(&saved).unwrap();
    assert_eq!(reopened.nodes.len(), 11);
    assert_eq!(reopened.required_version(), 4);
    for (extension, text) in [
        ("svg", reopened.export_svg().unwrap()),
        ("html", reopened.export_html().unwrap()),
    ] {
        assert!(text.contains(" C "));
        papercraft::document::atomic_write(
            &folder.join(format!("live-native.{extension}")),
            text.as_bytes(),
        )
        .unwrap();
    }
    assert_eq!(reopened.hit_test([-335.0, 375.0]), Some(1));
    assert_eq!(reopened.hit_test([-200.0, 418.0]), Some(4));
    assert_eq!(
        reopened.marquee_hits([-336.0, 374.0, -334.0, 376.0]),
        vec![1]
    );
    for name in ["document_undo", "document_redo"] {
        assert_eq!(tool(name, json!({"document_id":doc}))["isError"], false);
    }
    drop(tool);
    drop(rpc);
    drop(input);
    assert!(child.wait().unwrap().success());
    send.send(()).unwrap();
}

fn main() -> eframe::Result {
    let folder = std::path::absolute(
        std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/verification/live")),
    )
    .unwrap();
    std::fs::create_dir_all(&folder).unwrap();
    let executable = std::env::current_exe().unwrap();
    let binary = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "papercraft.exe"
        } else {
            "papercraft"
        });
    assert!(
        binary.exists(),
        "Build the papercraft binary before this check"
    );
    eframe::run_native(
        "PaperCraft live verification",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1380.0, 860.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            PaperCraftApp::configure(&cc.egui_ctx);
            let mut app = PaperCraftApp::from_document(Document::default());
            app.start_live(&cc.egui_ctx, &folder.join("sessions"))
                .unwrap();
            let session = app.live_session_path().unwrap().to_owned();
            let (send, result) = mpsc::channel();
            let client_folder = folder.clone();
            let (draft_ready, ready) = mpsc::channel();
            let (busy, busy_result) = mpsc::channel();
            let (resume, resumed) = mpsc::channel();
            std::thread::spawn(move || {
                client(binary, session, client_folder, send, ready, busy, resumed)
            });
            Ok(Box::new(Check {
                app,
                result,
                output: folder.join("live-native.png"),
                started: Instant::now(),
                finished: false,
                screenshot: false,
                fitted: false,
                frames: 0,
                draft_ready: Some(draft_ready),
                busy_result,
                resume: Some(resume),
                cancel_pen: false,
                resumed: false,
            }))
        }),
    )
}
