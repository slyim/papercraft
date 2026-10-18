fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([960.0, 640.0])
            // macOS: the tab strip shares the title bar with the window controls.
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    eframe::run_native(
        "Crumbs",
        options,
        Box::new(|cc| Ok(Box::new(crumbs::app::CrumbsApp::new(cc)))),
    )
}
