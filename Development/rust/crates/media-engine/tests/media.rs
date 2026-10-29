use editor_core::{Clip, ClipId, FrameRate, SourceId};
use media_engine::*;
use project_store::*;
use std::path::{Path, PathBuf};

#[test]
#[ignore = "requires the pinned native bundle and generated media fixtures; run test-rust-media.ps1"]
fn generated_media_export_matrix() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").expect("fixture directory"));
    let tools = Tools::discover(root.join("cache")).unwrap();
    let cancel = uncancelled();
    let mut sources = Vec::new();
    let mut hashes = Vec::new();
    let sound = root.join("sdr-with-audio.mkv");
    run(
        &tools.ffmpeg,
        &["-v", "error", "-nostdin", "-y", "-i"]
            .map(str::to_owned)
            .into_iter()
            .chain([root.join("sdr.mkv").to_string_lossy().into_owned()])
            .chain(
                [
                    "-f",
                    "lavfi",
                    "-i",
                    "aevalsrc=0.6*sin(2*PI*440*t)|-0.6*sin(2*PI*440*t):s=48000:d=3",
                    "-map",
                    "0:v:0",
                    "-map",
                    "1:a:0",
                    "-c:v",
                    "copy",
                    "-c:a",
                    "pcm_f32le",
                ]
                .map(str::to_owned),
            )
            .chain([sound.to_string_lossy().into_owned()])
            .collect::<Vec<_>>(),
        &cancel,
        |_| {},
    )
    .unwrap();
    for (i, name) in ["sdr", "pq", "hlg"].iter().enumerate() {
        let path = if *name == "sdr" {
            sound.clone()
        } else {
            root.join(format!("{name}.mkv"))
        };
        hashes.push(file_hash(&path).unwrap());
        sources.push(tools.probe(&path, SourceId(i as u64 + 1), &cancel).unwrap());
    }
    let gpu = std::env::var_os("SVE_MEDIA_GPU").is_some();
    let directory = root.join(if gpu { "exports-gpu" } else { "exports-cpu" });
    std::fs::create_dir_all(&directory).unwrap();
    for (profile, codec) in [
        (ColorProfile::Sdr, Codec::H264),
        (ColorProfile::Sdr, Codec::Av1),
        (ColorProfile::Hdr10, Codec::Hevc),
        (ColorProfile::Hdr10, Codec::Av1),
    ] {
        let mut document = SavedProject {
            version: 2,
            application_version: "1.1.0".into(),
            clips: vec![
                Clip {
                    id: ClipId(1),
                    source: SourceId(2),
                    source_in: 0.2,
                    source_out: 1.2,
                    timeline_start: 0.0,
                },
                Clip {
                    id: ClipId(2),
                    source: SourceId(3),
                    source_in: 0.4,
                    source_out: 1.4,
                    timeline_start: 1.5,
                },
                Clip {
                    id: ClipId(3),
                    source: SourceId(1),
                    source_in: 0.3,
                    source_out: 1.3,
                    timeline_start: 2.5,
                },
            ],
            metadata: ProjectMetadata {
                sources: sources.clone(),
                output: OutputSettings {
                    width: 320,
                    height: 180,
                    rate: FrameRate::new(30, 1).unwrap(),
                    profile,
                    codec,
                    quality: 30,
                    gpu,
                },
                ..Default::default()
            },
        };
        document.metadata.adjustments.insert(
            ClipId(3),
            ClipSettings {
                gain_db: -6.0,
                fade_in: 0.1,
                fade_out: 0.2,
                ..Default::default()
            },
        );
        let path = directory.join(format!("{profile:?}-{codec:?}.mp4"));
        tools.export(&document, &path, &cancel, |_| {}).unwrap();
        let verified = tools.probe(&path, SourceId(9), &cancel).unwrap();
        assert!(
            (verified.duration - 3.5).abs() < 0.1,
            "duration {}",
            verified.duration
        );
        let count = run(
            &tools.ffprobe,
            &[
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-count_frames",
                "-show_entries",
                "stream=nb_read_frames",
                "-of",
                "csv=p=0",
            ]
            .map(str::to_owned)
            .into_iter()
            .chain([path.to_string_lossy().into_owned()])
            .collect::<Vec<_>>(),
            &cancel,
            |_| {},
        )
        .unwrap();
        assert_eq!(String::from_utf8(count).unwrap().trim(), "105");
        assert_eq!(verified.hdr(), profile == ColorProfile::Hdr10);
        assert!(!verified.audio.is_empty());
        let stats = run(
            &tools.ffmpeg,
            &["-v", "error", "-ss", "1.2", "-i"]
                .map(str::to_owned)
                .into_iter()
                .chain([path.to_string_lossy().into_owned()])
                .chain(
                    [
                        "-vf",
                        "signalstats,metadata=print:file=-",
                        "-an",
                        "-frames:v",
                        "1",
                        "-f",
                        "null",
                        "-",
                    ]
                    .map(str::to_owned),
                )
                .collect::<Vec<_>>(),
            &cancel,
            |_| {},
        )
        .unwrap();
        let text = String::from_utf8(stats).unwrap();
        let level = text
            .lines()
            .find_map(|l| l.strip_prefix("lavfi.signalstats.YAVG="))
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!(
            (level
                - if verified.pixel_format.contains("10") {
                    64.0
                } else {
                    16.0
                })
            .abs()
                < 3.0,
            "gap is not black: {text}"
        );
        for (position, audible) in [("1.15", false), ("2.7", true)] {
            let samples = directory.join("samples.f32");
            run(
                &tools.ffmpeg,
                &["-v", "error", "-nostdin", "-y", "-ss", position, "-i"]
                    .map(str::to_owned)
                    .into_iter()
                    .chain([path.to_string_lossy().into_owned()])
                    .chain(
                        [
                            "-t", "0.1", "-vn", "-ac", "2", "-ar", "48000", "-f", "f32le",
                        ]
                        .map(str::to_owned),
                    )
                    .chain([samples.to_string_lossy().into_owned()])
                    .collect::<Vec<_>>(),
                &cancel,
                |_| {},
            )
            .unwrap();
            let data = std::fs::read(samples).unwrap();
            let peak = data
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()).abs())
                .fold(0.0, f32::max);
            assert!(
                if audible {
                    peak > 0.25 && peak < 0.36
                } else {
                    peak < 0.01
                },
                "audio mismatch at {position}: {peak}"
            );
        }
        // Existing destination survives cancellation and original-file rejection.
        let sentinel = directory.join("keep.mp4");
        std::fs::write(&sentinel, b"previous output").unwrap();
        let stopped = uncancelled();
        stopped.store(true, std::sync::atomic::Ordering::Release);
        assert!(
            tools
                .export(&document, &sentinel, &stopped, |_| {})
                .is_err()
        );
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"previous output");
        assert!(
            tools
                .export(&document, Path::new(&sources[0].path), &cancel, |_| {})
                .is_err()
        );
        let roundtrip = directory.join("test.sveproject");
        save_document(&roundtrip, &document).unwrap();
        assert_eq!(tools.load_project(&roundtrip, &cancel).unwrap(), document);
        let mut missing = document.clone();
        let old = missing.metadata.sources[0].path.clone();
        let absent = root
            .join("moved/missing.mkv")
            .to_string_lossy()
            .into_owned();
        missing.metadata.sources[0].path = absent.clone();
        save_document(&roundtrip, &missing).unwrap();
        assert_eq!(
            tools
                .load_project_relinked(&roundtrip, &[(absent, PathBuf::from(old))], &cancel)
                .unwrap(),
            document
        );
        let interrupted = uncancelled();
        let interrupt_signal = interrupted.clone();
        assert!(
            tools
                .export(&document, &sentinel, &interrupted, |progress| {
                    if progress.fraction > 0.0 {
                        interrupt_signal.store(true, std::sync::atomic::Ordering::Release);
                    }
                })
                .is_err()
        );
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"previous output");
    }
    for (source, hash) in sources.iter().zip(hashes) {
        assert_eq!(file_hash(Path::new(&source.path)).unwrap(), hash);
    }
}
