//! Headless frame-time benchmark for large documents.
//!
//! `cargo run --release --example bench -- [layers...]` prints the median time of idle
//! frames, select-all, idle frames with everything selected, drag frames of the whole
//! selection, and one undo, for each layer count (default 1000, 5000, 10000). Frames include egui tessellation, not GPU upload.

use eframe::egui;
use papercraft::{
    app::PaperCraftApp,
    document::{Document, Node, NodeKind},
};
use std::time::{Duration, Instant};

const SIZE: egui::Vec2 = egui::vec2(1380.0, 860.0);

fn document(layers: usize) -> Document {
    let mut doc = Document {
        width: 4000.0,
        height: 4000.0,
        ..Document::default()
    };
    for i in 0..layers {
        let column = (i % 100) as f32;
        let row = (i / 100) as f32;
        let text = i % 4 == 0;
        doc.nodes.push(Node {
            id: i as u64 + 1,
            name: format!("Layer {i}"),
            kind: if text {
                NodeKind::Text
            } else {
                NodeKind::Rectangle
            },
            x: column * 40.0,
            y: row * 40.0,
            width: 32.0,
            height: 32.0,
            fill: [124, 155, 214, 255],
            radius: 4.0,
            text: if text { format!("T{i}") } else { String::new() },
            font_size: 12.0,
            hidden: false,
            locked: false,
            fill_token: None,
        });
    }
    doc.next_id = layers as u64 + 1;
    doc
}

fn frame(ctx: &egui::Context, app: &mut PaperCraftApp, events: Vec<egui::Event>) -> Duration {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        events,
        ..Default::default()
    };
    let start = Instant::now();
    let output = ctx.run_ui(input, |ui| app.show(ui));
    let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
    start.elapsed()
}

fn artboard(ctx: &egui::Context, app: &mut PaperCraftApp) -> egui::Rect {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        ..Default::default()
    };
    let fill = egui::Color32::from_rgb(248, 247, 243);
    ctx.run_ui(input, |ui| app.show(ui))
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap()
}

/// Median in milliseconds; less sensitive than the mean to scheduler noise.
fn median(samples: &[Duration]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2].as_secs_f64() * 1000.0
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn main() {
    let counts: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let counts = if counts.is_empty() {
        vec![1_000, 5_000, 10_000]
    } else {
        counts
    };
    println!("layers  idle ms  select-all ms  selected idle ms  drag ms  undo ms");
    for layers in counts {
        let ctx = egui::Context::default();
        PaperCraftApp::configure(&ctx);
        let mut app = PaperCraftApp::from_document(document(layers));
        for _ in 0..3 {
            frame(&ctx, &mut app, Vec::new());
        }
        let idle: Vec<_> = (0..20).map(|_| frame(&ctx, &mut app, Vec::new())).collect();

        let start = Instant::now();
        app.editor.select_all();
        let select_all = start.elapsed();

        let selected: Vec<_> = (0..20).map(|_| frame(&ctx, &mut app, Vec::new())).collect();

        // Press inside the first layer, at the artboard origin.
        let hit = artboard(&ctx, &mut app).min + egui::vec2(2.0, 2.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(hit), button(hit, true)],
        );
        let drag: Vec<_> = (1..=20)
            .map(|step| {
                let pos = hit + egui::vec2(step as f32 * 2.0, step as f32);
                frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pos)])
            })
            .collect();
        frame(
            &ctx,
            &mut app,
            vec![button(hit + egui::vec2(40.0, 20.0), false)],
        );
        frame(&ctx, &mut app, Vec::new());

        assert!(app.editor.can_undo(), "the drag did not move the selection");
        let start = Instant::now();
        app.editor.undo();
        let undo = start.elapsed();

        println!(
            "{layers:>6}  {:>7.2}  {:>13.2}  {:>16.2}  {:>7.2}  {:>7.3}",
            median(&idle),
            select_all.as_secs_f64() * 1000.0,
            median(&selected),
            median(&drag),
            undo.as_secs_f64() * 1000.0,
        );
    }
}
