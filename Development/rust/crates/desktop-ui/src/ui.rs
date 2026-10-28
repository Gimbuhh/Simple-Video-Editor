mod interaction_verification;
mod preview_benchmark;
mod theme;
mod thumbnails;
use crate::playback::{Playback, Request};
use editor_core::{Clip, ClipId, Command, Editor, SourceId, Timeline};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use media_engine::{Cancel, Tools, uncancelled};
use project_store::*;
use raw_window_handle::HasWindowHandle;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

enum Event {
    Tools(Result<Tools, String>),
    Imported(Vec<MediaSource>, Vec<String>, Option<f64>),
    Opened(Result<(SavedProject, PathBuf, bool), String>),
    Saved(Result<(SavedProject, PathBuf), String>),
    Recovery(u64, SavedProject, Result<(), String>),
    Waveform(MediaSource, Vec<f32>),
    Thumbnail(MediaSource, egui::ColorImage),
    Cache(Result<(usize, u64), String>),
    Progress(f64, String),
    Exported(Result<(PathBuf, bool), String>),
    Missing(PathBuf, Vec<String>),
}
#[derive(Clone)]
struct EventSender {
    sender: mpsc::Sender<Event>,
    context: egui::Context,
}
impl EventSender {
    fn send(&self, event: Event) -> bool {
        let sent = self.sender.send(event).is_ok();
        self.context.request_repaint();
        sent
    }
}
struct Drag {
    base: Timeline,
    clip: ClipId,
    start: f64,
    mode: u8,
    source_in: f64,
    source_out: f64,
    pointer_x: f32,
    playhead: f64,
    preview_source: f64,
}
type RelinkState = (PathBuf, Vec<String>, Vec<(String, PathBuf)>);
struct Scrub {
    owner: u8,
    resume: bool,
}
struct Settling {
    generation: Option<u64>,
    resume: bool,
    started: Instant,
}
struct Stepping {
    generation: Option<u64>,
    count: i32,
    trim: Option<(ClipId, bool)>,
}
pub struct EditorApp {
    // The actor must be destroyed before its native child window.
    playback: Option<Playback>,
    video: platform_windows::VideoWindow,
    owner: isize,
    editor: ProjectEditor,
    tools: Option<Tools>,
    tx: EventSender,
    rx: mpsc::Receiver<Event>,
    jobs: Vec<std::thread::JoinHandle<()>>,
    cancel: Cancel,
    busy: bool,
    exporting: bool,
    progress: f64,
    message: String,
    path: Option<PathBuf>,
    saved: SavedProject,
    dirty_cache: bool,
    selected: Option<ClipId>,
    library_selected: Option<SourceId>,
    library_preview: bool,
    search: String,
    playhead: f64,
    playing: bool,
    scrub: Option<Scrub>,
    settling: Option<Settling>,
    stepping: Option<Stepping>,
    ignore_scrub_until_release: bool,
    last_tick: Instant,
    generation: u64,
    request_key: String,
    volume: f64,
    zoom: f32,
    drag: Option<Drag>,
    preview_timeline: Option<Timeline>,
    waveforms: BTreeMap<SourceId, Arc<Vec<f32>>>,
    thumbnails: BTreeMap<SourceId, egui::TextureHandle>,
    timeline_thumbnails: Option<thumbnails::TimelineThumbnails>,
    recovery: Arc<Recovery>,
    recovery_saved: Option<SavedProject>,
    recovery_tick: Instant,
    startup_recovery: Option<RecoveryClaim>,
    restoring: Option<RecoveryClaim>,
    close_approved: bool,
    settings: bool,
    hdr_preview: bool,
    software_preview: bool,
    preview_texture: Option<egui::TextureHandle>,
    preview_frame: Option<Arc<native_mpv::Frame>>,
    controls: BTreeMap<&'static str, Rect>,
    timeline_focus: Option<(egui::Id, ClipId)>,
    keyboard_menu: Option<(egui::Id, Pos2)>,
    verification: Option<crate::verification::Verification>,
    relink: Option<RelinkState>,
    startup_paths: Vec<PathBuf>,
    timeline_drop: Option<(Rect, f32, f32)>,
    window_title: String,
}
impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> media_engine::Result<Self> {
        let owner = match cc.window_handle()?.as_raw() {
            raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get(),
            _ => return Err("Windows is required".into()),
        };
        let video = platform_windows::VideoWindow::new(owner)?;
        theme::configure(&cc.egui_ctx);
        let verification = crate::verification::Verification::arguments();
        let data = if let Some(test) = &verification {
            test.directory.join("data")
        } else {
            PathBuf::from(
                std::env::var_os("LOCALAPPDATA")
                    .ok_or("Windows application data directory is unavailable")?,
            )
            .join("SimpleVideoEditor")
        };
        let recovery = Arc::new(Recovery::new(&data)?);
        let startup_paths = crate::startup::paths(std::env::args_os().skip(1));
        // Explicit files take priority, as in 1.0.x. Other interrupted sessions
        // remain available on the next launch without filenames.
        let startup_recovery = if startup_paths.is_empty() {
            recovery.candidates().into_iter().next()
        } else {
            None
        };
        let editor = Editor::with_metadata(Timeline::default(), ProjectMetadata::default())?;
        let saved = SavedProject::capture(&editor);
        let (sender, rx) = mpsc::channel();
        let tx = EventSender {
            sender,
            context: cc.egui_ctx.clone(),
        };
        let sender = tx.clone();
        let cache = data.join("cache");
        let job = std::thread::spawn(move || {
            let _ = sender.send(Event::Tools(
                Tools::discover(cache).map_err(|e| e.to_string()),
            ));
        });
        Ok(Self {
            playback: None,
            video,
            owner,
            editor,
            tools: None,
            tx,
            rx,
            jobs: vec![job],
            cancel: uncancelled(),
            busy: false,
            exporting: false,
            progress: 0.0,
            message: "Starting media tools…".into(),
            path: None,
            saved,
            dirty_cache: false,
            selected: None,
            library_selected: None,
            library_preview: false,
            search: String::new(),
            playhead: 0.0,
            playing: false,
            scrub: None,
            settling: None,
            stepping: None,
            ignore_scrub_until_release: false,
            last_tick: Instant::now(),
            generation: 0,
            request_key: String::new(),
            volume: 70.0,
            zoom: 90.0,
            drag: None,
            preview_timeline: None,
            waveforms: BTreeMap::new(),
            thumbnails: BTreeMap::new(),
            timeline_thumbnails: None,
            recovery,
            recovery_saved: None,
            recovery_tick: Instant::now(),
            startup_recovery,
            restoring: None,
            close_approved: false,
            settings: false,
            hdr_preview: true,
            software_preview: std::env::args_os().any(|a| a == "--software-preview"),
            preview_texture: None,
            preview_frame: None,
            controls: BTreeMap::new(),
            timeline_focus: None,
            keyboard_menu: None,
            verification,
            relink: None,
            startup_paths,
            timeline_drop: None,
            window_title: String::new(),
        })
    }
    fn dirty(&self) -> bool {
        self.dirty_cache
    }
    fn update_dirty(&mut self) {
        self.dirty_cache = SavedProject::capture(&self.editor) != self.saved;
    }
    fn history(&mut self, redo: bool) {
        let interpretations = self.editor.metadata().interpretations.clone();
        self.editor.metadata_mut().view = self.current_view();
        if if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        } {
            self.playing = false;
            self.scrub = None;
            self.settling = None;
            self.stepping = None;
            self.drag = None;
            self.preview_timeline = None;
            self.ignore_scrub_until_release = true;
            match self.editor.metadata().view {
                Some(ViewSelection::Clip(id)) => {
                    self.selected = Some(id);
                    self.library_preview = false;
                }
                Some(ViewSelection::Recording(id)) => {
                    self.library_selected = Some(id);
                    self.library_preview = true;
                }
                None => {}
            }
            self.refresh_interpretations(&interpretations);
            self.sync_selection();
            self.playhead = if self.library_preview {
                0.0
            } else {
                self.selected_clip().map_or(0.0, |c| c.timeline_start)
            };
            self.update_dirty();
            self.request_key.clear();
        }
    }
    fn current_view(&self) -> Option<ViewSelection> {
        if self.library_preview {
            self.library_selected.map(ViewSelection::Recording)
        } else {
            self.selected.map(ViewSelection::Clip)
        }
    }
    fn sync_selection(&mut self) {
        if self
            .selected
            .is_none_or(|id| !self.editor.timeline().clips().iter().any(|c| c.id == id))
        {
            self.selected = preview_at(
                self.editor.timeline(),
                self.editor.metadata(),
                self.playhead,
            )
            .map(|(id, _, _)| id)
            .or_else(|| self.editor.timeline().clips().first().map(|c| c.id));
        }
        if self
            .library_selected
            .is_none_or(|id| !self.editor.metadata().sources.iter().any(|s| s.id == id))
        {
            self.library_selected = self.editor.metadata().sources.first().map(|s| s.id);
        }
        if self.editor.timeline().clips().is_empty() {
            self.library_preview = self.library_selected.is_some();
        }
        if self.library_preview && self.library_selected.is_none() {
            self.library_preview = false;
        }
        if !self.library_preview
            && let Some(clip) = self.selected_clip()
        {
            self.library_selected = Some(clip.source);
        }
        self.playhead = self.playhead.clamp(0.0, self.preview_duration());
    }
    fn discard(&mut self) -> bool {
        self.playing = false;
        !self.dirty()
            || platform_windows::confirm(self.owner, "Discard unsaved changes?", "Unsaved project")
    }
    fn fail(&mut self, message: impl ToString) {
        self.message = message.to_string();
    }
    fn clear_preview(&mut self) {
        drop(self.playback.take());
        self.timeline_thumbnails = None;
        self.preview_frame = None;
        self.preview_texture = None;
    }
    fn new_project(&mut self) {
        if self.busy || !self.discard() {
            return;
        }
        self.cancel.store(true, Ordering::Release);
        self.clear_preview();
        self.cancel = uncancelled();
        self.editor =
            Editor::with_metadata(Timeline::default(), ProjectMetadata::default()).unwrap();
        self.saved = SavedProject::capture(&self.editor);
        self.update_dirty();
        self.path = None;
        self.playhead = 0.0;
        self.selected = None;
        self.library_selected = None;
        self.library_preview = false;
        self.search.clear();
        self.message.clear();
        self.request_key.clear();
        self.waveforms.clear();
        self.thumbnails.clear();
        self.scrub = None;
        self.settling = None;
        self.stepping = None;
        self.drag = None;
        self.preview_timeline = None;
        self.recovery.clear();
        self.recovery_saved = None;
    }
    fn open_failed(&mut self, message: impl ToString) {
        if self.restoring.is_some() {
            self.startup_recovery = self.restoring.take();
        }
        self.fail(message);
    }
    fn timeline(&self) -> &Timeline {
        self.preview_timeline
            .as_ref()
            .unwrap_or(self.editor.timeline())
    }
    fn apply(&mut self, command: Command) {
        if self.busy && !self.exporting {
            return;
        }
        self.editor.metadata_mut().view = self.current_view();
        let select_new = matches!(
            &command,
            Command::Split { .. }
                | Command::Duplicate { .. }
                | Command::Append { .. }
                | Command::Insert { .. }
        );
        let previous: std::collections::BTreeSet<_> = if select_new {
            self.editor
                .timeline()
                .clips()
                .iter()
                .map(|c| c.id)
                .collect()
        } else {
            Default::default()
        };
        match edit_project(&mut self.editor, command) {
            Ok(()) => {
                self.playing = false;
                self.scrub = None;
                self.settling = None;
                self.stepping = None;
                self.library_preview = false;
                if select_new
                    && let Some(clip) = self
                        .editor
                        .timeline()
                        .clips()
                        .iter()
                        .find(|c| !previous.contains(&c.id))
                {
                    self.selected = Some(clip.id);
                    self.playhead = clip.timeline_start;
                }
                self.sync_selection();
                self.update_dirty();
                self.request_key.clear();
            }
            Err(error) => self.fail(error),
        }
    }
    fn metadata(&mut self, mut metadata: ProjectMetadata) {
        if self.busy && !self.exporting {
            return;
        }
        self.stepping = None;
        let view = self.current_view();
        self.editor.metadata_mut().view = view;
        metadata.view = view;
        let interpretations = self.editor.metadata().interpretations.clone();
        let document = SavedProject {
            metadata: metadata.clone(),
            ..SavedProject::capture(&self.editor)
        };
        match document.validate() {
            Ok(()) => {
                if let Err(e) = self.editor.apply_with_metadata(
                    Command::Replace {
                        clips: self.editor.timeline().clips().to_vec(),
                    },
                    metadata,
                ) {
                    self.fail(e);
                }
                self.request_key.clear();
            }
            Err(e) => self.fail(e),
        }
        self.update_dirty();
        self.refresh_interpretations(&interpretations);
    }
    fn refresh_interpretations(&mut self, previous: &BTreeMap<SourceId, SourceInterpretation>) {
        let changed: Vec<_> = self
            .editor
            .metadata()
            .sources
            .iter()
            .filter(|s| previous.get(&s.id) != self.editor.metadata().interpretations.get(&s.id))
            .map(|s| s.id)
            .collect();
        if changed.is_empty() {
            return;
        }
        for id in changed {
            self.waveforms.remove(&id);
            self.thumbnails.remove(&id);
        }
        self.start_waveforms();
    }
    fn add(&mut self, ids: Vec<SourceId>) {
        self.add_at(ids, None);
    }
    fn add_at(&mut self, ids: Vec<SourceId>, position: Option<f64>) {
        let previous: std::collections::BTreeSet<_> = self
            .editor
            .timeline()
            .clips()
            .iter()
            .map(|c| c.id)
            .collect();
        let sections = ids
            .iter()
            .filter_map(|id| self.editor.metadata().sources.iter().find(|s| s.id == *id))
            .map(|s| {
                (
                    Clip {
                        id: ClipId(0),
                        source: s.id,
                        source_in: 0.0,
                        source_out: s.duration,
                        timeline_start: 0.0,
                    },
                    s.rate,
                )
            })
            .collect();
        self.apply(if let Some(position) = position {
            Command::Insert {
                sections,
                position,
                snap: 10.0 / f64::from(self.zoom),
            }
        } else {
            Command::Append {
                sections: sections.into_iter().map(|(clip, _)| clip).collect(),
            }
        });
        if let Some(clip) = self
            .editor
            .timeline()
            .clips()
            .iter()
            .find(|c| !previous.contains(&c.id))
            .cloned()
        {
            self.selected = Some(clip.id);
            self.library_preview = false;
            self.seek(clip.timeline_start);
        }
    }
    fn seek(&mut self, time: f64) {
        self.stepping = None;
        self.playhead = time.clamp(0.0, self.preview_duration());
        if !self.library_preview
            && let Some((id, source, _)) =
                preview_at(self.timeline(), self.editor.metadata(), self.playhead)
        {
            self.selected = Some(id);
            self.library_selected = Some(source);
        }
        self.request_key.clear();
    }
    fn step_frame(&mut self, direction: i32) {
        self.playing = false;
        self.scrub = None;
        self.settling = None;
        if !self.library_preview
            && direction < 0
            && let Some((id, source, time)) =
                preview_at(self.timeline(), self.editor.metadata(), self.playhead)
            && let (Some(clip), Some(source)) = (
                self.timeline().clips().iter().find(|c| c.id == id),
                self.editor
                    .metadata()
                    .sources
                    .iter()
                    .find(|s| s.id == source),
            )
            && time <= clip.source_in + 0.000001
        {
            self.seek(self.playhead - source.rate.frame_seconds());
            return;
        }
        if !self.library_preview && self.timeline().source_at(self.playhead).is_none() {
            self.seek(
                self.playhead
                    + f64::from(direction) * self.editor.metadata().output.rate.frame_seconds(),
            );
            return;
        }
        let count = (self
            .stepping
            .as_ref()
            .filter(|s| s.trim.is_none())
            .map_or(0, |s| s.count)
            + direction)
            .clamp(-120, 120);
        self.stepping = Some(Stepping {
            generation: None,
            count,
            trim: None,
        });
        self.request_key.clear();
    }
    fn play_intent(&self) -> bool {
        self.scrub
            .as_ref()
            .map(|s| s.resume)
            .or_else(|| self.settling.as_ref().map(|s| s.resume))
            .unwrap_or(self.playing)
    }
    fn toggle_playback(&mut self) {
        if self.stepping.take().is_some() {
            self.request_key.clear();
        }
        if let Some(scrub) = &mut self.scrub {
            scrub.resume = !scrub.resume;
            return;
        }
        if let Some(settling) = &mut self.settling {
            settling.resume = !settling.resume;
            return;
        }
        if self.playhead >= self.preview_duration() {
            self.seek(0.0);
        }
        if self
            .playback
            .as_ref()
            .is_some_and(|p| p.status().error.is_some())
        {
            self.request_key.clear();
        }
        self.playing = !self.playing && self.preview_duration() > 0.0;
        self.last_tick = Instant::now();
    }
    fn begin_scrub(&mut self, owner: u8) {
        if self.ignore_scrub_until_release || self.scrub.is_some() {
            return;
        }
        let resume = self.play_intent();
        self.stepping = None;
        self.request_key.clear();
        self.settling = None;
        self.scrub = Some(Scrub { owner, resume });
        self.playing = false;
    }
    fn end_scrub(&mut self, resume: Option<bool>) {
        if let Some(scrub) = self.scrub.take() {
            let time = settle_preview_time(
                self.timeline(),
                self.editor.metadata(),
                self.playhead,
                if self.library_preview {
                    self.library_selected
                } else {
                    None
                },
            );
            self.seek(time);
            self.settling = Some(Settling {
                generation: None,
                resume: resume.unwrap_or(scrub.resume),
                started: Instant::now(),
            });
            self.request_key.clear();
        }
    }
    fn preview_duration(&self) -> f64 {
        if self.library_preview {
            self.library_selected
                .and_then(|id| self.editor.metadata().sources.iter().find(|s| s.id == id))
                .map_or(0.0, |s| s.duration)
        } else {
            self.timeline().duration()
        }
    }
    fn preview_output(&self) -> OutputSettings {
        let mut output = self.editor.metadata().output.clone();
        if self.library_preview
            && let Some(source) = self
                .library_selected
                .and_then(|id| self.editor.metadata().sources.iter().find(|s| s.id == id))
        {
            let scale = (16384.0 / source.display_width.max(source.display_height)).min(1.0);
            output.width = ((source.display_width * scale).round() as u32)
                .max(2)
                .next_multiple_of(2);
            output.height = ((source.display_height * scale).round() as u32)
                .max(2)
                .next_multiple_of(2);
        }
        output
    }
    fn selected_clip(&self) -> Option<Clip> {
        self.selected
            .and_then(|id| self.editor.timeline().clips().iter().find(|c| c.id == id))
            .cloned()
    }
    fn split(&mut self) {
        if self.library_preview {
            return;
        }
        if let Some((id, source, time)) = self.timeline().source_at(self.playhead)
            && let Some(rate) = self
                .editor
                .metadata()
                .sources
                .iter()
                .find(|s| s.id == source)
                .map(|s| s.rate)
        {
            self.selected = Some(id);
            self.apply(Command::Split {
                clip: id,
                source_time: time,
                rate,
            });
        }
    }
    fn trim(&mut self, start: Option<f64>, end: Option<f64>) {
        if self.library_preview {
            return;
        }
        if let Some(clip) = self.selected_clip()
            && let Some(source) = self
                .editor
                .metadata()
                .sources
                .iter()
                .find(|s| s.id == clip.source)
                .cloned()
        {
            let retained = clip.source_in + self.playhead - clip.timeline_start;
            self.apply(Command::Trim {
                clip: clip.id,
                start: start.unwrap_or(clip.source_in),
                end: end.unwrap_or(clip.source_out),
                source_duration: source.duration,
                rate: source.rate,
            });
            if let Some(updated) = self
                .editor
                .timeline()
                .clips()
                .iter()
                .find(|c| c.id == clip.id)
            {
                let time = updated.timeline_start
                    + retained.clamp(
                        updated.source_in,
                        (updated.source_out - source.rate.frame_seconds()).max(updated.source_in),
                    )
                    - updated.source_in;
                self.seek(time);
            }
        }
    }
    fn quick_trim(&mut self, before: bool) {
        if self.library_preview {
            return;
        }
        if let Some(clip) = self.selected_clip() {
            let relative = self.playhead - clip.timeline_start;
            if relative < 0.0 || relative >= clip.duration() {
                return;
            }
            let current = clip.source_in + relative;
            let status = self
                .playback
                .as_ref()
                .map(|p| p.status())
                .unwrap_or_default();
            // seek() invalidates the request key before the next UI tick
            // assigns a generation. An old ready status cannot acknowledge it.
            let current = if status.ready
                && status.generation == self.generation
                && !self.playing
                && !self.request_key.is_empty()
            {
                status.position.clamp(
                    clip.source_in,
                    (clip.source_out - 0.000001).max(clip.source_in),
                )
            } else {
                current
            };
            self.playing = false;
            self.scrub = None;
            self.settling = None;
            self.seek(clip.timeline_start + current - clip.source_in);
            self.stepping = Some(Stepping {
                generation: None,
                count: if before { 0 } else { 1 },
                trim: Some((clip.id, before)),
            });
        }
    }
    fn import(&mut self, paths: Vec<PathBuf>) {
        self.import_at(paths, None);
    }
    fn import_at(&mut self, paths: Vec<PathBuf>, position: Option<f64>) {
        let Some(tools) = self.tools.clone() else {
            return;
        };
        if self.busy || paths.is_empty() {
            return;
        }
        if paths.len() > 10_000_usize.saturating_sub(self.editor.metadata().sources.len()) {
            self.fail("The recording library is limited to 10,000 items");
            return;
        }
        self.busy = true;
        self.cancel = uncancelled();
        self.message = "Reading recordings…".into();
        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        let next = self
            .editor
            .metadata()
            .sources
            .iter()
            .map(|s| s.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .filter(|n| n.checked_add(paths.len() as u64).is_some());
        let Some(next) = next else {
            self.busy = false;
            self.fail("The project has exhausted its recording identities");
            return;
        };
        self.jobs.push(std::thread::spawn(move || {
            let mut sources = Vec::new();
            let mut errors = Vec::new();
            for (i, path) in paths.iter().enumerate() {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                match tools.probe(path, SourceId(next + i as u64), &cancel) {
                    Ok(source) => sources.push(source),
                    Err(e) => errors.push(format!(
                        "{}: {e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )),
                }
            }
            let _ = tx.send(Event::Imported(sources, errors, position));
        }));
    }
    fn open(&mut self, path: PathBuf) {
        if self.busy || !self.discard() {
            return;
        }
        self.open_relinked(path, Vec::new());
    }
    fn open_relinked(&mut self, path: PathBuf, replacements: Vec<(String, PathBuf)>) {
        let Some(tools) = self.tools.clone() else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.cancel = uncancelled();
        self.message = "Opening project…".into();
        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        let recovery = self.recovery.clone();
        let restoring = self.restoring.is_some();
        self.jobs.push(std::thread::spawn(move || {
            if replacements.is_empty() {
                match tools.project_sources(&path).and_then(|paths| {
                    paths
                        .into_iter()
                        .map(|p| {
                            media_path::RecordingPath::authorize(std::path::Path::new(&p))
                                .map(|a| (p, a.as_path().is_file()))
                                .map_err(Into::into)
                        })
                        .collect::<media_engine::Result<Vec<_>>>()
                }) {
                    Ok(paths) => {
                        let missing: Vec<_> = paths
                            .into_iter()
                            .filter(|(_, exists)| !exists)
                            .map(|(p, _)| p)
                            .collect();
                        if !missing.is_empty() {
                            let _ = tx.send(Event::Missing(path, missing));
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Opened(Err(e.to_string())));
                        return;
                    }
                }
            }
            let result = (|| {
                let value = read_document(&path)?;
                let legacy = value.as_object().is_some_and(|m| {
                    m.iter()
                        .any(|(k, v)| k.eq_ignore_ascii_case("Version") && v.as_u64() == Some(1))
                });
                let document = tools.load_project_relinked(&path, &replacements, &cancel)?;
                if restoring {
                    recovery.save(&document)?;
                } else {
                    recovery.clear();
                }
                Ok((document, path, legacy))
            })()
            .map_err(|e: Box<dyn std::error::Error + Send + Sync>| e.to_string());
            let _ = tx.send(Event::Opened(result));
        }));
    }
    fn save(&mut self, force_dialog: bool) {
        if self.busy {
            return;
        }
        let path = if force_dialog || self.path.is_none() {
            match platform_windows::choose_files(self.owner, true, true) {
                Ok(paths) => paths.into_iter().next(),
                Err(e) => {
                    self.fail(e);
                    None
                }
            }
        } else {
            self.path.clone()
        };
        let Some(path) = path else { return };
        let document = SavedProject::capture(&self.editor);
        let tx = self.tx.clone();
        self.busy = true;
        let recovery = self.recovery.clone();
        self.jobs.push(std::thread::spawn(move || {
            let result = save_document(&path, &document)
                .map(|()| {
                    recovery.clear();
                    (document, path)
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::Saved(result));
        }));
    }
    fn export(&mut self) {
        let Some(tools) = self.tools.clone() else {
            return;
        };
        if self.busy {
            return;
        }
        let path = match platform_windows::choose_files(self.owner, true, false) {
            Ok(p) => p.into_iter().next(),
            Err(e) => {
                self.fail(e);
                None
            }
        };
        let Some(path) = path else { return };
        self.playing = false;
        self.busy = true;
        self.exporting = true;
        self.progress = 0.0;
        self.cancel = uncancelled();
        self.message = "Preparing export…".into();
        let document = SavedProject::capture(&self.editor);
        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        self.jobs.push(std::thread::spawn(move || {
            let result = tools
                .export_adaptive(&document, &path, &cancel, |p| {
                    let _ = tx.send(Event::Progress(p.fraction, p.message));
                })
                .map(|gpu| (path, gpu))
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::Exported(result));
        }));
    }
    fn start_waveforms(&mut self) {
        let Some(tools) = self.tools.clone() else {
            return;
        };
        let sources: Vec<_> = self
            .editor
            .metadata()
            .sources
            .iter()
            .filter(|s| !self.waveforms.contains_key(&s.id))
            .map(|s| {
                s.interpreted(
                    self.editor
                        .metadata()
                        .interpretations
                        .get(&s.id)
                        .copied()
                        .unwrap_or_default(),
                )
            })
            .collect();
        if sources.is_empty() {
            return;
        }
        for source in &sources {
            self.waveforms.insert(source.id, Arc::new(Vec::new()));
        }
        let waveform_sources = sources.clone();
        let waveform_tools = tools.clone();
        let waveform_tx = self.tx.clone();
        let waveform_cancel = self.cancel.clone();
        self.jobs.push(std::thread::spawn(move || {
            for source in waveform_sources {
                if waveform_cancel.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(peaks) = waveform_tools.waveform(&source, &waveform_cancel) {
                    let _ = waveform_tx.send(Event::Waveform(source, peaks));
                }
            }
        }));
        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        self.jobs.push(std::thread::spawn(move || {
            for source in sources {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(image) = tools.thumbnail(&source, &cancel).and_then(read_thumbnail) {
                    let image = image.to_rgba8();
                    let color = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    let _ = tx.send(Event::Thumbnail(source, color));
                }
            }
        }));
    }
    fn is_current_source(&self, source: &MediaSource) -> bool {
        self.editor.metadata().sources.iter().any(|s| {
            s.id == source.id
                && s.interpreted(
                    self.editor
                        .metadata()
                        .interpretations
                        .get(&s.id)
                        .copied()
                        .unwrap_or_default(),
                ) == *source
        })
    }
    fn events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Missing(path, missing) => {
                    self.busy = false;
                    self.relink = Some((path, missing, Vec::new()));
                    self.message = "Locate the missing recordings to open this project".into();
                }
                Event::Tools(result) => match result {
                    Ok(tools) => {
                        self.tools = Some(tools);
                        self.message.clear();
                    }
                    Err(e) => self.fail(e),
                },
                Event::Imported(sources, errors, position) => {
                    self.busy = false;
                    let mut metadata = self.editor.metadata().clone();
                    let paths: Vec<_> = sources.iter().map(|s| s.path.clone()).collect();
                    let empty = metadata.sources.is_empty();
                    for source in sources {
                        if !metadata
                            .sources
                            .iter()
                            .any(|s| platform_windows::ordinal_ignore_case(&s.path, &source.path))
                        {
                            metadata.sources.push(source);
                        }
                    }
                    if empty && let Some(s) = metadata.sources.first() {
                        let scale = (16384.0 / s.display_width.max(s.display_height)).min(1.0);
                        metadata.output.width = ((s.display_width * scale).round() as u32)
                            .max(2)
                            .next_multiple_of(2);
                        metadata.output.height = ((s.display_height * scale).round() as u32)
                            .max(2)
                            .next_multiple_of(2);
                        metadata.output.rate = if s.rate.fps() <= 240.0 {
                            s.rate
                        } else {
                            editor_core::FrameRate::new(60, 1).unwrap()
                        };
                    }
                    self.metadata(metadata);
                    if position.is_some() {
                        let ids = paths
                            .iter()
                            .filter_map(|path| {
                                self.editor
                                    .metadata()
                                    .sources
                                    .iter()
                                    .find(|s| platform_windows::ordinal_ignore_case(&s.path, path))
                                    .map(|s| s.id)
                            })
                            .collect();
                        self.add_at(ids, position);
                    }
                    if self.library_selected.is_none() {
                        self.library_selected =
                            self.editor.metadata().sources.first().map(|s| s.id);
                        self.library_preview = self.library_selected.is_some();
                        self.playhead = 0.0;
                        self.request_key.clear();
                    }
                    self.start_waveforms();
                    self.message = if errors.is_empty() {
                        String::new()
                    } else {
                        errors.join("\n")
                    };
                }
                Event::Opened(result) => {
                    self.busy = false;
                    match result {
                        Ok((document, path, legacy)) => match document.clone().editor() {
                            Ok(editor) => {
                                self.clear_preview();
                                self.editor = editor;
                                let restored = self.restoring.is_some();
                                self.saved = if restored {
                                    SavedProject::capture(
                                        &Editor::with_metadata(
                                            Timeline::default(),
                                            ProjectMetadata::default(),
                                        )
                                        .unwrap(),
                                    )
                                } else {
                                    document
                                };
                                self.path = if legacy || restored { None } else { Some(path) };
                                self.update_dirty();
                                if let Some(claim) = self.restoring.take() {
                                    claim.discard();
                                }
                                self.selected = None;
                                self.library_preview = false;
                                self.playhead = 0.0;
                                self.request_key.clear();
                                self.waveforms.clear();
                                self.thumbnails.clear();
                                self.recovery_saved = None;
                                self.scrub = None;
                                self.settling = None;
                                self.playing = false;
                                self.library_selected = None;
                                self.sync_selection();
                                self.start_waveforms();
                                self.message = if legacy {
                                    "Legacy project opened · Save a new copy; 1.0.x cannot read 1.1 projects".into()
                                } else {
                                    "Project opened".into()
                                };
                            }
                            Err(e) => self.open_failed(e),
                        },
                        Err(e) => self.open_failed(e),
                    }
                }
                Event::Saved(result) => {
                    self.busy = false;
                    match result {
                        Ok((document, path)) => {
                            self.saved = document;
                            self.update_dirty();
                            self.path = Some(path);
                            self.recovery_saved = None;
                            self.message = "Project saved".into();
                        }
                        Err(e) => self.fail(e),
                    }
                }
                Event::Recovery(revision, document, result) => {
                    if revision == self.recovery.revision()
                        && self.recovery_saved.as_ref() == Some(&document)
                        && let Err(error) = result
                    {
                        self.recovery_saved = None;
                        self.message = format!("Recovery save failed; retrying: {error}");
                    }
                }
                Event::Waveform(source, peaks) => {
                    if self.is_current_source(&source) {
                        self.waveforms.insert(source.id, Arc::new(peaks));
                    }
                }
                Event::Thumbnail(source, image) => {
                    if self.is_current_source(&source) {
                        self.thumbnails.insert(
                            source.id,
                            ctx.load_texture(
                                format!("thumbnail-{}", source.id.0),
                                image,
                                egui::TextureOptions::LINEAR,
                            ),
                        );
                    }
                }
                Event::Cache(result) => {
                    self.busy = false;
                    match result {
                        Ok((files, bytes)) => {
                            self.message = format!(
                                "Cleared {files} cached previews · {:.1} MB",
                                bytes as f64 / 1048576.0
                            )
                        }
                        Err(e) => self.fail(e),
                    }
                }
                Event::Progress(progress, message) => {
                    self.progress = progress;
                    self.message = message;
                }
                Event::Exported(result) => {
                    self.busy = false;
                    self.exporting = false;
                    match result {
                        Ok((path, gpu)) => {
                            self.message = format!(
                                "Exported {} · {}",
                                path.display(),
                                if gpu { "NVIDIA" } else { "CPU" }
                            )
                        }
                        Err(e) => self.fail(e),
                    }
                }
            }
        }
        self.jobs.retain(|job| !job.is_finished());
    }
    fn top(&mut self, ctx: &egui::Context) {
        for key in [
            "file-new",
            "file-open",
            "file-save-as",
            "software-preview",
            "clear-cache",
            "export-settings",
        ] {
            self.controls.remove(key);
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(14, 6)),
            )
            .show(ctx, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let file = ui
                        .menu_button("File", |ui| {
                            let new = ui.add_enabled(!self.busy, egui::Button::new("New project"));
                            self.controls.insert("file-new", new.rect);
                            if new
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.new_project();
                                ui.close();
                            }
                            let open = ui.add_enabled(
                                !self.busy && self.tools.is_some(),
                                egui::Button::new("Open project…"),
                            );
                            self.controls.insert("file-open", open.rect);
                            if open
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                ui.close();
                                if let Ok(paths) =
                                    platform_windows::choose_files(self.owner, false, true)
                                    && let Some(path) = paths.into_iter().next()
                                {
                                    self.open(path);
                                }
                            }
                            ui.separator();
                            let save = ui.add_enabled(
                                !self.busy && !self.editor.timeline().clips().is_empty(),
                                egui::Button::new("Save as…"),
                            );
                            self.controls.insert("file-save-as", save.rect);
                            if save
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.save(true);
                                ui.close();
                            }
                        })
                        .response
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    self.controls.insert("file-menu", file.rect);
                    let title_width = (ui.available_width() - 270.0).clamp(0.0, 480.0);
                    ui.add_sized(
                        Vec2::new(title_width, 34.0),
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{}{}",
                                self.path
                                    .as_ref()
                                    .and_then(|p| p.file_stem())
                                    .map(|p| p.to_string_lossy())
                                    .unwrap_or("Untitled".into()),
                                if self.dirty() { " •" } else { "" }
                            ))
                            .color(theme::SECONDARY),
                        )
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let export = ui
                            .add_enabled(
                                !self.busy && !self.editor.timeline().clips().is_empty(),
                                theme::button(egui::RichText::new("Export").color(Color32::WHITE))
                                    .fill(theme::PRIMARY)
                                    .frame(true)
                                    .min_size(Vec2::new(100.0, 34.0)),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        self.controls.insert("export", export.rect);
                        if export.clicked() {
                            self.export();
                        }
                        self.more_options(ui, ctx);
                        if ui
                            .add_enabled(
                                !self.busy && !self.editor.timeline().clips().is_empty(),
                                theme::button("Save"),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.save(false);
                        }
                    });
                });
                ui.add_space(4.0);
            });
        let audio_warning = self
            .playback
            .as_ref()
            .and_then(|p| p.status().audio_warning);
        if self.busy || self.exporting || !self.message.is_empty() || audio_warning.is_some() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.busy {
                        ui.spinner();
                    }
                    if !self.message.is_empty() {
                        ui.label(&self.message);
                    }
                    if let Some(warning) = &audio_warning {
                        ui.label(warning);
                    }
                    if self.busy
                        && ui
                            .add(theme::button("Cancel"))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                    {
                        self.cancel.store(true, Ordering::Release);
                    }
                });
                if self.exporting {
                    ui.add(egui::ProgressBar::new(self.progress as f32).show_percentage());
                }
            });
        }
    }
    fn more_options(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let menu = ui.menu_button("…", |ui| {
            let settings = ui.button("Export settings…").on_hover_cursor(egui::CursorIcon::PointingHand);
            self.controls.insert("export-settings", settings.rect);
            if settings.clicked() { self.settings = !self.settings; ui.close(); }
            ui.separator();
            let software = ui.add_enabled(!self.busy, egui::Checkbox::new(&mut self.software_preview, "Software preview"))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Use CPU-rendered SDR video if native preview has problems.");
            self.controls.insert("software-preview", software.rect);
            if software.changed() && let Some(tools) = self.tools.clone() {
                self.stepping = None;
                if self.playback.take().is_some() {
                    self.playback = Some(Playback::new(tools, self.video.handle(), self.software_preview, ctx.clone()));
                }
                self.request_key.clear(); ui.close();
            }
            ui.separator();
            let cache = ui.add_enabled(!self.busy && self.tools.is_some(), egui::Button::new("Clear generated cache"))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Remove temporary thumbnails and waveforms to free disk space. Recordings and projects stay untouched.");
            self.controls.insert("clear-cache", cache.rect);
            if cache.clicked() && let Some(tools) = self.tools.clone() {
                self.busy = true;
                let protected = self.editor.metadata().sources.iter().map(|s|PathBuf::from(&s.path)).collect::<Vec<_>>();
                let tx = self.tx.clone();
                self.jobs.push(std::thread::spawn(move || {let _ = tx.send(Event::Cache(tools.clear_cache(&protected).map_err(|e|e.to_string())));}));
                ui.close();
            }
        }).response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("More options");
        menu.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "More options")
        });
        self.controls.insert("more-options", menu.rect);
    }
    fn library(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("recordings-graphite").frame(theme::panel(theme::PANEL)).resizable(true).default_width(280.0).width_range(250.0..=360.0).show(ctx,|ui| {
            if self.busy && !self.exporting{ui.disable();}
            for key in ["recording-search", "add-selected", "add-all", "first-recording"] { self.controls.remove(key); }
            ui.horizontal(|ui| {
                ui.heading("Recordings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!self.busy && self.tools.is_some(), theme::button("+ Import").frame(true).stroke(Stroke::new(1.0,theme::BORDER))).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
                        && let Ok(paths) = platform_windows::choose_files(self.owner,false,false) { self.import(paths); }
                });
            });
            if self.editor.metadata().sources.is_empty() { return; }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let clear = !self.search.is_empty();
                let width = (ui.available_width()-if clear {42.0}else{0.0}).max(1.0);
                let response=ui.add_sized(Vec2::new(width,30.0),egui::TextEdit::singleline(&mut self.search).id(egui::Id::new("recording-search-input")).hint_text("Search recordings").desired_width(width));
                self.controls.insert("recording-search",response.rect);
                if response.has_focus() && ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)){self.search.clear();response.surrender_focus();}
                if clear && ui.add(theme::button("×")).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Clear search").clicked(){self.search.clear();}
            });
            let sources=self.editor.metadata().sources.clone();
            let search=self.search.to_lowercase();
            let visible:Vec<_>=sources.iter().filter(|s|s.name().to_lowercase().contains(&search)).collect();
            ui.horizontal(|ui|{
                let response=ui.add_enabled(!self.busy && visible.iter().any(|s|Some(s.id)==self.library_selected),theme::button("Add to timeline")).on_hover_cursor(egui::CursorIcon::PointingHand);self.controls.insert("add-selected",response.rect);if response.clicked() && let Some(id)=self.library_selected{self.add(vec![id]);}
                if sources.len()>1{let response=ui.add_enabled(!self.busy,theme::button("Add all")).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Adds the complete library, including hidden search results, as one undoable edit");self.controls.insert("add-all",response.rect);if response.clicked(){self.add(sources.iter().map(|s|s.id).collect());}}
            });
            if !search.is_empty(){ui.label(format!("{} of {} recordings",visible.len(),sources.len()));}
            ui.separator();
            egui::ScrollArea::vertical().show_rows(ui,82.0,visible.len().max(1),|ui,rows| {
                if visible.is_empty(){ui.label("No recordings match your search.");}
                for source in visible.iter().skip(rows.start).take(rows.end-rows.start) {
                    let details=format!("{} · {} × {} · {}",time(source.duration),source.width,source.height,if source.hdr(){"HDR"}else{"SDR"});
                    let (rect,response)=ui.push_id(("recording",source.id.0),|ui|ui.allocate_exact_size(Vec2::new(ui.available_width(),82.0),Sense::click_and_drag())).inner;
                    if sources.first().is_some_and(|first|first.id==source.id){self.controls.insert("first-recording",rect);}
                    let selected=self.library_selected==Some(source.id);
                    response.widget_info(||egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel,ui.is_enabled(),selected,format!("{}, {details}",source.name())));
                    ui.painter().rect_filled(rect,5.0,if selected{theme::SELECTED}else if response.hovered(){theme::HOVER}else{Color32::TRANSPARENT});
                    if selected {ui.painter().rect_stroke(rect.shrink(0.5),5.0,Stroke::new(1.0,theme::ACCENT),egui::StrokeKind::Inside);}
                    let thumbnail=Rect::from_min_size(rect.min+Vec2::new(8.0,14.0),Vec2::new(96.0,54.0));
                    if let Some(texture)=self.thumbnails.get(&source.id){ui.painter().image(texture.id(),thumbnail,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE);}
                    let text=Rect::from_min_max(Pos2::new(thumbnail.right()+10.0,rect.top()+9.0),rect.max-Vec2::splat(8.0));
                    let painter=ui.painter().with_clip_rect(text);
                    let mut title=egui::text::LayoutJob::simple(source.name(),FontId::proportional(14.0),theme::TEXT,text.width());
                    title.wrap.max_rows=2;
                    let title=ui.fonts(|fonts|fonts.layout_job(title));
                    painter.galley(text.min,title,theme::TEXT);
                    let summary=if source.hdr(){format!("{} · HDR",time(source.duration))}else{time(source.duration)};
                    painter.text(Pos2::new(text.left(),rect.bottom()-23.0),Align2::LEFT_TOP,summary,FontId::proportional(12.5),theme::SECONDARY);
                    let response=response.on_hover_text(format!("{}\n{details}",source.path));
                    response.dnd_set_drag_payload(source.id);
                    if response.clicked() { self.library_selected=Some(source.id);self.library_preview=true;self.playing=false;self.seek(0.0); }
                    if response.double_clicked() { self.add(vec![source.id]); }
                    response.context_menu(|ui| {
                        if ui.button("Add to timeline").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { self.add(vec![source.id]);ui.close(); }
                        if ui.button("Remove from project").on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Removes this recording and its timeline sections; the original file stays unchanged").clicked(){
                            self.editor.metadata_mut().view=self.current_view();
                            match remove_source(&mut self.editor,source.id){Ok(())=>{self.playing=false;self.stepping=None;self.selected=None;self.library_selected=None;self.library_preview=false;self.playhead=0.0;self.sync_selection();self.update_dirty();self.request_key.clear();},Err(error)=>self.fail(error)}
                            ui.close();
                        }
                    });
                }
            });
        });
    }
    fn timeline_ui(&mut self, ctx: &egui::Context) {
        self.timeline_focus = None;
        egui::TopBottomPanel::bottom("timeline")
            .frame(theme::panel(theme::PANEL))
            .resizable(true)
            .default_height(220.0)
            .height_range(220.0..=450.0)
            .show(ctx, |ui| {
                if self.busy && !self.exporting {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    ui.heading("Timeline");
                    if self.timeline().clips().is_empty()
                        && !self.editor.can_undo()
                        && !self.editor.can_redo()
                    {
                        return;
                    }
                    ui.separator();
                    let undo = ui
                        .add_enabled(self.editor.can_undo(), theme::button("Undo"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    self.controls.insert("undo", undo.rect);
                    if undo.clicked() {
                        self.history(false);
                        self.request_key.clear();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), theme::button("Redo"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.history(true);
                        self.request_key.clear();
                    }
                    if ui
                        .add_enabled(
                            !self.library_preview
                                && self.timeline().source_at(self.playhead).is_some(),
                            theme::button("Split"),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Split at the playhead (S / Ctrl+B)")
                        .clicked()
                    {
                        self.split();
                    }
                    if ui
                        .add_enabled(
                            self.selected.is_some() && !self.library_preview,
                            theme::button("Delete"),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && let Some(clip) = self.selected
                    {
                        self.apply(Command::Delete { clip });
                        self.selected = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(theme::button("Fit"))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.zoom = ((ui.available_width().max(400.0) - 80.0)
                                / self.timeline().duration().max(5.0) as f32)
                                .clamp(0.05, 500.0);
                        }
                        ui.add(
                            egui::Slider::new(&mut self.zoom, 0.05..=500.0)
                                .logarithmic(true)
                                .show_value(false)
                                .text("Zoom"),
                        );
                    });
                });
                let zoom = self.zoom;
                let duration = self.timeline().duration().max(10.0);
                let width = (duration as f32 * zoom + 100.0).max(ui.available_width());
                let height = (ui.available_height() - 8.0).max(150.0);
                egui::ScrollArea::horizontal()
                    .id_salt("timeline-scroll")
                    .show(ui, |ui| {
                        let (rect, response) = ui
                            .allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 4.0, theme::CANVAS);
                        let origin = rect.left() + 65.0;
                        let second_step = if zoom > 180.0 {
                            0.5
                        } else if zoom > 55.0 {
                            1.0
                        } else if zoom > 12.0 {
                            5.0
                        } else if zoom > 2.0 {
                            30.0
                        } else {
                            300.0
                        };
                        let visible = painter.clip_rect();
                        self.timeline_drop = Some((rect.intersect(visible), origin, zoom));
                        let first = ((visible.left() - origin).max(0.0) / zoom / second_step)
                            .floor() as u32;
                        let last = ((visible.right() - origin).max(0.0) / zoom / second_step).ceil()
                            as u32;
                        let show_tracks = !self.timeline().clips().is_empty()
                            || egui::DragAndDrop::payload::<SourceId>(ctx).is_some()
                            || ui.input(|i| !i.raw.hovered_files.is_empty());
                        if show_tracks {
                            for n in first..=last {
                                let seconds = n as f32 * second_step;
                                let x = origin + seconds * zoom;
                                painter.line_segment(
                                    [Pos2::new(x, rect.top() + 23.0), Pos2::new(x, rect.bottom())],
                                    Stroke::new(1.0, Color32::from_gray(42)),
                                );
                                painter.text(
                                    Pos2::new(x + 4.0, rect.top() + 10.0),
                                    Align2::LEFT_CENTER,
                                    time(f64::from(seconds)),
                                    FontId::monospace(12.0),
                                    theme::SECONDARY,
                                );
                            }
                            painter.text(
                                Pos2::new(rect.left() + 8.0, rect.top() + 52.0),
                                Align2::LEFT_CENTER,
                                "VIDEO",
                                FontId::proportional(12.0),
                                theme::SECONDARY,
                            );
                            painter.text(
                                Pos2::new(rect.left() + 8.0, rect.top() + 110.0),
                                Align2::LEFT_CENTER,
                                "AUDIO",
                                FontId::proportional(12.0),
                                theme::SECONDARY,
                            );
                        }
                        if let Some(id) = response.dnd_release_payload::<SourceId>()
                            && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
                        {
                            self.add_at(
                                vec![*id],
                                Some(f64::from((pointer.x - origin) / zoom).max(0.0)),
                            );
                        }
                        let clips = self.timeline().clips().to_vec();
                        let mut over_clip = false;
                        for clip in clips {
                            let x = origin + clip.timeline_start as f32 * zoom;
                            let w = (clip.duration() as f32 * zoom).max(3.0);
                            let video = Rect::from_min_size(
                                Pos2::new(x, rect.top() + 29.0),
                                Vec2::new(w, 49.0),
                            );
                            let audio = Rect::from_min_size(
                                Pos2::new(x, rect.top() + 82.0),
                                Vec2::new(w, 38.0),
                            );
                            let linked = video.union(audio);
                            if !linked.intersects(visible)
                                && self.drag.as_ref().is_none_or(|drag| drag.clip != clip.id)
                            {
                                continue;
                            }
                            if clip.id == ClipId(1) {
                                self.controls.insert("first-clip", linked);
                            }
                            let selected = self.selected == Some(clip.id);
                            let source = self
                                .editor
                                .metadata()
                                .sources
                                .iter()
                                .find(|s| s.id == clip.source)
                                .cloned();
                            painter.rect_filled(
                                video,
                                4.0,
                                if selected {
                                    theme::SELECTED
                                } else {
                                    theme::SURFACE
                                },
                            );
                            if let Some(source) = &source {
                                self.paint_filmstrip(&painter, video, visible, &clip, source, zoom);
                            }
                            painter.rect_filled(audio, 4.0, theme::AUDIO);
                            if selected {
                                painter.rect_stroke(
                                    linked.expand(1.0),
                                    4.0,
                                    Stroke::new(1.5, theme::ACCENT),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if selected && video.width() >= 16.0 {
                                for x in [video.left(), video.right() - 6.0] {
                                    let handle = Rect::from_min_size(
                                        Pos2::new(x, video.top()),
                                        Vec2::new(6.0, video.height()),
                                    );
                                    painter.rect_filled(handle, 2.0, theme::ACCENT);
                                    painter.line_segment(
                                        [
                                            Pos2::new(x + 3.0, video.center().y - 6.0),
                                            Pos2::new(x + 3.0, video.center().y + 6.0),
                                        ],
                                        Stroke::new(1.5, theme::TEXT),
                                    );
                                }
                            }
                            let cp = painter.with_clip_rect(video.intersect(visible));
                            cp.rect_filled(
                                Rect::from_min_size(video.min, Vec2::new(video.width(), 18.0)),
                                0.0,
                                Color32::from_black_alpha(170),
                            );
                            cp.text(
                                video.min + Vec2::new(6.0, 3.0),
                                Align2::LEFT_TOP,
                                source.as_ref().map(|s| s.name()).unwrap_or_default(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );
                            if let Some(peaks) = self.waveforms.get(&clip.source)
                                && !peaks.is_empty()
                            {
                                let left = audio.left().max(visible.left());
                                let right = audio.right().min(visible.right());
                                for pixel in (left as i32..right as i32).step_by(3) {
                                    let t = clip.source_in
                                        + f64::from(pixel as f32 - audio.left()) / f64::from(zoom);
                                    let peak = peaks
                                        .get((t * 100.0) as usize)
                                        .copied()
                                        .unwrap_or(0.0)
                                        .min(1.0);
                                    let h = peak * 16.0;
                                    painter.line_segment(
                                        [
                                            Pos2::new(pixel as f32, audio.center().y - h),
                                            Pos2::new(pixel as f32, audio.center().y + h),
                                        ],
                                        Stroke::new(1.0, theme::WAVE),
                                    );
                                }
                            }
                            let interaction = ui.interact(
                                linked,
                                ui.id().with(("clip", clip.id.0)),
                                Sense::click_and_drag(),
                            );
                            if interaction.has_focus() {
                                self.timeline_focus = Some((interaction.id, clip.id));
                                ui.memory_mut(|m| {
                                    m.set_focus_lock_filter(
                                        interaction.id,
                                        egui::EventFilter {
                                            horizontal_arrows: true,
                                            ..Default::default()
                                        },
                                    );
                                });
                                painter.rect_stroke(
                                    linked.shrink(3.0),
                                    2.0,
                                    Stroke::new(1.5, Color32::WHITE),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            interaction.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::SelectableLabel,
                                    ui.is_enabled(),
                                    self.selected == Some(clip.id),
                                    format!(
                                        "{}, linked video and audio, {} on timeline, in {}, out {}",
                                        source
                                            .as_ref()
                                            .map_or_else(|| "Recording".into(), |s| s.name()),
                                        time(clip.timeline_start),
                                        time(clip.source_in),
                                        time(clip.source_out)
                                    ),
                                )
                            });
                            over_clip |= interaction.hovered();
                            if interaction.clicked() {
                                self.selected = Some(clip.id);
                                self.library_preview = false;
                                self.playing = false;
                                let position = interaction.interact_pointer_pos().map_or(
                                    clip.timeline_start,
                                    |p| {
                                        clip.timeline_start + f64::from((p.x - video.left()) / zoom)
                                    },
                                );
                                self.seek(position.min(clip.timeline_end()));
                            }
                            if interaction.drag_started() {
                                self.stepping = None;
                                let new_selection =
                                    self.library_preview || self.selected != Some(clip.id);
                                let preview_source = if new_selection {
                                    clip.source_in
                                } else {
                                    clip.source_in + self.playhead - clip.timeline_start
                                };
                                if new_selection {
                                    self.playhead = clip.timeline_start;
                                }
                                self.selected = Some(clip.id);
                                self.library_preview = false;
                                self.playing = false;
                                let px = ui
                                    .input(|i| i.pointer.press_origin())
                                    .map_or(linked.center().x, |p| p.x);
                                let edge = 8.0_f32.min(video.width() / 3.0);
                                let mode = if px - video.left() < edge {
                                    1
                                } else if video.right() - px < edge {
                                    2
                                } else {
                                    0
                                };
                                self.drag = Some(Drag {
                                    base: self.editor.timeline().clone(),
                                    clip: clip.id,
                                    start: clip.timeline_start,
                                    mode,
                                    source_in: clip.source_in,
                                    source_out: clip.source_out,
                                    pointer_x: px,
                                    playhead: self.playhead,
                                    preview_source,
                                });
                            }
                            if interaction.dragged()
                                && let (Some(drag), Some(source)) = (&self.drag, &source)
                            {
                                let delta = f64::from(
                                    (interaction
                                        .interact_pointer_pos()
                                        .map_or(drag.pointer_x, |p| p.x)
                                        - drag.pointer_x)
                                        / zoom,
                                );
                                let frame = source.rate.frame_seconds();
                                let mut preview = Editor::new(drag.base.clone()).unwrap();
                                let command = match drag.mode {
                                    1 => Command::PreviewTrim {
                                        clip: drag.clip,
                                        start: (drag.source_in
                                            + (delta / frame).round_ties_even() * frame)
                                            .clamp(0.0, drag.source_out - frame),
                                        end: drag.source_out,
                                        source_duration: source.duration,
                                        rate: source.rate,
                                    },
                                    2 => Command::PreviewTrim {
                                        clip: drag.clip,
                                        start: drag.source_in,
                                        end: (drag.source_out
                                            + (delta / frame).round_ties_even() * frame)
                                            .clamp(drag.source_in + frame, source.duration),
                                        source_duration: source.duration,
                                        rate: source.rate,
                                    },
                                    _ => Command::Move {
                                        clip: drag.clip,
                                        position: drag.start + delta,
                                        rate: source.rate,
                                        snap: 8.0 / f64::from(zoom),
                                    },
                                };
                                if preview.apply(command).is_ok() {
                                    let retained = preview
                                        .timeline()
                                        .clips()
                                        .iter()
                                        .find(|c| c.id == drag.clip)
                                        .unwrap();
                                    self.playhead = retained.timeline_start
                                        + drag.preview_source.clamp(
                                            retained.source_in,
                                            (retained.source_out - frame).max(retained.source_in),
                                        )
                                        - retained.source_in;
                                    self.preview_timeline = Some(preview.timeline().clone());
                                    self.request_key.clear();
                                }
                            }
                            if interaction.drag_stopped() {
                                if let (Some(preview), Some(drag), Some(source)) = (
                                    self.preview_timeline.take(),
                                    self.drag.take(),
                                    source.as_ref(),
                                ) && preview != drag.base
                                {
                                    let retained =
                                        preview.clips().iter().find(|c| c.id == drag.clip).unwrap();
                                    let command = if drag.mode == 0 {
                                        Command::Replace {
                                            clips: preview.clips().to_vec(),
                                        }
                                    } else {
                                        Command::Trim {
                                            clip: drag.clip,
                                            start: retained.source_in,
                                            end: retained.source_out,
                                            source_duration: source.duration,
                                            rate: source.rate,
                                        }
                                    };
                                    self.apply(command);
                                    if let Some(retained) = self
                                        .editor
                                        .timeline()
                                        .clips()
                                        .iter()
                                        .find(|c| c.id == drag.clip)
                                    {
                                        self.playhead = retained.timeline_start
                                            + drag.preview_source.clamp(
                                                retained.source_in,
                                                (retained.source_out - source.rate.frame_seconds())
                                                    .max(retained.source_in),
                                            )
                                            - retained.source_in;
                                        self.request_key.clear();
                                    }
                                }
                                self.drag = None;
                            }
                            if interaction.secondary_clicked()
                                && (self.selected != Some(clip.id) || self.library_preview)
                            {
                                self.library_preview = false;
                                self.playing = false;
                                self.seek(clip.timeline_start);
                            }
                            let menu_id = egui::Popup::default_response_id(&interaction);
                            let mut menu = egui::Popup::context_menu(&interaction);
                            if interaction.secondary_clicked() {
                                self.keyboard_menu = None;
                            }
                            if self.selected == Some(clip.id)
                                && !self.library_preview
                                && (interaction.has_focus() || !ctx.wants_keyboard_input())
                                && ctx.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::F10))
                            {
                                self.keyboard_menu = Some((menu_id, interaction.rect.center()));
                                menu = menu.open_memory(Some(egui::SetOpenCommand::Bool(true)));
                            }
                            if let Some((id, position)) = self.keyboard_menu
                                && id == menu_id
                            {
                                menu = menu.at_position(position);
                            }
                            let restore_menu_focus = egui::Popup::is_id_open(ctx, menu_id)
                                && ctx.input(|i| i.key_pressed(egui::Key::Escape));
                            menu.show(|ui| {
                                self.selected = Some(clip.id);
                                if ui
                                    .button("Duplicate")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    if let Some(source) = &source {
                                        self.apply(Command::Duplicate {
                                            clip: clip.id,
                                            rate: source.rate,
                                        });
                                    }
                                    ui.close();
                                }
                                if ui
                                    .button("Move earlier")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.apply(Command::Swap {
                                        clip: clip.id,
                                        direction: -1,
                                    });
                                    ui.close();
                                }
                                if ui
                                    .button("Move later")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.apply(Command::Swap {
                                        clip: clip.id,
                                        direction: 1,
                                    });
                                    ui.close();
                                }
                                if ui
                                    .button("Delete linked section")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.apply(Command::Delete { clip: clip.id });
                                    ui.close();
                                }
                            });
                            if restore_menu_focus && !egui::Popup::is_id_open(ctx, menu_id) {
                                ctx.memory_mut(|m| m.request_focus(interaction.id));
                            }
                            if self.keyboard_menu.is_some_and(|(id, _)| id == menu_id)
                                && !egui::Popup::is_id_open(ctx, menu_id)
                            {
                                self.keyboard_menu = None;
                            }
                        }
                        if !over_clip
                            && !self.ignore_scrub_until_release
                            && (response.clicked()
                                || response.dragged()
                                || response.is_pointer_button_down_on())
                            && self.drag.is_none()
                            && let Some(p) = response.interact_pointer_pos()
                        {
                            self.library_preview = false;
                            self.begin_scrub(2);
                            let target = f64::from((p.x - origin) / zoom)
                                .clamp(0.0, self.timeline().duration());
                            if target != self.playhead {
                                self.seek(target);
                            }
                        }
                        if self.scrub.as_ref().is_some_and(|s| s.owner == 2)
                            && !ui.input(|i| i.pointer.primary_down())
                        {
                            self.end_scrub(None);
                        }
                        self.controls.insert(
                            "ruler",
                            Rect::from_min_size(
                                Pos2::new(origin, rect.top()),
                                Vec2::new(rect.width() - 65.0, 24.0),
                            ),
                        );
                        if show_tracks {
                            let x = origin + self.playhead as f32 * zoom;
                            painter.line_segment(
                                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                                Stroke::new(2.0, theme::ACCENT),
                            );
                        }
                    });
            });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        for control in [
            "audio-mute",
            "audio-volume",
            "framing-mode",
            "recording-interpretation",
        ] {
            self.controls.remove(control);
        }
        if !self.settings
            && self.selected_clip().is_none()
            && !(self.library_preview && self.library_selected.is_some())
        {
            return;
        }
        egui::SidePanel::right("inspector-graphite")
            .frame(theme::panel(theme::PANEL))
            .resizable(true)
            .default_width(250.0)
            .width_range(240.0..=350.0)
            .show(ctx, |ui| {
                if self.busy && !self.exporting{ui.disable();}
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut metadata = self.editor.metadata().clone();
                    let before = metadata.clone();
                    let recording=if self.library_preview{self.library_selected}else{self.selected_clip().map(|c|c.source)};
                    if self.settings {
                        ui.heading("Output");
                        if metadata.output.profile==ColorProfile::Hdr10{
                            let display=self.playback.as_ref().and_then(|p|p.status().display);
                            let state=match display {Some(display) if display.hdr_active=>"Windows HDR is active on the preview display",Some(display) if display.state_known=>"Windows HDR is inactive on the preview display",_=>"Display HDR state unavailable"};
                            ui.checkbox(&mut self.hdr_preview,"HDR preview").on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(format!("{state}. Uses tone-mapped SDR when Windows HDR is inactive or the preview uses software rendering."));
                        }
                        ui.horizontal(|ui| {
                            ui.label("Size");
                            ui.add(
                                egui::DragValue::new(&mut metadata.output.width)
                                    .range(2..=16384)
                                    .speed(2),
                            );
                            ui.label("×");
                            ui.add(
                                egui::DragValue::new(&mut metadata.output.height)
                                    .range(2..=16384)
                                    .speed(2),
                            );
                        });
                        egui::ComboBox::from_label("Frame rate")
                            .selected_text(metadata.output.rate.rational())
                            .show_ui(ui, |ui| {
                                for rate in [
                                    "24/1",
                                    "25/1",
                                    "30/1",
                                    "30000/1001",
                                    "50/1",
                                    "60/1",
                                    "60000/1001",
                                    "120/1",
                                ] {
                                    let value = editor_core::FrameRate::parse(rate).unwrap();
                                    ui.selectable_value(&mut metadata.output.rate, value, rate).on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                            }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        egui::ComboBox::from_label("Color")
                            .selected_text(match metadata.output.profile {ColorProfile::Sdr=>"SDR",ColorProfile::Hdr10=>"HDR10"})
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut metadata.output.profile,
                                    ColorProfile::Sdr,
                                    "SDR · Rec.709",
                                ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                ui.selectable_value(
                                    &mut metadata.output.profile,
                                    ColorProfile::Hdr10,
                                    "HDR10 · PQ / Rec.2020",
                                ).on_hover_cursor(egui::CursorIcon::PointingHand);
                            }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        egui::ComboBox::from_label("Codec")
                            .selected_text(match metadata.output.codec {Codec::Av1=>"AV1",Codec::Hevc=>"HEVC",Codec::H264=>"H.264"})
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut metadata.output.codec, Codec::Av1, "AV1").on_hover_cursor(egui::CursorIcon::PointingHand);
                                ui.selectable_value(
                                    &mut metadata.output.codec,
                                    Codec::Hevc,
                                    "HEVC",
                                ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                if metadata.output.profile == ColorProfile::Sdr {
                                    ui.selectable_value(
                                        &mut metadata.output.codec,
                                        Codec::H264,
                                        "H.264",
                                    ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                            }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if metadata.output.profile == ColorProfile::Hdr10
                            && metadata.output.codec == Codec::H264
                        {
                            metadata.output.codec = Codec::Hevc;
                        }
                        ui.checkbox(&mut metadata.output.gpu, "NVIDIA GPU encoding").on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.add(
                            egui::Slider::new(&mut metadata.output.quality, 0..=51).text("Quality"),
                        ).on_hover_text("Lower values preserve more detail and create larger files.");
                        ui.separator();
                    }
                    if !self.library_preview && let Some(clip) = self.selected_clip() {
                        ui.horizontal(|ui| {
                            ui.heading("Trim").on_hover_text(format!("Timeline position: {}", time(clip.timeline_start)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if let Some(source) = metadata.sources.iter().find(|s| s.id == clip.source) {
                                    let reset = ui.add_enabled(clip.source_in != 0.0 || clip.source_out != source.duration, theme::button("Reset"))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text("Restore the full recording");
                                    self.controls.insert("trim-reset", reset.rect);
                                    if reset.clicked() { self.trim(Some(0.0), Some(source.duration)); }
                                }
                            });
                        });
                        let mut start = clip.source_in;
                        let mut end = clip.source_out;
                        egui::Grid::new("trim-fields").show(ui, |ui| {
                            let label = ui.label("In");
                            let response = trim_input(ui, &mut start, ("trim-in", clip.id.0), label.id);
                            self.controls.insert("trim-in", response.rect);
                            if response.changed() { self.trim(Some(start), None); }
                            ui.end_row();
                            let label = ui.label("Out");
                            let response = trim_input(ui, &mut end, ("trim-out", clip.id.0), label.id);
                            self.controls.insert("trim-out", response.rect);
                            if response.changed() { self.trim(None, Some(end)); }
                            ui.end_row();
                        });
                        ui.horizontal(|ui| {
                            if ui.add(theme::button("Trim before")).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Trim before the playhead (Q)").clicked() {
                                self.quick_trim(true);
                            }
                            if ui.add(theme::button("Trim after")).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Trim after the playhead (W)").clicked() {
                                self.quick_trim(false);
                            }
                        });
                        let mut adjustment = metadata.adjustments.get(&clip.id).cloned().unwrap_or_default();
                        let previous_adjustment=adjustment.clone();
                        ui.separator();
                        let audio = egui::CollapsingHeader::new("Audio")
                            .id_salt("section-audio")
                            .default_open(false)
                            .show_unindented(ui, |ui| {
                        if let Some(source) = metadata.sources.iter().find(|s| s.id == clip.source)
                        {
                            egui::ComboBox::from_label("Track")
                                .selected_text(
                                    adjustment
                                        .stream
                                        .map_or("Default".into(), |s| format!("Stream {s}")),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut adjustment.stream, None, "Default").on_hover_cursor(egui::CursorIcon::PointingHand);
                                    for stream in &source.audio {
                                        ui.selectable_value(
                                            &mut adjustment.stream,
                                            Some(stream.index),
                                            format!("{} · {} ch", stream.label, stream.channels),
                                        ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                    }
                                }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        }
                        let mute = ui.checkbox(&mut adjustment.muted, "Mute").on_hover_cursor(egui::CursorIcon::PointingHand);
                        self.controls.insert("audio-mute", mute.rect);
                        let volume = ui.scope(|ui| {
                            ui.spacing_mut().slider_width = (ui.available_width() - 140.0).clamp(50.0, 100.0);
                            ui.add(egui::Slider::new(&mut adjustment.gain_db, -60.0..=24.0)
                                .text("Volume").suffix(" dB"))
                        }).inner;
                        self.controls.insert("audio-volume", volume.rect);
                        ui.horizontal(|ui| {
                            ui.label("Fade in");
                            ui.add(
                                egui::DragValue::new(&mut adjustment.fade_in)
                                    .range(0.0..=clip.duration())
                                    .speed(0.05)
                                    .suffix(" s"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Fade out");
                            ui.add(
                                egui::DragValue::new(&mut adjustment.fade_out)
                                    .range(0.0..=clip.duration())
                                    .speed(0.05)
                                    .suffix(" s"),
                            );
                        });
                        });
                        self.controls.insert("inspector-audio", audio.header_response.rect);
                        audio.header_response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let framing = egui::CollapsingHeader::new("Framing")
                            .id_salt("section-framing")
                            .default_open(false)
                            .show_unindented(ui, |ui| {
                        let mode = egui::ComboBox::from_id_salt("framing-mode")
                            .selected_text(format!("{:?}", adjustment.framing))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut adjustment.framing, Framing::Fit, "Fit").on_hover_cursor(egui::CursorIcon::PointingHand);
                                ui.selectable_value(&mut adjustment.framing, Framing::Fill, "Fill").on_hover_cursor(egui::CursorIcon::PointingHand);
                                ui.selectable_value(&mut adjustment.framing, Framing::Crop, "Crop").on_hover_cursor(egui::CursorIcon::PointingHand);
                            }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        self.controls.insert("framing-mode", mode.rect);
                        egui::ComboBox::from_label("Rotate")
                            .selected_text(format!("{}°", adjustment.rotation))
                            .show_ui(ui, |ui| {
                                for value in [0, 90, 180, 270] {
                                    ui.selectable_value(
                                        &mut adjustment.rotation,
                                        value,
                                        format!("{value}°"),
                                    ).on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                            }).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if adjustment.framing == Framing::Crop {
                            for (index, label) in
                                ["Left", "Top", "Width", "Height"].iter().enumerate()
                            {
                                ui.add(
                                    egui::Slider::new(&mut adjustment.crop[index], 0.0..=1.0)
                                        .text(*label),
                                );
                            }
                        }
                        });
                        self.controls.insert("inspector-framing", framing.header_response.rect);
                        framing.header_response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if adjustment!=previous_adjustment{metadata.adjustments.insert(clip.id,adjustment);}
                    }
                    if let Some(source)=recording.and_then(|id|metadata.sources.iter().find(|s|s.id==id)){
                        let color = egui::CollapsingHeader::new("Recording color")
                            .id_salt("recording-color")
                            .default_open(false)
                            .show_unindented(ui, |ui| {
                        let mut interpretation=metadata.interpretations.get(&source.id).copied().unwrap_or_default();
                        let interpretation_response = egui::ComboBox::from_id_salt("source-interpretation").selected_text(match interpretation {SourceInterpretation::Auto=>"From recording",SourceInterpretation::Sdr709=>"SDR Rec.709",SourceInterpretation::Srgb=>"SDR sRGB",SourceInterpretation::Pq2020=>"HDR PQ / Rec.2020",SourceInterpretation::Hlg2020=>"HDR HLG / Rec.2020"}).show_ui(ui,|ui|{for (value,label) in [(SourceInterpretation::Auto,"From recording"),(SourceInterpretation::Sdr709,"SDR Rec.709"),(SourceInterpretation::Srgb,"SDR sRGB"),(SourceInterpretation::Pq2020,"HDR PQ / Rec.2020"),(SourceInterpretation::Hlg2020,"HDR HLG / Rec.2020")]{ui.selectable_value(&mut interpretation,value,label).on_hover_cursor(egui::CursorIcon::PointingHand);}}).response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        self.controls.insert("recording-interpretation", interpretation_response.rect);
                        if interpretation!=SourceInterpretation::Auto || metadata.interpretations.contains_key(&source.id){metadata.interpretations.insert(source.id,interpretation);}
                        });
                        self.controls.insert("inspector-color", color.header_response.rect);
                        color.header_response.on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(format!("{} · {} · {}",source.transfer,source.primaries,source.matrix));
                        if source.transfer=="unknown" || source.primaries=="unknown" || source.matrix=="unknown"{ui.label("Color metadata missing").on_hover_text("Unspecified metadata uses SDR Rec.709 for YUV or sRGB for RGB. Choose an interpretation if the recording was made differently.");}
                        if source.hdr() && source.primaries!="bt2020"{ui.label("Conflicting HDR metadata").on_hover_text("HDR transfer and color primaries disagree. Confirm the intended interpretation.");}

                    }
                    if metadata != before {
                        self.metadata(metadata);
                    }
                });
            });
    }
    fn preview_key(
        &self,
        mapped: Option<(ClipId, SourceId, f64)>,
        output: &OutputSettings,
        hdr_preview: bool,
    ) -> String {
        let adjustment = mapped
            .and_then(|(id, _, _)| self.editor.metadata().adjustments.get(&id))
            .cloned()
            .unwrap_or_default();
        let source = mapped
            .and_then(|(_, id, _)| {
                self.editor
                    .metadata()
                    .sources
                    .iter()
                    .find(|source| source.id == id)
            })
            .map(|source| {
                source.interpreted(
                    self.editor
                        .metadata()
                        .interpretations
                        .get(&source.id)
                        .copied()
                        .unwrap_or_default(),
                )
            });
        format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}",
            mapped.map(|(id, source, _)| (id, source)),
            source,
            adjustment,
            output,
            self.library_preview,
            hdr_preview
        )
    }
    fn preview(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::panel(theme::CANVAS))
            .show(ctx, |ui| {
                if self.editor.metadata().sources.is_empty() {
                    for key in [
                        "preview",
                        "preview-viewport",
                        "scrubber",
                        "frame-back",
                        "frame-forward",
                        "position-label",
                    ] {
                        self.controls.remove(key);
                    }
                    self.video.layout(0, 0, 1, 1, false);
                    return;
                }

                ui.horizontal(|ui| {
                    ui.weak(if self.library_preview {
                        "Recording"
                    } else {
                        "Timeline"
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status = self
                            .playback
                            .as_ref()
                            .map(|p| p.status())
                            .unwrap_or_default();
                        let (mode, detail) = if status.software {
                            ("SDR", "SDR software preview")
                        } else if self.hdr_preview
                            && status.display.as_ref().is_some_and(|d| d.hdr_active)
                            && status.gamma.as_deref() == Some("pq")
                        {
                            ("HDR", "Windows HDR preview")
                        } else if self.hdr_preview {
                            ("SDR", "SDR tone-mapped preview")
                        } else {
                            ("SDR", "SDR reference preview")
                        };
                        let source_id = if self.library_preview {
                            self.library_selected
                        } else {
                            preview_at(self.timeline(), self.editor.metadata(), self.playhead)
                                .map(|(_, id, _)| id)
                        };
                        let hdr_source = self
                            .editor
                            .metadata()
                            .sources
                            .iter()
                            .find(|source| Some(source.id) == source_id)
                            .is_some_and(|source| {
                                source
                                    .interpreted(
                                        self.editor
                                            .metadata()
                                            .interpretations
                                            .get(&source.id)
                                            .copied()
                                            .unwrap_or_default(),
                                    )
                                    .hdr()
                            });
                        if hdr_source || mode == "HDR" {
                            ui.small(mode).on_hover_text(detail);
                        }
                    });
                });
                let available = Vec2::new(
                    ui.available_width(),
                    (ui.available_height() - 104.0).max(1.0),
                );
                let (viewport, _) = ui.allocate_exact_size(available, Sense::hover());
                let output = self.preview_output();
                let aspect = output.width as f32 / output.height as f32;
                let width = available.x.min(available.y * aspect);
                let rect =
                    Rect::from_center_size(viewport.center(), Vec2::new(width, width / aspect));
                self.controls.insert("preview", rect);
                self.controls.insert("preview-viewport", viewport);
                ui.painter().rect_filled(rect, 4.0, Color32::BLACK);
                let status = self
                    .playback
                    .as_ref()
                    .map(|p| p.status())
                    .unwrap_or_default();
                let mapped = if self.library_preview {
                    self.library_selected.map(|source| (ClipId(0), source, 0.0))
                } else {
                    preview_at(self.timeline(), self.editor.metadata(), self.playhead)
                };
                let has_source = mapped.is_some();
                let preview_output = crate::playback::preview_output(
                    self.preview_output(),
                    rect.width() * ctx.pixels_per_point(),
                    rect.height() * ctx.pixels_per_point(),
                    self.software_preview || status.software,
                );
                let hdr_preview = self.hdr_preview
                    && self.editor.metadata().output.profile == ColorProfile::Hdr10
                    && !status.software
                    && status.display.as_ref().is_some_and(|d| d.hdr_active);
                let expected_key = self.preview_key(mapped, &preview_output, hdr_preview);
                let visible = has_source
                    && status.presentation_key.as_ref() == Some(&expected_key)
                    && status.error.is_none()
                    && self.startup_recovery.is_none()
                    && !ctx.is_popup_open();
                let visible = visible && self.relink.is_none();
                let scale = ctx.pixels_per_point();
                self.video.layout(
                    (rect.left() * scale).round() as i32,
                    (rect.top() * scale).round() as i32,
                    (rect.width() * scale).round() as i32,
                    (rect.height() * scale).round() as i32,
                    visible
                        && !status.software
                        && thumbnails::drag_bounds(ctx).is_none_or(|drag| !drag.intersects(rect)),
                );
                if visible && status.software {
                    if let Some(frame) = status.frame {
                        if self
                            .preview_frame
                            .as_ref()
                            .is_none_or(|previous| !Arc::ptr_eq(previous, &frame))
                        {
                            let mut rgb = Vec::with_capacity(frame.width * frame.height * 3);
                            for row in frame.pixels.chunks(frame.stride).take(frame.height) {
                                for pixel in row[..frame.width * 4].chunks_exact(4) {
                                    rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                                }
                            }
                            let image =
                                egui::ColorImage::from_rgb([frame.width, frame.height], &rgb);
                            if let Some(texture) = &mut self.preview_texture {
                                texture.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                self.preview_texture = Some(ctx.load_texture(
                                    "software-preview",
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.preview_frame = Some(frame);
                        }
                        if let Some(texture) = &self.preview_texture {
                            ui.painter().image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                } else if !visible && has_source && !ctx.is_popup_open() {
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "Preparing preview…",
                        FontId::proportional(19.0),
                        theme::SECONDARY,
                    );
                }
                let duration = self.preview_duration();
                let mut position = self.playhead;
                let slider_width = ui.spacing().slider_width;
                ui.spacing_mut().slider_width = ui.available_width() - 16.0;
                let scrubber = preview_scrubber(ui, &mut position, duration);
                ui.spacing_mut().slider_width = slider_width;
                self.controls.insert("scrubber", scrubber.rect);
                if !self.ignore_scrub_until_release
                    && (scrubber.drag_started()
                        || scrubber.is_pointer_button_down_on()
                        || scrubber.changed())
                {
                    self.begin_scrub(1);
                    if position != self.playhead {
                        self.seek(position);
                    }
                }
                if self.scrub.as_ref().is_some_and(|s| s.owner == 1)
                    && !ui.input(|i| i.pointer.primary_down())
                {
                    self.end_scrub(None);
                }
                let wide = ui.available_width() >= 570.0;
                if wide {
                    ui.columns(3, |columns| {
                        columns[0].with_layout(
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                self.transport_time(ui, duration);
                            },
                        );
                        columns[1].with_layout(
                            egui::Layout::left_to_right(egui::Align::Center)
                                .with_main_align(egui::Align::Center),
                            |ui| {
                                self.transport_buttons(ui);
                            },
                        );
                        columns[2].with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                Self::preview_volume(ui, &mut self.volume);
                            },
                        );
                    });
                } else {
                    ui.horizontal(|ui| {
                        self.transport_buttons(ui);
                        self.transport_time(ui, duration);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Self::preview_volume(ui, &mut self.volume);
                    });
                }
            });
    }
    fn transport_time(&mut self, ui: &mut egui::Ui, duration: f64) {
        let response = ui.label(
            egui::RichText::new(format!("{} / {}", time(self.playhead), time(duration)))
                .monospace()
                .size(13.0)
                .color(theme::TEXT),
        );
        self.controls.insert("position-label", response.rect);
    }
    fn transport_buttons(&mut self, ui: &mut egui::Ui) {
        let back =
            theme::transport_button(ui, theme::TransportIcon::Previous, "Previous frame (←)");
        self.controls.insert("frame-back", back.rect);
        if back.clicked() {
            self.step_frame(-1);
        }
        let play = theme::transport_button(
            ui,
            if self.play_intent() {
                theme::TransportIcon::Pause
            } else {
                theme::TransportIcon::Play
            },
            if self.play_intent() {
                "Pause (Space)"
            } else {
                "Play (Space)"
            },
        );
        self.controls.insert("transport-play", play.rect);
        if play.clicked() {
            self.toggle_playback();
        }
        let next = theme::transport_button(ui, theme::TransportIcon::Next, "Next frame (→)");
        self.controls.insert("frame-forward", next.rect);
        if next.clicked() {
            self.step_frame(1);
        }
    }
    fn preview_volume(ui: &mut egui::Ui, volume: &mut f64) {
        let width = ui.spacing().slider_width;
        ui.spacing_mut().slider_width = 80.0;
        ui.add(
            egui::Slider::new(volume, 0.0..=100.0)
                .text("Volume")
                .show_value(false),
        );
        ui.spacing_mut().slider_width = width;
    }
    fn playback(&mut self, ctx: &egui::Context) {
        let elapsed = self.last_tick.elapsed().as_secs_f64().min(0.25);
        self.last_tick = Instant::now();
        let status = self
            .playback
            .as_ref()
            .map(|p| p.status())
            .unwrap_or_default();
        if let Some(error) = &status.error {
            self.stepping = None;
            self.playing = false;
            self.message = error.clone();
        }
        if self
            .stepping
            .as_ref()
            .is_some_and(|s| s.generation == Some(status.generation) && status.ready)
        {
            let stepping = self.stepping.take().unwrap();
            if let Some((id, before)) = stepping.trim {
                if let Some(clip) = self
                    .editor
                    .timeline()
                    .clips()
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
                {
                    let current = status.seek_position.clamp(
                        clip.source_in,
                        (clip.source_out - 0.000001).max(clip.source_in),
                    );
                    self.selected = Some(id);
                    self.playhead = clip.timeline_start + current - clip.source_in;
                    let end = if status.ended {
                        clip.source_out
                    } else {
                        status.position.min(clip.source_out)
                    };
                    if before {
                        self.trim(Some(current), None);
                    } else if end > current + 0.000001 && end < clip.source_out - 0.000001 {
                        self.trim(None, Some(end));
                    } else {
                        self.seek(self.playhead);
                    }
                }
            } else {
                let outside_retained = !self.library_preview
                    && preview_at(self.timeline(), self.editor.metadata(), self.playhead)
                        .and_then(|(id, _, _)| self.timeline().clips().iter().find(|c| c.id == id))
                        .is_some_and(|c| {
                            status.position < c.source_in || status.position >= c.source_out
                        });
                let position = if self.library_preview {
                    status.position
                } else {
                    preview_at(self.timeline(), self.editor.metadata(), self.playhead)
                        .and_then(|(id, _, _)| self.timeline().clips().iter().find(|c| c.id == id))
                        .map_or(self.playhead, |clip| {
                            if status.ended {
                                clip.timeline_end()
                            } else {
                                clip.timeline_start
                                    + (status.position - clip.source_in).clamp(0.0, clip.duration())
                            }
                        })
                };
                if outside_retained
                    || !self.library_preview
                        && preview_at(self.timeline(), self.editor.metadata(), position)
                            .map(|(id, _, _)| id)
                            != preview_at(self.timeline(), self.editor.metadata(), self.playhead)
                                .map(|(id, _, _)| id)
                {
                    self.seek(position);
                } else {
                    self.playhead = position.clamp(0.0, self.preview_duration());
                }
            }
        }
        if self
            .settling
            .as_ref()
            .is_some_and(|s| s.generation == Some(status.generation) && status.ready)
        {
            let settling = self.settling.take().unwrap();
            self.playing = settling.resume && self.playhead < self.preview_duration();
        }
        if self
            .settling
            .as_ref()
            .is_some_and(|s| s.started.elapsed() > Duration::from_secs(20))
        {
            self.settling = None;
            self.playing = false;
            self.message = "Preview seek timed out · scrub again to retry".into();
        }
        let gap = !self.library_preview && self.timeline().source_at(self.playhead).is_none();
        if self.playing && (gap || status.ready && status.generation == self.generation) {
            if gap {
                self.playhead += elapsed;
            } else if self.library_preview {
                self.playhead = if status.ended {
                    self.preview_duration()
                } else {
                    status.position
                };
            } else if let Some((id, _, _)) = self.timeline().source_at(self.playhead)
                && let Some(clip) = self.timeline().clips().iter().find(|c| c.id == id)
            {
                self.playhead = if status.ended {
                    clip.timeline_end()
                } else {
                    clip.timeline_start
                        + (status.position - clip.source_in).clamp(0.0, clip.duration())
                };
            }
            if self.playhead >= self.preview_duration() {
                self.playhead = self.preview_duration();
                self.playing = false;
            }
        }
        let mapped = if self.library_preview {
            self.library_selected
                .and_then(|id| self.editor.metadata().sources.iter().find(|s| s.id == id))
                .map(|s| {
                    (
                        ClipId(0),
                        s.id,
                        self.playhead
                            .min((s.duration - s.rate.frame_seconds()).max(0.0)),
                    )
                })
        } else {
            preview_at(self.timeline(), self.editor.metadata(), self.playhead)
        };
        let source = mapped
            .and_then(|(_, id, _)| self.editor.metadata().sources.iter().find(|s| s.id == id))
            .map(|s| {
                s.interpreted(
                    self.editor
                        .metadata()
                        .interpretations
                        .get(&s.id)
                        .copied()
                        .unwrap_or_default(),
                )
            });
        if self.playing
            && !self.library_preview
            && let Some((id, source, _)) = mapped
        {
            self.selected = Some(id);
            self.library_selected = Some(source);
        }
        let adjustment = mapped
            .and_then(|(id, _, _)| self.editor.metadata().adjustments.get(&id))
            .cloned()
            .unwrap_or_default();
        let hdr_preview = self.hdr_preview
            && self.editor.metadata().output.profile == ColorProfile::Hdr10
            && !status.software
            && status.display.as_ref().is_some_and(|d| d.hdr_active);
        let rect = self
            .controls
            .get("preview")
            .copied()
            .unwrap_or(Rect::NOTHING);
        let output = crate::playback::preview_output(
            self.preview_output(),
            rect.width() * ctx.pixels_per_point(),
            rect.height() * ctx.pixels_per_point(),
            self.software_preview || status.software,
        );
        let key = self.preview_key(mapped, &output, hdr_preview);
        if self.request_key != key {
            if self
                .stepping
                .as_ref()
                .is_some_and(|s| s.generation.is_some())
            {
                self.stepping = None;
            }
            self.generation += 1;
            self.request_key = key;
        }
        if let Some(settling) = &mut self.settling {
            settling.generation.get_or_insert(self.generation);
        }
        if let Some(stepping) = &mut self.stepping {
            stepping.generation.get_or_insert(self.generation);
        }
        if source.is_some()
            && (self.playback.is_none()
                || status.error.is_some() && status.generation != self.generation)
            && let Some(tools) = self.tools.clone()
        {
            drop(self.playback.take());
            self.playback = Some(Playback::new(
                tools,
                self.video.handle(),
                self.software_preview,
                ctx.clone(),
            ));
        }
        if let Some(playback) = &self.playback {
            let end = mapped
                .and_then(|(id, _, _)| {
                    self.timeline()
                        .clips()
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.source_out)
                })
                .or_else(|| source.as_ref().map(|s| s.duration))
                .unwrap_or(0.0);
            playback.submit(Request {
                generation: self.generation,
                presentation_key: self.request_key.clone(),
                source,
                adjustment,
                output,
                position: mapped.map_or(0.0, |(_, _, time)| time),
                start: mapped
                    .and_then(|(id, _, _)| {
                        self.timeline()
                            .clips()
                            .iter()
                            .find(|c| c.id == id)
                            .map(|c| c.source_in)
                    })
                    .unwrap_or(0.0),
                end,
                playing: self.playing,
                scrubbing: self.scrub.is_some(),
                volume: self.volume,
                hdr_preview,
                steps: self.stepping.as_ref().map_or(0, |s| s.count),
            });
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.busy && !self.exporting {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel.store(true, Ordering::Release);
            }
            return;
        }
        if self.relink.is_some() || self.startup_recovery.is_some() {
            return;
        }
        if egui::Popup::is_any_open(ctx) {
            return;
        }
        if ctx.input(|i| i.modifiers.command) {
            if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("recording-search-input")));
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::I)) {
                if let Ok(paths) = platform_windows::choose_files(self.owner, false, false) {
                    self.import(paths);
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                if let Ok(paths) = platform_windows::choose_files(self.owner, false, true)
                    && let Some(path) = paths.into_iter().next()
                {
                    self.open(path);
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.new_project();
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                self.save(ctx.input(|i| i.modifiers.shift));
                return;
            }
        }
        let search = egui::Id::new("recording-search-input");
        if ctx.memory(|m| m.had_focus_last_frame(search))
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.search.clear();
            ctx.memory_mut(|m| m.surrender_focus(search));
            return;
        }
        let timeline_focused = self
            .timeline_focus
            .is_some_and(|(id, _)| ctx.memory(|m| m.has_focus(id)));
        if ctx.wants_keyboard_input() && !timeline_focused
            || self.busy && !self.exporting
            || self.relink.is_some()
            || self.startup_recovery.is_some()
        {
            return;
        }
        // A focused section owns editing keys, unlike an input field. Consume
        // Space before the section widget can also interpret it as activation.
        if timeline_focused
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space))
        {
            self.toggle_playback();
        }
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                self.end_scrub(Some(false));
                self.ignore_scrub_until_release = input.pointer.primary_down();
                if let Some(drag) = self.drag.take() {
                    self.playhead = drag.playhead;
                }
                self.preview_timeline = None;
                self.playing = false;
                self.request_key.clear();
            }
            if input.key_pressed(egui::Key::Space) {
                self.toggle_playback();
            }
            if input.modifiers.command {
                if input.key_pressed(egui::Key::B) {
                    self.split();
                }
                if input.key_pressed(egui::Key::Z) {
                    if input.modifiers.shift {
                        self.history(true);
                    } else {
                        self.history(false);
                    }
                    self.request_key.clear();
                }
                if input.key_pressed(egui::Key::Y) {
                    self.history(true);
                    self.request_key.clear();
                }
                if input.key_pressed(egui::Key::S) {
                    self.save(input.modifiers.shift);
                }
            } else {
                if input.key_pressed(egui::Key::S) {
                    self.split();
                }
                if input.key_pressed(egui::Key::Delete)
                    && !self.library_preview
                    && let Some(clip) = self.selected
                {
                    self.apply(Command::Delete { clip });
                    self.selected = None;
                }
                if input.modifiers.alt
                    && !self.library_preview
                    && let Some(clip) = self.selected
                {
                    if input.key_pressed(egui::Key::ArrowLeft) {
                        self.apply(Command::Swap {
                            clip,
                            direction: -1,
                        });
                    }
                    if input.key_pressed(egui::Key::ArrowRight) {
                        self.apply(Command::Swap { clip, direction: 1 });
                    }
                    return;
                }
                if input.key_pressed(egui::Key::ArrowLeft) {
                    self.step_frame(-1);
                }
                if input.key_pressed(egui::Key::ArrowRight) {
                    self.step_frame(1);
                }
                if let Some(clip) = self.selected_clip() {
                    let time = clip.source_in + self.playhead - clip.timeline_start;
                    if input.key_pressed(egui::Key::Q) {
                        self.quick_trim(true);
                    }
                    if input.key_pressed(egui::Key::W) {
                        self.quick_trim(false);
                    }
                    if input.key_pressed(egui::Key::I) {
                        self.trim(Some(time), None);
                    }
                    if input.key_pressed(egui::Key::O) {
                        self.trim(None, Some(time));
                    }
                }
            }
        });
    }
}
impl eframe::App for EditorApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(test) = &mut self.verification {
            // Injected events target this isolated context even when another
            // application has OS focus. Ordinary sessions retain native focus.
            if !test.capture_hold {
                raw.focused = true;
                // Keep renderer screenshots, but do not mix the user's native
                // pointer/keys with the verifier's isolated input sequence.
                raw.events
                    .retain(|event| matches!(event, egui::Event::Screenshot { .. }));
                raw.modifiers = egui::Modifiers::NONE;
            }
            if let Some(egui::Event::Key { modifiers, .. }) = test.input.first() {
                raw.modifiers = *modifiers;
            }
            raw.events.append(&mut test.input);
            raw.dropped_files.append(&mut test.dropped_files);
        }
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.ignore_scrub_until_release = false;
        }
        self.events(ctx);
        if self.tools.is_some()
            && !self.busy
            && self.startup_recovery.is_none()
            && !self.startup_paths.is_empty()
        {
            let paths = std::mem::take(&mut self.startup_paths);
            if paths.len() == 1 && crate::startup::is_project(&paths[0]) {
                self.open(paths[0].clone());
            } else {
                self.import(paths);
            }
        }
        self.shortcuts(ctx);
        self.top(ctx);
        self.controls.remove("drag-thumbnail");
        if self.timeline_thumbnails.is_none()
            && !self.editor.metadata().sources.is_empty()
            && let Some(tools) = self.tools.clone()
        {
            self.timeline_thumbnails =
                Some(thumbnails::TimelineThumbnails::new(tools, ctx.clone()));
        }
        if let Some(cache) = &mut self.timeline_thumbnails {
            cache.begin(ctx);
        }
        self.timeline_drop = None;
        self.timeline_ui(ctx);
        if let Some(cache) = &self.timeline_thumbnails {
            cache.finish();
        }
        self.library(ctx);
        self.inspector(ctx);
        self.preview(ctx);
        self.drag_thumbnail(ctx);
        self.playback(ctx);
        self.verify(ctx);
        if let Some((path, missing, mut replacements)) = self.relink.take() {
            let mut keep = true;
            let mut load = false;
            egui::Modal::new(egui::Id::new("relink")).show(ctx,|ui|{
                ui.heading("Locate missing recordings");
                let index=replacements.len();ui.label(format!("Recording {} of {}",index+1,missing.len()));ui.label(&missing[index]);
                ui.horizontal(|ui|{
                    if ui.add(theme::button("Locate recording…")).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Choose the same original recording in its new location. The current project stays open until all recordings pass validation.").clicked(){match platform_windows::choose_files(self.owner,false,false){Ok(paths)=>{if let Some(replacement)=paths.into_iter().next(){replacements.push((missing[index].clone(),replacement));if replacements.len()==missing.len(){keep=false;load=true;}}},Err(e)=>self.fail(e)}}
                    if ui.add(theme::button("Cancel")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){keep=false;}
                });
            });
            if load {
                self.open_relinked(path, replacements);
            } else if keep {
                self.relink = Some((path, missing, replacements));
            } else if self.restoring.is_some() {
                self.startup_recovery = self.restoring.take();
            }
        }
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() && self.relink.is_none() && self.startup_recovery.is_none() {
            if dropped.len() == 1 && crate::startup::is_project(&dropped[0]) {
                self.open(dropped[0].clone());
            } else {
                let pointer = if self.verification.is_some() {
                    ctx.input(|i| i.pointer.latest_pos())
                } else {
                    platform_windows::cursor_position(self.owner)
                        .ok()
                        .map(|(x, y)| {
                            Pos2::new(
                                x as f32 / ctx.pixels_per_point(),
                                y as f32 / ctx.pixels_per_point(),
                            )
                        })
                };
                let position = self.timeline_drop.and_then(|(rect, origin, zoom)| {
                    pointer
                        .filter(|p| rect.contains(*p))
                        .map(|p| f64::from((p.x - origin) / zoom).max(0.0))
                });
                self.import_at(dropped, position);
            }
        }
        if self.recovery_tick.elapsed() > Duration::from_secs(2) && (!self.busy || self.exporting) {
            self.recovery_tick = Instant::now();
            let document = SavedProject::capture(&self.editor);
            if self.dirty() && self.recovery_saved.as_ref() != Some(&document) {
                self.recovery_saved = Some(document.clone());
                let recovery = self.recovery.clone();
                let revision = recovery.revision();
                let tx = self.tx.clone();
                self.jobs.push(std::thread::spawn(move || {
                    let result = recovery
                        .save_revision(&document, revision)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Event::Recovery(revision, document, result));
                }));
            }
        }
        if self.startup_recovery.is_some() && self.tools.is_some() && !self.busy {
            egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
                ui.heading("Recover unsaved project");
                ui.horizontal(|ui| {
                    if ui
                        .add(theme::button("Recover"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && self.discard()
                        && let Some(claim) = self.startup_recovery.take()
                    {
                        let path = claim.path.clone();
                        self.restoring = Some(claim);
                        self.open_relinked(path, Vec::new());
                    }
                    if ui
                        .add(theme::button("Discard recovery"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && let Some(claim) = self.startup_recovery.take()
                    {
                        claim.discard();
                    }
                });
            });
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_approved {
            if self.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.message = "Cancel the active operation before closing".into();
            } else if self.discard() {
                self.close_approved = true;
                self.cancel.store(true, Ordering::Release);
                self.recovery.clear();
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        let title = format!(
            "{}Simple Video Editor {}",
            if self.dirty() { "• " } else { "" },
            env!("CARGO_PKG_VERSION")
        );
        if title != self.window_title {
            self.window_title.clone_from(&title);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        if self.playing || self.settling.is_some() || self.stepping.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if self.dirty() && (!self.busy || self.exporting) {
            ctx.request_repaint_after(
                Duration::from_secs(2).saturating_sub(self.recovery_tick.elapsed()),
            );
        }
        if let Some(test) = &self.verification {
            ctx.request_repaint_after(if test.launch_files {
                // Launch verification must rely on the same event wakeups as
                // ordinary startup; this is only its failure deadline.
                Duration::from_secs(31).saturating_sub(test.started.elapsed())
            } else if test.capture_hold && test.phase == 80 {
                Duration::from_millis(250)
            } else {
                Duration::from_millis(16)
            });
        }
    }
}
impl Drop for EditorApp {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        drop(self.playback.take());
        for job in self.jobs.drain(..) {
            let _ = job.join();
        }
        if self.close_approved {
            self.recovery.clear();
        }
    }
}
fn preview_scrubber(ui: &mut egui::Ui, position: &mut f64, duration: f64) -> egui::Response {
    let response = ui.add(
        egui::Slider::new(position, 0.0..=duration.max(0.001))
            .show_value(false)
            .trailing_fill(true),
    );
    response
        .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *position, "Preview position"));
    response
}
fn trim_input(
    ui: &mut egui::Ui,
    value: &mut f64,
    id: (&'static str, u64),
    label: egui::Id,
) -> egui::Response {
    let original = *value;
    ui.push_id(id, |ui| {
        ui.visuals_mut().button_frame = true;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = theme::SURFACE;
        ui.add_sized(
            Vec2::new(132.0, 32.0),
            egui::DragValue::new(value)
                .speed(0.01)
                .fixed_decimals(3)
                .suffix(" s")
                .custom_formatter(|v, _| format!("{v:.3}"))
                .custom_parser(move |text| {
                    if text.trim() == format!("{original:.3}") {
                        Some(original)
                    } else {
                        editor_core::parse_timecode(text)
                    }
                }),
        )
    })
    .inner
    .labelled_by(label)
}
fn read_thumbnail(path: PathBuf) -> media_engine::Result<image::DynamicImage> {
    if std::fs::metadata(&path)?.len() > 128_000 {
        return Err("Cached thumbnail exceeds its limit".into());
    }
    let mut reader = image::ImageReader::open(path)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(160);
    limits.max_image_height = Some(90);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}
fn time(seconds: f64) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}.{:03}", ms / 60000, ms / 1000 % 60, ms % 1000)
}
#[cfg(test)]
mod accessibility_tests {
    use super::*;
    #[test]
    fn primary_editing_controls_expose_names_roles_and_values() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut start = 0.125;
        let mut end = 2.875;
        let mut position = 1.5;
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let label = ui.label("In");
                trim_input(ui, &mut start, ("trim-in", 1), label.id);
                let label = ui.label("Out");
                trim_input(ui, &mut end, ("trim-out", 1), label.id);
                preview_scrubber(ui, &mut position, 3.0);
                theme::transport_button(ui, theme::TransportIcon::Play, "Play (Space)");
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("Missing accessibility tree");
        for (name, value) in [("In", 0.125), ("Out", 2.875)] {
            let control = tree
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.role() == egui::accesskit::Role::SpinButton
                        && node.numeric_value() == Some(value))
                    .then_some(node)
                })
                .expect("Trim field lost its spinbox role or exact value");
            assert!(
                control.labelled_by().iter().any(|id| tree
                    .nodes
                    .iter()
                    .any(|(label_id, label)| label_id == id && label.value() == Some(name))),
                "Trim field is not associated with its visible label"
            );
        }
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.role() == egui::accesskit::Role::Slider
                    && node.label() == Some("Preview position")
                    && node.numeric_value() == Some(1.5))
        );
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.role() == egui::accesskit::Role::Button
                    && node.label() == Some("Play (Space)"))
        );
    }
}
