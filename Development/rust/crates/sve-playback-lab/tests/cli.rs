use std::process::Command;

#[test]
fn malformed_arguments_do_not_start_native_playback() {
    for args in [
        vec![],
        vec!["unknown", "missing.dll", "C:/missing.mp4"],
        vec!["software", "missing.dll"],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_sve-playback-lab"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

#[test]
fn remote_and_device_recordings_are_rejected_before_library_loading() {
    for path in [
        "https://invalid/clip.mp4",
        "\\\\server\\clip.mp4",
        "\\\\?\\C:\\clip.mp4",
        "clip.mp4",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sve-playback-lab"))
            .args(["software", "nonexistent.dll", path])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("local drive"));
    }
}
