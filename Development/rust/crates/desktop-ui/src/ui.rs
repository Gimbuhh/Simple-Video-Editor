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
const MINT: Color32 = Color32::from_rgb(106, 218, 187);
enum Event {
    Tools(Result<Tools, String>),
    Imported(Vec<MediaSource>, Vec<String>),
    Opened(Result<(SavedProject, PathBuf, bool), String>),
    Saved(Result<(SavedProject, PathBuf), String>),
    Waveform(SourceId, String, Vec<f32>),
    Thumbnail(SourceId, String, egui::ColorImage),
    Cache(Result<(usize, u64), String>),
    Progress(f64, String),
    Exported(Result<(PathBuf, bool), String>),
    Missing(PathBuf, Vec<String>),
}
struct Drag {
    base: Timeline,
    clip: ClipId,
    start: f64,
    mode: u8,
    source_in: f64,
    source_out: f64,
    pointer_x: f32,
}
type RelinkState = (PathBuf, Vec<String>, Vec<(String, PathBuf)>);
pub struct EditorApp {
    // The actor must be destroyed before its native child window.
    playback: Option<Playback>,
    video: platform_windows::VideoWindow,
    owner: isize,
    editor: ProjectEditor,
    tools: Option<Tools>,
    tx: mpsc::Sender<Event>,
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
    last_tick: Instant,
    generation: u64,
    request_key: String,
    volume: f64,
    zoom: f32,
    drag: Option<Drag>,
    preview_timeline: Option<Timeline>,
    waveforms: BTreeMap<SourceId, Arc<Vec<f32>>>,
    thumbnails: BTreeMap<SourceId, egui::TextureHandle>,
    recovery: Arc<Recovery>,
    recovery_saved: Option<SavedProject>,
    recovery_tick: Instant,
    startup_recovery: Option<RecoveryClaim>,
    restoring: Option<RecoveryClaim>,
    close_approved: bool,
    settings: bool,
    hdr_preview: bool,
    preview_texture: Option<egui::TextureHandle>,
    preview_frame: Option<Arc<native_mpv::Frame>>,
    controls: BTreeMap<&'static str, Rect>,
    verification: Option<crate::verification::Verification>,
    relink: Option<RelinkState>,
}
impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> media_engine::Result<Self> {
        let owner = match cc.window_handle()?.as_raw() {
            raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get(),
            _ => return Err("Windows is required".into()),
        };
        let video = platform_windows::VideoWindow::new(owner)?;
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(24, 27, 31);
        visuals.window_fill = Color32::from_rgb(30, 34, 39);
        visuals.selection.bg_fill = Color32::from_rgb(36, 91, 79);
        visuals.selection.stroke = Stroke::new(1.0, MINT);
        cc.egui_ctx.set_visuals(visuals);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        cc.egui_ctx.set_style(style);
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
        let startup_recovery = recovery.candidates().into_iter().next();
        let editor = Editor::with_metadata(Timeline::default(), ProjectMetadata::default())?;
        let saved = SavedProject::capture(&editor);
        let (tx, rx) = mpsc::channel();
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
            last_tick: Instant::now(),
            generation: 0,
            request_key: String::new(),
            volume: 70.0,
            zoom: 90.0,
            drag: None,
            preview_timeline: None,
            waveforms: BTreeMap::new(),
            thumbnails: BTreeMap::new(),
            recovery,
            recovery_saved: None,
            recovery_tick: Instant::now(),
            startup_recovery,
            restoring: None,
            close_approved: false,
            settings: false,
            hdr_preview: false,
            preview_texture: None,
            preview_frame: None,
            controls: BTreeMap::new(),
            verification,
            relink: None,
        })
    }
    fn dirty(&self) -> bool {
        self.dirty_cache
    }
    fn update_dirty(&mut self) {
        self.dirty_cache = SavedProject::capture(&self.editor) != self.saved;
    }
    fn history(&mut self, redo: bool) {
        if if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        } {
            self.update_dirty();
            self.request_key.clear();
        }
    }
    fn discard(&mut self) -> bool {
        self.playing = false;
        !self.dirty()
            || platform_windows::confirm(self.owner, "Discard unsaved changes?", "Unsaved project")
    }
    fn fail(&mut self, message: impl ToString) {
        self.message = message.to_string();
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
        match edit_project(&mut self.editor, command) {
            Ok(()) => {
                self.update_dirty();
                self.request_key.clear();
                self.library_preview = false;
            }
            Err(error) => self.fail(error),
        }
    }
    fn metadata(&mut self, metadata: ProjectMetadata) {
        if self.busy && !self.exporting {
            return;
        }
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
    }
    fn add(&mut self, ids: Vec<SourceId>) {
        let sections = ids
            .iter()
            .filter_map(|id| self.editor.metadata().sources.iter().find(|s| s.id == *id))
            .map(|s| Clip {
                id: ClipId(0),
                source: s.id,
                source_in: 0.0,
                source_out: s.duration,
                timeline_start: 0.0,
            })
            .collect();
        self.apply(Command::Append { sections });
    }
    fn seek(&mut self, time: f64) {
        self.playhead = time.clamp(0.0, self.preview_duration());
        self.request_key.clear();
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
    fn selected_clip(&self) -> Option<Clip> {
        self.selected
            .and_then(|id| self.editor.timeline().clips().iter().find(|c| c.id == id))
            .cloned()
    }
    fn split(&mut self) {
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
        if let Some(clip) = self.selected_clip()
            && let Some(source) = self
                .editor
                .metadata()
                .sources
                .iter()
                .find(|s| s.id == clip.source)
        {
            self.apply(Command::Trim {
                clip: clip.id,
                start: start.unwrap_or(clip.source_in),
                end: end.unwrap_or(clip.source_out),
                source_duration: source.duration,
                rate: source.rate,
            });
        }
    }
    fn import(&mut self, paths: Vec<PathBuf>) {
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
            let _ = tx.send(Event::Imported(sources, errors));
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
        self.jobs.push(std::thread::spawn(move || {
            let result = save_document(&path, &document)
                .map(|()| (document, path))
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
            .cloned()
            .collect();
        for source in &sources {
            self.waveforms.insert(source.id, Arc::new(Vec::new()));
        }
        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        self.jobs.push(std::thread::spawn(move || {
            for source in sources {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(peaks) = tools.waveform(&source, &cancel) {
                    let _ = tx.send(Event::Waveform(source.id, source.path.clone(), peaks));
                }
                if let Ok(image) = tools
                    .thumbnail(&source, &cancel)
                    .and_then(|path| image::open(path).map_err(Into::into))
                {
                    let image = image.to_rgba8();
                    let color = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    let _ = tx.send(Event::Thumbnail(source.id, source.path.clone(), color));
                }
            }
        }));
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
                        self.playback = Some(Playback::new(tools.clone(), self.video.handle()));
                        self.tools = Some(tools);
                        self.message = "Import recordings to begin".into();
                    }
                    Err(e) => self.fail(e),
                },
                Event::Imported(sources, errors) => {
                    self.busy = false;
                    let mut metadata = self.editor.metadata().clone();
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
                    self.start_waveforms();
                    self.message = if errors.is_empty() {
                        "Recordings ready · double-click to add a linked section".into()
                    } else {
                        errors.join("\n")
                    };
                }
                Event::Opened(result) => {
                    self.busy = false;
                    match result {
                        Ok((document, path, legacy)) => match document.clone().editor() {
                            Ok(editor) => {
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
                                self.start_waveforms();
                                self.message = if legacy {
                                    "Legacy project opened · Save creates a 1.1 project".into()
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
                            self.message = "Project saved".into();
                        }
                        Err(e) => self.fail(e),
                    }
                }
                Event::Waveform(id, path, peaks) => {
                    if self
                        .editor
                        .metadata()
                        .sources
                        .iter()
                        .any(|s| s.id == id && s.path == path)
                    {
                        self.waveforms.insert(id, Arc::new(peaks));
                    }
                }
                Event::Thumbnail(id, path, image) => {
                    if self
                        .editor
                        .metadata()
                        .sources
                        .iter()
                        .any(|s| s.id == id && s.path == path)
                    {
                        self.thumbnails.insert(
                            id,
                            ctx.load_texture(
                                format!("thumbnail-{}", id.0),
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
        self.jobs.retain_mut(|job| !job.is_finished());
    }
    fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("SIMPLE VIDEO EDITOR")
                        .strong()
                        .size(17.0)
                        .color(MINT),
                );
                ui.label("1.1");
                ui.separator();
                ui.label(format!(
                    "{}{}",
                    self.path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .map(|p| p.to_string_lossy())
                        .unwrap_or("Untitled".into()),
                    if self.dirty() { " •" } else { "" }
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !self.busy && !self.editor.timeline().clips().is_empty(),
                            egui::Button::new("Export video").fill(Color32::from_rgb(38, 99, 83)),
                        )
                        .clicked()
                    {
                        self.export();
                    }
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Save"))
                        .clicked()
                    {
                        self.save(false);
                    }
                    if ui
                        .add_enabled(
                            !self.busy && self.tools.is_some(),
                            egui::Button::new("Open"),
                        )
                        .clicked()
                        && let Ok(paths) = platform_windows::choose_files(self.owner, false, true)
                        && let Some(path) = paths.into_iter().next()
                    {
                        self.open(path);
                    }
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("New"))
                        .clicked()
                        && self.discard()
                    {
                        self.editor =
                            Editor::with_metadata(Timeline::default(), ProjectMetadata::default())
                                .unwrap();
                        self.saved = SavedProject::capture(&self.editor);
                        self.update_dirty();
                        self.path = None;
                        self.playhead = 0.0;
                        self.selected = None;
                        self.library_preview = false;
                        self.request_key.clear();
                        self.waveforms.clear();
                        self.recovery.clear();
                        self.recovery_saved = None;
                    }
                    if ui.button("Output").clicked() {
                        self.settings = !self.settings;
                    }
                });
            });
            ui.add_space(4.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                ui.label(&self.message);
                if self.busy && ui.button("Cancel").clicked() {
                    self.cancel.store(true, Ordering::Release);
                }
            });
            if self.exporting {
                ui.add(egui::ProgressBar::new(self.progress as f32).show_percentage());
            }
        });
    }
    fn library(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("recordings").resizable(true).default_width(280.0).width_range(220.0..=460.0).show(ctx,|ui| {
            ui.horizontal(|ui| {
                ui.heading("Recordings");ui.label(self.editor.metadata().sources.len().to_string());
                if ui.add_enabled(!self.busy && self.tools.is_some(),egui::Button::new("Cache")).on_hover_text("Clear generated thumbnails and waveforms").clicked() && let Some(tools)=self.tools.clone(){
                    self.busy=true;let tx=self.tx.clone();self.jobs.push(std::thread::spawn(move ||{let _=tx.send(Event::Cache(tools.clear_cache().map_err(|e|e.to_string())));}));
                }
            });
            if ui.add_enabled(!self.busy && self.tools.is_some(),egui::Button::new("+ Import recordings").min_size(Vec2::new(ui.available_width(),32.0))).clicked() && let Ok(paths)=platform_windows::choose_files(self.owner,false,false) { self.import(paths); }
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search recordings").desired_width(f32::INFINITY));
            let sources=self.editor.metadata().sources.clone();
            if ui.add_enabled(!sources.is_empty(),egui::Button::new("Add all to timeline")).on_hover_text("Adds the complete library as one undoable edit").clicked() { self.add(sources.iter().map(|s|s.id).collect()); }
            ui.separator();let search=self.search.to_lowercase();
            egui::ScrollArea::vertical().show(ui,|ui| {
                if sources.is_empty() { ui.add_space(32.0);ui.label("Drop recordings here or import files. Your original recordings stay untouched."); }
                for source in sources.iter().filter(|s|s.name().to_lowercase().contains(&search)) {
                    let label=format!("{}\n{} · {} × {} · {}",source.name(),time(source.duration),source.width,source.height,if source.hdr(){"HDR"}else{"SDR"});
                    let button=if let Some(texture)=self.thumbnails.get(&source.id) { egui::Button::image_and_text(egui::Image::new((texture.id(),Vec2::new(80.0,45.0))),label) }else{egui::Button::new(label)};
                    let response=ui.add(button.selected(self.library_selected==Some(source.id)).min_size(Vec2::new(ui.available_width(),64.0)).sense(Sense::click_and_drag()));
                    response.dnd_set_drag_payload(source.id);
                    if response.clicked() { self.library_selected=Some(source.id);self.library_preview=true;self.playing=false;self.playhead=0.0;self.request_key.clear(); }
                    if response.double_clicked() { self.add(vec![source.id]); }
                    response.context_menu(|ui| { if ui.button("Add to timeline").clicked() { self.add(vec![source.id]);ui.close(); } });
                }
            });
        });
    }
    fn timeline_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .default_height(260.0)
            .height_range(220.0..=450.0)
            .show(ctx, |ui| {
                if self.busy && !self.exporting {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    ui.heading("Timeline");
                    ui.label("LINKED VIDEO + AUDIO");
                    ui.separator();
                    if ui
                        .add_enabled(self.editor.can_undo(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        self.history(false);
                        self.request_key.clear();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        self.history(true);
                        self.request_key.clear();
                    }
                    if ui.button("Split [S]").clicked() {
                        self.split();
                    }
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Delete"))
                        .clicked()
                        && let Some(clip) = self.selected
                    {
                        self.apply(Command::Delete { clip });
                        self.selected = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Fit").clicked() {
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
                        painter.rect_filled(rect, 4.0, Color32::from_rgb(17, 20, 24));
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
                        let first = ((visible.left() - origin).max(0.0) / zoom / second_step)
                            .floor() as u32;
                        let last = ((visible.right() - origin).max(0.0) / zoom / second_step).ceil()
                            as u32;
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
                                FontId::monospace(11.0),
                                Color32::GRAY,
                            );
                        }
                        painter.text(
                            Pos2::new(rect.left() + 8.0, rect.top() + 52.0),
                            Align2::LEFT_CENTER,
                            "VIDEO",
                            FontId::proportional(10.0),
                            Color32::GRAY,
                        );
                        painter.text(
                            Pos2::new(rect.left() + 8.0, rect.top() + 110.0),
                            Align2::LEFT_CENTER,
                            "AUDIO",
                            FontId::proportional(10.0),
                            Color32::GRAY,
                        );
                        if let Some(id) = response.dnd_release_payload::<SourceId>() {
                            self.add(vec![*id]);
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
                                Vec2::new(w, 52.0),
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
                                    Color32::from_rgb(45, 112, 95)
                                } else {
                                    Color32::from_rgb(39, 74, 68)
                                },
                            );
                            painter.rect_filled(audio, 4.0, Color32::from_rgb(35, 57, 63));
                            if selected {
                                painter.rect_stroke(
                                    linked.expand(1.0),
                                    4.0,
                                    Stroke::new(1.5, MINT),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let cp = painter.with_clip_rect(video.intersect(visible).shrink(6.0));
                            cp.text(
                                Pos2::new(video.left() + 7.0, video.top() + 12.0),
                                Align2::LEFT_TOP,
                                source.as_ref().map(|s| s.name()).unwrap_or_default(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );
                            cp.text(
                                Pos2::new(video.left() + 7.0, video.top() + 30.0),
                                Align2::LEFT_TOP,
                                time(clip.duration()),
                                FontId::monospace(10.0),
                                Color32::LIGHT_GRAY,
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
                                    let h = peak * 21.0;
                                    painter.line_segment(
                                        [
                                            Pos2::new(pixel as f32, audio.center().y - h),
                                            Pos2::new(pixel as f32, audio.center().y + h),
                                        ],
                                        Stroke::new(1.0, MINT),
                                    );
                                }
                            }
                            let interaction = ui.interact(
                                linked,
                                ui.id().with(("clip", clip.id.0)),
                                Sense::click_and_drag(),
                            );
                            over_clip |= interaction.hovered();
                            if interaction.clicked() {
                                self.selected = Some(clip.id);
                                self.library_preview = false;
                                self.playing = false;
                                self.seek(clip.timeline_start);
                            }
                            if interaction.drag_started() {
                                self.selected = Some(clip.id);
                                self.library_preview = false;
                                self.playing = false;
                                let px = ui
                                    .input(|i| i.pointer.press_origin())
                                    .map_or(linked.center().x, |p| p.x);
                                let mode = if px - video.left() < 8.0 {
                                    1
                                } else if video.right() - px < 8.0 {
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
                                    1 => Command::Trim {
                                        clip: drag.clip,
                                        start: ((drag.source_in + delta) / frame).round_ties_even()
                                            * frame,
                                        end: drag.source_out,
                                        source_duration: source.duration,
                                        rate: source.rate,
                                    },
                                    2 => Command::Trim {
                                        clip: drag.clip,
                                        start: drag.source_in,
                                        end: ((drag.source_out + delta) / frame).round_ties_even()
                                            * frame,
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
                                    self.preview_timeline = Some(preview.timeline().clone());
                                    self.request_key.clear();
                                }
                            }
                            if interaction.drag_stopped() {
                                if let Some(preview) = self.preview_timeline.take() {
                                    self.apply(Command::Replace {
                                        clips: preview.clips().to_vec(),
                                    });
                                }
                                self.drag = None;
                            }
                            interaction.context_menu(|ui| {
                                self.selected = Some(clip.id);
                                if ui.button("Duplicate").clicked() {
                                    if let Some(source) = &source {
                                        self.apply(Command::Duplicate {
                                            clip: clip.id,
                                            rate: source.rate,
                                        });
                                    }
                                    ui.close();
                                }
                                if ui.button("Move earlier").clicked() {
                                    self.apply(Command::Swap {
                                        clip: clip.id,
                                        direction: -1,
                                    });
                                    ui.close();
                                }
                                if ui.button("Move later").clicked() {
                                    self.apply(Command::Swap {
                                        clip: clip.id,
                                        direction: 1,
                                    });
                                    ui.close();
                                }
                                if ui.button("Delete linked section").clicked() {
                                    self.apply(Command::Delete { clip: clip.id });
                                    ui.close();
                                }
                            });
                        }
                        if !over_clip
                            && (response.clicked() || response.dragged())
                            && self.drag.is_none()
                            && let Some(p) = response.interact_pointer_pos()
                        {
                            self.library_preview = false;
                            self.playing = false;
                            self.seek(f64::from((p.x - origin) / zoom).max(0.0));
                        }
                        let x = origin + self.playhead as f32 * zoom;
                        painter.line_segment(
                            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                            Stroke::new(2.0, MINT),
                        );
                        if self.editor.timeline().clips().is_empty() {
                            painter.text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                "Add recordings to build your timeline",
                                FontId::proportional(15.0),
                                Color32::GRAY,
                            );
                        }
                    });
            });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        if !self.settings && self.selected_clip().is_none() {
            return;
        }
        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(245.0)
            .width_range(220.0..=350.0)
            .show(ctx, |ui| {
                if self.busy && !self.exporting{ui.disable();}
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut metadata = self.editor.metadata().clone();
                    let before = metadata.clone();
                    if self.settings {
                        ui.heading("Output");
                        ui.label("Choose the format for your finished video.");
                        if metadata.output.profile==ColorProfile::Hdr10{ui.checkbox(&mut self.hdr_preview,"Use HDR display preview").on_hover_text("Requires Windows HDR and a compatible display. Leave off for the SDR reference preview.");}else{self.hdr_preview=false;}
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
                                    ui.selectable_value(&mut metadata.output.rate, value, rate);
                                }
                            });
                        egui::ComboBox::from_label("Color")
                            .selected_text(format!("{:?}", metadata.output.profile))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut metadata.output.profile,
                                    ColorProfile::Sdr,
                                    "SDR · Rec.709",
                                );
                                ui.selectable_value(
                                    &mut metadata.output.profile,
                                    ColorProfile::Hdr10,
                                    "HDR10 · PQ / Rec.2020",
                                );
                            });
                        egui::ComboBox::from_label("Codec")
                            .selected_text(format!("{:?}", metadata.output.codec))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut metadata.output.codec, Codec::Av1, "AV1");
                                ui.selectable_value(
                                    &mut metadata.output.codec,
                                    Codec::Hevc,
                                    "HEVC",
                                );
                                if metadata.output.profile == ColorProfile::Sdr {
                                    ui.selectable_value(
                                        &mut metadata.output.codec,
                                        Codec::H264,
                                        "H.264",
                                    );
                                }
                            });
                        if metadata.output.profile == ColorProfile::Hdr10
                            && metadata.output.codec == Codec::H264
                        {
                            metadata.output.codec = Codec::Hevc;
                        }
                        ui.checkbox(&mut metadata.output.gpu, "NVIDIA GPU encoding");
                        ui.add(
                            egui::Slider::new(&mut metadata.output.quality, 0..=51).text("Quality"),
                        );
                        ui.small("Lower values preserve more detail and create larger files.");
                        ui.separator();
                    }
                    if let Some(clip) = self.selected_clip() {
                        ui.heading("Selected section");
                        ui.label(format!("{} on timeline", time(clip.timeline_start)));
                        let mut start = clip.source_in;
                        let mut end = clip.source_out;
                        ui.horizontal(|ui| {
                            ui.label("In");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut start)
                                        .speed(0.01)
                                        .fixed_decimals(3)
                                        .suffix(" s"),
                                )
                                .changed()
                            {
                                self.trim(Some(start), None);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Out");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut end)
                                        .speed(0.01)
                                        .fixed_decimals(3)
                                        .suffix(" s"),
                                )
                                .changed()
                            {
                                self.trim(None, Some(end));
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Set in [Q]").clicked() {
                                self.trim(
                                    Some(clip.source_in + self.playhead - clip.timeline_start),
                                    None,
                                );
                            }
                            if ui.button("Set out [W]").clicked() {
                                self.trim(
                                    None,
                                    Some(clip.source_in + self.playhead - clip.timeline_start),
                                );
                            }
                        });
                        if ui.button("Reset trim").clicked()
                            && let Some(s) = metadata.sources.iter().find(|s| s.id == clip.source)
                        {
                            self.trim(Some(0.0), Some(s.duration));
                        }
                        let mut adjustment = metadata.adjustments.get(&clip.id).cloned().unwrap_or_default();
                        let previous_adjustment=adjustment.clone();
                        ui.separator();
                        ui.strong("Audio");
                        if let Some(source) = metadata.sources.iter().find(|s| s.id == clip.source)
                        {
                            egui::ComboBox::from_label("Track")
                                .selected_text(
                                    adjustment
                                        .stream
                                        .map_or("Default".into(), |s| format!("Stream {s}")),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut adjustment.stream, None, "Default");
                                    for stream in &source.audio {
                                        ui.selectable_value(
                                            &mut adjustment.stream,
                                            Some(stream.index),
                                            format!("{} · {} ch", stream.label, stream.channels),
                                        );
                                    }
                                });
                        }
                        ui.checkbox(&mut adjustment.muted, "Mute section");
                        ui.add(
                            egui::Slider::new(&mut adjustment.gain_db, -60.0..=24.0)
                                .text("Gain dB"),
                        );
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
                        ui.separator();
                        ui.strong("Framing");
                        egui::ComboBox::from_label("Mode")
                            .selected_text(format!("{:?}", adjustment.framing))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut adjustment.framing, Framing::Fit, "Fit");
                                ui.selectable_value(&mut adjustment.framing, Framing::Fill, "Fill");
                                ui.selectable_value(&mut adjustment.framing, Framing::Crop, "Crop");
                            });
                        egui::ComboBox::from_label("Rotate")
                            .selected_text(format!("{}°", adjustment.rotation))
                            .show_ui(ui, |ui| {
                                for value in [0, 90, 180, 270] {
                                    ui.selectable_value(
                                        &mut adjustment.rotation,
                                        value,
                                        format!("{value}°"),
                                    );
                                }
                            });
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
                        if adjustment!=previous_adjustment{metadata.adjustments.insert(clip.id,adjustment);}
                    }
                    if metadata != before {
                        self.metadata(metadata);
                    }
                });
            });
    }
    fn preview(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(if self.library_preview {
                    "Recording preview"
                } else {
                    "Timeline preview"
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = self
                        .playback
                        .as_ref()
                        .map(|p| p.status())
                        .unwrap_or_default();
                    ui.small(if status.software {
                        "SDR software preview"
                    } else if self.hdr_preview && status.gamma.as_deref() == Some("pq") {
                        "HDR display preview"
                    } else if self.hdr_preview {
                        "SDR display fallback"
                    } else {
                        "SDR reference"
                    });
                });
            });
            let available = Vec2::new(
                ui.available_width(),
                (ui.available_height() - 100.0).max(100.0),
            );
            let (rect, _) = ui.allocate_exact_size(available, Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::BLACK);
            let status = self
                .playback
                .as_ref()
                .map(|p| p.status())
                .unwrap_or_default();
            let has_source = if self.library_preview {
                self.library_selected.is_some()
            } else {
                self.timeline().source_at(self.playhead).is_some()
            };
            let visible = has_source
                && status.ready
                && status.generation == self.generation
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
                visible && !status.software,
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
                        let image = egui::ColorImage::from_rgb([frame.width, frame.height], &rgb);
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
            } else if !visible {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    if has_source {
                        "Preparing preview…"
                    } else if self.timeline().clips().is_empty() {
                        "Your story starts here"
                    } else {
                        "Empty timeline gap"
                    },
                    FontId::proportional(19.0),
                    Color32::GRAY,
                );
            }
            let duration = self.preview_duration();
            let mut position = self.playhead;
            ui.spacing_mut().slider_width = ui.available_width() - 16.0;
            if ui
                .add(
                    egui::Slider::new(&mut position, 0.0..=duration.max(0.001))
                        .show_value(false)
                        .trailing_fill(true),
                )
                .changed()
            {
                self.playing = false;
                self.seek(position);
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if self.playing { "Pause" } else { "Play" })
                    .clicked()
                {
                    if self.playhead >= duration {
                        self.seek(0.0);
                    }
                    self.playing = !self.playing;
                }
                let frame = self.editor.metadata().output.rate.frame_seconds();
                if ui.button("◀ Frame").clicked() {
                    self.playing = false;
                    self.seek(self.playhead - frame);
                }
                if ui.button("Frame ▶").clicked() {
                    self.playing = false;
                    self.seek(self.playhead + frame);
                }
                ui.monospace(format!("{} / {}", time(self.playhead), time(duration)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.volume, 0.0..=100.0)
                            .text("Volume")
                            .show_value(false),
                    );
                });
            });
        });
    }
    fn playback(&mut self) {
        let elapsed = self.last_tick.elapsed().as_secs_f64().min(0.25);
        self.last_tick = Instant::now();
        let status = self
            .playback
            .as_ref()
            .map(|p| p.status())
            .unwrap_or_default();
        if let Some(error) = status.error {
            self.playing = false;
            self.message = error;
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
                .map(|id| (ClipId(0), id, self.playhead))
        } else {
            self.timeline().source_at(self.playhead)
        };
        let source = mapped
            .and_then(|(_, id, _)| self.editor.metadata().sources.iter().find(|s| s.id == id))
            .cloned();
        let adjustment = mapped
            .and_then(|(id, _, _)| self.editor.metadata().adjustments.get(&id))
            .cloned()
            .unwrap_or_default();
        let key = format!(
            "{:?}|{:?}|{:?}|{}|{}",
            mapped.map(|(id, source, _)| (id, source)),
            adjustment,
            self.editor.metadata().output,
            self.library_preview,
            self.hdr_preview
        );
        if self.request_key != key {
            self.generation += 1;
            self.request_key = key;
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
                source,
                adjustment,
                output: self.editor.metadata().output.clone(),
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
                volume: self.volume,
                hdr_preview: self.hdr_preview,
            });
        }
    }
    fn verify(&mut self, ctx: &egui::Context) {
        let Some(mut test) = self.verification.take() else {
            return;
        };
        let result = (|| -> media_engine::Result<()> {
            if test.started.elapsed() > Duration::from_secs(45) {
                return Err(format!(
                    "Desktop verification timed out at phase {}: {}",
                    test.phase, self.message
                )
                .into());
            }
            if let Some(error) = self.playback.as_ref().and_then(|p| p.status().error) {
                return Err(error.into());
            }
            match test.phase {
                0 if self.tools.is_some() => {
                    self.import(vec![test.source.clone()]);
                    test.phase = 1;
                }
                1 if !self.busy => {
                    let id = self
                        .editor
                        .metadata()
                        .sources
                        .first()
                        .ok_or("Fixture failed to import")?
                        .id;
                    self.add(vec![id]);
                    self.selected = self.editor.timeline().clips().first().map(|c| c.id);
                    self.seek(0.5);
                    test.checks
                        .push("Recording import and linked append".into());
                    test.phase = 2;
                }
                2 if self.playback.as_ref().is_some_and(|p| {
                    let status = p.status();
                    status.ready
                        && status.generation == self.generation
                        && (!status.software || status.frame.is_some())
                }) =>
                {
                    test.checks
                        .push("Embedded D3D11 source load and shared preview filter".into());
                    test.key(egui::Key::S, false);
                    test.phase = 3;
                }
                3 => {
                    if self.editor.timeline().clips().len() != 2 {
                        return Err(
                            "Split keyboard input did not create two linked sections".into()
                        );
                    }
                    test.checks
                        .push("Split through actual egui keyboard input".into());
                    test.key(egui::Key::Z, true);
                    test.phase = 4;
                }
                4 => {
                    if self.editor.timeline().clips().len() != 1 {
                        return Err("Undo keyboard input failed".into());
                    }
                    test.key(egui::Key::Y, true);
                    test.phase = 5;
                }
                5 => {
                    if self.editor.timeline().clips().len() != 2 {
                        return Err("Redo keyboard input failed".into());
                    }
                    test.checks
                        .push("Undo and redo through actual keyboard input".into());
                    let document = SavedProject::capture(&self.editor);
                    let path = test.directory.join("roundtrip.sveproject");
                    save_document(&path, &document)?;
                    let loaded = self
                        .tools
                        .as_ref()
                        .unwrap()
                        .load_project(&path, &uncancelled())?;
                    if document != loaded {
                        return Err("Saved project changed on reload".into());
                    }
                    test.checks
                        .push("Atomic save, media re-probe and exact project roundtrip".into());
                    self.settings = true;
                    test.phase = 8;
                }
                8..=12 => {
                    let target = [0.2, 1.7, 0.8, 2.1, 1.25][(test.phase - 8) as usize];
                    self.seek(target);
                    test.phase += 1;
                }
                13 if self.playback.as_ref().is_some_and(|p| {
                    let status = p.status();
                    status.ready
                        && status.generation == self.generation
                        && (status.position - 1.25).abs() < 0.06
                        && (!status.software || status.frame.is_some())
                }) =>
                {
                    let status = self.playback.as_ref().unwrap().status();
                    if status.software {
                        let frame = status.frame.unwrap();
                        if !frame.pixels.iter().any(|b| *b > 32) {
                            return Err("Software preview produced no visible frame".into());
                        }
                        if matches!(status.gamma.as_deref(), Some("pq" | "hlg")) {
                            return Err(
                                "Software fallback retained an HDR transfer function".into()
                            );
                        }
                        test.checks.push(
                            "HDR-capable import rendered into a nonblack SDR software frame".into(),
                        );
                    }
                    test.checks
                        .push("Latest rapid seek wins and settles within one frame".into());
                    test.key(egui::Key::Space, false);
                    test.phase = 14;
                }
                14 if self.playhead > 1.4 => {
                    test.key(egui::Key::Space, false);
                    test.phase = 15;
                }
                15 if !self.playing => {
                    test.checks.push(
                        "Playback uses the media clock and pauses through keyboard input".into(),
                    );
                    test.expected = Some(self.editor.timeline().clips().to_vec());
                    let anchor = self
                        .controls
                        .get("first-clip")
                        .ok_or("Missing timeline interaction rectangle")?
                        .center();
                    test.anchor = Some(anchor);
                    test.pointer(anchor, Some(true));
                    test.phase = 16;
                }
                16 => {
                    test.pointer(test.anchor.unwrap() + Vec2::new(300.0, 0.0), None);
                    test.phase = 17;
                }
                17 => {
                    test.pointer(test.anchor.unwrap() + Vec2::new(300.0, 0.0), Some(false));
                    test.phase = 18;
                }
                18 => {
                    let clip = self
                        .editor
                        .timeline()
                        .clips()
                        .iter()
                        .find(|c| c.id == ClipId(1))
                        .unwrap();
                    if clip.timeline_start < 3.0 {
                        return Err(
                            "Pointer drag did not move the linked section into free space".into(),
                        );
                    }
                    test.key(egui::Key::Z, true);
                    test.phase = 19;
                }
                19 => {
                    if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                        return Err("Pointer move did not undo in one step".into());
                    }
                    test.checks
                        .push("Linked pointer drag commits one undoable move".into());
                    test.pointer(test.anchor.unwrap(), Some(true));
                    test.phase = 20;
                }
                20 => {
                    test.pointer(test.anchor.unwrap() + Vec2::new(300.0, 0.0), None);
                    test.phase = 21;
                }
                21 => {
                    test.key(egui::Key::Escape, false);
                    test.phase = 22;
                }
                22 => {
                    test.pointer(test.anchor.unwrap() + Vec2::new(300.0, 0.0), Some(false));
                    test.phase = 23;
                }
                23 => {
                    if self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        || self.preview_timeline.is_some()
                    {
                        return Err("Escape failed to cancel the pointer gesture".into());
                    }
                    test.checks
                        .push("Escape restores the complete pre-drag timeline".into());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    test.phase = 6;
                }
                6 => {
                    for event in ctx.input(|i| i.events.clone()) {
                        if let egui::Event::Screenshot { image, .. } = event {
                            test.screenshot(&image)?;
                            test.checks.push("Actual desktop layout captured".into());
                            test.report();
                            self.close_approved = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            test.phase = 7;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            test.failure = Some(e.to_string());
            test.report();
            self.close_approved = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.verification = Some(test);
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input()
            || self.busy && !self.exporting
            || self.relink.is_some()
            || self.startup_recovery.is_some()
        {
            return;
        }
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                self.drag = None;
                self.preview_timeline = None;
                self.playing = false;
                self.request_key.clear();
            }
            if input.key_pressed(egui::Key::Space) {
                self.playing = !self.playing;
            }
            if input.modifiers.command {
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
                    && let Some(clip) = self.selected
                {
                    self.apply(Command::Delete { clip });
                    self.selected = None;
                }
                if input.key_pressed(egui::Key::ArrowLeft) {
                    self.playing = false;
                    self.seek(self.playhead - self.editor.metadata().output.rate.frame_seconds());
                }
                if input.key_pressed(egui::Key::ArrowRight) {
                    self.playing = false;
                    self.seek(self.playhead + self.editor.metadata().output.rate.frame_seconds());
                }
                if let Some(clip) = self.selected_clip() {
                    let time = clip.source_in + self.playhead - clip.timeline_start;
                    if input.key_pressed(egui::Key::Q) {
                        self.trim(Some(time), None);
                    }
                    if input.key_pressed(egui::Key::W) {
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
            if let Some(egui::Event::Key { modifiers, .. }) = test.input.first() {
                raw.modifiers = *modifiers;
            }
            raw.events.append(&mut test.input);
        }
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.events(ctx);
        self.shortcuts(ctx);
        self.top(ctx);
        self.timeline_ui(ctx);
        self.library(ctx);
        self.inspector(ctx);
        self.preview(ctx);
        self.playback();
        self.verify(ctx);
        if let Some((path, missing, mut replacements)) = self.relink.take() {
            let mut keep = true;
            let mut load = false;
            egui::Modal::new(egui::Id::new("relink")).show(ctx,|ui|{
                ui.heading("Locate missing recordings");
                let index=replacements.len();ui.label(format!("Recording {} of {}",index+1,missing.len()));ui.label(&missing[index]);
                ui.label("Choose the same original recording in its new location. The current project stays open until all recordings pass validation.");
                ui.horizontal(|ui|{
                    if ui.button("Locate recording…").clicked(){match platform_windows::choose_files(self.owner,false,false){Ok(paths)=>{if let Some(replacement)=paths.into_iter().next(){replacements.push((missing[index].clone(),replacement));if replacements.len()==missing.len(){keep=false;load=true;}}},Err(e)=>self.fail(e)}}
                    if ui.button("Cancel").clicked(){keep=false;}
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
            if dropped.len() == 1 && dropped[0].extension().is_some_and(|s| s == "sveproject") {
                self.open(dropped[0].clone());
            } else {
                self.import(dropped);
            }
        }
        if self.recovery_tick.elapsed() > Duration::from_secs(2) && !self.busy {
            self.recovery_tick = Instant::now();
            let document = SavedProject::capture(&self.editor);
            if self.dirty() && self.recovery_saved.as_ref() != Some(&document) {
                self.recovery_saved = Some(document.clone());
                let recovery = self.recovery.clone();
                let revision = recovery.revision();
                self.jobs.push(std::thread::spawn(move || {
                    let _ = recovery.save_revision(&document, revision);
                }));
            }
        }
        if self.startup_recovery.is_some() && self.tools.is_some() && !self.busy {
            egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
                ui.heading("Recover unsaved project");
                ui.label("An interrupted editing session has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Recover").clicked()
                        && self.discard()
                        && let Some(claim) = self.startup_recovery.take()
                    {
                        let path = claim.path.clone();
                        self.restoring = Some(claim);
                        self.open_relinked(path, Vec::new());
                    }
                    if ui.button("Discard recovery").clicked()
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
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}Simple Video Editor 1.1",
            if self.dirty() { "• " } else { "" }
        )));
        ctx.request_repaint_after(Duration::from_millis(if self.playing || self.busy {
            16
        } else {
            50
        }));
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
fn time(seconds: f64) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}.{:03}", ms / 60000, ms / 1000 % 60, ms % 1000)
}
