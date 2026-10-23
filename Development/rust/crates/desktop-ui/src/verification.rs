use eframe::egui;
use std::{path::PathBuf, time::Instant};
pub fn record_desktop_renderer(cc: &eframe::CreationContext<'_>) -> std::io::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|v| v != "--verify-desktop") {
        return Ok(());
    }
    let renderer = cc
        .wgpu_render_state
        .as_ref()
        .ok_or_else(|| std::io::Error::other("Missing desktop renderer"))?;
    let adapter = renderer.adapter.get_info();
    if adapter.backend != wgpu::Backend::Dx12 {
        return Err(std::io::Error::other("Desktop did not select Direct3D 12"));
    }
    if let Some(directory) = args.get(3).map(PathBuf::from) {
        std::fs::write(
            directory.join("graphics.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "backend": format!("{:?}", adapter.backend),
                "adapter": adapter.name,
                "driver": adapter.driver,
                "format": format!("{:?}", renderer.target_format),
            }))?,
        )?;
    }
    Ok(())
}
pub fn install_desktop_diagnostics() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "--verify-desktop")
        && let Some(directory) = args.get(3).map(PathBuf::from)
        && std::fs::create_dir_all(&directory).is_ok()
    {
        let _ = std::fs::write(directory.join("phase.txt"), "graphics initialization");
        platform_windows::trace_verification_crashes(directory.join("native-crash.txt"));
    }
}
pub fn media() -> media_engine::Result<()> {
    use editor_core::{Clip, ClipId, FrameRate, SourceId};
    use media_engine::{Tools, file_hash, uncancelled};
    use project_store::*;
    let args: Vec<_> = std::env::args_os().collect();
    let fixtures = PathBuf::from(args.get(2).ok_or("Missing fixture directory")?);
    let directory = PathBuf::from(args.get(3).ok_or("Missing report directory")?);
    std::fs::create_dir_all(&directory)?;
    let result = (|| -> media_engine::Result<Vec<String>> {
        let tools = Tools::discover(directory.join("cache"))?;
        let cancel = uncancelled();
        let path = fixtures.join("sdr.mkv");
        let hash = file_hash(&path)?;
        let source = tools.probe(&path, SourceId(1), &cancel)?;
        let document = SavedProject {
            version: 2,
            application_version: env!("CARGO_PKG_VERSION").into(),
            clips: vec![Clip {
                id: ClipId(1),
                source: source.id,
                source_in: 0.2,
                source_out: 1.2,
                timeline_start: 0.25,
            }],
            metadata: ProjectMetadata {
                sources: vec![source],
                output: OutputSettings {
                    width: 320,
                    height: 180,
                    rate: FrameRate::new(30, 1)?,
                    gpu: false,
                    codec: Codec::H264,
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let project = directory.join("portable.sveproject");
        save_document(&project, &document)?;
        if tools.load_project(&project, &cancel)? != document {
            return Err("Portable project roundtrip mismatch".into());
        }
        tools.export(&document, &directory.join("portable.mp4"), &cancel, |_| {})?;
        let recording = media_path::RecordingPath::authorize(&path)?;
        let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::SoftwareSdr)?;
        player.load(&recording)?;
        player.seek(0.5)?;
        let deadline = Instant::now() + std::time::Duration::from_secs(8);
        let mut visible = false;
        while Instant::now() < deadline {
            player.pump()?;
            if player
                .take_frame()
                .is_some_and(|frame| frame.pixels.iter().any(|b| *b > 32))
            {
                visible = true;
                break;
            }
        }
        if !visible {
            return Err("Portable playback produced no video frame".into());
        }
        drop(player);
        if file_hash(&path)? != hash {
            return Err("Portable check changed its source".into());
        }
        Ok(vec![
            "Pinned media bundle".into(),
            "Schema-2 persistence and exact re-probe".into(),
            "CPU export including a leading black/silent gap".into(),
            "Software libmpv preview and seek".into(),
            "Original recording unchanged".into(),
        ])
    })();
    let report = match &result {
        Ok(checks) => serde_json::json!({"passed":true,"checks":checks}),
        Err(error) => serde_json::json!({"passed":false,"failure":error.to_string()}),
    };
    std::fs::write(
        directory.join("portable.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    result.map(|_| ())
}
pub struct Verification {
    pub capture_hold: bool,
    pub capture_hdr: bool,
    pub graphics_only: bool,
    pub vfr: bool,
    pub startup_only: bool,
    pub source: PathBuf,
    pub secondary: Option<PathBuf>,
    pub scrub_target: f64,
    pub directory: PathBuf,
    pub phase: u8,
    pub started: Instant,
    pub input: Vec<egui::Event>,
    pub checks: Vec<String>,
    pub failure: Option<String>,
    pub anchor: Option<egui::Pos2>,
    pub expected: Option<Vec<editor_core::Clip>>,
    pub layout: usize,
    pub focus_steps: usize,
    pub keyboard_position: f64,
    pub scale_layouts: bool,
    pub scale: usize,
    pub was_software: bool,
    pub recovery_lock: Option<std::fs::File>,
    pub precision: Option<project_store::SavedProject>,
    pub launch_files: bool,
    pub dropped_files: Vec<egui::DroppedFile>,
    pub recorded_phase: Option<u8>,
    pub fail_native: bool,
}
impl Verification {
    pub fn arguments() -> Option<Self> {
        let args: Vec<_> = std::env::args_os().collect();
        let launch_directory = std::env::var_os("SVE_VERIFY_LAUNCH_DIRECTORY");
        let launch_files = launch_directory.is_some();
        if !launch_files && args.get(1).is_none_or(|v| v != "--verify-desktop") {
            return None;
        }
        let source = if launch_files {
            crate::startup::paths(args.iter().skip(1).cloned())
                .into_iter()
                .next()?
        } else {
            PathBuf::from(args.get(2)?)
        };
        let secondary = args
            .iter()
            .position(|v| v == "--second-recording")
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from);
        let directory = if let Some(directory) = launch_directory {
            PathBuf::from(directory)
        } else {
            PathBuf::from(args.get(3)?)
        };
        std::fs::create_dir_all(&directory).ok()?;
        Some(Self {
            capture_hold: args.iter().any(|a| a == "--capture-hold"),
            capture_hdr: args.iter().any(|a| a == "--capture-hdr"),
            graphics_only: args.iter().any(|a| a == "--graphics-startup-only"),
            startup_only: args.iter().any(|a| a == "--startup-only"),
            vfr: source
                .file_name()
                .is_some_and(|name| name == "variable.mkv"),
            source,
            secondary,
            scrub_target: 1.6031,
            directory,
            phase: 0,
            started: Instant::now(),
            input: Vec::new(),
            checks: Vec::new(),
            failure: None,
            anchor: None,
            expected: None,
            layout: 0,
            focus_steps: 0,
            keyboard_position: 0.0,
            scale_layouts: args.iter().any(|a| a == "--scale-layouts"),
            scale: 0,
            was_software: false,
            recovery_lock: None,
            precision: None,
            launch_files,
            dropped_files: Vec::new(),
            recorded_phase: None,
            fail_native: args.iter().any(|a| a == "--fail-native-preview"),
        })
    }
    pub fn key(&mut self, key: egui::Key, command: bool) {
        let modifiers = egui::Modifiers {
            ctrl: command,
            command,
            ..Default::default()
        };
        for pressed in [true, false] {
            self.input.push(egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            });
        }
    }
    pub fn layout_scale(&self) -> f32 {
        if self.scale_layouts {
            [1.0, 1.25, 1.5, 1.75, 2.0][self.scale]
        } else {
            1.0
        }
    }
    pub fn layout_size(&self) -> egui::Vec2 {
        [
            egui::vec2(1000.0, 740.0),
            egui::vec2(1320.0, 880.0),
            egui::vec2(1800.0, 740.0),
            egui::vec2(1100.0, 1000.0),
        ][self.layout]
    }
    pub fn pointer(&mut self, pos: egui::Pos2, pressed: Option<bool>) {
        self.input.push(egui::Event::PointerMoved(pos));
        if let Some(pressed) = pressed {
            self.input.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
    }
    pub fn report(&self) {
        let value = serde_json::json!({"passed":self.failure.is_none(),"checks":self.checks,"failure":self.failure});
        let _ = std::fs::write(
            self.directory.join("desktop.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        );
    }
    pub fn screenshot(&self, image: &egui::ColorImage) -> std::io::Result<()> {
        use std::io::Write;
        let width = image.size[0] as u32;
        let height = image.size[1] as u32;
        let size = width * height * 4;
        let mut output = std::fs::File::create(self.directory.join("desktop.bmp"))?;
        output.write_all(b"BM")?;
        output.write_all(&(54 + size).to_le_bytes())?;
        output.write_all(&[0; 4])?;
        output.write_all(&54_u32.to_le_bytes())?;
        output.write_all(&40_u32.to_le_bytes())?;
        output.write_all(&width.to_le_bytes())?;
        output.write_all(&(-(height as i32)).to_le_bytes())?;
        output.write_all(&1_u16.to_le_bytes())?;
        output.write_all(&32_u16.to_le_bytes())?;
        output.write_all(&[0; 24])?;
        for pixel in &image.pixels {
            output.write_all(&[pixel.b(), pixel.g(), pixel.r(), 255])?;
        }
        Ok(())
    }
}
