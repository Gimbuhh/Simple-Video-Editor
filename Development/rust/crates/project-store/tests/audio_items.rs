use editor_core::{Clip, ClipId, Command, Editor, FrameRate, SourceId, Timeline};
use project_store::*;

fn editor() -> ProjectEditor {
    let source: MediaSource=serde_json::from_value(serde_json::json!({
        "id":1,"path":"C:\\recording.mp4","duration":20.0,"width":320,"height":180,
        "display_width":320.0,"display_height":180.0,"rate":{"numerator":30,"denominator":1},
        "codec":"h264","audio":[{"index":2,"label":"Game","channels":2}],
        "transfer":"bt709","primaries":"bt709","matrix":"bt709","range":"tv","pixel_format":"yuv420p"
    })).unwrap();
    let clips = vec![
        Clip {
            id: ClipId(1),
            source: SourceId(1),
            source_in: 1.123456789,
            source_out: 4.234567891,
            timeline_start: 2.34567891,
        },
        Clip {
            id: ClipId(2),
            source: SourceId(1),
            source_in: 6.0,
            source_out: 8.0,
            timeline_start: 9.0,
        },
    ];
    Editor::with_metadata(
        Timeline::new(clips).unwrap(),
        ProjectMetadata {
            sources: vec![source],
            ..Default::default()
        },
    )
    .unwrap()
}
#[test]
fn separation_volume_independent_edits_roundtrip_and_undo_preserve_video() {
    let mut editor = editor();
    let original = SavedProject::capture(&editor);
    editor.metadata_mut().adjustments.insert(
        ClipId(1),
        ClipSettings {
            gain_db: -6.0,
            stream: Some(2),
            fade_in: 0.2,
            fade_out: 0.3,
            ..Default::default()
        },
    );
    let before = SavedProject::capture(&editor);
    let id = separate_audio(&mut editor, ClipId(1)).unwrap();
    let separated = SavedProject::capture(&editor);
    assert_eq!(separated.clips, before.clips);
    assert_eq!(separated.version, 3);
    assert_eq!(
        separated.metadata.audio_items[0].clip.source_in.to_bits(),
        before.clips[0].source_in.to_bits()
    );
    assert_eq!(
        separated.metadata.audio_items[0].settings,
        before.metadata.adjustments[&ClipId(1)]
    );
    assert!(separate_audio(&mut editor, ClipId(1)).is_err());
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), before);
    assert!(editor.redo());
    assert_eq!(SavedProject::capture(&editor), separated);
    let rate = FrameRate::new(30, 1).unwrap();
    edit_audio(
        &mut editor,
        Command::Move {
            clip: id,
            position: 4.0,
            rate,
            snap: 0.0,
        },
    )
    .unwrap();
    assert_eq!(editor.timeline().clips(), before.clips);
    edit_audio(
        &mut editor,
        Command::Split {
            clip: id,
            source_time: 2.123456789,
            rate,
        },
    )
    .unwrap();
    assert_eq!(editor.metadata().audio_items.len(), 2);
    assert!(
        editor
            .metadata()
            .audio_items
            .iter()
            .all(|a| a.settings.gain_db == -6.0)
    );
    let second = editor.metadata().audio_items[1].clip.id;
    let snapshot = SavedProject::capture(&editor);
    let mut metadata = editor.metadata().clone();
    metadata.audio_items[1].settings.gain_db = 6.0;
    editor
        .apply_with_metadata(
            Command::Replace {
                clips: editor.timeline().clips().to_vec(),
            },
            metadata,
        )
        .unwrap();
    assert_eq!(editor.metadata().audio_items[0].settings.gain_db, -6.0);
    assert_eq!(editor.metadata().audio_items[1].settings.gain_db, 6.0);
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), snapshot);
    assert!(editor.redo());
    let saved = SavedProject::capture(&editor);
    saved.validate().unwrap();
    let loaded: SavedProject =
        serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
    assert_eq!(
        loaded
            .clone()
            .editor()
            .map(|e| SavedProject::capture(&e))
            .unwrap(),
        saved
    );
    edit_project(&mut editor, Command::Delete { clip: ClipId(1) }).unwrap();
    assert_eq!(editor.metadata().audio_items.len(), 2);
    edit_audio(&mut editor, Command::Delete { clip: second }).unwrap();
    assert_eq!(editor.metadata().audio_items.len(), 1);
    remove_source(&mut editor, SourceId(1)).unwrap();
    assert!(editor.timeline().clips().is_empty());
    assert!(editor.metadata().audio_items.is_empty());
    assert_eq!(original.version, 2);
}
#[test]
fn audio_validation_and_rejected_edits_are_atomic() {
    let mut editor = editor();
    let id = separate_audio(&mut editor, ClipId(1)).unwrap();
    let before = SavedProject::capture(&editor);
    let mut invalid = before.clone();
    invalid.version = 2;
    assert!(invalid.validate().is_err());
    for gain in [f64::NAN, f64::INFINITY, -61.0, 25.0] {
        let mut invalid = before.clone();
        invalid.metadata.audio_items[0].settings.gain_db = gain;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = before.clone();
    invalid.metadata.audio_items[0].settings.stream = Some(50);
    assert!(invalid.validate().is_err());
    let mut invalid = before.clone();
    invalid.metadata.audio_items[0].clip.source_out = 30.0;
    assert!(invalid.validate().is_err());
    let mut invalid = before.clone();
    invalid
        .metadata
        .audio_items
        .push(invalid.metadata.audio_items[0].clone());
    assert!(invalid.validate().is_err());
    assert!(
        edit_audio(
            &mut editor,
            Command::Split {
                clip: id,
                source_time: 0.0,
                rate: FrameRate::new(30, 1).unwrap()
            }
        )
        .is_err()
    );
    assert_eq!(SavedProject::capture(&editor), before);
    let mut metadata = editor.metadata().clone();
    metadata.audio_items[0].clip.timeline_start = 9.0;
    editor
        .apply_with_metadata(
            Command::Replace {
                clips: editor.timeline().clips().to_vec(),
            },
            metadata,
        )
        .unwrap();
    // A second separation cannot overwrite an existing independent item.
    let before = SavedProject::capture(&editor);
    assert!(separate_audio(&mut editor, ClipId(2)).is_err());
    assert_eq!(SavedProject::capture(&editor), before);
}

#[test]
fn independent_trim_duplicate_and_delete_undo_preserve_other_lane_and_gaps() {
    let mut editor = editor();
    let video = editor.timeline().clone();
    let first = separate_audio(&mut editor, ClipId(1)).unwrap();
    separate_audio(&mut editor, ClipId(2)).unwrap();
    let before = SavedProject::capture(&editor);
    let rate = FrameRate::new(30, 1).unwrap();
    edit_audio(
        &mut editor,
        Command::Trim {
            clip: first,
            start: 0.5,
            end: 4.234567891,
            source_duration: 20.0,
            rate,
        },
    )
    .unwrap();
    assert_eq!(editor.timeline(), &video);
    assert_eq!(editor.metadata().audio_items[0].clip.source_in, 0.5);
    assert_eq!(
        editor.metadata().audio_items[1],
        before.metadata.audio_items[1]
    );
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), before);
    edit_audio(&mut editor, Command::Duplicate { clip: first, rate }).unwrap();
    assert_eq!(editor.metadata().audio_items.len(), 3);
    assert_eq!(editor.timeline(), &video);
    assert_eq!(
        editor.metadata().audio_items.last().unwrap(),
        before.metadata.audio_items.last().unwrap()
    );
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), before);
    edit_audio(&mut editor, Command::Delete { clip: first }).unwrap();
    assert_eq!(
        editor.metadata().audio_items[0],
        before.metadata.audio_items[1]
    );
    assert!(editor.undo());
    assert_eq!(SavedProject::capture(&editor), before);
}
