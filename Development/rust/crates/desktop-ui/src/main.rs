#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod playback;
mod ui;
mod verification;
use eframe::egui;
fn main() -> eframe::Result {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--verify-media")
    {
        if let Err(error) = verification::media() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 900.0])
            .with_min_inner_size([1000.0, 740.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Simple Video Editor",
        options,
        Box::new(|cc| Ok(Box::new(ui::EditorApp::new(cc)?))),
    )
}
