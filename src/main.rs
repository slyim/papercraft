#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(status) = papercraft::cli::run(&args) {
        std::process::exit(status);
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(papercraft::ui::window_icon())
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([960.0, 640.0])
            // macOS: the tab strip shares the title bar with the window controls.
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    eframe::run_native(
        "PaperCraft",
        options,
        Box::new(|cc| {
            #[cfg(target_os = "macos")]
            craft_quit::install_app_menu(&cc.egui_ctx, "PaperCraft")?;
            Ok(Box::new(papercraft::app::PaperCraftApp::new(cc)))
        }),
    )
}
