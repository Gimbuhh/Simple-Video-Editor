//! Disposable rendering experiment. Not the application, and never an HDR
//! certification: native output reports still require physical display tests.
use native_mpv::{Mode, Player, Result};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let benchmark = args.len() == 4 && args[3] == "--benchmark";
    if args.len() != 3 && !benchmark {
        return Err("Usage: sve-playback-lab <software|native-sdr|native-auto> <absolute-libmpv.dll> <absolute-recording> [--benchmark]".into());
    }
    let mode = match args[0].to_str() {
        Some("software") => Mode::SoftwareSdr,
        Some("native-sdr") => Mode::NativeSdr,
        Some("native-auto") => Mode::NativeAuto,
        _ => return Err("Unknown rendering mode".into()),
    };
    let recording = media_path::RecordingPath::authorize(Path::new(&args[2]))?;
    let mut cycles = Vec::new();
    // Repeated create/load/play/seek/drop exercises real ownership and cleanup.
    for _ in 0..3 {
        let load_started = Instant::now();
        let player = Player::open(Path::new(&args[1]), mode)?;
        player.load(&recording)?;
        let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;
        // A second same-source load must wait for its own file-loaded event,
        // not return using the previous recording's still-readable properties.
        player.load(&recording)?;
        let started = Instant::now();
        player.set_paused(false)?;
        player.wait_for(5.0, |p| {
            p.property("time-pos")
                .and_then(|s| s.parse::<f64>().ok())
                .is_some_and(|t| t >= 0.2)
        })?;
        player.set_paused(true)?;
        let duration: f64 = player
            .property("duration")
            .ok_or("Missing duration")?
            .parse()?;
        for time in [f64::NAN, f64::INFINITY, -1.0, duration] {
            if player.seek(time).is_ok() {
                return Err("Invalid seek was accepted".into());
            }
        }
        let mut seek_ms = Vec::new();
        let seek_targets = if benchmark {
            [0.82, 0.07, 0.62, 0.22, 0.91, 0.12, 0.73, 0.37]
                .map(|fraction| fraction * duration)
                .to_vec()
        } else {
            vec![1.0, 0.25, 1.5, 0.5]
        };
        for &time in &seek_targets {
            let seek_start = Instant::now();
            player.seek(time)?;
            seek_ms.push(seek_start.elapsed().as_secs_f64() * 1000.0);
            if player.property("pause").as_deref() != Some("yes") {
                return Err("Seeking changed pause intent".into());
            }
        }
        let mut frame_summary = None;
        if mode == Mode::SoftwareSdr {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                player.pump()?;
                if let Some(frame) = player.take_frame() {
                    let colored_pixels = frame
                        .pixels
                        .chunks_exact(4)
                        .filter(|pixel| pixel[..3].iter().any(|v| *v > 16))
                        .count();
                    if colored_pixels == 0 {
                        continue;
                    }
                    frame_summary = Some(
                        serde_json::json!({"width":frame.width, "height":frame.height,
                        "stride":frame.stride, "nonBlackPixels":colored_pixels}),
                    );
                    break;
                }
            }
            if frame_summary.is_none() {
                return Err("Software renderer produced no non-black frame".into());
            }
        } else {
            player.wait_for(5.0, |p| p.property("video-target-params").is_some())?;
        }
        let mut report = serde_json::Map::new();
        for name in [
            "mpv-version",
            "ffmpeg-version",
            "current-vo",
            "current-gpu-context",
            "video-params",
            "video-target-params",
            "display-names",
            "display-fps",
            "estimated-display-fps",
            "options/target-colorspace-hint",
            "options/target-trc",
            "options/target-prim",
            "options/d3d11-output-format",
            "hwdec-current",
            "pause",
            "time-pos",
        ] {
            report.insert(
                name.into(),
                player
                    .property(name)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        cycles.push(serde_json::json!({"mode":mode.label(), "elapsedMs":started.elapsed().as_secs_f64() * 1000.0,
            "openAndFirstLoadMs":load_ms,"seekTargetsSeconds":seek_targets,"seekMs":seek_ms, "softwareFrame":frame_summary, "properties":report}));
        // Quit while actively playing to exercise teardown without a stop bridge.
        player.set_paused(false)?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"cycles":cycles,
        "benchmark":benchmark,"filesystemCache":"uncontrolled","hdrPresentationVerified":false, "captureCompatibilityVerified":false, "toolkitSelected":false}))?
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Playback lab failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
