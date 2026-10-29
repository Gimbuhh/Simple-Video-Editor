use super::*;
use std::{collections::VecDeque, sync::Mutex};
type Key = (SourceId, u64);
#[derive(Clone)]
struct Sample {
    key: Key,
    source: MediaSource,
    time: f64,
}
struct Queue {
    pending: VecDeque<Sample>,
    running: Option<Sample>,
}
struct Cached {
    source: MediaSource,
    texture: Option<egui::TextureHandle>,
    used: u64,
}
pub(super) struct TimelineThumbnails {
    queue: Arc<Mutex<Queue>>,
    receiver: mpsc::Receiver<(Sample, Option<egui::ColorImage>)>,
    worker: Option<std::thread::JoinHandle<()>>,
    cancel: Cancel,
    textures: BTreeMap<Key, Cached>,
    wanted: BTreeMap<Key, Sample>,
    frame: u64,
}
impl TimelineThumbnails {
    pub(super) fn new(tools: Tools, context: egui::Context) -> Self {
        let queue = Arc::new(Mutex::new(Queue {
            pending: VecDeque::<Sample>::new(),
            running: None,
        }));
        let cancel = uncancelled();
        let (sender, receiver) = mpsc::channel();
        let (work, stopped) = (queue.clone(), cancel.clone());
        let worker = std::thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let next = {
                    let mut work = work.lock().unwrap();
                    let next = work.pending.pop_front();
                    work.running = next.clone();
                    next
                };
                if let Some(sample) = next {
                    let image = tools
                        .thumbnail_at(&sample.source, sample.time, &stopped)
                        .and_then(read_thumbnail)
                        .ok()
                        .map(|image| {
                            let rgba = image.to_rgba8();
                            egui::ColorImage::from_rgba_unmultiplied(
                                [rgba.width() as usize, rgba.height() as usize],
                                rgba.as_raw(),
                            )
                        });
                    if sender.send((sample, image)).is_err() {
                        break;
                    }
                    work.lock().unwrap().running = None;
                    context.request_repaint();
                } else {
                    std::thread::park();
                }
            }
        });
        Self {
            queue,
            receiver,
            worker: Some(worker),
            cancel,
            textures: BTreeMap::new(),
            wanted: BTreeMap::new(),
            frame: 0,
        }
    }
    pub(super) fn begin(&mut self, ctx: &egui::Context) {
        self.frame = self.frame.wrapping_add(1);
        self.wanted.clear();
        while let Ok((sample, image)) = self.receiver.try_recv() {
            let texture = image.map(|image| {
                ctx.load_texture(
                    format!("timeline-{}-{}", sample.key.0.0, sample.key.1),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            });
            self.textures.insert(
                sample.key,
                Cached {
                    source: sample.source,
                    texture,
                    used: self.frame,
                },
            );
        }
        while self.textures.len() > 128 {
            let key = *self
                .textures
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .unwrap()
                .0;
            self.textures.remove(&key);
        }
    }
    pub(super) fn texture(&mut self, source: &MediaSource, time: f64) -> Option<egui::TextureId> {
        let time = (time * 10.0).round() / 10.0;
        let time = time.clamp(
            0.0,
            (source.duration - source.rate.frame_seconds()).max(0.0),
        );
        let key = (source.id, time.to_bits());
        if !self.wanted.contains_key(&key) && self.wanted.len() >= 128 {
            return None;
        }
        self.wanted.entry(key).or_insert_with(|| Sample {
            key,
            source: source.clone(),
            time,
        });
        if let Some(entry) = self.textures.get_mut(&key)
            && entry.source == *source
        {
            entry.used = self.frame;
            return entry.texture.as_ref().map(egui::TextureHandle::id);
        }
        None
    }
    pub(super) fn sample_count(&self, source: SourceId) -> usize {
        self.textures
            .iter()
            .filter(|((id, _), entry)| *id == source && entry.texture.is_some())
            .count()
    }
    pub(super) fn finish(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.pending = self
            .wanted
            .values()
            .filter(|sample| {
                self.textures
                    .get(&sample.key)
                    .is_none_or(|entry| entry.source != sample.source)
            })
            .filter(|sample| {
                queue.running.as_ref().is_none_or(|running| {
                    running.key != sample.key || running.source != sample.source
                })
            })
            .cloned()
            .collect();
        if !queue.pending.is_empty()
            && let Some(worker) = &self.worker
        {
            worker.thread().unpark();
        }
    }
}
impl Drop for TimelineThumbnails {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
impl EditorApp {
    pub(super) fn paint_filmstrip(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        visible: Rect,
        clip: &Clip,
        source: &MediaSource,
        zoom: f32,
    ) {
        let source = source.interpreted(
            self.editor
                .metadata()
                .interpretations
                .get(&source.id)
                .copied()
                .unwrap_or_default(),
        );
        let tile_width = rect.height() * 16.0 / 9.0;
        let first = ((visible.left() - rect.left()).max(0.0) / tile_width).floor() as usize;
        let last = ((visible.right().min(rect.right()) - rect.left()).max(0.0) / tile_width).ceil()
            as usize;
        let painter = painter.with_clip_rect(rect.intersect(visible));
        for index in first..last.min(first + 128) {
            let tile = Rect::from_min_size(
                Pos2::new(rect.left() + index as f32 * tile_width, rect.top()),
                Vec2::new(tile_width, rect.height()),
            );
            let time = (clip.source_in + f64::from(index as f32 * tile_width / zoom))
                .min(clip.source_out - source.rate.frame_seconds());
            let texture = self
                .timeline_thumbnails
                .as_mut()
                .and_then(|cache| cache.texture(&source, time))
                .or_else(|| self.thumbnails.get(&source.id).map(egui::TextureHandle::id));
            if let Some(texture) = texture {
                painter.image(
                    texture,
                    tile,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
    }
    pub(super) fn paint_drop_preview(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        canvas: Rect,
        visible: Rect,
        origin: f32,
        zoom: f32,
    ) {
        if !ui.is_enabled() {
            return;
        }
        let Some(id) = egui::DragAndDrop::payload::<SourceId>(ui.ctx()) else {
            return;
        };
        let Some(pointer) = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| canvas.intersect(visible).contains(*p))
        else {
            return;
        };
        let Some(source) = self
            .editor
            .metadata()
            .sources
            .iter()
            .find(|s| s.id == *id)
            .cloned()
        else {
            return;
        };
        let position = f64::from((pointer.x - origin) / zoom).max(0.0);
        let Some(clip) = drop_clip(self.editor.timeline(), &source, position, zoom) else {
            return;
        };
        let x = origin + clip.timeline_start as f32 * zoom;
        let width = (clip.duration() as f32 * zoom).max(3.0);
        let video = Rect::from_min_size(Pos2::new(x, canvas.top() + 29.0), Vec2::new(width, 49.0));
        let audio = Rect::from_min_size(Pos2::new(x, canvas.top() + 82.0), Vec2::new(width, 38.0));
        let linked = video.union(audio);
        if !linked.intersect(visible).is_positive() {
            return;
        }
        let painter = painter.with_clip_rect(visible.intersect(canvas));
        painter.rect_filled(video, 4.0, theme::SELECTED);
        self.paint_filmstrip(&painter, video, visible, &clip, &source, zoom);
        painter.rect_filled(video, 4.0, theme::ACCENT.gamma_multiply(0.18));
        painter.rect_filled(audio, 4.0, theme::SELECTED);
        painter.rect_stroke(
            linked.shrink(0.5),
            4.0,
            Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_size(video.min, Vec2::new(video.width(), 18.0)),
            0.0,
            Color32::from_black_alpha(170),
        );
        painter.with_clip_rect(video.intersect(visible)).text(
            video.min + Vec2::new(6.0, 3.0),
            Align2::LEFT_TOP,
            source.name(),
            FontId::proportional(12.0),
            theme::TEXT,
        );
        self.controls.insert("drop-preview", linked);
    }
    pub(super) fn drag_thumbnail(&mut self, ctx: &egui::Context) {
        // The in-track preview replaces the floating card while its target is visible.
        if self.controls.contains_key("drop-preview") {
            return;
        }
        let Some(id) = egui::DragAndDrop::payload::<SourceId>(ctx) else {
            return;
        };
        let Some(source) = self
            .editor
            .metadata()
            .sources
            .iter()
            .find(|source| source.id == *id)
        else {
            return;
        };
        let Some(rect) = drag_bounds(ctx) else {
            return;
        };
        let pos = rect.min;
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("recording-drag-preview"),
        ));
        painter.rect_filled(rect, 4.0, theme::SURFACE);
        if let Some(texture) = self.thumbnails.get(id.as_ref()) {
            painter.image(
                texture.id(),
                Rect::from_min_size(pos + Vec2::splat(8.0), Vec2::new(160.0, 90.0)),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        painter.with_clip_rect(rect.shrink(8.0)).text(
            pos + Vec2::new(8.0, 102.0),
            Align2::LEFT_TOP,
            source.name(),
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        self.controls.insert("drag-thumbnail", rect);
    }
}

fn drop_clip(timeline: &Timeline, source: &MediaSource, position: f64, zoom: f32) -> Option<Clip> {
    if timeline.clips().len() >= editor_core::MAX_CLIPS {
        return None;
    }
    let id = ClipId(
        timeline
            .clips()
            .iter()
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)?,
    );
    let mut clip = Clip {
        id,
        source: source.id,
        source_in: 0.0,
        source_out: source.duration,
        timeline_start: 0.0,
    };
    let mut clips = timeline.clips().to_vec();
    clips.push(clip.clone());
    clip.timeline_start =
        editor_core::available_position(&clips, id, position, source.rate, 10.0 / f64::from(zoom))
            .ok()?;
    Some(clip)
}

pub(super) fn drag_bounds(ctx: &egui::Context) -> Option<Rect> {
    egui::DragAndDrop::payload::<SourceId>(ctx)?;
    let pointer = ctx.input(|i| i.pointer.latest_pos())?;
    let size = Vec2::new(176.0, 126.0);
    let screen = ctx.screen_rect();
    Some(Rect::from_min_size(
        (pointer + Vec2::new(14.0, 16.0))
            .min(screen.max - size)
            .max(screen.min),
        size,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> MediaSource {
        MediaSource {
            id: SourceId(1),
            path: "fixture.mp4".into(),
            duration: 2000.0,
            width: 160,
            height: 90,
            display_width: 160.0,
            display_height: 90.0,
            rotation: 0.0,
            sample_aspect_ratio: 1.0,
            rate: editor_core::FrameRate::new(30, 1).unwrap(),
            codec: "h264".into(),
            video_stream: 0,
            audio: vec![],
            transfer: "bt709".into(),
            primaries: "bt709".into(),
            matrix: "bt709".into(),
            range: "tv".into(),
            pixel_format: "yuv420p".into(),
            chroma_location: String::new(),
        }
    }
    #[test]
    fn drop_preview_matches_insert_for_gaps_overlap_snapping_and_source_rates() {
        let mut source = source();
        source.duration = 1.25;
        for fps in [24, 30, 60] {
            source.rate = editor_core::FrameRate::new(fps, 1).unwrap();
            for timeline in [
                Timeline::default(),
                Timeline::new(vec![
                    Clip {
                        id: ClipId(7),
                        source: SourceId(9),
                        source_in: 0.2,
                        source_out: 2.2,
                        timeline_start: 0.5,
                    },
                    Clip {
                        id: ClipId(11),
                        source: SourceId(9),
                        source_in: 1.0,
                        source_out: 2.0,
                        timeline_start: 5.0,
                    },
                ])
                .unwrap(),
            ] {
                for position in [0.0, 0.2, 0.5, 1.5, 2.49, 2.55, 3.7, 4.8, 6.01] {
                    for zoom in [5.0, 90.0, 500.0] {
                        let original = timeline.clone();
                        let preview = drop_clip(&timeline, &source, position, zoom).unwrap();
                        let mut editor = Editor::new(timeline.clone()).unwrap();
                        editor
                            .apply(Command::Insert {
                                sections: vec![(
                                    Clip {
                                        id: ClipId(0),
                                        source: source.id,
                                        source_in: 0.0,
                                        source_out: source.duration,
                                        timeline_start: 0.0,
                                    },
                                    source.rate,
                                )],
                                position,
                                snap: 10.0 / f64::from(zoom),
                            })
                            .unwrap();
                        let inserted = editor
                            .timeline()
                            .clips()
                            .iter()
                            .find(|c| !original.clips().iter().any(|old| old.id == c.id))
                            .unwrap();
                        assert_eq!(preview.timeline_start, inserted.timeline_start);
                        assert_eq!(preview.duration(), inserted.duration());
                        assert_eq!(timeline, original);
                        assert!(editor.undo());
                        assert_eq!(editor.timeline(), &original);
                    }
                }
            }
        }
    }

    #[test]
    fn visible_sample_budget_includes_cached_frames() {
        let ctx = egui::Context::default();
        let (_, receiver) = mpsc::channel();
        let mut cache = TimelineThumbnails {
            queue: Arc::new(Mutex::new(Queue {
                pending: VecDeque::new(),
                running: None,
            })),
            receiver,
            worker: None,
            cancel: uncancelled(),
            textures: BTreeMap::new(),
            wanted: BTreeMap::new(),
            frame: 0,
        };
        let source = source();
        let texture = ctx.load_texture(
            "cached-frame",
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 255]),
            egui::TextureOptions::LINEAR,
        );
        for time in 0..128 {
            cache.textures.insert(
                (source.id, f64::from(time).to_bits()),
                Cached {
                    source: source.clone(),
                    texture: Some(texture.clone()),
                    used: 0,
                },
            );
        }
        cache.begin(&ctx);
        for time in 0..512 {
            cache.texture(&source, f64::from(time));
        }
        cache.finish();
        assert_eq!(cache.wanted.len(), 128);
        assert_eq!(cache.textures.len(), 128);
        assert!(cache.queue.lock().unwrap().pending.is_empty());

        // Moving to an uncached region resets the visible budget, without
        // continually evicting still-visible cached frames on every repaint.
        cache.begin(&ctx);
        for time in 512..1024 {
            cache.texture(&source, f64::from(time));
        }
        cache.finish();
        assert_eq!(cache.wanted.len(), 128);
        assert_eq!(cache.queue.lock().unwrap().pending.len(), 128);
    }
}
