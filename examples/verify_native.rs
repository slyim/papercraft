use eframe::egui;
use papercraft::{
    app::{PaperCraftApp, Tool, View},
    document::{
        Align, Axis, ColorTarget, Document, Editor, MAX_COORDINATE, NEW_TEXT, Node, NodeKind,
        atomic_write,
    },
    recents::Recents,
};
use std::path::PathBuf;

const STARTUP_FRAMES: usize = 8;
const TEXT: &str = "Launch notes, draft two";
const TOKEN_COLOR: [u8; 4] = [176, 72, 44, 255];
const SCENARIOS: &[&str] = &[
    "editor",
    "selected",
    "tool",
    "exchange",
    "export",
    "quit",
    "dashboard",
    "list",
    "empty",
    "theme",
    "bound",
    "arranged",
    "resize",
    "snapping",
    "text",
    "marquee",
    "long-text",
    "text-token",
    "text-tabs",
    "typography",
    "offboard",
    "cancel",
    "selection-1b",
    "marquee-1b",
    "creation-1b",
    "navigation-1b",
    "starter-frame",
    "pen-v4",
    "pen-cancel",
];

struct NativeCheck {
    app: PaperCraftApp,
    output: PathBuf,
    frames: usize,
    started: std::time::Instant,
    hover: Option<egui::Pos2>,
    scenario: String,
    initial_document: Document,
    initial_revision: u64,
    placed_text: Option<Node>,
    placed_revision: u64,
    script_frame: usize,
    script_started: bool,
    screenshot_requested: bool,
    finished: bool,
    /// Input events sent on each frame, by frame index.
    script: Vec<Vec<egui::Event>>,
}

impl eframe::App for NativeCheck {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::logic(&mut self.app, ctx, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Ask the native window system for focus; never fabricate a focused input
        // flag or focus the app's private text widget from the harness.
        if self.frames == 0 || (self.frames == 4 && !raw_input.focused) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        for event in &raw_input.events {
            if matches!(
                event,
                egui::Event::WindowFocused(_)
                    | egui::Event::Key {
                        physical_key: Some(_),
                        ..
                    }
            ) {
                eprintln!(
                    "Native focus/unscripted key at frame {}: {event:?}",
                    self.script_frame
                );
            }
        }
        if self.frames < STARTUP_FRAMES {
            return;
        }
        if !self.script_started {
            if !raw_input.focused {
                return;
            }
            self.script_started = true;
        }
        if self.scenario != "quit" && self.script_frame < self.script.len() {
            assert!(
                raw_input.focused,
                "Native window lost focus during scripted input"
            );
        }
        raw_input
            .events
            .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        if let Some(events) = self.script.get(self.script_frame) {
            raw_input.events.extend(events.iter().cloned());
        } else if let Some(pos) = self.hover {
            raw_input.events.push(egui::Event::PointerMoved(pos));
        }
        // Model token edits are setup stimuli, not substitutes for placing or
        // typing text. Give the real canvas a frame to reconcile each change.
        if self.scenario == "text-token" {
            let token = self.initial_document.tokens[0].id;
            match self.script_frame {
                2 => self.app.editor.set_token_value(token, TOKEN_COLOR).unwrap(),
                3 => self.app.editor.delete_token(token).unwrap(),
                4 => {
                    let replacement = self.app.editor.create_token([10, 20, 200, 255]).unwrap();
                    assert_eq!(replacement, token, "Fixture must exercise token ID reuse");
                }
                _ => {}
            }
        }
        if self.scenario == "export" && (self.script_frame == 2 || self.script_frame == 3) {
            let pos = egui::pos2(ctx.content_rect().right() - 42.0, 65.0);
            raw_input.events.push(egui::Event::PointerMoved(pos));
            raw_input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: self.script_frame == 2,
                modifiers: egui::Modifiers::NONE,
            });
        }
        self.script_frame += 1;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.finished {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.app.show(ui);
        self.frames += 1;
        if (self.scenario == "selection-1b" && self.script_frame == 2)
            || (self.scenario == "marquee-1b" && self.script_frame == 3)
        {
            assert!(
                self.app.editor.selection().is_empty(),
                "Empty shape corner/clipped segment must not select a bounding box"
            );
        }
        // The click's release frame cannot focus a newly created TextEdit.
        // Inspect the placeholder only after two subsequent idle input frames.
        if self.script_frame == 10 && is_text_scenario(&self.scenario) {
            let node = self
                .app
                .editor
                .selected_node()
                .expect("Text tool did not place a layer");
            assert_eq!(node.kind, NodeKind::Text);
            assert_eq!(
                node.text, NEW_TEXT,
                "Text placement must select the placeholder"
            );
            assert_eq!(
                self.app.editor.document.nodes.len(),
                self.initial_document.nodes.len() + 1
            );
            assert!(
                ui.ctx().egui_wants_keyboard_input(),
                "Placed text did not gain focus"
            );
            self.placed_text = Some(node.clone());
            self.placed_revision = self.app.editor.revision();
        }
        if self.scenario == "quit" && self.script_frame == 2 {
            let before = self.app.editor.document.clone();
            self.app.editor.document.name = "Unsaved native quit check".into();
            self.app.editor.commit(before).unwrap();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            #[cfg(target_os = "macos")]
            assert!(craft_quit::on_menu_event(
                ui.ctx(),
                craft_quit::QUIT_MENU_ID
            ));
            #[cfg(not(target_os = "macos"))]
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Leave several complete paints after the last release/typing event.
        // In particular, resize/marquee releases must precede screenshot capture.
        if !self.screenshot_requested && self.script_frame == self.script.len() + 4 {
            self.verify_scenario(ui.ctx());
            self.screenshot_requested = true;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let screenshot = ui.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(screenshot) = screenshot {
            assert!(self.screenshot_requested, "Unrequested screenshot");
            let rgba: Vec<_> = screenshot
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
            image::save_buffer(
                &self.output,
                &rgba,
                screenshot.width() as u32,
                screenshot.height() as u32,
                image::ColorType::Rgba8,
            )
            .expect("Cannot save verification screenshot");
            assert!(
                screenshot.pixels.iter().any(|p| p.r() > 100),
                "Screenshot contains no visible light content"
            );
            println!("Native rendering verified: {}", self.output.display());
            if self.scenario == "quit" {
                assert_eq!(self.app.editor.document.name, "Unsaved native quit check");
                self.app.editor.undo(); // Restore the saved state before ending this check.
                println!("Unsaved document survived simulated native Quit/minimize dispatch.");
            }
            // Scripted edits are discarded so that closing does not prompt.
            for index in 0..self.app.tab_count() {
                self.app.activate_tab(index);
                while self.app.editor.can_undo() {
                    self.app.editor.undo();
                }
            }
            self.finished = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        assert!(
            self.started.elapsed() < std::time::Duration::from_secs(10),
            "Native scenario timed out: native_focused={}, script_started={}, script_frame={}, screenshot_requested={}",
            ui.input(|i| i.focused),
            self.script_started,
            self.script_frame,
            self.screenshot_requested
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

impl NativeCheck {
    fn save_1b_artifacts(&self) {
        let folder = self.output.parent().unwrap();
        let doc = &self.app.editor.document;
        let path = folder.join("interaction.papercraft");
        doc.save(&path).unwrap();
        assert_eq!(Document::load(&path).unwrap(), *doc);
        atomic_write(
            &folder.join("interaction.png"),
            &papercraft::render::export_png_region(doc, [-80.0, 0.0, 700.0, 500.0], 1.0).unwrap(),
        )
        .unwrap();
        for (format, result) in [("html", doc.export_html()), ("svg", doc.export_svg())] {
            match result {
                Ok(text) => atomic_write(
                    &folder.join(format!("interaction.{format}")),
                    text.as_bytes(),
                )
                .unwrap(),
                Err(message) => {
                    assert!(message.contains("ancestor clipping"));
                    println!("Expected {format} rejection: {message}");
                }
            }
        }
    }

    fn verify_scenario(&mut self, ctx: &egui::Context) {
        match self.scenario.as_str() {
            "starter-frame" => {
                let doc = self.app.editor.document.clone();
                for (before, after) in self.initial_document.nodes.iter().zip(&doc.nodes) {
                    assert!(
                        (after.x - before.x - 40.0).abs() < 0.05
                            && (after.y - before.y - 20.0).abs() < 0.05
                    );
                    assert_eq!([after.width, after.height], [before.width, before.height]);
                }
                assert_eq!(
                    self.app.editor.selected_node().unwrap().name,
                    "A place to begin"
                );
                assert_eq!(
                    self.app.editor.selected_node().unwrap().kind,
                    NodeKind::Frame
                );
                self.save_1b_artifacts();
                self.app.editor.undo();
                assert_eq!(self.app.editor.document, self.initial_document);
                self.app.editor.redo();
                assert_eq!(self.app.editor.document, doc);
            }
            "pen-cancel" => {
                assert_eq!(self.app.editor.document, self.initial_document);
                assert_eq!(self.app.editor.revision(), self.initial_revision);
                assert!(!self.app.editor.can_undo());
            }
            "pen-v4" => {
                let doc = self.app.editor.document.clone();
                let path = doc.nodes.last().unwrap();
                assert_eq!(path.kind, NodeKind::Path);
                assert!(path.closed);
                assert_eq!(path.anchors.len(), 3);
                assert_eq!(doc.required_version(), 4);
                let a = &papercraft::vector::world_anchors(path)[0];
                let near = |p: [f32; 2], q: [f32; 2]| {
                    assert!(
                        (p[0] - q[0]).abs() < 0.1 && (p[1] - q[1]).abs() < 0.1,
                        "{p:?} != {q:?}"
                    )
                };
                near(a.outgoing.unwrap(), [250.0, 100.0]);
                near(a.incoming.unwrap(), [70.0, 260.0]);
                assert_eq!(a.mode, papercraft::vector::PointMode::Corner);
                assert!(doc.export_svg().unwrap().contains(" C "));
                assert!(doc.export_html().unwrap().contains(" C "));
                self.save_1b_artifacts();
                self.app.editor.undo();
                self.app.editor.undo();
                self.app.editor.undo();
                assert_eq!(self.app.editor.document, self.initial_document);
                self.app.editor.redo();
                self.app.editor.redo();
                self.app.editor.redo();
                assert_eq!(self.app.editor.document, doc);
                self.app.editor.select(Some(path.id));
            }
            "offboard" => {
                let node = self
                    .app
                    .editor
                    .selected_node()
                    .expect("Offboard creation failed");
                assert_eq!(
                    [node.x, node.y, node.width, node.height],
                    [-60.0, 100.0, 40.0, 60.0]
                );
                let doc = self.app.editor.document.clone();
                let folder = self.output.parent().unwrap();
                let path = folder.join("human-offboard.papercraft");
                doc.save(&path).unwrap();
                assert_eq!(Document::load(&path).unwrap(), doc);
                atomic_write(
                    &folder.join("human-offboard-region.png"),
                    &papercraft::render::export_png_region(&doc, node.bounds(), 2.0).unwrap(),
                )
                .unwrap();
                atomic_write(
                    &folder.join("human-offboard.svg"),
                    doc.export_svg().unwrap().as_bytes(),
                )
                .unwrap();
                atomic_write(
                    &folder.join("human-offboard.html"),
                    doc.export_html().unwrap().as_bytes(),
                )
                .unwrap();
                self.app.editor.undo();
                assert!(self.app.editor.document.nodes.is_empty());
                self.app.editor.redo();
                assert_eq!(self.app.editor.document, doc);
            }
            "selection-1b" | "marquee-1b" | "navigation-1b" => {
                let ids: &[u64] = match self.scenario.as_str() {
                    "selection-1b" => &[2],
                    "marquee-1b" => &[8],
                    _ => &[],
                };
                assert_eq!(self.app.editor.selection(), ids);
                assert_eq!(self.app.editor.document, self.initial_document);
                assert_eq!(self.app.editor.revision(), self.initial_revision);
                assert!(!self.app.editor.can_undo());
                self.save_1b_artifacts();
            }
            "creation-1b" => {
                let n = self.app.editor.document.nodes.last().unwrap();
                assert_eq!([n.x, n.y, n.width, n.height], [40.0, 30.0, 160.0, 90.0]);
                let after = self.app.editor.document.clone();
                self.app.editor.undo();
                assert_eq!(self.app.editor.document, self.initial_document);
                self.app.editor.redo();
                assert_eq!(self.app.editor.document, after);
                self.save_1b_artifacts();
            }
            "cancel" => {
                assert_eq!(self.app.editor.document, self.initial_document);
                assert_eq!(self.app.editor.revision(), self.initial_revision);
                assert!(
                    self.app.editor.can_redo(),
                    "Escape must preserve existing redo"
                );
                self.app.editor.redo();
                assert_eq!(
                    self.app.editor.document.nodes.len(),
                    self.initial_document.nodes.len() + 1
                );
                self.app.editor.undo();
            }
            "resize" | "snapping" => {
                let (index, expected) = if self.scenario == "resize" {
                    (0, [100.0, 100.0, 160.0, 140.0])
                } else {
                    (1, [200.0, 100.0, 100.0, 100.0])
                };
                let node = &self.app.editor.document.nodes[index];
                assert_eq!([node.x, node.y, node.width, node.height], expected);
                assert_eq!(self.app.editor.selection(), &[node.id]);
                assert!(
                    self.app.editor.can_undo(),
                    "Drag must commit history on release"
                );
            }
            "marquee" => {
                let expected: Vec<_> = self.initial_document.nodes[..2]
                    .iter()
                    .map(|n| n.id)
                    .collect();
                assert_eq!(self.app.editor.selection(), expected);
                assert_eq!(self.app.editor.document, self.initial_document);
                assert_eq!(self.app.editor.revision(), self.initial_revision);
                assert!(
                    !self.app.editor.can_undo(),
                    "Marquee must not create undo history"
                );
                assert!(
                    !self.app.editor.can_redo(),
                    "Marquee must not create redo history"
                );
            }
            scenario if is_text_scenario(scenario) => {
                let placed = self
                    .placed_text
                    .as_ref()
                    .expect("Text placement checkpoint was not reached");
                let node = self
                    .app
                    .editor
                    .selected_node()
                    .expect("Text selection was lost");
                let expected = scenario_text(scenario);
                assert_eq!(
                    node.text, expected,
                    "Native typing/undo did not produce expected text"
                );
                assert_eq!(self.app.editor.selection(), &[placed.id]);
                assert_eq!(node.kind, NodeKind::Text);
                assert_eq!((node.x, node.y), (100.0, 300.0));
                assert_eq!(
                    self.app.editor.document.nodes.len(),
                    self.initial_document.nodes.len() + 1
                );
                let measured = ctx.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(
                            expected,
                            egui::FontId::proportional(node.font_size),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .ceil()
                });
                let height = if measured.x + 2.0 > MAX_COORDINATE {
                    ctx.fonts_mut(|fonts| {
                        fonts
                            .layout(
                                node.text.clone(),
                                egui::FontId::proportional(node.font_size),
                                egui::Color32::WHITE,
                                MAX_COORDINATE,
                            )
                            .size()
                            .ceil()
                            .y
                    })
                } else {
                    measured.y
                };
                assert_eq!(node.width, (measured.x + 2.0).clamp(1.0, MAX_COORDINATE));
                assert_eq!(node.height, height.clamp(1.0, MAX_COORDINATE));
                assert!(
                    node.width > placed.width,
                    "Text must grow from its placeholder width"
                );
                self.app
                    .editor
                    .document
                    .validate()
                    .expect("Native text geometry is invalid");
                assert!(
                    !ctx.egui_wants_keyboard_input(),
                    "Escape must finish inline editing"
                );
                assert!(
                    self.app.editor.can_undo(),
                    "Native text must enter document history"
                );
                assert!(
                    self.app.editor.revision() > self.placed_revision,
                    "Escape must commit the typed text, not just the layer creation"
                );
                if scenario == "long-text" {
                    assert!(
                        measured.x > MAX_COORDINATE,
                        "Fixture must exceed the text geometry limit"
                    );
                    assert_eq!(node.width, MAX_COORDINATE);
                }
                if scenario == "text-token" {
                    assert_eq!((node.fill, node.fill_token), (TOKEN_COLOR, None));
                    assert_eq!(node.font_size, self.initial_document.nodes[0].font_size);
                }
                if scenario == "text-tabs" {
                    assert_eq!((self.app.tab_count(), self.app.active_tab()), (2, 1));
                    let second = self.app.editor.document.clone();
                    let second_id = node.id;
                    self.app.activate_tab(0);
                    let first = self.app.editor.selected_node().unwrap();
                    assert_eq!(first.id, second_id);
                    assert_eq!(first.kind, NodeKind::Text);
                    assert_eq!(first.text, TEXT);
                    self.app.activate_tab(1);
                    assert_eq!(
                        self.app.editor.document, second,
                        "Tab activation changed text state"
                    );
                }
            }
            "quit" => {
                assert!(
                    self.app.document_open(),
                    "Native Quit discarded the document"
                );
                assert_eq!(self.app.editor.document.name, "Unsaved native quit check");
            }
            _ => {}
        }
        println!("Native scenario postconditions verified: {}", self.scenario);
    }
}

fn scenario_text(scenario: &str) -> String {
    match scenario {
        "long-text" => "W".repeat(10_000),
        "text-tabs" => "Second tab launch notes".into(),
        _ => TEXT.into(),
    }
}

fn is_text_scenario(scenario: &str) -> bool {
    scenario.starts_with("text") || scenario == "long-text"
}

/// Model workflow: create → bind/edit tokens → arrange → undo/redo → save → reopen →
/// HTML/SVG export. Returns the themed starter used by the `theme`, `bound` and
/// `arranged` scenarios.
fn token_workflow(directory: &std::path::Path) -> Editor {
    let mut editor = Editor::new(Document::example());
    let ids: Vec<u64> = editor.document.nodes.iter().map(|n| n.id).collect();
    let ink = editor
        .create_token_from(ColorTarget::Node(ids[1]))
        .unwrap()
        .unwrap();
    editor.rename_token(ink, "ink").unwrap();
    editor.bind(ColorTarget::Node(ids[2]), Some(ink)).unwrap();
    let surface = editor
        .create_token_from(ColorTarget::Node(ids[0]))
        .unwrap()
        .unwrap();
    editor.rename_token(surface, "surface").unwrap();
    let accent = editor
        .create_token_from(ColorTarget::Node(ids[3]))
        .unwrap()
        .unwrap();
    editor.rename_token(accent, "accent").unwrap();
    editor
        .bind(ColorTarget::Node(ids[5]), Some(accent))
        .unwrap();
    let paper = editor
        .create_token_from(ColorTarget::Background)
        .unwrap()
        .unwrap();
    editor.rename_token(paper, "paper").unwrap();
    editor.set_token_value(accent, [176, 72, 44, 255]).unwrap();
    for id in [ids[3], ids[5]] {
        assert_eq!(
            editor.document.target_color(ColorTarget::Node(id)),
            Some([176, 72, 44, 255])
        );
    }
    editor.set_selection([ids[1], ids[2], ids[3]]);
    let themed = editor.document.clone();
    editor.align(Align::Left).unwrap();
    editor.distribute(Axis::Vertical).unwrap();
    let arranged = editor.document.clone();
    editor.undo();
    editor.undo();
    assert_eq!(editor.document, themed);
    editor.redo();
    editor.redo();
    assert_eq!(editor.document, arranged);
    let path = directory.join("workflow.papercraft");
    editor.document.save(&path).unwrap();
    let reopened = Document::load(&path).unwrap();
    assert_eq!(reopened, editor.document);
    assert_eq!(reopened.version, 2);
    let html = reopened.export_html().unwrap();
    assert!(html.contains("--color-accent:rgba(176,72,44,1.0000);"));
    let svg = reopened.export_svg().unwrap();
    assert_eq!(svg.matches("fill=\"#b0482c\"").count(), 2);
    atomic_write(&directory.join("workflow.html"), html.as_bytes()).unwrap();
    atomic_write(&directory.join("workflow.svg"), svg.as_bytes()).unwrap();
    println!("Model token/arrange/undo/redo/save/reopen/HTML+SVG workflow verified.");
    // Return the themed, unarranged document for screenshots.
    editor.undo();
    editor.undo();
    editor
}

/// Usage: `verify_native [directory] [scenario] [WIDTHxHEIGHT] [hover=X,Y]`.
/// Supported scenarios are listed by `verify_native --help`.
fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Ok(());
    }
    let directory = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/verification"));
    let scenario = args.get(1).cloned().unwrap_or_else(|| "editor".into());
    if !SCENARIOS.contains(&scenario.as_str()) {
        eprintln!("Unknown native verification scenario: {scenario}");
        print_usage();
        std::process::exit(2);
    }
    let size = args
        .get(2)
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?]))
        .unwrap_or([1380.0, 860.0]);
    let hover = args
        .iter()
        .find_map(|a| a.strip_prefix("hover="))
        .and_then(|s| s.split_once(','))
        .and_then(|(x, y)| Some(egui::pos2(x.parse().ok()?, y.parse().ok()?)));
    std::fs::create_dir_all(&directory).expect("Cannot create verification directory");
    // Model lifecycle check, separate from native UI interaction.
    let mut editor = Editor::new(Document::default());
    editor.add(NodeKind::Text).unwrap();
    let before = editor.document.clone();
    editor.document.nodes[0].text = "İstanbul <&> 第二行".into();
    editor.commit(before.clone()).unwrap();
    let edited = editor.document.clone();
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_eq!(editor.document, edited);
    let lifecycle = directory.join("lifecycle.papercraft");
    editor.document.save(&lifecycle).unwrap();
    let reopened = Document::load(&lifecycle).unwrap();
    assert_eq!(reopened, edited);
    atomic_write(
        &directory.join("lifecycle.html"),
        reopened.export_html().unwrap().as_bytes(),
    )
    .unwrap();
    atomic_write(
        &directory.join("lifecycle.svg"),
        reopened.export_svg().unwrap().as_bytes(),
    )
    .unwrap();
    println!("Model create/edit/undo/redo/save/reopen/HTML+SVG export verified.");
    let themed = token_workflow(&directory);
    let document = if matches!(
        scenario.as_str(),
        "selection-1b" | "marquee-1b" | "navigation-1b"
    ) {
        selection_1b_document()
    } else if scenario == "starter-frame" {
        let mut doc = Document::starter(true);
        doc.nodes[1].locked = true;
        doc.nodes.last_mut().unwrap().hidden = true;
        doc
    } else if matches!(scenario.as_str(), "pen-v4" | "pen-cancel") {
        Document::starter(false)
    } else if scenario == "creation-1b" {
        let mut editor = Editor::new(Document::default());
        editor
            .add_at(NodeKind::Rectangle, 200.0, 120.0, 100.0, 100.0)
            .unwrap();
        editor.document.nodes[0].locked = true;
        editor.document
    } else if scenario == "offboard" {
        Document::default()
    } else if matches!(
        scenario.as_str(),
        "resize" | "snapping" | "marquee" | "cancel"
    ) {
        let mut editor = Editor::new(Document::default());
        let positions: &[f32] = if scenario == "marquee" {
            &[100.0, 300.0, 600.0]
        } else {
            &[100.0, 300.0]
        };
        for &x in positions {
            editor
                .add_at(NodeKind::Rectangle, x, 100.0, 100.0, 100.0)
                .unwrap();
        }
        editor.document
    } else if scenario == "text-token" {
        let mut editor = Editor::new(Document::default());
        editor
            .add_at(NodeKind::Text, 100.0, 100.0, 220.0, 60.0)
            .unwrap();
        editor.document.nodes[0].font_size = 44.0;
        let id = editor.document.nodes[0].id;
        editor
            .create_token_from(ColorTarget::Node(id))
            .unwrap()
            .unwrap();
        editor.document
    } else if scenario == "text-tabs" {
        Document::starter(false)
    } else if is_text_scenario(&scenario) {
        Document::default()
    } else if scenario == "exchange" {
        serde_json::from_str(include_str!("../tests/fixtures/svg-exchange.papercraft")).unwrap()
    } else if matches!(scenario.as_str(), "theme" | "bound" | "arranged") {
        themed.document.clone()
    } else {
        Document::example()
    };
    document
        .save(&directory.join("starter.papercraft"))
        .expect("Cannot save starter document");
    assert_eq!(
        Document::load(&directory.join("starter.papercraft")).unwrap(),
        document
    );
    for (format, result) in [
        ("html", document.export_html()),
        ("svg", document.export_svg()),
    ] {
        match result {
            Ok(text) => atomic_write(
                &directory.join(format!("starter.{format}")),
                text.as_bytes(),
            )
            .expect("Cannot export starter"),
            Err(message) if scenario.ends_with("-1b") => {
                assert!(message.contains("ancestor clipping"))
            }
            Err(message) => panic!("Cannot export starter: {message}"),
        }
    }
    let output = if scenario == "editor" && args.len() < 3 {
        directory.join("papercraft-desktop.png")
    } else {
        directory.join(format!(
            "papercraft-{scenario}-{}x{}{}.png",
            size[0],
            size[1],
            if hover.is_some() { "-hover" } else { "" }
        ))
    };
    // Typography: centered heading with an explicit line height, right-aligned body.
    let document = if scenario == "typography" {
        let mut styled = document.clone();
        styled.nodes[1].text_align = papercraft::document::TextAlign::Center;
        styled.nodes[1].line_height = Some(64.0);
        styled.nodes[2].text_align = papercraft::document::TextAlign::Right;
        styled.nodes[2].line_height = Some(36.0);
        styled.normalize();
        styled
    } else {
        document
    };
    let mut app = PaperCraftApp::from_document(document.clone());
    match scenario.as_str() {
        "resize" => app.editor.select(Some(document.nodes[0].id)),
        "cancel" => {
            app.editor.add(NodeKind::Text).unwrap();
            app.editor.undo();
            app.editor.select(Some(document.nodes[0].id));
            app.snapping = false;
        }
        "selected" => app.editor.select(document.nodes.get(3).map(|n| n.id)),
        "tool" => app.tool = Tool::Rectangle,
        "text-token" => app.editor.select(Some(document.nodes[0].id)),
        "typography" => app.editor.select(Some(document.nodes[1].id)),
        "theme" => {
            app.theme = true;
            app.selected_token = document
                .tokens
                .iter()
                .find(|t| t.name == "accent")
                .map(|t| t.id);
        }
        "bound" => app.editor.select(document.nodes.get(3).map(|n| n.id)),
        "arranged" => app.editor.set_selection(themed.selection().iter().copied()),
        "dashboard" | "list" | "empty" => {
            let mut recents = if scenario == "empty" {
                Recents::in_memory()
            } else {
                Recents::load(directory.join("recents.json"))
            };
            if scenario != "empty" {
                let second = Document {
                    name: "Pricing page".into(),
                    canvas: [32, 34, 40, 255],
                    background: [255, 255, 255, 255],
                    ..Default::default()
                };
                let mut editor = papercraft::document::Editor::new(second);
                editor
                    .add_at(
                        papercraft::document::NodeKind::Rectangle,
                        60.0,
                        60.0,
                        840.0,
                        120.0,
                    )
                    .unwrap();
                editor
                    .add_at(
                        papercraft::document::NodeKind::Text,
                        80.0,
                        220.0,
                        600.0,
                        80.0,
                    )
                    .unwrap();
                editor
                    .document
                    .save(&directory.join("pricing.papercraft"))
                    .unwrap();
                recents
                    .touch(&directory.join("pricing.papercraft"))
                    .unwrap();
                recents
                    .touch(&directory.join("starter.papercraft"))
                    .unwrap();
                recents
                    .touch(&directory.join("missing.papercraft"))
                    .unwrap();
            }
            app = PaperCraftApp::from_document(Document::default()).with_recents(recents);
            app.view = View::Dashboard;
            if scenario == "list" {
                app.set_list_view(true);
            }
            if scenario == "empty" {
                assert!(app.close_document());
            }
        }
        _ => {}
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size),
        ..Default::default()
    };
    eframe::run_native(
        "PaperCraft verification",
        options,
        Box::new(move |cc| {
            #[cfg(target_os = "macos")]
            craft_quit::install_app_menu(&cc.egui_ctx, "PaperCraft verification")?;
            PaperCraftApp::configure(&cc.egui_ctx);
            // Capture stable menu/modal states rather than a partially faded frame.
            cc.egui_ctx
                .global_style_mut(|style| style.animation_time = 0.0);
            let initial_revision = app.editor.revision();
            let script = script(&scenario, size, &document);
            Ok(Box::new(NativeCheck {
                app,
                output,
                frames: 0,
                started: std::time::Instant::now(),
                hover,
                scenario,
                initial_document: document,
                initial_revision,
                placed_text: None,
                placed_revision: 0,
                script_frame: 0,
                script_started: false,
                screenshot_requested: false,
                finished: false,
                script,
            }))
        }),
    )
}

fn selection_1b_document() -> Document {
    let mut doc = Document {
        nodes: vec![
            Node {
                id: 1,
                x: -50.0,
                y: 100.0,
                radius: 40.0,
                fill: [80, 140, 200, 255],
                ..Node::default()
            },
            Node {
                id: 2,
                kind: NodeKind::Ellipse,
                x: 120.0,
                y: 100.0,
                fill: [130, 100, 185, 255],
                ..Node::default()
            },
            Node {
                id: 3,
                kind: NodeKind::Triangle,
                x: 300.0,
                y: 100.0,
                fill: [230, 130, 70, 255],
                ..Node::default()
            },
            Node {
                id: 4,
                x: 480.0,
                y: 100.0,
                fill: [0; 4],
                stroke: Some([50, 140, 100, 255]),
                stroke_width: 6.0,
                ..Node::default()
            },
            Node {
                id: 5,
                kind: NodeKind::Path,
                x: 140.0,
                y: 300.0,
                width: 220.0,
                height: 140.0,
                points: vec![[0.0, 0.0], [1.0, 1.0]],
                fill: [0; 4],
                stroke: Some([60, 110, 170, 255]),
                stroke_width: 6.0,
                ..Node::default()
            },
            Node {
                id: 6,
                kind: NodeKind::Frame,
                x: -60.0,
                y: 280.0,
                width: 90.0,
                height: 200.0,
                clip: true,
                fill: [0; 4],
                ..Node::default()
            },
            Node {
                id: 7,
                kind: NodeKind::Frame,
                parent: Some(6),
                x: -60.0,
                y: 280.0,
                width: 70.0,
                height: 200.0,
                clip: true,
                fill: [0; 4],
                ..Node::default()
            },
            Node {
                id: 8,
                kind: NodeKind::Path,
                parent: Some(7),
                x: -60.0,
                y: 280.0,
                width: 200.0,
                height: 200.0,
                points: vec![[0.0, 0.0], [1.0, 1.0]],
                fill: [0; 4],
                stroke: Some([200, 80, 90, 255]),
                stroke_width: 6.0,
                ..Node::default()
            },
        ],
        next_id: 9,
        ..Document::default()
    };
    doc.normalize();
    doc.validate().unwrap();
    doc
}

fn print_usage() {
    eprintln!("Usage: verify_native [directory] [scenario] [WIDTHxHEIGHT] [hover=X,Y]");
    eprintln!(
        "Scenarios (editor is the default): {}",
        SCENARIOS.join(", ")
    );
}

/// Screen position of artboard point `(x, y)` for the fitted starter view, from the
/// editor's shared panel dimensions and fit margins.
fn artboard_point(size: [f32; 2], document: &Document, x: f32, y: f32) -> egui::Pos2 {
    let canvas = egui::Rect::from_min_max(
        egui::pos2(
            papercraft::ui::SIDEBAR + papercraft::ui::RAIL,
            papercraft::ui::TAB_BAR,
        ),
        egui::pos2(size[0] - papercraft::ui::INSPECTOR, size[1]),
    );
    let zoom = ((canvas.width() - 2.0 * papercraft::ui::SPACE_3XL) / document.width)
        .min((canvas.height() - 2.0 * papercraft::ui::SPACE_3XL) / document.height)
        .clamp(0.001, 2.0);
    let board = egui::vec2(document.width, document.height) * zoom;
    canvas.center() - board / 2.0 + egui::vec2(x, y) * zoom
}

/// Scripted creation, text, marquee, resize and snapped-move input.
fn script(scenario: &str, size: [f32; 2], document: &Document) -> Vec<Vec<egui::Event>> {
    let at = |x, y| artboard_point(size, document, x, y);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let key = |key, pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    match scenario {
        "starter-frame" => {
            let a = at(8.0, 8.0);
            let b = at(48.0, 28.0);
            vec![
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![egui::Event::PointerMoved(b)],
                vec![button(b, false)],
            ]
        }
        "pen-v4" | "pen-cancel" => {
            let mut frames = vec![
                vec![key(egui::Key::P, true)],
                vec![key(egui::Key::P, false)],
            ];
            let mut click = |x, y| {
                let p = at(x, y);
                frames.extend([
                    vec![egui::Event::PointerMoved(p), button(p, true)],
                    vec![button(p, false)],
                    vec![],
                    vec![],
                ]);
            };
            if scenario == "pen-cancel" {
                click(150.0, 170.0);
                click(370.0, 170.0);
                frames.extend([
                    vec![key(egui::Key::Escape, true)],
                    vec![key(egui::Key::Escape, false)],
                ]);
                return frames;
            }
            let a = at(150.0, 170.0);
            let h = at(210.0, 110.0);
            frames.extend([
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![egui::Event::PointerMoved(h)],
                vec![button(h, false)],
                vec![],
            ]);
            for (x, y) in [(370.0, 170.0), (330.0, 330.0), (150.0, 170.0)] {
                let p = at(x, y);
                frames.extend([
                    vec![egui::Event::PointerMoved(p), button(p, true)],
                    vec![button(p, false)],
                    vec![],
                    vec![],
                ]);
            }
            frames.extend([
                vec![key(egui::Key::Enter, true)],
                vec![key(egui::Key::Enter, false)],
                vec![],
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![button(a, false)],
                vec![],
            ]);
            let b = at(230.0, 80.0);
            frames.extend([
                vec![egui::Event::PointerMoved(h), button(h, true)],
                vec![egui::Event::PointerMoved(b)],
                vec![button(b, false)],
                vec![],
            ]);
            let c = at(250.0, 100.0);
            let alt = |p, pressed| egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::ALT,
            };
            frames.extend([
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                    egui::Event::PointerMoved(b),
                    alt(b, true),
                ],
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                    egui::Event::PointerMoved(c),
                ],
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                    alt(c, false),
                ],
                vec![],
            ]);
            frames
        }
        "selection-1b" => {
            let corner = at(122.0, 102.0);
            let center = at(170.0, 150.0);
            let mut frames = vec![
                vec![egui::Event::PointerMoved(corner), button(corner, true)],
                vec![button(corner, false)],
            ];
            frames.extend(vec![vec![]; 20]);
            frames.extend([
                vec![egui::Event::PointerMoved(center), button(center, true)],
                vec![button(center, false)],
            ]);
            frames
        }
        "marquee-1b" => {
            let (a, b) = (at(-55.0, 380.0), at(0.0, 430.0));
            let (c, d) = (at(-65.0, 270.0), at(-40.0, 310.0));
            let mut frames = vec![
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![egui::Event::PointerMoved(b)],
                vec![button(b, false)],
            ];
            frames.extend(vec![vec![]; 20]);
            frames.extend([
                vec![egui::Event::PointerMoved(c), button(c, true)],
                vec![egui::Event::PointerMoved(d)],
                vec![button(d, false)],
            ]);
            frames
        }
        "creation-1b" => {
            let (a, b) = (at(40.0, 30.0), at(197.0, 117.0));
            vec![
                vec![key(egui::Key::R, true)],
                vec![key(egui::Key::R, false)],
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![egui::Event::PointerMoved(at(180.0, 110.0))],
                vec![egui::Event::PointerMoved(b), button(b, false)],
            ]
        }
        "navigation-1b" => {
            let a = at(100.0, 100.0);
            let b = a + egui::vec2(60.0, 30.0);
            let mut frames = vec![
                vec![key(egui::Key::R, true)],
                vec![key(egui::Key::R, false)],
                vec![key(egui::Key::Space, true)],
                vec![egui::Event::PointerMoved(a), button(a, true)],
                vec![egui::Event::PointerMoved(a + egui::vec2(30.0, 15.0))],
                vec![key(egui::Key::Space, false)],
                vec![egui::Event::PointerMoved(b), button(b, false)],
            ];
            frames.push(vec![key(egui::Key::Escape, true)]);
            frames.push(vec![key(egui::Key::Escape, false)]);
            frames.push(vec![
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                egui::Event::Key {
                    key: egui::Key::Num0,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                },
            ]);
            frames.push(vec![egui::Event::Key {
                key: egui::Key::Num0,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::SHIFT,
            }]);
            frames.push(vec![
                egui::Event::PointerMoved(at(350.0, 250.0)),
                egui::Event::Zoom(1.5),
            ]);
            frames
        }
        "offboard" => {
            let focus = at(20.0, 20.0);
            let (start, end) = (at(-60.0, 100.0), at(-20.0, 160.0));
            vec![
                vec![egui::Event::PointerMoved(focus), button(focus, true)],
                vec![button(focus, false)],
                vec![key(egui::Key::R, true)],
                vec![key(egui::Key::R, false)],
                vec![egui::Event::PointerMoved(start), button(start, true)],
                vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
                vec![egui::Event::PointerMoved(end)],
                vec![button(end, false)],
            ]
        }
        "cancel" => {
            let (start, end) = (at(150.0, 150.0), at(190.0, 170.0));
            vec![
                vec![egui::Event::PointerMoved(start), button(start, true)],
                vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
                vec![egui::Event::PointerMoved(end)],
                vec![key(egui::Key::Escape, true)],
                vec![key(egui::Key::Escape, false)],
                vec![button(end, false)],
            ]
        }
        "resize" | "snapping" => {
            let (start, end) = if scenario == "resize" {
                (at(200.0, 200.0), at(260.0, 240.0))
            } else {
                (at(350.0, 150.0), at(253.0, 150.0))
            };
            vec![
                vec![],
                vec![egui::Event::PointerMoved(start), button(start, true)],
                vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
                vec![egui::Event::PointerMoved(end)],
                vec![],
                vec![],
                vec![button(end, false)],
            ]
        }
        scenario if is_text_scenario(scenario) => {
            // Canvas click establishes ordinary application focus, then the T
            // shortcut selects the real text tool. Press/release occupy separate
            // native frames; two idle frames let the new text editor take focus.
            let focus = at(20.0, 20.0);
            let p = at(100.0, 300.0);
            let create = |text| {
                vec![
                    vec![egui::Event::PointerMoved(focus), button(focus, true)],
                    vec![button(focus, false)],
                    vec![],
                    vec![key(egui::Key::T, true)],
                    vec![key(egui::Key::T, false)],
                    vec![],
                    vec![egui::Event::PointerMoved(p), button(p, true)],
                    vec![button(p, false)],
                    vec![],
                    vec![],
                    vec![egui::Event::Text(text)],
                    vec![],
                    vec![key(egui::Key::Escape, true)],
                    vec![key(egui::Key::Escape, false)],
                    vec![],
                ]
            };
            if scenario != "text-tabs" {
                return create(scenario_text(scenario));
            }
            let command = |key, pressed| {
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    },
                ]
            };
            let mut frames = create(TEXT.into());
            frames.extend([
                command(egui::Key::N, true),
                command(egui::Key::N, false),
                vec![],
            ]);
            frames.extend(create(scenario_text(scenario)));
            // IDs are tab-local. Undo in a fresh inline editor in
            // tab B must not revive any text from the previous tab/editor.
            frames.extend([
                vec![key(egui::Key::Enter, true)],
                vec![key(egui::Key::Enter, false)],
                vec![],
                command(egui::Key::Z, true),
                command(egui::Key::Z, false),
                vec![key(egui::Key::Escape, true)],
                vec![key(egui::Key::Escape, false)],
                vec![],
            ]);
            frames
        }
        "marquee" => vec![
            vec![],
            vec![
                egui::Event::PointerMoved(at(10.0, 10.0)),
                button(at(10.0, 10.0), true),
            ],
            vec![egui::Event::PointerMoved(at(200.0, 200.0))],
            vec![egui::Event::PointerMoved(at(450.0, 380.0))],
            vec![button(at(450.0, 380.0), false)],
            vec![],
        ],
        _ => vec![vec![]; 4],
    }
}
