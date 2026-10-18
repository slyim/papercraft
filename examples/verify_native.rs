use eframe::egui;
use papercraft::{
    app::{PaperCraftApp, Tool, View},
    document::{Align, Axis, ColorTarget, Document, Editor, NodeKind, atomic_write},
    recents::Recents,
};
use std::path::PathBuf;

struct NativeCheck {
    app: PaperCraftApp,
    output: PathBuf,
    frames: usize,
    hover: Option<egui::Pos2>,
    quit_check: bool,
    export_menu: bool,
}

impl eframe::App for NativeCheck {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::logic(&mut self.app, ctx, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(pos) = self.hover {
            raw_input.events.push(egui::Event::PointerMoved(pos));
        }
        if self.export_menu && (self.frames == 2 || self.frames == 3) {
            let pos = egui::pos2(ctx.content_rect().right() - 42.0, 65.0);
            raw_input.events.push(egui::Event::PointerMoved(pos));
            raw_input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: self.frames == 2,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
        self.frames += 1;
        if self.quit_check && self.frames == 2 {
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
        if self.frames == 6 {
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
            if self.quit_check {
                assert_eq!(self.app.editor.document.name, "Unsaved native quit check");
                self.app.editor.undo(); // Restore the saved state before ending this check.
                println!("Unsaved document survived simulated native Quit/minimize dispatch.");
            }
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        assert!(
            self.frames < 120,
            "Screenshot was not returned by the renderer"
        );
        ui.ctx().request_repaint();
    }
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
/// Scenarios: `editor` (default), `selected`, `tool`, `exchange`, `export`, `quit`, `dashboard`, `list`, `empty`, `theme`, `bound`, `arranged`.
fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let directory = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/verification"));
    let scenario = args.get(1).cloned().unwrap_or_else(|| "editor".into());
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
    let document = if scenario == "exchange" {
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
    atomic_write(
        &directory.join("starter.html"),
        document.export_html().unwrap().as_bytes(),
    )
    .expect("Cannot export starter HTML");
    atomic_write(
        &directory.join("starter.svg"),
        document.export_svg().unwrap().as_bytes(),
    )
    .expect("Cannot export starter SVG");
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
    let mut app = PaperCraftApp::from_document(document.clone());
    match scenario.as_str() {
        "selected" => app.editor.select(document.nodes.get(3).map(|n| n.id)),
        "tool" => app.tool = Tool::Rectangle,
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
            Ok(Box::new(NativeCheck {
                app,
                output,
                frames: 0,
                hover,
                quit_check: scenario == "quit",
                export_menu: scenario == "export",
            }))
        }),
    )
}
