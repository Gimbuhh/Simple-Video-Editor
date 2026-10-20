use super::*;
use editor_core::{Editor, FrameRate};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioStream {
    pub index: u32,
    pub label: String,
    pub channels: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaSource {
    pub id: SourceId,
    pub path: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub display_width: f64,
    pub display_height: f64,
    pub rate: FrameRate,
    pub codec: String,
    pub audio: Vec<AudioStream>,
    pub transfer: String,
    pub primaries: String,
    pub matrix: String,
    pub range: String,
    pub pixel_format: String,
}
impl MediaSource {
    pub fn hdr(&self) -> bool {
        matches!(self.transfer.as_str(), "smpte2084" | "arib-std-b67")
    }
    pub fn name(&self) -> String {
        Path::new(&self.path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColorProfile {
    #[default]
    Sdr,
    Hdr10,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Codec {
    #[default]
    Av1,
    H264,
    Hevc,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Framing {
    #[default]
    Fit,
    Fill,
    Crop,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    pub width: u32,
    pub height: u32,
    pub rate: FrameRate,
    pub profile: ColorProfile,
    pub codec: Codec,
    pub quality: u32,
    pub gpu: bool,
}
impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            rate: FrameRate::new(60, 1).unwrap(),
            profile: ColorProfile::Sdr,
            codec: Codec::Av1,
            quality: 28,
            gpu: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipSettings {
    pub stream: Option<u32>,
    pub gain_db: f64,
    pub muted: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    pub framing: Framing,
    pub rotation: u16,
    pub crop: [f64; 4],
}
impl Default for ClipSettings {
    fn default() -> Self {
        Self {
            stream: None,
            gain_db: 0.0,
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            framing: Framing::Fit,
            rotation: 0,
            crop: [0.0, 0.0, 1.0, 1.0],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectMetadata {
    pub sources: Vec<MediaSource>,
    pub output: OutputSettings,
    pub adjustments: BTreeMap<ClipId, ClipSettings>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProject {
    #[serde(rename = "Version")]
    pub version: u32,
    #[serde(rename = "ApplicationVersion")]
    pub application_version: String,
    pub clips: Vec<Clip>,
    pub metadata: ProjectMetadata,
}
pub type ProjectEditor = Editor<ProjectMetadata>;
/// Timeline and clip settings commit together, including inherited settings for
/// new split/duplicate identities. A rejected edit preserves history and state.
pub fn edit_project(
    editor: &mut ProjectEditor,
    command: editor_core::Command,
) -> Result<(), ProjectError> {
    let metadata = editor.metadata().clone();
    let inherited = match &command {
        editor_core::Command::Split { clip, .. } | editor_core::Command::Duplicate { clip, .. } => {
            metadata.adjustments.get(clip).cloned()
        }
        _ => None,
    };
    let old: std::collections::BTreeSet<_> =
        editor.timeline().clips().iter().map(|c| c.id).collect();
    editor
        .apply_checked(command, metadata, |timeline, metadata| {
            let active: std::collections::BTreeSet<_> =
                timeline.clips().iter().map(|c| c.id).collect();
            metadata.adjustments.retain(|id, _| active.contains(id));
            for clip in timeline.clips() {
                if !old.contains(&clip.id)
                    && let Some(value) = &inherited
                {
                    metadata.adjustments.insert(clip.id, value.clone());
                }
                if let Some(value) = metadata.adjustments.get_mut(&clip.id) {
                    value.fade_in = value.fade_in.min(clip.duration());
                    value.fade_out = value.fade_out.min(clip.duration());
                }
            }
            SavedProject {
                version: 2,
                application_version: "1.1.0".into(),
                clips: timeline.clips().to_vec(),
                metadata: metadata.clone(),
            }
            .validate()
            .map_err(|_| {
                editor_core::EditError(
                    "Edited project has invalid source, audio or framing settings",
                )
            })
        })
        .map_err(|e| error(e.to_string()))
}
impl SavedProject {
    pub fn capture(editor: &ProjectEditor) -> Self {
        Self {
            version: 2,
            application_version: "1.1.0".into(),
            clips: editor.timeline().clips().to_vec(),
            metadata: editor.metadata().clone(),
        }
    }
    pub fn editor(self) -> Result<ProjectEditor, ProjectError> {
        self.validate()?;
        Editor::with_metadata(
            Timeline::new(self.clips).map_err(|e| error(e.to_string()))?,
            self.metadata,
        )
        .map_err(|e| error(e.to_string()))
    }
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.version != 2 || self.metadata.sources.len() > MAX_SOURCES {
            return Err(error("Unsupported or oversized project"));
        }
        Timeline::new(self.clips.clone()).map_err(|e| error(e.to_string()))?;
        let output = &self.metadata.output;
        if output.profile == ColorProfile::Hdr10 && output.codec == Codec::H264
            || output.width < 2
            || output.height < 2
            || output.width > 16384
            || output.height > 16384
            || !output.width.is_multiple_of(2)
            || !output.height.is_multiple_of(2)
            || !output.rate.fps().is_finite()
            || output.rate.fps() <= 0.0
            || output.rate.fps() > 240.0
            || output.quality > 51
        {
            return Err(error("Invalid output profile"));
        }
        let mut ids = std::collections::HashSet::new();
        for source in &self.metadata.sources {
            validate_reference(&source.path)?;
            if source.id.0 == 0 || source.id.0 == u64::MAX {
                return Err(error("Invalid recording identity"));
            }
            if !ids.insert(source.id.0)
                || !source.duration.is_finite()
                || source.duration <= 0.0
                || source.duration > editor_core::MAX_TIMELINE_SECONDS
                || source.width == 0
                || source.height == 0
                || source.width > 32768
                || source.height > 32768
                || source.audio.len() > 256
                || !source.rate.fps().is_finite()
                || source.rate.fps() <= 0.0
                || !source.display_width.is_finite()
                || !source.display_height.is_finite()
                || source.display_width <= 0.0
                || source.display_height <= 0.0
            {
                return Err(error("Invalid recording metadata"));
            }
        }
        for clip in &self.clips {
            let source = self
                .metadata
                .sources
                .iter()
                .find(|s| s.id == clip.source)
                .ok_or_else(|| error("Clip refers to an unknown recording"))?;
            if clip.source_out > source.duration + 0.001
                || clip.duration() < source.rate.frame_seconds() - 0.0001
            {
                return Err(error(
                    "Trim must stay within its recording and keep at least one frame",
                ));
            }
            let adjustment = self
                .metadata
                .adjustments
                .get(&clip.id)
                .cloned()
                .unwrap_or_default();
            if !adjustment.gain_db.is_finite()
                || !(-60.0..=24.0).contains(&adjustment.gain_db)
                || !adjustment.fade_in.is_finite()
                || !adjustment.fade_out.is_finite()
                || adjustment.fade_in < 0.0
                || adjustment.fade_out < 0.0
                || adjustment.fade_in > clip.duration()
                || adjustment.fade_out > clip.duration()
                || !matches!(adjustment.rotation, 0 | 90 | 180 | 270)
                || adjustment
                    .crop
                    .iter()
                    .any(|v| !v.is_finite() || *v < 0.0 || *v > 1.0)
                || adjustment.crop[2] <= 0.0
                || adjustment.crop[3] <= 0.0
                || adjustment.crop[0] + adjustment.crop[2] > 1.000001
                || adjustment.crop[1] + adjustment.crop[3] > 1.000001
                || adjustment
                    .stream
                    .is_some_and(|i| !source.audio.iter().any(|s| s.index == i))
            {
                return Err(error("Invalid clip audio or framing settings"));
            }
        }
        Ok(())
    }
}
pub fn read_document(path: &Path) -> Result<serde_json::Value, ProjectError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| error(e.to_string()))?
        .take((MAX_PROJECT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| error(e.to_string()))?;
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err(error("Project exceeds 8 MiB"));
    }
    let text = if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        if !bytes.len().is_multiple_of(4) {
            return Err(error("Invalid UTF-32 project"));
        }
        let little = bytes[0] == 0xff;
        bytes[4..]
            .chunks_exact(4)
            .map(|v| {
                let value = if little {
                    u32::from_le_bytes(v.try_into().unwrap())
                } else {
                    u32::from_be_bytes(v.try_into().unwrap())
                };
                char::from_u32(value).ok_or_else(|| error("Invalid UTF-32 scalar"))
            })
            .collect::<Result<String, _>>()?
    } else if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if bytes.len() % 2 != 0 {
            return Err(error("Invalid UTF-16 project"));
        }
        String::from_utf16(
            &bytes[2..]
                .chunks_exact(2)
                .map(|v| {
                    if little {
                        u16::from_le_bytes([v[0], v[1]])
                    } else {
                        u16::from_be_bytes([v[0], v[1]])
                    }
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| error(e.to_string()))?
    } else {
        String::from_utf8(
            bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(&bytes)
                .to_vec(),
        )
        .map_err(|e| error(e.to_string()))?
    };
    serde_json::from_str(&text).map_err(|e| error(e.to_string()))
}
pub fn save_document(path: &Path, document: &SavedProject) -> Result<(), ProjectError> {
    document.validate()?;
    if !path.is_absolute() {
        return Err(error("Choose an absolute project path"));
    }
    for source in &document.metadata.sources {
        let authorized = media_path::RecordingPath::authorize(Path::new(&source.path))
            .map_err(|e| error(e.to_string()))?;
        if platform_windows::ordinal_ignore_case(&path.to_string_lossy(), &source.path)
            || path.exists()
                && authorized.as_path().exists()
                && platform_windows::same_file(path, authorized.as_path())
                    .map_err(|e| error(e.to_string()))?
        {
            return Err(error("A project cannot overwrite a source recording"));
        }
    }
    let data = serde_json::to_vec_pretty(document).map_err(|e| error(e.to_string()))?;
    if data.len() > MAX_PROJECT_BYTES {
        return Err(error("Project exceeds 8 MiB"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("Choose an absolute project path"))?;
    std::fs::create_dir_all(parent).map_err(|e| error(e.to_string()))?;
    let temporary = path.with_extension(format!("{}.tmp", unique_id()));
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        platform_windows::replace_file(&temporary, path)
    })();
    let _ = std::fs::remove_file(&temporary);
    result.map_err(|e| error(e.to_string()))
}
pub fn unique_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

pub struct Recovery {
    pub path: std::path::PathBuf,
    lease: Option<std::fs::File>,
    writes: std::sync::Mutex<()>,
    generation: std::sync::atomic::AtomicU64,
}
impl Recovery {
    pub fn new(data: &Path) -> std::io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = data.join("recovery");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.sveproject", unique_id()));
        let lease = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(path.with_extension("sveproject.lock"))?;
        Ok(Self {
            path,
            lease: Some(lease),
            writes: std::sync::Mutex::new(()),
            generation: std::sync::atomic::AtomicU64::new(0),
        })
    }
    pub fn save(&self, document: &SavedProject) -> Result<(), ProjectError> {
        self.save_revision(document, self.revision())
    }
    pub fn revision(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn save_revision(
        &self,
        document: &SavedProject,
        revision: u64,
    ) -> Result<(), ProjectError> {
        let _guard = self.writes.lock().unwrap();
        if revision != self.revision() {
            return Ok(());
        }
        save_document(&self.path, document)
    }
    pub fn clear(&self) {
        let _guard = self.writes.lock().unwrap();
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let _ = std::fs::remove_file(&self.path);
    }
    pub fn candidates(&self) -> Vec<RecoveryClaim> {
        use std::os::windows::fs::OpenOptionsExt;
        let mut candidates: Vec<_> = std::fs::read_dir(self.path.parent().unwrap())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p != &self.path && p.extension().is_some_and(|s| s == "sveproject"))
            .collect();
        let legacy = self
            .path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("recovery.sveproject");
        if legacy.is_file() {
            candidates.push(legacy);
        }
        candidates.sort_by_key(|p| {
            std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok())
        });
        let mut result = Vec::new();
        for path in candidates {
            if let Ok(lease) = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(path.with_extension("sveproject.lock"))
            {
                result.push(RecoveryClaim {
                    path,
                    lease: Some(lease),
                });
            }
        }
        result
    }
}
pub struct RecoveryClaim {
    pub path: std::path::PathBuf,
    lease: Option<std::fs::File>,
}
impl Drop for Recovery {
    fn drop(&mut self) {
        drop(self.lease.take());
        let _ = std::fs::remove_file(self.path.with_extension("sveproject.lock"));
    }
}
impl Drop for RecoveryClaim {
    fn drop(&mut self) {
        drop(self.lease.take());
        let _ = std::fs::remove_file(self.path.with_extension("sveproject.lock"));
    }
}
impl RecoveryClaim {
    pub fn discard(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
