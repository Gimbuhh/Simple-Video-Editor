use editor_core::{FrameRate, SourceId};
use media_engine::*;
use project_store::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn source() -> MediaSource {
    MediaSource {
        id: SourceId(1),
        path: "fixture.mp4".into(),
        duration: 2.0,
        width: 320,
        height: 180,
        display_width: 320.0,
        display_height: 180.0,
        rotation: 0.0,
        sample_aspect_ratio: 1.0,
        rate: FrameRate::new(30, 1).unwrap(),
        codec: "h264".into(),
        video_stream: 0,
        audio: vec![],
        transfer: "bt709".into(),
        primaries: "bt709".into(),
        matrix: "bt709".into(),
        range: "tv".into(),
        pixel_format: "yuv420p".into(),
        chroma_location: String::new(),
    }
}
#[test]
fn hdr_conversion_preserves_known_chroma_and_normalizes_missing_metadata() {
    let mut source = source();
    source.transfer = "smpte2084".into();
    source.primaries = "bt2020".into();
    source.matrix = "bt2020nc".into();
    let settings = ClipSettings::default();
    let output = OutputSettings::default();
    for (metadata, expected) in [
        ("", "left"),
        ("unknown", "left"),
        ("left", "left"),
        ("center", "center"),
        ("topleft", "topleft"),
    ] {
        source.chroma_location = metadata.into();
        let prefix = format!("setparams=chroma_location={expected},");
        assert!(
            preview_filters(&source, &settings, &output, SdrToneMapping::Adaptive)
                .starts_with(&prefix)
        );
        assert!(
            video_filters(&source, &settings, &output, false, SdrToneMapping::Adaptive)
                .starts_with(&prefix)
        );
        let converted = color_transform(&source, ColorProfile::Sdr, SdrToneMapping::Adaptive);
        assert!(converted.contains(&format!("chroma_location={expected}")));
        assert!(converted.contains("libplacebo="));
        assert!(
            !color_transform(&source, ColorProfile::Sdr, SdrToneMapping::Cpu)
                .contains("libplacebo=")
        );
        assert!(
            !color_transform(&source, ColorProfile::Hdr10, SdrToneMapping::Adaptive)
                .contains("libplacebo=")
        );
    }
    let mut document = serde_json::to_value(&source).unwrap();
    document.as_object_mut().unwrap().remove("chroma_location");
    let restored: MediaSource = serde_json::from_value(document).unwrap();
    assert!(
        restored.chroma_location.is_empty(),
        "Older project metadata must remain readable"
    );
}

#[test]
fn native_hdr_mapping_requires_supported_color_and_fitted_geometry() {
    let mut source = source();
    source.transfer = "smpte2084".into();
    source.primaries = "bt2020".into();
    source.matrix = "bt2020nc".into();
    let settings = ClipSettings::default();
    let output = OutputSettings::default();
    assert!(direct_hdr_sdr_preview(&source, &settings, &output));
    source.transfer = "arib-std-b67".into();
    assert!(direct_hdr_sdr_preview(&source, &settings, &output));
    source.transfer = "bt709".into();
    assert!(!direct_hdr_sdr_preview(&source, &settings, &output));
    source.transfer = "smpte2084".into();
    assert!(!direct_hdr_sdr_preview(
        &source,
        &settings,
        &OutputSettings {
            profile: ColorProfile::Hdr10,
            ..output.clone()
        }
    ));
    assert!(!direct_hdr_sdr_preview(
        &source,
        &ClipSettings {
            framing: Framing::Crop,
            ..settings.clone()
        },
        &output
    ));
    assert!(!direct_hdr_sdr_preview(
        &source,
        &ClipSettings {
            rotation: 90,
            ..settings.clone()
        },
        &output
    ));
    source.sample_aspect_ratio = 2.0;
    assert!(!direct_hdr_sdr_preview(&source, &settings, &output));
    source.sample_aspect_ratio = 1.0;
    source.matrix = "bt709".into();
    assert!(!direct_hdr_sdr_preview(&source, &settings, &output));
}

#[test]
fn complex_presentations_keep_the_shared_transform() {
    let source = source();
    let settings = ClipSettings::default();
    let output = OutputSettings::default();
    assert!(direct_preview(&source, &settings, &output));
    for field in [
        "transfer",
        "primaries",
        "matrix",
        "pixels",
        "rotation",
        "sar",
    ] {
        let mut changed = source.clone();
        match field {
            "transfer" => changed.transfer = "smpte2084".into(),
            "primaries" => changed.primaries = "bt2020".into(),
            "matrix" => changed.matrix = "smpte170m".into(),
            "pixels" => changed.pixel_format = "gbrp".into(),
            "rotation" => changed.rotation = 90.0,
            "sar" => changed.sample_aspect_ratio = 2.0,
            _ => unreachable!(),
        }
        assert!(!direct_preview(&changed, &settings, &output), "{field}");
        assert!(
            preview_mpv_filter(&changed, &settings, &output, SdrToneMapping::Cpu)
                .starts_with("lavfi=[")
        );
    }
    for framing in [Framing::Fill, Framing::Crop] {
        assert!(!direct_preview(
            &source,
            &ClipSettings {
                framing,
                ..settings.clone()
            },
            &output
        ));
    }
    assert!(!direct_preview(
        &source,
        &ClipSettings {
            rotation: 90,
            ..settings.clone()
        },
        &output
    ));
    assert!(!direct_preview(
        &source,
        &settings,
        &OutputSettings {
            profile: ColorProfile::Hdr10,
            ..output.clone()
        }
    ));
    assert!(!direct_preview(
        &source,
        &settings,
        &OutputSettings {
            height: 1920,
            ..output
        }
    ));
}
fn software_frame(player: &native_mpv::Player) -> image::RgbImage {
    let serial = player.refresh_frame();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        player.pump().unwrap();
        if let Some(frame) = player.take_frame()
            && frame.serial == serial
        {
            let mut rgb = image::RgbImage::new(frame.width as u32, frame.height as u32);
            for (y, row) in frame
                .pixels
                .chunks(frame.stride)
                .take(frame.height)
                .enumerate()
            {
                for (x, pixel) in row[..frame.width * 4].chunks_exact(4).enumerate() {
                    rgb.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgb([pixel[2], pixel[1], pixel[0]]),
                    );
                }
            }
            return rgb;
        }
        assert!(Instant::now() < deadline, "No SDR preview frame");
    }
}
fn compare(actual: &image::RgbImage, expected: &image::RgbImage) {
    for y in [45, 135] {
        for x in [20, 60, 100, 140, 180, 220, 260, 300] {
            let a = actual.get_pixel(x, y).0;
            let e = expected.get_pixel(x, y).0;
            for channel in 0..3 {
                assert!(
                    (i16::from(a[channel]) - i16::from(e[channel])).abs() < 8,
                    "SDR color/range changed at {x},{y}: {a:?} vs {e:?}"
                );
            }
        }
    }
}
#[test]
#[ignore = "requires pinned native tools; GPU mode checks direct hardware surfaces"]
fn direct_sdr_preview_preserves_colors_levels_and_exact_seeks() {
    let root =
        PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").unwrap()).join("direct-preview");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    let cancel = uncancelled();
    let chart = root.join("chart.png");
    image::RgbImage::from_fn(320, 180, |x, y| {
        image::Rgb(if y < 90 {
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]][(x / 80) as usize]
        } else {
            let level = [0, 16, 48, 80, 112, 176, 224, 255][(x / 40) as usize];
            [level, level, level]
        })
    })
    .save(&chart)
    .unwrap();
    let output = OutputSettings {
        width: 320,
        height: 180,
        ..Default::default()
    };
    for (range, label, pixels, encoder) in [
        ("limited", "tv", "yuv420p", "libx264"),
        ("full", "pc", "yuv420p", "libx264"),
        ("limited", "tv", "yuv420p10le", "libx265"),
        ("full", "pc", "yuv420p10le", "libx265"),
    ] {
        let path = root.join(format!("{label}-{pixels}.mp4"));
        let mut args: Vec<String> = ["-v", "error", "-nostdin", "-y", "-loop", "1", "-i"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        args.push(chart.to_string_lossy().into_owned());
        args.extend(["-t", "2", "-vf"].into_iter().map(str::to_owned));
        args.push(format!(
            "scale=320:180:in_range=full:out_range={range}:out_color_matrix=bt709,format={pixels}"
        ));
        args.extend(
            [
                "-c:v",
                encoder,
                "-preset",
                "ultrafast",
                "-crf",
                "12",
                "-color_range",
                label,
                "-color_trc",
                "bt709",
                "-color_primaries",
                "bt709",
                "-colorspace",
                "bt709",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        if encoder == "libx265" {
            args.extend(
                ["-x265-params", "pools=2:log-level=error"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        args.push(path.to_string_lossy().into_owned());
        run(&tools.ffmpeg, &args, &cancel, |_| {}).unwrap();
        let mut source = tools.probe(&path, SourceId(1), &cancel).unwrap();
        assert!(direct_preview(&source, &ClipSettings::default(), &output));
        let hash = file_hash(&path).unwrap();
        let mut reference = None;
        // Include untagged footage: the direct path must use the shared graph's
        // explicit Rec.709 defaults, rather than mpv's resolution heuristics.
        for variant in ["shared", "direct", "untagged"] {
            if variant == "untagged" {
                source.transfer = "unknown".into();
                source.primaries = "unknown".into();
                source.matrix = "unknown".into();
            }
            let p = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::SoftwareSdr).unwrap();
            p.resize_software(320, 180).unwrap();
            p.set(
                "vf",
                &if variant == "shared" {
                    format!(
                        "lavfi=[{}]",
                        tools.preview_filters(&source, &ClipSettings::default(), &output)
                    )
                } else {
                    tools.preview_mpv_filter(&source, &ClipSettings::default(), &output)
                },
            )
            .unwrap();
            p.load(&media_path::RecordingPath::authorize(&path).unwrap())
                .unwrap();
            p.seek(0.5).unwrap();
            let frame = software_frame(&p);
            frame
                .save(root.join(format!("{label}-{pixels}-{variant}.png")))
                .unwrap();
            if let Some(expected) = &reference {
                compare(&frame, expected);
            } else {
                reference = Some(frame);
            }
        }
        if std::env::var_os("SVE_MEDIA_GPU").is_some() {
            let p = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::NativeSdr).unwrap();
            p.set("hwdec", "auto").unwrap();
            p.set(
                "vf",
                &tools.preview_mpv_filter(&source, &ClipSettings::default(), &output),
            )
            .unwrap();
            p.load(&media_path::RecordingPath::authorize(Path::new(&source.path)).unwrap())
                .unwrap();
            p.seek(0.5).unwrap();
            assert_eq!(p.property("hwdec-current").as_deref(), Some("d3d11va"));
            let screenshot = root.join(format!("{label}-{pixels}-hardware.png"));
            p.command(&["screenshot-to-file", screenshot.to_str().unwrap(), "video"])
                .unwrap();
            compare(
                &image::open(screenshot).unwrap().into_rgb8(),
                reference.as_ref().unwrap(),
            );
            let position: f64 = p.property("time-pos").unwrap().parse().unwrap();
            assert!((position - 0.5).abs() < source.rate.frame_seconds() + 0.002);
        }
        assert_eq!(file_hash(&path).unwrap(), hash);
    }
}
