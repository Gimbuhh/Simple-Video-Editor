use std::process::Command;

#[test]
fn actual_executable_imports_without_touching_media_or_project() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../project-store/tests/fixtures/gaps-library.json");
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sve-inspect"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["sourceCount"], 2);
    assert_eq!(summary["clipCount"], 2);
    assert_eq!(summary["durationSeconds"], 5.0);
    assert_eq!(before, std::fs::read(path).unwrap());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixtures"));
}

#[test]
fn invalid_invocations_exit_unsuccessfully() {
    for args in [vec![], vec!["missing.sveproject"], vec!["one", "two"]] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_sve-inspect"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
