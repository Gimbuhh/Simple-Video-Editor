#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod playback;
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
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1360.0, 900.0])
        .with_min_inner_size([1000.0, 740.0]);
    if capture_hold {
        viewport = viewport.with_active(false);
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew {
                    // Keep the interface off OpenGL, including software video
                    // preview. Capture coexistence is verified separately.
                    instance_descriptor: wgpu::InstanceDescriptor {
                        backends: wgpu::Backends::DX12,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
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
