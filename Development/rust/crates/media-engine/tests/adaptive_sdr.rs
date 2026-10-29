use editor_core::{Clip, ClipId, FrameRate, SourceId};
use image::RgbImage;
use media_engine::*;
use project_store::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn compare(actual: &RgbImage, reference: &RgbImage, limit: f64) -> f64 {
    assert_eq!(actual.dimensions(), reference.dimensions());
    let error = actual
        .as_raw()
        .iter()
        .zip(reference.as_raw())
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / actual.as_raw().len() as f64;
    assert!(
        error <= limit,
        "SDR reference mismatch: mean channel error {error:.3} > {limit}"
    );
    error
}
fn frame(tools: &Tools, path: &Path, time: f64, filter: Option<String>, out: &Path) -> RgbImage {
    let mut args = vec![
        "-v".into(),
        "error".into(),
        "-nostdin".into(),
        "-y".into(),
        "-ss".into(),
        format!("{time:.9}"),
        "-i".into(),
        path.to_string_lossy().into_owned(),
    ];
    if let Some(filter) = filter {
        args.extend(["-vf".into(), filter]);
    }
    args.extend([
        "-frames:v".into(),
        "1".into(),
        out.to_string_lossy().into_owned(),
    ]);
    run(&tools.ffmpeg, &args, &uncancelled(), |_| {}).unwrap();
    image::open(out).unwrap().into_rgb8()
}
fn software_frame(player: &native_mpv::Player) -> RgbImage {
    let serial = player.refresh_frame();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        player.pump().unwrap();
        if let Some(f) = player.take_frame()
            && f.serial == serial
        {
            return RgbImage::from_fn(f.width as u32, f.height as u32, |x, y| {
                let i = y as usize * f.stride + x as usize * 4;
                image::Rgb([f.pixels[i + 2], f.pixels[i + 1], f.pixels[i]])
            });
        }
        assert!(Instant::now() < deadline, "No settled software frame");
    }
}

fn reference_white(image: &RgbImage) -> f64 {
    let (w, h) = image.dimensions();
    let mut values = Vec::new();
    for y in h / 4..3 * h / 4 {
        for x in w / 16..3 * w / 16 {
            values.extend(image.get_pixel(x, y).0);
        }
    }
    values.sort_unstable();
    f64::from(values[values.len() / 2])
}

#[test]
#[ignore = "requires pinned native bundle and NVIDIA; run test-rust-media.ps1 -Gpu"]
fn tiny_hdr_cursor_does_not_dim_the_entire_sdr_frame() {
    if std::env::var_os("SVE_MEDIA_GPU").is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").expect("fixtures"))
        .join("tiny-hdr-cursor");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    assert_eq!(tools.hdr_sdr_tone_mapping(), SdrToneMapping::Adaptive);
    let mut measurements = Vec::new();
    for cursor in [false, true] {
        // Independent absolute-light fixture: normal 170-nit UI white and a
        // 5,000-nit cursor occupying less than 0.01% of the frame. The cursor
        // must not reduce every other pixel's exposure during SDR conversion.
        let mut raw = Vec::with_capacity(1280 * 720 * 12);
        for _ in 0..3 {
            for y in 0..720 {
                for x in 0..1280 {
                    let nits: f32 = if cursor && (640..646).contains(&x) && (350..360).contains(&y)
                    {
                        5000.0
                    } else {
                        [170.0, 50.0, 18.0, 2.0][x / 320]
                    };
                    raw.extend(nits.to_le_bytes());
                }
            }
        }
        let input = root.join(format!("linear-{cursor}.gbrpf32le"));
        std::fs::write(&input, raw).unwrap();
        let path = root.join(format!("cursor-{cursor}.mp4"));
        let args = vec![
            "-v".into(), "error".into(), "-nostdin".into(), "-y".into(),
            "-stream_loop".into(), "-1".into(), "-f".into(), "rawvideo".into(),
            "-pixel_format".into(), "gbrpf32le".into(), "-video_size".into(), "1280x720".into(),
            "-framerate".into(), "30".into(), "-i".into(), input.to_string_lossy().into_owned(),
            "-t".into(), "1".into(), "-vf".into(),
            "zscale=pin=bt2020:tin=linear:min=gbr:rin=full:p=bt2020:t=smpte2084:m=bt2020nc:r=full:npl=1,format=yuv420p10le,setparams=range=full:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc".into(),
            "-c:v".into(), "av1_nvenc".into(), "-preset".into(), "p1".into(),
            "-rc".into(), "constqp".into(), "-qp".into(), "8".into(), path.to_string_lossy().into_owned(),
        ];
        run(&tools.ffmpeg, &args, &uncancelled(), |_| {}).unwrap();
        let hash = file_hash(&path).unwrap();
        let source = tools.probe(&path, SourceId(1), &uncancelled()).unwrap();
        let output = OutputSettings {
            width: 960,
            height: 540,
            gpu: false,
            ..Default::default()
        };
        let converted = frame(
            &tools,
            &path,
            0.3,
            Some(tools.video_filters(&source, &Default::default(), &output, false)),
            &root.join(format!("converted-{cursor}.png")),
        );
        let mut values = vec![reference_white(&converted)];
        for mode in [native_mpv::Mode::SoftwareSdr, native_mpv::Mode::NativeSdr] {
            let p = native_mpv::Player::open(&tools.mpv, mode).unwrap();
            if mode == native_mpv::Mode::SoftwareSdr {
                p.resize_software(960, 540).unwrap();
            } else {
                p.adaptive_sdr(true).unwrap();
                p.set("hwdec", "auto").unwrap();
            }
            let vf = if mode == native_mpv::Mode::SoftwareSdr {
                tools.preview_mpv_filter(&source, &Default::default(), &output)
            } else {
                tools.native_preview_mpv_filter(&source, &Default::default(), &output)
            };
            p.set("vf", &vf).unwrap();
            p.command(&["loadfile", source.path.as_str(), "replace"])
                .unwrap();
            p.wait_for(15.0, |p| p.property("video-out-params/w").is_some())
                .unwrap();
            p.seek(0.3).unwrap();
            let image = if mode == native_mpv::Mode::SoftwareSdr {
                software_frame(&p)
            } else {
                let screenshot = root.join(format!("native-{cursor}.png"));
                p.command(&["screenshot-to-file", screenshot.to_str().unwrap(), "window"])
                    .unwrap();
                image::open(screenshot).unwrap().into_rgb8()
            };
            values.push(reference_white(&image));
        }
        let thumbnail = image::open(tools.thumbnail_at(&source, 0.3, &uncancelled()).unwrap())
            .unwrap()
            .into_rgb8();
        values.push(reference_white(&thumbnail));
        let exported = root.join(format!("export-{cursor}.mp4"));
        tools
            .export(
                &SavedProject {
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
                        sources: vec![source],
                        output,
                        ..Default::default()
                    },
                },
                &exported,
                &uncancelled(),
                |_| {},
            )
            .unwrap();
        values.push(reference_white(&frame(
            &tools,
            &exported,
            0.0,
            None,
            &root.join(format!("exported-{cursor}.png")),
        )));
        assert_eq!(file_hash(&path).unwrap(), hash);
        eprintln!("cursor={cursor}: converted/software/native/thumbnail/export white={values:?}");
        assert!(
            values.iter().all(|v| *v >= 215.0),
            "Reference white becomes gray: {values:?}"
        );
        measurements.push(values);
    }
    for (baseline, cursor) in measurements[0].iter().zip(&measurements[1]) {
        assert!(
            (baseline - cursor).abs() <= 8.0,
            "A tiny cursor changes the rest of the image: {baseline} -> {cursor}"
        );
    }
    std::fs::write(
        root.join("white-levels.json"),
        serde_json::to_vec_pretty(&measurements).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires pinned native bundle; adaptive reference checks require -Gpu"]
fn adaptive_sdr_matches_independent_reference_in_preview_and_export() {
    if std::env::var_os("SVE_MEDIA_GPU").is_none() {
        eprintln!("Adaptive SDR reference requires -Gpu; CPU fallback has its own HDR regression.");
        return;
    }
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").expect("fixtures"))
        .join("adaptive-sdr");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    assert_eq!(
        tools.hdr_sdr_tone_mapping(),
        SdrToneMapping::Adaptive,
        "Adaptive processor unavailable"
    );
    let cancel = uncancelled();
    let original = std::env::var_os("SVE_HDR_SDR_SOURCE").map(PathBuf::from);
    let mut cases = Vec::new();
    if let Some(path) = original {
        let time = std::env::var("SVE_HDR_SDR_TIME")
            .unwrap_or("0".into())
            .parse::<f64>()
            .unwrap();
        cases.push((path, time, "recording".to_owned()));
    } else {
        let chart = root.join("chart.png");
        RgbImage::from_fn(320, 180, |x, y| {
            image::Rgb(if y < 90 {
                [[230, 30, 30], [30, 230, 30], [30, 30, 230], [240, 240, 240]][(x / 80) as usize]
            } else {
                let v = (x * 255 / 319) as u8;
                [v, v, v]
            })
        })
        .save(&chart)
        .unwrap();
        for range in ["full", "limited"] {
            for (nits, transfer) in [
                (203, "smpte2084"),
                (1000, "smpte2084"),
                (1000, "arib-std-b67"),
            ] {
                let label = format!("{range}-{nits}-{transfer}");
                let path = root.join(format!("{label}.mp4"));
                run(&tools.ffmpeg,&vec!["-v".into(),"error".into(),"-nostdin".into(),"-y".into(),"-loop".into(),"1".into(),"-i".into(),chart.to_string_lossy().into_owned(),"-t".into(),"1".into(),"-vf".into(),format!("format=gbrpf32le,zscale=pin=bt709:tin=iec61966-2-1:min=gbr:rin=full:p=bt2020:t={transfer}:m=bt2020nc:r={range}:npl={nits},format=yuv420p10le,setparams=range={range}:color_primaries=bt2020:color_trc={transfer}:colorspace=bt2020nc"),"-c:v".into(),"av1_nvenc".into(),"-preset".into(),"p1".into(),"-rc".into(),"constqp".into(),"-qp".into(),"8".into(),path.to_string_lossy().into_owned()],&cancel,|_|{}).unwrap();
                cases.push((path, 0.0, label));
            }
        }
    }
    let mut reports = Vec::new();
    for (path, start, label) in cases {
        let dir = root.join(&label);
        std::fs::create_dir_all(&dir).unwrap();
        let hash = file_hash(&path).unwrap();
        let source = tools.probe(&path, SourceId(1), &cancel).unwrap();
        assert!(source.hdr());
        let output = OutputSettings {
            width: 960,
            height: 540,
            rate: FrameRate::new(30, 1).unwrap(),
            quality: 14,
            gpu: false,
            codec: Codec::H264,
            ..Default::default()
        };
        // Independent, literal libplacebo reference, matching the user-approved
        // renderer. It deliberately does not call the production graph builder.
        let chroma = match source.chroma_location.as_str() {
            "left" | "center" | "topleft" | "top" | "bottomleft" | "bottom" => {
                source.chroma_location.as_str()
            }
            _ => "left",
        };
        let reference_filter = format!(
            "setparams=chroma_location={chroma},libplacebo=w=960:h=540:colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:format=yuv420p:tonemapping=auto:peak_detect=1:percentile=99.99:smoothing_period=0:extra_opts=allow_delayed_peak=no"
        );
        let reference = frame(
            &tools,
            &path,
            start,
            Some(reference_filter.clone()),
            &dir.join("reference.png"),
        );
        let converted = frame(
            &tools,
            &path,
            start,
            Some(tools.video_filters(&source, &Default::default(), &output, false)),
            &dir.join("converted.png"),
        );
        let converted_error = compare(&converted, &reference, 3.5);
        let mut errors = Vec::new();
        for mode in [native_mpv::Mode::SoftwareSdr, native_mpv::Mode::NativeSdr] {
            let p = native_mpv::Player::open(&tools.mpv, mode).unwrap();
            if mode == native_mpv::Mode::SoftwareSdr {
                p.resize_software(960, 540).unwrap();
            }
            if mode == native_mpv::Mode::NativeSdr {
                p.adaptive_sdr(true).unwrap();
                p.set("target-trc", "srgb").unwrap();
                p.set("hwdec", "auto").unwrap();
            }
            p.set(
                "vf",
                &if mode == native_mpv::Mode::NativeSdr {
                    tools.native_preview_mpv_filter(&source, &Default::default(), &output)
                } else {
                    tools.preview_mpv_filter(&source, &Default::default(), &output)
                },
            )
            .unwrap();
            p.set("start", &start.to_string()).unwrap();
            p.command(&["loadfile", source.path.as_str(), "replace"])
                .unwrap();
            p.wait_for(15.0, |p| p.property("video-out-params/w").is_some())
                .unwrap();
            if mode == native_mpv::Mode::SoftwareSdr {
                assert_eq!(
                    p.property("video-out-params/gamma").as_deref(),
                    Some("bt.1886")
                );
            }
            p.seek(start).unwrap();
            let pts = p
                .property("time-pos")
                .and_then(|s| s.parse::<f64>().ok())
                .expect("Presented frame timestamp");
            eprintln!("{label} {mode:?} requested={start:.9} presented={pts:.9}");
            let matching = frame(
                &tools,
                &path,
                (pts - 0.000001).max(0.0),
                Some(if mode == native_mpv::Mode::NativeSdr {
                    reference_filter.replace("color_trc=bt709", "color_trc=iec61966-2-1")
                } else {
                    reference_filter.clone()
                }),
                &dir.join(format!("reference-{mode:?}.png")),
            );
            let captured = dir.join(format!("{mode:?}.png"));
            let im = if mode == native_mpv::Mode::SoftwareSdr {
                let im = software_frame(&p);
                im.save(&captured).unwrap();
                im
            } else {
                p.command(&["screenshot-to-file", captured.to_str().unwrap(), "window"])
                    .unwrap();
                image::open(&captured).unwrap().into_rgb8()
            };
            errors.push((format!("{mode:?}"), compare(&im, &matching, 4.0)));
            // Seeking away and back must not inherit the other scene's peak.
            p.seek((start + 0.25).min(source.duration - 0.05)).unwrap();
            p.seek(start).unwrap();
            if mode == native_mpv::Mode::SoftwareSdr {
                compare(&software_frame(&p), &im, 1.0);
            } else {
                p.command(&["screenshot-to-file", captured.to_str().unwrap(), "window"])
                    .unwrap();
                compare(&image::open(&captured).unwrap().into_rgb8(), &im, 1.0);
            }
        }
        for (gpu, codec) in [
            (false, Codec::H264),
            (true, Codec::H264),
            (true, Codec::Av1),
        ] {
            let exported = dir.join(format!("export-{gpu}-{codec:?}.mp4"));
            let document = SavedProject {
                version: 2,
                application_version: APPLICATION_VERSION.into(),
                clips: vec![Clip {
                    id: ClipId(1),
                    source: source.id,
                    source_in: start,
                    source_out: start + 0.5,
                    timeline_start: 0.0,
                }],
                metadata: ProjectMetadata {
                    sources: vec![source.clone()],
                    output: OutputSettings {
                        gpu,
                        codec,
                        ..output.clone()
                    },
                    ..Default::default()
                },
            };
            tools.export(&document, &exported, &cancel, |_| {}).unwrap();
            let probe = tools.probe(&exported, SourceId(2), &cancel).unwrap();
            assert_eq!(
                (
                    &*probe.transfer,
                    &*probe.primaries,
                    &*probe.matrix,
                    &*probe.range
                ),
                ("bt709", "bt709", "bt709", "tv")
            );
            let decoded = frame(
                &tools,
                &exported,
                0.0,
                None,
                &dir.join(format!("decoded-{gpu}-{codec:?}.png")),
            );
            errors.push((
                format!("{gpu}-{codec:?}"),
                compare(&decoded, &reference, 4.0),
            ));
        }
        let thumbnail = image::open(tools.thumbnail_at(&source, start, &cancel).unwrap())
            .unwrap()
            .into_rgb8();
        // Match the thumbnail's native-resolution tone mapping followed by
        // YUV downscaling. Resizing an already RGB-converted 960px reference
        // changes chroma resampling and is not a valid pixel comparison.
        let small_filter = format!(
            "{},scale=160:90",
            reference_filter.replace("w=960:h=540:", "")
        );
        let small = frame(
            &tools,
            &path,
            start,
            Some(small_filter),
            &dir.join("reference-thumbnail.png"),
        );
        let thumbnail_error = compare(&thumbnail, &small, 3.0);
        assert_eq!(file_hash(&path).unwrap(), hash);
        reports.push(serde_json::json!({"case":label,"sourceSha256":hash,"mapping":"Adaptive","convertedMeanError":converted_error,"thumbnailMeanError":thumbnail_error,"previewAndExportErrors":errors}));
    }
    std::fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .unwrap();
}
