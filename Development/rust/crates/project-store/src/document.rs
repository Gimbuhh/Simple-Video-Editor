use super::*;
use editor_core::{Editor, FrameRate};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
pub const APPLICATION_VERSION: &str = env!("SVE_APPLICATION_VERSION");

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
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "square_pixels")]
    pub sample_aspect_ratio: f64,
    pub rate: FrameRate,
    pub codec: String,
    #[serde(default)]
    pub video_stream: u32,
    pub audio: Vec<AudioStream>,
    pub transfer: String,
    pub primaries: String,
    pub matrix: String,
    pub range: String,
    pub pixel_format: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chroma_location: String,
}
fn square_pixels() -> f64 {
    1.0
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceInterpretation {
    #[default]
    Auto,
    Sdr709,
    Srgb,
    Pq2020,
    Hlg2020,
}
impl MediaSource {
    pub fn interpreted(&self, interpretation: SourceInterpretation) -> Self {
        let mut source = self.clone();
        let (transfer, primaries) = match interpretation {
            SourceInterpretation::Auto => return source,
            SourceInterpretation::Sdr709 => ("bt709", "bt709"),
            SourceInterpretation::Srgb => ("iec61966-2-1", "bt709"),
            SourceInterpretation::Pq2020 => ("smpte2084", "bt2020"),
            SourceInterpretation::Hlg2020 => ("arib-std-b67", "bt2020"),
        };
        source.transfer = transfer.into();
        source.primaries = primaries.into();
        source.matrix = if source.pixel_format.starts_with("gbr")
            || source.pixel_format.starts_with("rgb")
            || source.pixel_format.starts_with("bgr")
        {
            "gbr"
        } else if primaries == "bt2020" {
            "bt2020nc"
        } else {
            "bt709"
        }
        .into();
        source
    }
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
    /// Audio lives in an independent timeline item after separation.
    pub audio_separated: bool,
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
            audio_separated: false,
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
    pub audio_items: Vec<AudioItem>,
    pub sources: Vec<MediaSource>,
    pub output: OutputSettings,
    pub adjustments: BTreeMap<ClipId, ClipSettings>,
    pub interpretations: BTreeMap<SourceId, SourceInterpretation>,
    /// Captured with undo/redo; never written into project or recovery files.
    #[serde(skip)]
    pub view: Option<ViewSelection>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewSelection {
    Clip(ClipId),
    Audio(ClipId),
    Recording(SourceId),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioItem {
    pub clip: Clip,
    pub settings: ClipSettings,
}
impl ProjectMetadata {
    pub fn audio_timeline(&self) -> Result<Timeline, editor_core::EditError> {
        Timeline::new(self.audio_items.iter().map(|a| a.clip.clone()).collect())
    }
    pub fn duration(&self, timeline: &Timeline) -> f64 {
        self.audio_items
            .iter()
            .map(|a| a.clip.timeline_end())
            .fold(timeline.duration(), f64::max)
    }
    pub fn schema_version(&self) -> u32 {
        if !self.audio_items.is_empty() || self.adjustments.values().any(|a| a.audio_separated) {
            3
        } else {
            2
        }
    }
}

/// Detach without rounding original source timestamps or changing video placement.
pub fn separate_audio(editor: &mut ProjectEditor, id: ClipId) -> Result<ClipId, ProjectError> {
    let mut metadata = editor.metadata().clone();
    let mut clip = editor
        .timeline()
        .clips()
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| error("Select a video section"))?;
    let source = metadata
        .sources
        .iter()
        .find(|s| s.id == clip.source)
        .ok_or_else(|| error("Unknown recording"))?;
    if source.audio.is_empty()
        || metadata
            .adjustments
            .get(&id)
            .is_some_and(|a| a.audio_separated)
    {
        return Err(error("This section has no linked audio"));
    }
    clip.id = ClipId(
        metadata
            .audio_items
            .iter()
            .map(|a| a.clip.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .filter(|id| *id < u64::MAX)
            .ok_or_else(|| error("Audio identity exhausted"))?,
    );
    let settings = metadata.adjustments.entry(id).or_default();
    metadata.audio_items.push(AudioItem {
        clip: clip.clone(),
        settings: settings.clone(),
    });
    settings.audio_separated = true;
    metadata
        .audio_items
        .sort_by(|a, b| a.clip.timeline_start.total_cmp(&b.clip.timeline_start));
    commit_audio_metadata(editor, metadata)?;
    Ok(clip.id)
}

pub fn edit_audio(
    editor: &mut ProjectEditor,
    command: editor_core::Command,
) -> Result<(), ProjectError> {
    let mut metadata = editor.metadata().clone();
    let inherited = match &command {
        editor_core::Command::Split { clip, .. } | editor_core::Command::Duplicate { clip, .. } => {
            metadata
                .audio_items
                .iter()
                .find(|a| a.clip.id == *clip)
                .map(|a| a.settings.clone())
        }
        _ => None,
    };
    let mut audio = Editor::new(
        metadata
            .audio_timeline()
            .map_err(|e| error(e.to_string()))?,
    )
    .map_err(|e| error(e.to_string()))?;
    audio.apply(command).map_err(|e| error(e.to_string()))?;
    metadata.audio_items = audio
        .timeline()
        .clips()
        .iter()
        .map(|clip| {
            let mut settings = metadata
                .audio_items
                .iter()
                .find(|a| a.clip.id == clip.id)
                .map(|a| a.settings.clone())
                .or_else(|| inherited.clone())
                .unwrap_or_default();
            settings.fade_in = settings.fade_in.min(clip.duration());
            settings.fade_out = settings.fade_out.min(clip.duration());
            AudioItem {
                clip: clip.clone(),
                settings,
            }
        })
        .collect();
    commit_audio_metadata(editor, metadata)
}
fn commit_audio_metadata(
    editor: &mut ProjectEditor,
    metadata: ProjectMetadata,
) -> Result<(), ProjectError> {
    let mut document = SavedProject::capture(editor);
    document.version = metadata.schema_version();
    document.metadata = metadata;
    document.validate()?;
    editor
        .apply_with_metadata(
            editor_core::Command::Replace {
                clips: document.clips,
            },
            document.metadata,
        )
        .map_err(|e| error(e.to_string()))
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
/// Preview the final retained frame at the sequence endpoint. Timeline source
/// intervals themselves remain exclusive for edits and export planning.
pub fn preview_at(
    timeline: &Timeline,
    metadata: &ProjectMetadata,
    time: f64,
) -> Option<(ClipId, SourceId, f64)> {
    if let Some(at) = timeline.source_at(time) {
        return Some(at);
    }
    if time.is_finite() && time >= timeline.duration() && timeline.duration() > 0.0 {
        let clip = timeline.clips().last()?;
        let rate = metadata.sources.iter().find(|s| s.id == clip.source)?.rate;
        return Some((
            clip.id,
            clip.source,
            (clip.source_out - rate.frame_seconds()).max(clip.source_in),
        ));
    }
    None
}
pub fn settle_preview_time(
    timeline: &Timeline,
    metadata: &ProjectMetadata,
    time: f64,
    recording: Option<SourceId>,
) -> f64 {
    if let Some(source) = recording.and_then(|id| metadata.sources.iter().find(|s| s.id == id)) {
        let fps = source.rate.fps();
        return ((time * fps).round_ties_even() / fps).clamp(
            0.0,
            (source.duration - source.rate.frame_seconds()).max(0.0),
        );
    }
    if let Some((id, source, position)) = preview_at(timeline, metadata, time)
        && let Some(clip) = timeline.clips().iter().find(|c| c.id == id)
        && let Some(source) = metadata.sources.iter().find(|s| s.id == source)
    {
        let fps = source.rate.fps();
        let position =
            (clip.source_in + ((position - clip.source_in) * fps).round_ties_even() / fps).clamp(
                clip.source_in,
                (clip.source_out - source.rate.frame_seconds()).max(clip.source_in),
            );
        return clip.timeline_start + position - clip.source_in;
    }
    time
}
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
                version: metadata.schema_version(),
                application_version: APPLICATION_VERSION.into(),
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
/// Remove a library identity and all of its placements in one history step.
/// Remaining placements retain their original timeline positions and gaps.
pub fn remove_source(editor: &mut ProjectEditor, id: SourceId) -> Result<(), ProjectError> {
    let mut document = SavedProject::capture(editor);
    document.metadata.view = editor.metadata().view;
    document.metadata.sources.retain(|s| s.id != id);
    document.metadata.interpretations.remove(&id);
    document.clips.retain(|c| c.source != id);
    document
        .metadata
        .audio_items
        .retain(|a| a.clip.source != id);
    document
        .metadata
        .adjustments
        .retain(|id, _| document.clips.iter().any(|c| c.id == *id));
    document.validate()?;
    editor
        .apply_with_metadata(
            editor_core::Command::Replace {
                clips: document.clips,
            },
            document.metadata,
        )
        .map_err(|e| error(e.to_string()))
}
impl SavedProject {
    pub fn capture(editor: &ProjectEditor) -> Self {
        let mut metadata = editor.metadata().clone();
        metadata.view = None;
        Self {
            version: metadata.schema_version(),
            application_version: APPLICATION_VERSION.into(),
            clips: editor.timeline().clips().to_vec(),
            metadata,
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
        if !matches!(self.version, 2 | 3)
            || self.version < self.metadata.schema_version()
            || self.metadata.sources.len() > MAX_SOURCES
        {
            return Err(error("Unsupported or oversized project"));
        }
        if self.clips.len() + self.metadata.audio_items.len() > editor_core::MAX_CLIPS {
            return Err(error("Too many timeline items"));
        }
        if !self.metadata.audio_items.is_empty() {
            // Apply the same bounded source/trim/stream/gain validation to the audio lane.
            let mut audio = self.clone();
            audio.clips = self
                .metadata
                .audio_items
                .iter()
                .map(|a| a.clip.clone())
                .collect();
            audio.metadata.adjustments = self
                .metadata
                .audio_items
                .iter()
                .map(|a| (a.clip.id, a.settings.clone()))
                .collect();
            audio.metadata.audio_items.clear();
            for item in &self.metadata.audio_items {
                if item.settings.audio_separated
                    || self
                        .metadata
                        .sources
                        .iter()
                        .find(|s| s.id == item.clip.source)
                        .is_none_or(|s| s.audio.is_empty())
                {
                    return Err(error("Audio item needs a recording audio stream"));
                }
            }
            audio.validate()?;
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
                || !source.rotation.is_finite()
                || !(0.0..360.0).contains(&source.rotation)
                || !source.sample_aspect_ratio.is_finite()
                || source.sample_aspect_ratio <= 0.0
                || source.sample_aspect_ratio > 100.0
            {
                return Err(error("Invalid recording metadata"));
            }
        }
        for clip in &self.clips {
            if clip.id.0 == 0 || clip.id.0 == u64::MAX {
                return Err(error("Invalid section identity"));
            }
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
