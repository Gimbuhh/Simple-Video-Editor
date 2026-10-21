use editor_core::{Clip, ClipId, FrameRate, SourceId};
use media_engine::*;
use project_store::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn invoke(tools: &Tools, args: Vec<String>) {
    run(&tools.ffmpeg, &args, &uncancelled(), |_| {}).unwrap();
}
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}
fn rgb(path: &Path) -> Vec<u8> {
    image::open(path).unwrap().into_rgb8().into_raw()
}
fn error(left: &[u8], right: &[u8]) -> f64 {
    assert_eq!(left.len(), right.len());
    left.iter()
        .zip(right)
        .map(|(a, b)| (f64::from(*a) - f64::from(*b)).abs())
        .sum::<f64>()
        / left.len() as f64
}
#[test]
#[ignore = "requires the pinned native bundle and generated media directory"]
fn rotation_anamorphic_rgb_and_preview_export_agree() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").unwrap()).join("geometry");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    let base = root.join("corners.mp4");
    let pattern = "color=black:s=320x180:r=30:d=3,drawbox=x=0:y=0:w=160:h=90:c=red:t=fill,drawbox=x=160:y=0:w=160:h=90:c=green:t=fill,drawbox=x=0:y=90:w=160:h=90:c=blue:t=fill,drawbox=x=160:y=90:w=160:h=90:c=white:t=fill,setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=limited";
    let mut a = args(&[
        "-v",
        "error",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        pattern,
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
    ]);
    a.push(base.to_string_lossy().into_owned());
    invoke(&tools, a);
    let rotated = root.join("rotated.mp4");
    let mut a = args(&[
        "-v",
        "error",
        "-nostdin",
        "-y",
        "-display_rotation:v:0",
        "90",
        "-i",
    ]);
    a.push(base.to_string_lossy().into_owned());
    a.extend(args(&["-c", "copy"]));
    a.push(rotated.to_string_lossy().into_owned());
    invoke(&tools, a);
    let anamorphic = root.join("anamorphic.mp4");
    let mut a = args(&["-v", "error", "-nostdin", "-y", "-i"]);
    a.push(base.to_string_lossy().into_owned());
    a.extend(args(&[
        "-vf",
        "setsar=2",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
    ]));
    a.push(anamorphic.to_string_lossy().into_owned());
    invoke(&tools, a);
    let rgb_input = root.join("rgb.mkv");
    let mut a = args(&[
        "-v",
        "error",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:r=30:d=3,format=gbrp10le",
        "-c:v",
        "ffv1",
        "-color_trc",
        "iec61966-2-1",
        "-color_primaries",
        "bt709",
        "-color_range",
        "pc",
    ]);
    a.push(rgb_input.to_string_lossy().into_owned());
    invoke(&tools, a);
    let multiple = root.join("multiple.mkv");
    let mut a = args(&[
        "-v",
        "error",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=3",
        "-f",
        "lavfi",
        "-i",
        "color=red:s=320x180:r=30:d=3,setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=limited",
        "-f",
        "lavfi",
        "-i",
        "color=blue:s=640x360:r=30:d=3",
        "-map",
        "0:a",
        "-map",
        "1:v",
        "-map",
        "2:v",
        "-c:a",
        "pcm_s16le",
        "-c:v",
        "ffv1",
        "-disposition:v:0",
        "0",
        "-disposition:v:1",
        "default",
    ]);
    a.push(multiple.to_string_lossy().into_owned());
    invoke(&tools, a);
    for path in [&base, &rotated, &anamorphic, &rgb_input, &multiple] {
        let cancel = uncancelled();
        let source = tools.probe(path, SourceId(1), &cancel).unwrap();
        if path == &multiple {
            assert_eq!(source.video_stream, 1);
            assert_eq!(source.width, 320);
        }
        let output = OutputSettings {
            width: 320,
            height: 180,
            rate: FrameRate::new(30, 1).unwrap(),
            profile: ColorProfile::Sdr,
            codec: Codec::H264,
            gpu: false,
            quality: 16,
        };
        if path == &rotated {
            assert!((source.display_width - 180.0).abs() < 1e-8);
            assert!((source.display_height - 320.0).abs() < 1e-8);
        }
        if path == &anamorphic {
            assert!((source.display_width - 640.0).abs() < 1e-8);
            assert_eq!(source.sample_aspect_ratio, 2.0);
        }
        let reference = root.join("reference.bmp");
        let mut a = args(&["-v", "error", "-nostdin", "-y"]);
        if path == &rgb_input {
            a.push("-noautorotate".into());
        }
        a.push("-i".into());
        a.push(path.to_string_lossy().into_owned());
        a.extend(["-map".into(), format!("0:{}", source.video_stream)]);
        let filter = if path == &rgb_input {
            video_filters(&source, &ClipSettings::default(), &output, false)
        } else {
            "scale=320:180:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad=320:180:(ow-iw)/2:(oh-ih)/2".into()
        };
        a.extend([
            "-vf".into(),
            filter,
            "-frames:v".into(),
            "1".into(),
            reference.to_string_lossy().into_owned(),
        ]);
        invoke(&tools, a);
        let expected = rgb(&reference);
        let player = native_mpv::Player::open(&tools.mpv, native_mpv::Mode::SoftwareSdr).unwrap();
        player.set("video-rotate", "no").unwrap();
        player
            .set(
                "vf",
                &format!(
                    "lavfi=[{}]",
                    video_filters(&source, &ClipSettings::default(), &output, false)
                ),
            )
            .unwrap();
        player
            .load(&media_path::RecordingPath::authorize(path).unwrap())
            .unwrap();
        let serial = if path == &multiple {
            player.set("vid", "1").unwrap();
            player.seek(0.25).unwrap();
            player.refresh_frame()
        } else {
            0
        };
        let deadline = Instant::now() + Duration::from_secs(8);
        let frame = loop {
            player.pump().unwrap();
            if let Some(frame) = player.take_frame()
                && frame.serial >= serial
                && frame.pixels.iter().any(|b| *b > 32)
            {
                break frame;
            }
            assert!(Instant::now() < deadline, "No preview frame");
        };
        let mut preview = Vec::new();
        for row in frame.pixels.chunks(frame.stride).take(frame.height) {
            for pixel in row[..frame.width * 4].chunks_exact(4) {
                preview.extend([pixel[2], pixel[1], pixel[0]]);
            }
        }
        assert!(
            error(&preview, &expected) < 7.0,
            "Preview geometry/color differs for {}: {}",
            path.display(),
            error(&preview, &expected)
        );
        drop(player);
        let document = SavedProject {
            version: 2,
            application_version: "1.1.0".into(),
            clips: vec![Clip {
                id: ClipId(1),
                source: source.id,
                source_in: 0.0,
                source_out: 1.0,
                timeline_start: 0.0,
            }],
            metadata: ProjectMetadata {
                sources: vec![source],
                output,
                ..Default::default()
            },
        };
        let exported = root.join("output.mp4");
        tools.export(&document, &exported, &cancel, |_| {}).unwrap();
        let decoded = root.join("decoded.bmp");
        let mut a = args(&["-v", "error", "-nostdin", "-y", "-i"]);
        a.push(exported.to_string_lossy().into_owned());
        a.extend(args(&["-frames:v", "1"]));
        a.push(decoded.to_string_lossy().into_owned());
        invoke(&tools, a);
        let actual = rgb(&decoded);
        assert!(
            error(&actual, &expected) < 7.0,
            "Export geometry/color differs for {}: {}",
            path.display(),
            error(&actual, &expected)
        );
    }
}
