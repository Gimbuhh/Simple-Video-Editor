use editor_core::{Clip, ClipId, FrameRate, SourceId};
use media_engine::*;
use project_store::*;
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const NITS: [f64; 12] = [
    0.0, 1.0, 5.0, 18.0, 50.0, 100.0, 203.0, 400.0, 1000.0, 2000.0, 4000.0, 10000.0,
];
const W: u32 = 384;
const H: u32 = 96;

// ST 2084 reference EOTF inverse, independent of the conversion under test.
fn pq(nits: f64) -> f64 {
    let y = (nits / 10000.0).powf(2610.0 / 16384.0);
    ((3424.0 / 4096.0 + 2413.0 / 128.0 * y) / (1.0 + 2392.0 / 128.0 * y)).powf(2523.0 / 32.0)
}
fn check_exposure(image: &image::RgbImage) {
    let (w, h) = image.dimensions();
    let levels: Vec<_> = (0..12)
        .map(|i| {
            let p = image.get_pixel(w * (i * 2 + 1) / 24, h / 2).0;
            assert!(
                p.iter().max().unwrap() - p.iter().min().unwrap() <= 3,
                "Neutral gained a cast: {p:?}"
            );
            u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])
        })
        .map(|v| v as f64 / 3.0)
        .collect();
    assert!(levels[0] < 3.0, "Raised black: {levels:?}");
    // 18/203 linear light is about code 93 in BT.1886, or 84 in sRGB.
    // Both are distinct from the previous 100-nit normalization (code 124).
    assert!(
        (80.0..=105.0).contains(&levels[3]),
        "18-nit midtone was lifted or crushed: {levels:?}"
    );
    assert!(
        (170.0..=195.0).contains(&levels[5]),
        "100-nit patch lost its brightness relationship: {levels:?}"
    );
    assert!(
        (185.0..=225.0).contains(&levels[6]),
        "203-nit reference white lost highlight headroom: {levels:?}"
    );
    assert!(
        levels[8] < 252.0 && levels[10] < 255.0,
        "HDR highlights clipped before the signal ceiling: {levels:?}"
    );
    assert!(
        levels.windows(2).all(|p| p[1] > p[0]),
        "Highlight/shadow steps flattened: {levels:?}"
    );
}
fn export_frame(tools: &Tools, path: &Path, out: &Path) {
    run(
        &tools.ffmpeg,
        &["-v", "error", "-nostdin", "-y", "-ss", "0.5", "-i"]
            .map(str::to_owned)
            .into_iter()
            .chain([path.to_string_lossy().into_owned()])
            .chain(["-frames:v", "1"].map(str::to_owned))
            .chain([out.to_string_lossy().into_owned()])
            .collect::<Vec<_>>(),
        &uncancelled(),
        |_| {},
    )
    .unwrap();
}

#[test]
#[ignore = "requires pinned native bundle; generates absolute-luminance PQ fixtures"]
fn hdr_reference_white_midtones_and_highlights_survive_sdr_conversion() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").expect("fixtures"))
        .join("hdr-exposure");
    std::fs::create_dir_all(&root).unwrap();
    let mut tools = Tools::discover(root.join("cache")).unwrap();
    tools.tone_mapping = SdrToneMapping::Cpu;
    let cancel = uncancelled();
    let gpu = std::env::var_os("SVE_MEDIA_GPU").is_some();
    for (range, label) in [("full", "pc"), ("limited", "tv")] {
        let dir = root.join(format!("{label}-{}", if gpu { "gpu" } else { "cpu" }));
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("pq.yuv");
        let mut file = std::fs::File::create(&raw).unwrap();
        for _ in 0..H {
            for x in 0..W {
                let e = pq(NITS[(x / 32) as usize]);
                let value = if range == "full" {
                    e * 1023.0
                } else {
                    64.0 + e * 876.0
                };
                file.write_all(&(value.round() as u16).to_le_bytes())
                    .unwrap();
            }
        }
        for _ in 0..W * H / 2 {
            file.write_all(&512_u16.to_le_bytes()).unwrap();
        }
        drop(file);
        let source_path = dir.join("pq.mp4");
        let mut args = [
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-stream_loop",
            "-1",
            "-f",
            "rawvideo",
            "-pixel_format",
            "yuv420p10le",
            "-video_size",
            "384x96",
            "-framerate",
            "30",
            "-i",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(raw.to_string_lossy().into_owned());
        args.extend(["-vf".into(),format!("setparams=range={range}:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc")]);
        args.extend(
            [
                "-t",
                "1",
                "-c:v",
                if gpu { "av1_nvenc" } else { "libaom-av1" },
            ]
            .map(str::to_owned),
        );
        args.extend(
            if gpu {
                vec!["-preset", "p1", "-rc", "constqp", "-qp", "8"]
            } else {
                vec!["-cpu-used", "8", "-crf", "8", "-threads", "2"]
            }
            .into_iter()
            .map(str::to_owned),
        );
        args.extend(
            [
                "-color_range",
                label,
                "-color_trc",
                "smpte2084",
                "-color_primaries",
                "bt2020",
                "-colorspace",
                "bt2020nc",
            ]
            .map(str::to_owned),
        );
        args.push(source_path.to_string_lossy().into_owned());
        run(&tools.ffmpeg, &args, &cancel, |_| {}).unwrap();
        let source = tools.probe(&source_path, SourceId(1), &cancel).unwrap();
        let original = file_hash(&source_path).unwrap();
        assert_eq!(source.range, label);
        assert_eq!(source.transfer, "smpte2084");
        assert_eq!(source.primaries, "bt2020");
        let output = OutputSettings {
            width: W,
            height: H,
            rate: FrameRate::new(30, 1).unwrap(),
            codec: Codec::H264,
            gpu: false,
            ..Default::default()
        };
        let reference = dir.join("converted.png");
        run(
            &tools.ffmpeg,
            &[
                "-v".into(),
                "error".into(),
                "-nostdin".into(),
                "-y".into(),
                "-i".into(),
                source.path.clone(),
                "-vf".into(),
                tools.video_filters(&source, &Default::default(), &output, false),
                "-frames:v".into(),
                "1".into(),
                reference.to_string_lossy().into_owned(),
            ],
            &cancel,
            |_| {},
        )
        .unwrap();
        check_exposure(&image::open(&reference).unwrap().into_rgb8());
        check_exposure(
            &image::open(tools.thumbnail_at(&source, 0.5, &cancel).unwrap())
                .unwrap()
                .into_rgb8(),
        );
        let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::SoftwareSdr).unwrap();
        player.resize_software(W as usize, H as usize).unwrap();
        player
            .set(
                "vf",
                &tools.preview_mpv_filter(&source, &Default::default(), &output),
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
            assert!(Instant::now() < deadline, "No preview");
        };
        let preview = image::RgbImage::from_fn(frame.width as u32, frame.height as u32, |x, y| {
            let i = y as usize * frame.stride + x as usize * 4;
            image::Rgb([frame.pixels[i + 2], frame.pixels[i + 1], frame.pixels[i]])
        });
        check_exposure(&preview);
        preview.save(dir.join("software.png")).unwrap();
        drop(player);
        if gpu {
            let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::NativeSdr).unwrap();
            player.set("hwdec", "auto-copy").unwrap();
            player
                .set(
                    "vf",
                    &tools.preview_mpv_filter(&source, &Default::default(), &output),
                )
                .unwrap();
            player
                .load(&media_path::RecordingPath::authorize(&source_path).unwrap())
                .unwrap();
            player.seek(0.5).unwrap();
            let capture = dir.join("native.png");
            player
                .command(&["screenshot-to-file", capture.to_str().unwrap(), "window"])
                .unwrap();
            check_exposure(&image::open(capture).unwrap().into_rgb8());
        }
        for profile in [ColorProfile::Sdr, ColorProfile::Hdr10] {
            let document = SavedProject {
                version: 2,
                application_version: APPLICATION_VERSION.into(),
                clips: vec![Clip {
                    id: ClipId(1),
                    source: source.id,
                    source_in: 0.0,
                    source_out: 1.0,
                    timeline_start: 0.0,
                }],
                metadata: ProjectMetadata {
                    sources: vec![source.clone()],
                    output: OutputSettings {
                        profile,
                        codec: if profile == ColorProfile::Sdr {
                            Codec::H264
                        } else {
                            Codec::Hevc
                        },
                        gpu,
                        ..output.clone()
                    },
                    ..Default::default()
                },
            };
            let exported = dir.join(format!("{profile:?}.mp4"));
            tools.export(&document, &exported, &cancel, |_| {}).unwrap();
            if profile == ColorProfile::Sdr {
                let capture = dir.join("export.png");
                export_frame(&tools, &exported, &capture);
                check_exposure(&image::open(capture).unwrap().into_rgb8());
            } else {
                // Decode PQ back to absolute linear luminance. SDR calibration
                // must never change HDR-to-HDR exposure.
                let bytes = run(
                    &tools.ffmpeg,
                    &[
                        "-v".into(),
                        "error".into(),
                        "-ss".into(),
                        "0.5".into(),
                        "-i".into(),
                        exported.to_string_lossy().into_owned(),
                        "-vf".into(),
                        "zscale=t=linear:npl=10000,format=gbrpf32le".into(),
                        "-frames:v".into(),
                        "1".into(),
                        "-f".into(),
                        "rawvideo".into(),
                        "-".into(),
                    ],
                    &cancel,
                    |_| {},
                )
                .unwrap();
                for (patch, expected) in [(3, 18.0), (6, 203.0), (8, 1000.0)] {
                    let offset = ((H / 2 * W + patch * 32 + 16) * 4) as usize;
                    let nits = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
                        as f64
                        * 10000.0;
                    assert!(
                        (nits - expected).abs() < expected * 0.12 + 1.0,
                        "HDR luminance changed: {nits} vs {expected}"
                    );
                }
            }
        }
        assert_eq!(file_hash(&source_path).unwrap(), original);
    }
}
