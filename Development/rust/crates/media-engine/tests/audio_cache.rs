use editor_core::SourceId;
use media_engine::*;
use std::path::PathBuf;
#[test]
#[ignore = "requires generated media and the pinned native bundle"]
fn waveform_delays_phase_cache_and_safe_cleanup() {
    let root = PathBuf::from(std::env::var_os("SVE_MEDIA_FIXTURES").unwrap());
    let tools = Tools::discover(root.join("audio-cache-tests")).unwrap();
    let cancel = uncancelled();
    let path = root.join("delayed-opposite-phase.mkv");
    let args = vec![
        "-v".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        root.join("sdr.mkv").to_string_lossy().into_owned(),
        "-itsoffset".into(),
        "0.5".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "aevalsrc=0.6*sin(2*PI*440*t)|-0.6*sin(2*PI*440*t):s=48000:d=2".into(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "pcm_f32le".into(),
        path.to_string_lossy().into_owned(),
    ];
    run(&tools.ffmpeg, &args, &cancel, |_| {}).unwrap();
    let source = tools.probe(&path, SourceId(1), &cancel).unwrap();
    let peaks = tools.waveform(&source, &cancel).unwrap();
    assert!(peaks.len() >= 240);
    assert!(peaks[..45].iter().all(|v| *v < 0.0001));
    assert!(peaks[55..150].iter().all(|v| *v > 0.5));
    assert_eq!(tools.waveform(&source, &cancel).unwrap(), peaks);
    let thumbnail = tools.thumbnail(&source, &cancel).unwrap();
    assert!(thumbnail.is_file());
    let first = tools.thumbnail_at(&source, 0.0, &cancel).unwrap();
    let later = tools.thumbnail_at(&source, 1.5, &cancel).unwrap();
    assert_ne!(first, later);
    assert_ne!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&later).unwrap()
    );
    assert_eq!(tools.thumbnail_at(&source, 1.5, &cancel).unwrap(), later);
    for time in [f64::NAN, -0.1, source.duration] {
        assert!(tools.thumbnail_at(&source, time, &cancel).is_err());
    }
    let unrelated = tools.cache.join("waveforms/keep-my-recording.mkv");
    std::fs::write(&unrelated, b"original").unwrap();
    let recovery = tools.cache.join("recovery.sveproject");
    std::fs::write(&recovery, b"saved recovery").unwrap();
    let protected = tools
        .cache
        .join("thumbnails")
        .join(format!("{}.bmp", "a".repeat(64)));
    std::fs::write(&protected, b"original with a cache-shaped name").unwrap();
    let linked = tools
        .cache
        .join("thumbnails")
        .join(format!("{}.bmp", "b".repeat(64)));
    if linked.exists() {
        std::fs::remove_file(&linked).unwrap();
    }
    std::fs::hard_link(&protected, &linked).unwrap();
    let (deleted, _) = tools.clear_cache(std::slice::from_ref(&protected)).unwrap();
    assert_eq!(deleted, 4);
    assert_eq!(std::fs::read(unrelated).unwrap(), b"original");
    assert_eq!(std::fs::read(recovery).unwrap(), b"saved recovery");
    assert_eq!(
        std::fs::read(&protected).unwrap(),
        b"original with a cache-shaped name"
    );
    assert!(linked.exists());
    std::fs::remove_file(linked).unwrap();
    std::fs::remove_file(protected).unwrap();
}
