//! Bounded, read-only schema-1 import. Parsing never accesses recordings.
//! Recording references are syntax-checked, NOT authorized for media access:
//! Windows drive/link validation must precede any future probe or playback.
use editor_core::{Clip, ClipId, SourceId, Timeline};
use serde::Deserialize;
use serde_json::Value;
use std::{fmt, io::Read};

pub const MAX_PROJECT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCES: usize = 10_000;

#[derive(Debug)]
pub struct ProjectError(String);
impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProjectError {}
fn error(message: impl Into<String>) -> ProjectError {
    ProjectError(message.into())
}

#[derive(Deserialize)]
struct LegacyDocument {
    version: u32,
    clips: Vec<LegacyClip>,
    sources: Option<Vec<String>>,
}
#[derive(Deserialize)]
struct LegacyClip {
    path: String,
    start: f64,
    end: f64,
    timelinestart: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: SourceId,
    pub path: String,
}
#[derive(Debug)]
pub struct ImportedProject {
    pub sources: Vec<Source>,
    pub timeline: Timeline,
}

/// This deliberately accepts missing local files for later relinking. No path
/// returned here may be sent to a native tool without platform authorization.
fn validate_reference(path: &str) -> Result<(), ProjectError> {
    let bytes = path.as_bytes();
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || path.chars().any(char::is_control)
    {
        return Err(error(
            "Recording references must be absolute local-drive paths",
        ));
    }
    Ok(())
}

// System.Text.Json's legacy reader is case insensitive. Canonicalize only
// schema objects, not string values or ignored extension objects.
fn canonicalize(value: Value) -> Result<Value, ProjectError> {
    let Value::Object(fields) = value else {
        return Err(error("Expected a project object"));
    };
    let mut result = serde_json::Map::new();
    for (key, mut value) in fields {
        let key = key.to_ascii_lowercase();
        if key == "clips"
            && let Value::Array(clips) = value
        {
            value = Value::Array(
                clips
                    .into_iter()
                    .map(canonicalize)
                    .collect::<Result<_, _>>()?,
            );
        }
        if result.insert(key, value).is_some() {
            return Err(error("Ambiguous duplicate project property"));
        }
    }
    Ok(Value::Object(result))
}

pub fn read_project(reader: impl Read) -> Result<ImportedProject, ProjectError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_PROJECT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| error(format!("Cannot read project: {e}")))?;
    parse_project(&bytes)
}

pub fn parse_project(bytes: &[u8]) -> Result<ImportedProject, ProjectError> {
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err(error("Project exceeds the 8 MiB limit"));
    }
    // File.ReadAllTextAsync in the legacy reader accepts a UTF-8 BOM.
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| error(format!("Invalid project JSON: {e}")))?;
    let legacy: LegacyDocument = serde_json::from_value(canonicalize(value)?)
        .map_err(|e| error(format!("Invalid schema-1 project: {e}")))?;
    if legacy.version != 1 {
        return Err(error("Unsupported project schema; expected version 1"));
    }
    if legacy.clips.len() > editor_core::MAX_CLIPS
        || legacy
            .sources
            .as_ref()
            .is_some_and(|s| s.len() > MAX_SOURCES)
    {
        return Err(error(
            "Project contains too many clips or library recordings",
        ));
    }
    let mut sources: Vec<Source> = Vec::new();
    let mut source_ids = std::collections::HashMap::new();
    let mut add_source = |path: String| -> Result<SourceId, ProjectError> {
        validate_reference(&path)?;
        let key = path.to_lowercase();
        if let Some(id) = source_ids.get(&key) {
            return Ok(*id);
        }
        let id = SourceId(sources.len() as u64 + 1);
        sources.push(Source { id, path });
        source_ids.insert(key, id);
        Ok(id)
    };
    for path in legacy.sources.unwrap_or_default() {
        add_source(path)?;
    }
    let mut clips = Vec::with_capacity(legacy.clips.len());
    let mut end = 0.0;
    for (index, clip) in legacy.clips.into_iter().enumerate() {
        let source = add_source(clip.path)?;
        let section = Clip {
            id: ClipId(index as u64 + 1),
            source,
            source_in: clip.start,
            source_out: clip.end,
            timeline_start: clip.timelinestart.unwrap_or(end),
        };
        end = section.timeline_end();
        clips.push(section);
    }
    let timeline = Timeline::new(clips).map_err(|e| error(e.to_string()))?;
    Ok(ImportedProject { sources, timeline })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_sequential_clips_preserve_exact_binary_trim_values() {
        let project = parse_project(include_bytes!("../tests/fixtures/sequential.json")).unwrap();
        assert_eq!(project.sources.len(), 2);
        assert_eq!(
            project.timeline.clips()[0].source_in.to_bits(),
            1.1500000000000001_f64.to_bits()
        );
        let first_duration = 2.15_f64 - 1.1500000000000001_f64;
        assert_eq!(
            project.timeline.clips()[1].timeline_start.to_bits(),
            first_duration.to_bits()
        );
        assert_eq!(
            project.timeline.duration().to_bits(),
            (first_duration + 1.0).to_bits()
        );
    }
    #[test]
    fn library_order_unused_sources_repeated_sections_and_gaps_survive() {
        let project = parse_project(include_bytes!("../tests/fixtures/gaps-library.json")).unwrap();
        assert_eq!(project.sources.len(), 2);
        assert!(project.sources[0].path.ends_with("unused.mp4"));
        let clips = project.timeline.clips();
        assert_eq!(clips[0].source, clips[1].source);
        assert_ne!(clips[0].id, clips[1].id);
        assert_eq!(clips[0].timeline_start, 1.0);
        assert_eq!(clips[1].timeline_start, 4.0);
        assert_eq!(project.timeline.duration(), 5.0);
    }
    #[test]
    fn mixed_case_properties_and_missing_optional_fields_are_supported() {
        let project = parse_project(
            br#"{"vErSiOn":1,"cLiPs":[{"pAtH":"C:/missing.mp4","sTaRt":0,"eNd":1}]}"#,
        )
        .unwrap();
        assert_eq!(project.timeline.duration(), 1.0);
        assert_eq!(project.sources[0].path, "C:/missing.mp4");
        assert!(parse_project(br#"{"Version":1,"version":2,"Clips":[]}"#).is_err());
    }
    #[test]
    fn network_device_relative_and_url_references_are_rejected_in_both_locations() {
        for path in [
            "\\\\server\\share\\clip.mp4",
            "\\\\?\\C:\\clip.mp4",
            "\\\\.\\C:\\clip.mp4",
            "\\??\\C:\\clip.mp4",
            "https://host.invalid/clip.mp4",
            "C:clip.mp4",
            "clip.mp4",
            "",
            "C:/bad\0.mp4",
        ] {
            let library = json!({"Version":1,"Clips":[],"Sources":[path]});
            assert!(
                parse_project(&serde_json::to_vec(&library).unwrap()).is_err(),
                "{path}"
            );
            let clips = json!({"Version":1,"Clips":[{"Path":path,"Start":0,"End":1}]});
            assert!(
                parse_project(&serde_json::to_vec(&clips).unwrap()).is_err(),
                "{path}"
            );
        }
    }
    #[test]
    fn malformed_future_overlapping_and_extreme_projects_fail() {
        for bytes in [
            br#"{}"#.as_slice(),
            br#"null"#,
            br#"{"Version":2,"Clips":[]}"#,
            br#"{"Version":1,"Clips":null}"#,
            br#"{"Version":1,"Clips":[null]}"#,
        ] {
            assert!(parse_project(bytes).is_err());
        }
        for (start, end, position) in [
            (-1.0, 1.0, 0.0),
            (2.0, 1.0, 0.0),
            (0.0, 1.0, -1.0),
            (0.0, 1.0, 1e20),
        ] {
            let project = json!({"Version":1,"Clips":[{"Path":"C:/a.mp4","Start":start,"End":end,"TimelineStart":position}]});
            assert!(parse_project(&serde_json::to_vec(&project).unwrap()).is_err());
        }
        let overlap = json!({"Version":1,"Clips":[
            {"Path":"C:/a.mp4","Start":0,"End":2},
            {"Path":"C:/a.mp4","Start":0,"End":2,"TimelineStart":1}]});
        assert!(parse_project(&serde_json::to_vec(&overlap).unwrap()).is_err());
    }
    #[test]
    fn utf8_bom_projects_import_without_rewriting() {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(br#"{"Version":1,"Clips":[],"Sources":null}"#);
        let project = parse_project(&bytes).unwrap();
        assert!(project.sources.is_empty());
        assert!(project.timeline.clips().is_empty());
    }

    #[test]
    fn resource_limits_apply_to_streams_and_collections() {
        assert!(read_project(std::io::repeat(b' ').take((MAX_PROJECT_BYTES + 1) as u64)).is_err());
        let mut project =
            json!({"Version":1,"Clips":[],"Sources":vec!["C:/a.mp4"; MAX_SOURCES + 1]});
        assert!(parse_project(&serde_json::to_vec(&project).unwrap()).is_err());
        project["Sources"] = json!([]);
        project["Clips"] = json!(vec![
            json!({"Path":"C:/a.mp4","Start":0,"End":1});
            editor_core::MAX_CLIPS + 1
        ]);
        assert!(parse_project(&serde_json::to_vec(&project).unwrap()).is_err());
    }
}
