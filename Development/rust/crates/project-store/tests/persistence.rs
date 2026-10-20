use editor_core::{Editor, Timeline};
use project_store::*;
fn root() -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/persistence-tests")
        .join(unique_id());
    std::fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn atomic_save_roundtrip_and_encoding_compatibility() {
    let root = root();
    let path = root.join("test.sveproject");
    let document = SavedProject::capture(
        &Editor::with_metadata(Timeline::default(), ProjectMetadata::default()).unwrap(),
    );
    save_document(&path, &document).unwrap();
    let value = read_document(&path).unwrap();
    assert_eq!(
        serde_json::from_value::<SavedProject>(value.clone()).unwrap(),
        document
    );
    let json = serde_json::to_string(&value).unwrap();
    for little in [true, false] {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for value in json.encode_utf16() {
            bytes.extend(if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_document(&path).unwrap(), value);
        let mut bytes = if little {
            vec![0xff, 0xfe, 0, 0]
        } else {
            vec![0, 0, 0xfe, 0xff]
        };
        for value in json.chars() {
            bytes.extend(if little {
                (value as u32).to_le_bytes()
            } else {
                (value as u32).to_be_bytes()
            });
        }
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_document(&path).unwrap(), value);
    }
    assert!(save_document(std::path::Path::new("relative.sveproject"), &document).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn recovery_never_claims_a_live_session_or_another_prompt() {
    let root = root();
    let document = SavedProject::capture(
        &Editor::with_metadata(Timeline::default(), ProjectMetadata::default()).unwrap(),
    );
    let first = Recovery::new(&root).unwrap();
    first.save(&document).unwrap();
    let second = Recovery::new(&root).unwrap();
    let old_revision = first.revision();
    first.clear();
    first.save_revision(&document, old_revision).unwrap();
    assert!(!first.path.exists());
    first.save(&document).unwrap();
    assert!(second.candidates().is_empty());
    drop(first);
    let claims = second.candidates();
    assert_eq!(claims.len(), 1);
    let third = Recovery::new(&root).unwrap();
    assert!(third.candidates().is_empty());
    claims[0].discard();
    drop(claims);
    drop(second);
    drop(third);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn clip_settings_follow_split_and_undo_without_leaking_into_new_recordings() {
    use editor_core::{Clip, ClipId, Command, FrameRate, SourceId};
    let rate = FrameRate::new(30, 1).unwrap();
    let source = MediaSource {
        id: SourceId(1),
        path: "C:\\recording.mp4".into(),
        duration: 5.0,
        width: 320,
        height: 180,
        display_width: 320.0,
        display_height: 180.0,
        rate,
        codec: "h264".into(),
        audio: vec![],
        transfer: "bt709".into(),
        primaries: "bt709".into(),
        matrix: "bt709".into(),
        range: "tv".into(),
        pixel_format: "yuv420p".into(),
    };
    let clip = Clip {
        id: ClipId(1),
        source: source.id,
        source_in: 0.0,
        source_out: 3.0,
        timeline_start: 0.0,
    };
    let mut metadata = ProjectMetadata {
        sources: vec![source],
        ..Default::default()
    };
    metadata.adjustments.insert(
        clip.id,
        ClipSettings {
            gain_db: -6.0,
            fade_in: 2.0,
            ..Default::default()
        },
    );
    let mut editor =
        Editor::with_metadata(Timeline::new(vec![clip.clone()]).unwrap(), metadata).unwrap();
    let original = SavedProject::capture(&editor);
    edit_project(
        &mut editor,
        Command::Split {
            clip: clip.id,
            source_time: 1.0,
            rate,
        },
    )
    .unwrap();
    let split = SavedProject::capture(&editor);
    split.validate().unwrap();
    assert_eq!(split.metadata.adjustments.len(), 2);
    assert_eq!(split.metadata.adjustments[&clip.id].fade_in, 1.0);
    assert_eq!(
        split.metadata.adjustments[&editor.timeline().clips()[1].id].gain_db,
        -6.0
    );
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), original);
    assert!(editor.redo());
    assert_eq!(SavedProject::capture(&editor), split);
    edit_project(
        &mut editor,
        Command::Append {
            sections: vec![clip],
        },
    )
    .unwrap();
    let added = editor.timeline().clips().last().unwrap().id;
    assert!(!editor.metadata().adjustments.contains_key(&added));
    let before = SavedProject::capture(&editor);
    assert!(
        edit_project(
            &mut editor,
            Command::Replace {
                clips: vec![Clip {
                    id: ClipId(99),
                    source: SourceId(99),
                    source_in: 0.0,
                    source_out: 1.0,
                    timeline_start: 0.0
                }]
            }
        )
        .is_err()
    );
    assert_eq!(SavedProject::capture(&editor), before);
}
