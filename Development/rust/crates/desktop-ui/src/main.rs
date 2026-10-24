#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod playback;
mod startup;
mod ui;
mod verification;
use eframe::egui;
fn main() -> eframe::Result {
    verification::install_desktop_diagnostics();
    let capture_hold = std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--verify-desktop")
        && std::env::args_os().any(|a| a == "--capture-hold");
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
    let icon = image::load_from_memory_with_format(
        include_bytes!("../../../../assets/app-icon.png"),
        image::ImageFormat::Png,
    )
    .expect("The embedded application icon must be a valid PNG")
    .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
    .into_rgba8();
    let mut viewport = egui::ViewportBuilder::default()
        .with_icon(egui::IconData {
            rgba: icon.into_raw(),
            width: 256,
            height: 256,
        })
        .with_inner_size([1360.0, 900.0])
        .with_min_inner_size([1000.0, 740.0]);
    if capture_hold {
        viewport = viewport.with_active(false);
    }
    let mut graphics = eframe::egui_wgpu::WgpuSetupCreateNew::default();
    let device_descriptor = graphics.device_descriptor.clone();
    graphics.device_descriptor = std::sync::Arc::new(move |adapter| {
        let mut descriptor = device_descriptor(adapter);
        descriptor.memory_hints = wgpu::MemoryHints::MemoryUsage;
        descriptor
    });
    graphics.instance_descriptor.backends = wgpu::Backends::DX12;
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(graphics),
            ..Default::default()
        },
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Simple Video Editor",
        options,
        Box::new(|cc| {
            verification::record_desktop_renderer(cc)?;
            Ok(Box::new(ui::EditorApp::new(cc)?))
        }),
    )
}
