use editor_core::{Clip, ClipId, Command, FrameRate, SourceId, Timeline};
use media_engine::*;
use project_store::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn rms(tools: &Tools, path: &Path, start: f64) -> f64 {
    let args = vec![
        "-v".into(),
        "error".into(),
        "-ss".into(),
        start.to_string(),
        "-i".into(),
        path.to_string_lossy().into_owned(),
        "-t".into(),
        "0.15".into(),
        "-vn".into(),
        "-ac".into(),
        "1".into(),
        "-f".into(),
        "f32le".into(),
        "-".into(),
    ];
    let bytes = run(&tools.ffmpeg, &args, &uncancelled(), |_| {}).unwrap();
    let samples: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| f64::from(f32::from_le_bytes(b.try_into().unwrap())))
        .collect();
    assert!(!samples.is_empty());
    (samples.iter().map(|s| s * s).sum::<f64>() / samples.len() as f64).sqrt()
}
#[test]
#[ignore = "requires pinned native bundle; run test-rust-media.ps1"]
fn separated_audio_exports_offsets_per_item_gain_mute_fades_and_audio_tail() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").expect("fixtures"));
    let tools = Tools::discover(root.join("cache")).unwrap();
    let cancel = uncancelled();
    let input = root.join("independent-audio-fixture.mkv");
    run(
        &tools.ffmpeg,
        &[
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x180:r=30:d=4",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.1*sin(2*PI*440*t):s=48000:d=4",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "pcm_s16le",
        ]
        .map(str::to_owned)
        .into_iter()
        .chain([input.to_string_lossy().into_owned()])
        .collect::<Vec<_>>(),
        &cancel,
        |_| {},
    )
    .unwrap();
    let hash = file_hash(&input).unwrap();
    let source = tools.probe(&input, SourceId(1), &cancel).unwrap();
    let clip = Clip {
        id: ClipId(1),
        source: source.id,
        source_in: 0.3,
        source_out: 2.3,
        timeline_start: 0.0,
    };
    let mut editor = ProjectEditor::with_metadata(
        Timeline::new(vec![clip]).unwrap(),
        ProjectMetadata {
            sources: vec![source],
            output: OutputSettings {
                width: 320,
                height: 180,
                rate: FrameRate::new(30, 1).unwrap(),
                codec: Codec::H264,
                gpu: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let audio = separate_audio(&mut editor, ClipId(1)).unwrap();
    edit_audio(
        &mut editor,
        Command::Split {
            clip: audio,
            source_time: 1.3,
            rate: FrameRate::new(30, 1).unwrap(),
        },
    )
    .unwrap();
    let mut saved = SavedProject::capture(&editor);
    saved.metadata.audio_items[0].clip.timeline_start = 0.5;
    saved.metadata.audio_items[0].settings.gain_db = -6.0;
    saved.metadata.audio_items[1].clip.timeline_start = 2.0;
    saved.metadata.audio_items[1].settings.gain_db = 6.0;
    let output = root.join("independent-audio-export.mp4");
    tools.export(&saved, &output, &cancel, |_| {}).unwrap();
    let probe = tools.probe(&output, SourceId(2), &cancel).unwrap();
    assert!((probe.duration - 3.0).abs() < 0.05);
    assert!(
        rms(&tools, &output, 0.1) < 0.0001,
        "Detached video still had embedded sound"
    );
    assert!(
        rms(&tools, &output, 1.65) < 0.0001,
        "Detached audio gap was filled"
    );
    let quiet = rms(&tools, &output, 0.8);
    let loud = rms(&tools, &output, 2.3);
    assert!(
        quiet > 0.01 && (loud / quiet - 10.0_f64.powf(12.0 / 20.0)).abs() < 0.15,
        "Per-item gain ratio: {quiet} / {loud}"
    );
    saved.clips.push(Clip {
        id: ClipId(2),
        source: SourceId(1),
        source_in: 0.3,
        source_out: 1.3,
        timeline_start: 2.0,
    });
    let mixed = root.join("independent-audio-mixed.mp4");
    tools.export(&saved, &mixed, &cancel, |_| {}).unwrap();
    let ratio = rms(&tools, &mixed, 2.3) / loud;
    assert!(
        (ratio - 1.5).abs() < 0.1,
        "Linked and independent audio were normalized or lost: {ratio}"
    );
    saved.metadata.audio_items[0].settings.muted = true;
    saved.metadata.audio_items[1].settings.fade_in = 0.8;
    saved.metadata.audio_items[1].settings.fade_out = 0.15;
    // Audio remains after deleting its video, with black video through its tail.
    saved.clips.clear();
    saved.metadata.adjustments = BTreeMap::new();
    let output = root.join("independent-audio-muted.mp4");
    tools.export(&saved, &output, &cancel, |_| {}).unwrap();
    assert!(rms(&tools, &output, 0.8) < 0.0001);
    assert!(rms(&tools, &output, 2.05) < rms(&tools, &output, 2.65) * 0.5);
    assert!(rms(&tools, &output, 2.9) < rms(&tools, &output, 2.65));
    assert_eq!(file_hash(&input).unwrap(), hash);
    let path = root.join("independent-audio.sveproject");
    save_document(&path, &saved).unwrap();
    let loaded = tools.load_project(&path, &cancel).unwrap();
    assert_eq!(loaded, saved);
}
