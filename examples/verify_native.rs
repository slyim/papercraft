use crumbs::{
    app::{CrumbsApp, Tool, View},
    document::{Document, atomic_write},
    recents::Recents,
};
use eframe::egui;
use std::path::PathBuf;

struct NativeCheck {
    app: CrumbsApp,
    output: PathBuf,
    frames: usize,
    hover: Option<egui::Pos2>,
}

impl eframe::App for NativeCheck {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(pos) = self.hover {
            raw_input.events.push(egui::Event::PointerMoved(pos));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
        self.frames += 1;
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
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        assert!(
            self.frames < 120,
            "Screenshot was not returned by the renderer"
        );
        ui.ctx().request_repaint();
    }
}

/// Usage: `verify_native [directory] [scenario] [WIDTHxHEIGHT] [hover=X,Y]`.
/// Scenarios: `editor` (default), `selected`, `tool`, `dashboard`, `list`, `empty`.
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
    let document = Document::example();
    document
        .save(&directory.join("starter.crumbs"))
        .expect("Cannot save starter document");
    assert_eq!(
        Document::load(&directory.join("starter.crumbs")).unwrap(),
        document
    );
    atomic_write(
        &directory.join("starter.html"),
        document.export_html().unwrap().as_bytes(),
    )
    .expect("Cannot export starter HTML");
    let output = if scenario == "editor" && args.len() < 3 {
        directory.join("crumbs-desktop.png")
    } else {
        directory.join(format!(
            "crumbs-{scenario}-{}x{}{}.png",
            size[0],
            size[1],
            if hover.is_some() { "-hover" } else { "" }
        ))
    };
    let mut app = CrumbsApp::from_document(document.clone());
    match scenario.as_str() {
        "selected" => app.editor.selected = document.nodes.get(3).map(|n| n.id),
        "tool" => app.tool = Tool::Rectangle,
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
                let mut editor = crumbs::document::Editor::new(second);
                editor
                    .add_at(
                        crumbs::document::NodeKind::Rectangle,
                        60.0,
                        60.0,
                        840.0,
                        120.0,
                    )
                    .unwrap();
                editor
                    .add_at(crumbs::document::NodeKind::Text, 80.0, 220.0, 600.0, 80.0)
                    .unwrap();
                editor
                    .document
                    .save(&directory.join("pricing.crumbs"))
                    .unwrap();
                recents.touch(&directory.join("pricing.crumbs")).unwrap();
                recents.touch(&directory.join("starter.crumbs")).unwrap();
                recents.touch(&directory.join("missing.crumbs")).unwrap();
            }
            app = CrumbsApp::from_document(Document::default()).with_recents(recents);
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
        "Crumbs verification",
        options,
        Box::new(move |cc| {
            CrumbsApp::configure(&cc.egui_ctx);
            Ok(Box::new(NativeCheck {
                app,
                output,
                frames: 0,
                hover,
            }))
        }),
    )
}
