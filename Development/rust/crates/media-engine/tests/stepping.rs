use editor_core::SourceId;
use media_engine::{Tools, run, uncancelled};
use native_mpv::{Mode, Player};
use std::path::PathBuf;

#[test]
#[ignore = "requires the pinned native bundle and generated media directory"]
fn frame_step_uses_variable_presentation_timestamps() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").unwrap()).join("stepping");
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::discover(root.join("cache")).unwrap();
    let path = root.join("variable.mkv");
    let mut args: Vec<String> = [
        "-v",
        "error",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=100:d=1",
        "-vf",
        "select='eq(n,0)+eq(n,4)+eq(n,14)+eq(n,18)+eq(n,38)+eq(n,42)+eq(n,82)'",
        "-fps_mode",
        "vfr",
        "-c:v",
        "ffv1",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    args.push(path.to_string_lossy().into_owned());
    run(&tools.ffmpeg, &args, &uncancelled(), |_| {}).unwrap();
    tools.probe(&path, SourceId(1), &uncancelled()).unwrap();
    let player = Player::open(&tools.mpv, Mode::SoftwareSdr).unwrap();
    player
        .load(&media_path::RecordingPath::authorize(&path).unwrap())
        .unwrap();
    player.seek(0.14).unwrap();
    for (count, expected) in [
        (1, 0.18),
        (1, 0.38),
        (-1, 0.18),
        (-1, 0.14),
        (2, 0.38),
        (-2, 0.14),
    ] {
        let actual = player.step_frames(count, || false).unwrap().unwrap();
        assert!(
            (actual - expected).abs() < 0.002,
            "step {count}: expected {expected}, got {actual}"
        );
        assert_eq!(player.property("pause").as_deref(), Some("yes"));
    }
    assert!(player.step_frames(1, || true).unwrap().is_none());
    player.seek(0.2).unwrap();
    let forward = player.step_frames(1, || false).unwrap().unwrap();
    assert!(
        (forward - 0.42).abs() < 0.002,
        "Seeking between VFR timestamps lost the next decoded frame: {forward}"
    );
}
