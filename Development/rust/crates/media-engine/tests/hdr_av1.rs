use editor_core::{Clip, ClipId, FrameRate, SourceId};
use media_engine::*;
use project_store::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn check_colors(image: &image::RgbImage) {
    let (w, h) = image.dimensions();
    for (index, channel) in [0, 1, 2].into_iter().enumerate() {
        let pixel = image.get_pixel(w * (index as u32 * 2 + 1) / 8, h / 4).0;
        assert!(pixel[channel] > 150, "Primary lost: {pixel:?}");
        for other in 0..3 {
            if other != channel {
                // Perceptual gamut mapping can mix secondary channels to
                // retain brightness. Require clear primary dominance rather
                // than treating that intentional compression as corruption.
                assert!(
                    pixel[channel] > pixel[other].saturating_add(100),
                    "Primary lost its hue: {pixel:?}"
                );
            }
        }
    }
    let white = image.get_pixel(w * 7 / 8, h / 4).0;
    assert!(*white.iter().min().unwrap() > 180);
    assert!(white.iter().max().unwrap() - white.iter().min().unwrap() < 8);
    let mut previous = 0u32;
    for index in 0..8 {
        let pixel = image.get_pixel(w * (index * 2 + 1) / 16, h * 3 / 4).0;
        assert!(
            pixel.iter().max().unwrap() - pixel.iter().min().unwrap() < 8,
            "Gray gained a color cast: {pixel:?}"
        );
        let level = pixel.into_iter().map(u32::from).sum::<u32>() / 3;
        assert!(
            level > previous + 5,
            "Gray ramp wrapped or flattened: {previous} -> {level}"
        );
        previous = level;
    }
}

#[test]
#[ignore = "requires the pinned native bundle; generates its own AV1/PQ color chart"]
fn full_and_limited_range_av1_pq_keep_colors_in_preview_thumbnail_and_export() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").unwrap()).join("hdr-av1");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    let cancel = uncancelled();
    let gpu = std::env::var_os("SVE_MEDIA_GPU").is_some();
    let chart_path = root.join("chart.png");
    // Exercise the reported capture's resolution/rate on NVIDIA hardware;
    // retain a small CPU fixture so the same color assertions stay cheap in CI.
    let (width, height, rate) = if gpu {
        (2560, 1440, "60000/1001")
    } else {
        (320, 180, "30")
    };
    let chart = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb(if y < height / 2 {
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]][(x * 4 / width) as usize]
        } else {
            let gray = [16, 48, 80, 112, 144, 176, 208, 240][(x * 8 / width) as usize];
            [gray, gray, gray]
        })
    });
    chart.save(&chart_path).unwrap();
    let output = OutputSettings {
        width: 320,
        height: 180,
        rate: FrameRate::new(30, 1).unwrap(),
        profile: ColorProfile::Sdr,
        codec: Codec::H264,
        gpu: false,
        quality: 14,
    };
    for (range, label) in [("full", "pc"), ("limited", "tv")] {
        let directory = root.join(format!("{label}-{}", if gpu { "gpu" } else { "cpu" }));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("pq-av1.mp4");
        let mut arguments: Vec<String> = [
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-loop",
            "1",
            "-framerate",
            rate,
            "-i",
        ]
        .map(str::to_owned)
        .to_vec();
        arguments.push(chart_path.to_string_lossy().into_owned());
        // A real HDR reference chart uses 203 cd/m² white, rather than an SDR
        // chart encoded into a PQ container with only 100 cd/m² headroom.
        arguments.extend(["-t".into(),"3".into(),"-vf".into(),format!("format=gbrpf32le,zscale=pin=bt709:tin=iec61966-2-1:min=gbr:rin=full:p=bt2020:t=smpte2084:m=bt2020nc:r={range}:npl=203,format=yuv420p10le")]);
        arguments.extend(["-c:v", if gpu { "av1_nvenc" } else { "libaom-av1" }].map(str::to_owned));
        arguments.extend(
            if gpu {
                vec!["-preset", "p1", "-rc", "constqp", "-qp", "14"]
            } else {
                vec!["-cpu-used", "8", "-threads", "2", "-crf", "14"]
            }
            .into_iter()
            .map(str::to_owned),
        );
        arguments.extend(
            [
                "-color_primaries",
                "bt2020",
                "-color_trc",
                "smpte2084",
                "-colorspace",
                "bt2020nc",
                "-color_range",
                label,
            ]
            .map(str::to_owned),
        );
        arguments.push(source_path.to_string_lossy().into_owned());
        run(&tools.ffmpeg, &arguments, &cancel, |_| {}).unwrap();
        let before = file_hash(&source_path).unwrap();
        let source = tools.probe(&source_path, SourceId(1), &cancel).unwrap();
        assert_eq!(source.codec, "av1");
        assert_eq!(source.transfer, "smpte2084");
        assert_eq!(source.range, label);
        assert!(source.pixel_format.contains("10"));

        let reference_path = directory.join("reference.bmp");
        let arguments = vec![
            "-v".into(),
            "error".into(),
            "-y".into(),
            "-i".into(),
            source.path.clone(),
            "-vf".into(),
            tools.video_filters(&source, &ClipSettings::default(), &output, false),
            "-frames:v".into(),
            "1".into(),
            reference_path.to_string_lossy().into_owned(),
        ];
        run(&tools.ffmpeg, &arguments, &cancel, |_| {}).unwrap();
        check_colors(&image::open(&reference_path).unwrap().into_rgb8());
        check_colors(
            &image::open(tools.thumbnail(&source, &cancel).unwrap())
                .unwrap()
                .into_rgb8(),
        );

        let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::SoftwareSdr).unwrap();
        player
            .set(
                "vf",
                &format!(
                    "lavfi=[{}]",
                    tools.preview_filters(&source, &ClipSettings::default(), &output)
                ),
            )
            .unwrap();
        player
            .command(&["loadfile", &source.path, "replace"])
            .unwrap();
        player
            .wait_for(10.0, |p| p.property("video-out-params/w").is_some())
            .unwrap();
        player.seek(0.5).unwrap();
        let serial = player.refresh_frame();
        let deadline = Instant::now() + Duration::from_secs(10);
        let frame = loop {
            player.pump().unwrap();
            if let Some(frame) = player.take_frame()
                && frame.serial == serial
            {
                break frame;
            }
            assert!(Instant::now() < deadline, "No post-seek preview frame");
        };
        let preview = image::RgbImage::from_fn(frame.width as u32, frame.height as u32, |x, y| {
            let offset = y as usize * frame.stride + x as usize * 4;
            image::Rgb([
                frame.pixels[offset + 2],
                frame.pixels[offset + 1],
                frame.pixels[offset],
            ])
        });
        check_colors(&preview);
        preview.save(directory.join("preview.bmp")).unwrap();
        drop(player);
        if gpu {
            let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::NativeSdr).unwrap();
            player.set("hwdec", "auto-copy").unwrap();
            player
                .set(
                    "vf",
                    &format!(
                        "lavfi=[{}]",
                        tools.preview_filters(&source, &ClipSettings::default(), &output)
                    ),
                )
                .unwrap();
            player
                .load(
                    &media_path::RecordingPath::authorize(std::path::Path::new(&source.path))
                        .unwrap(),
                )
                .unwrap();
            player.seek(0.5).unwrap();
            let decoder = player.property("hwdec-current").unwrap();
            assert_ne!(
                decoder, "no",
                "AV1 hardware-copy preview did not use the requested decoder"
            );
            let screenshot = directory.join("hardware-copy-preview.png");
            player
                .command(&["screenshot-to-file", screenshot.to_str().unwrap(), "video"])
                .unwrap();
            check_colors(&image::open(screenshot).unwrap().into_rgb8());
            std::fs::write(directory.join("hardware-copy-decoder.txt"), decoder).unwrap();
        }

        for profile in [ColorProfile::Sdr, ColorProfile::Hdr10] {
            let settings = OutputSettings {
                profile,
                codec: if profile == ColorProfile::Hdr10 {
                    Codec::Av1
                } else {
                    Codec::H264
                },
                gpu,
                ..output.clone()
            };
            let document = SavedProject {
                version: 2,
                application_version: APPLICATION_VERSION.into(),
                clips: vec![Clip {
                    id: ClipId(1),
                    source: source.id,
                    source_in: 0.0,
                    source_out: 0.5,
                    timeline_start: 0.0,
                }],
                metadata: ProjectMetadata {
                    sources: vec![source.clone()],
                    output: settings,
                    ..Default::default()
                },
            };
            let exported = directory.join(format!("export-{profile:?}.mp4"));
            tools.export(&document, &exported, &cancel, |_| {}).unwrap();
            let exported_source = tools.probe(&exported, SourceId(1), &cancel).unwrap();
            if profile == ColorProfile::Hdr10 {
                assert_eq!(exported_source.transfer, "smpte2084");
                assert!(exported_source.pixel_format.contains("10"));
            }
            let decoded = directory.join(format!("decoded-{profile:?}.bmp"));
            let arguments = vec![
                "-v".into(),
                "error".into(),
                "-y".into(),
                "-i".into(),
                exported.to_string_lossy().into_owned(),
                "-vf".into(),
                tools.video_filters(&exported_source, &ClipSettings::default(), &output, false),
                "-frames:v".into(),
                "1".into(),
                decoded.to_string_lossy().into_owned(),
            ];
            run(&tools.ffmpeg, &arguments, &cancel, |_| {}).unwrap();
            check_colors(&image::open(decoded).unwrap().into_rgb8());
        }
        assert_eq!(
            file_hash(&source_path).unwrap(),
            before,
            "Recording changed"
        );
    }
}
