use super::*;
use crate::audio_playback::{AudioPlayback, AudioRequest};

impl EditorApp {
    pub(super) fn has_linked_audio_lane(&self) -> bool {
        self.timeline().clips().iter().any(|c| {
            !self
                .editor
                .metadata()
                .adjustments
                .get(&c.id)
                .is_some_and(|a| a.audio_separated)
        })
    }
    pub(super) fn separate_selected_audio(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        self.editor.metadata_mut().view = self.current_view();
        match separate_audio(&mut self.editor, id) {
            Ok(id) => {
                self.selected_audio = Some(id);
                self.library_preview = false;
                self.playing = false;
                self.request_key.clear();
                self.update_dirty();
            }
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn apply_audio(&mut self, command: Command) {
        if self.busy && !self.exporting {
            return;
        }
        self.editor.metadata_mut().view = self.current_view();
        let new = matches!(&command, Command::Split { .. } | Command::Duplicate { .. });
        let old: Vec<_> = self
            .editor
            .metadata()
            .audio_items
            .iter()
            .map(|a| a.clip.id)
            .collect();
        match edit_audio(&mut self.editor, command) {
            Ok(()) => {
                if new {
                    self.selected_audio = self
                        .editor
                        .metadata()
                        .audio_items
                        .iter()
                        .find(|a| !old.contains(&a.clip.id))
                        .map(|a| a.clip.id);
                }
                self.playing = false;
                self.scrub = None;
                self.settling = None;
                self.stepping = None;
                self.request_key.clear();
                self.sync_selection();
                self.update_dirty();
            }
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn update_audio_playback(&mut self, ctx: &egui::Context) {
        let item = if self.library_preview {
            None
        } else {
            self.editor
                .metadata()
                .audio_items
                .iter()
                .find(|a| {
                    self.playhead >= a.clip.timeline_start && self.playhead < a.clip.timeline_end()
                })
                .cloned()
        };
        if item.is_some()
            && self.audio_playback.is_none()
            && let Some(tools) = self.tools.clone()
        {
            self.audio_playback = Some(AudioPlayback::new(tools, ctx.clone()));
        }
        if let Some(player) = &self.audio_playback {
            // Do not let sound run ahead while the video clock is loading or
            // completing a seek. Timeline gaps use the UI's monotonic clock.
            let clock_running = self.playing
                && (self.timeline().source_at(self.playhead).is_none()
                    || self.playback.as_ref().is_some_and(|p| {
                        let s = p.status();
                        s.ready && s.generation == self.generation
                    }));
            let request = item.and_then(|item| {
                self.editor
                    .metadata()
                    .sources
                    .iter()
                    .find(|s| s.id == item.clip.source)
                    .map(|source| AudioRequest {
                        position: item.clip.source_in + self.playhead - item.clip.timeline_start,
                        item,
                        source: source.clone(),
                        playing: clock_running && self.scrub.is_none() && self.settling.is_none(),
                        volume: self.volume,
                        serial: self.generation,
                    })
            });
            player.submit(request);
        }
    }
    pub(super) fn paint_audio_items(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        canvas: Rect,
        visible: Rect,
        origin: f32,
        zoom: f32,
    ) -> bool {
        for key in [
            "first-audio-item",
            "audio-item-menu",
            "audio-item-split",
            "audio-item-delete",
        ] {
            self.controls.remove(key);
        }
        let timeline = self
            .audio_preview
            .clone()
            .unwrap_or_else(|| self.editor.metadata().audio_timeline().unwrap_or_default());
        if timeline.clips().is_empty() {
            return false;
        }
        let second_lane = self.has_linked_audio_lane();
        if second_lane {
            painter.text(
                Pos2::new(canvas.left() + 8.0, canvas.top() + 147.0),
                Align2::LEFT_CENTER,
                "AUDIO 2",
                FontId::proportional(12.0),
                theme::SECONDARY,
            );
        }
        let mut hovered = false;
        for (index, clip) in timeline.clips().iter().enumerate() {
            let rect = Rect::from_min_size(
                Pos2::new(
                    origin + clip.timeline_start as f32 * zoom,
                    canvas.top() + if second_lane { 128.0 } else { 82.0 },
                ),
                Vec2::new((clip.duration() as f32 * zoom).max(3.0), 38.0),
            );
            if !rect.intersects(visible)
                && self.audio_drag.as_ref().is_none_or(|d| d.clip != clip.id)
            {
                continue;
            }
            if index == 0 {
                self.controls.insert("first-audio-item", rect);
            }
            let selected = self.selected_audio == Some(clip.id) && !self.library_preview;
            let settings = self
                .editor
                .metadata()
                .audio_items
                .iter()
                .find(|a| a.clip.id == clip.id)
                .map(|a| a.settings.clone())
                .unwrap_or_default();
            let cp = painter.with_clip_rect(rect.intersect(visible));
            cp.rect_filled(rect, 4.0, theme::AUDIO);
            if let Some(peaks) = self.waveforms.get(&clip.source) {
                for pixel in (rect.left().max(visible.left()) as i32
                    ..rect.right().min(visible.right()) as i32)
                    .step_by(3)
                {
                    let time =
                        clip.source_in + f64::from(pixel as f32 - rect.left()) / f64::from(zoom);
                    let h = if settings.muted {
                        0.0
                    } else {
                        peaks.get((time * 100.0) as usize).copied().unwrap_or(0.0)
                            * 10.0_f32.powf(settings.gain_db as f32 / 20.0)
                            * 14.0
                    }
                    .min(16.0);
                    cp.line_segment(
                        [
                            Pos2::new(pixel as f32, rect.center().y - h),
                            Pos2::new(pixel as f32, rect.center().y + h),
                        ],
                        Stroke::new(1.0, theme::WAVE),
                    );
                }
            }
            if selected {
                cp.rect_stroke(
                    rect.shrink(1.0),
                    4.0,
                    Stroke::new(1.5, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
                if rect.width() >= 16.0 {
                    for x in [rect.left() + 3.0, rect.right() - 3.0] {
                        cp.line_segment(
                            [
                                Pos2::new(x, rect.center().y - 6.0),
                                Pos2::new(x, rect.center().y + 6.0),
                            ],
                            Stroke::new(1.5, theme::TEXT),
                        );
                    }
                }
            }
            if rect.width() > 70.0 {
                cp.text(
                    rect.left_top() + Vec2::new(6.0, 3.0),
                    Align2::LEFT_TOP,
                    if settings.muted {
                        "Muted".into()
                    } else {
                        format!("{:+.1} dB", settings.gain_db)
                    },
                    FontId::proportional(12.0),
                    theme::TEXT,
                );
            }
            let response = ui.interact(
                rect,
                ui.id().with(("audio-item", clip.id.0)),
                Sense::click_and_drag(),
            );
            if response.has_focus() {
                self.timeline_focus = Some((response.id, clip.id));
                cp.rect_stroke(
                    rect.shrink(3.0),
                    2.0,
                    Stroke::new(1.5, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    ui.is_enabled(),
                    selected,
                    format!(
                        "Audio item {}, volume {:+.1} dB, {} on timeline, in {}, out {}",
                        clip.id.0,
                        settings.gain_db,
                        time(clip.timeline_start),
                        time(clip.source_in),
                        time(clip.source_out)
                    ),
                )
            });
            hovered |= response.hovered();
            if response.clicked() || response.secondary_clicked() || response.has_focus() {
                self.selected_audio = Some(clip.id);
                self.library_preview = false;
                if response.clicked() {
                    self.playing = false;
                    let position = response
                        .interact_pointer_pos()
                        .map_or(clip.timeline_start, |p| {
                            clip.timeline_start + f64::from((p.x - rect.left()) / zoom)
                        });
                    self.seek(position.clamp(clip.timeline_start, clip.timeline_end()));
                }
            }
            let source = self
                .editor
                .metadata()
                .sources
                .iter()
                .find(|s| s.id == clip.source)
                .cloned();
            if response.drag_started() {
                self.selected_audio = Some(clip.id);
                self.library_preview = false;
                self.playing = false;
                self.stepping = None;
                let px = ui
                    .input(|i| i.pointer.press_origin())
                    .map_or(rect.center().x, |p| p.x);
                self.audio_drag = Some(Drag {
                    base: self.editor.metadata().audio_timeline().unwrap(),
                    clip: clip.id,
                    start: clip.timeline_start,
                    mode: clip_drag_mode(rect, px),
                    source_in: clip.source_in,
                    source_out: clip.source_out,
                    pointer_x: px,
                    playhead: self.playhead,
                    preview_source: clip.source_in,
                });
            }
            if response.dragged()
                && let (Some(drag), Some(source)) = (&self.audio_drag, &source)
            {
                let delta = f64::from(
                    (response
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
                        start: (drag.source_in + (delta / frame).round_ties_even() * frame)
                            .clamp(0.0, drag.source_out - frame),
                        end: drag.source_out,
                        source_duration: source.duration,
                        rate: source.rate,
                    },
                    2 => Command::PreviewTrim {
                        clip: drag.clip,
                        start: drag.source_in,
                        end: (drag.source_out + (delta / frame).round_ties_even() * frame)
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
                    self.audio_preview = Some(preview.timeline().clone());
                }
            }
            if response.drag_stopped()
                && let (Some(preview), Some(drag)) =
                    (self.audio_preview.take(), self.audio_drag.take())
                && preview != drag.base
            {
                // Commit exactly the non-rippling geometry shown during the gesture.
                self.apply_audio(Command::Replace {
                    clips: preview.clips().to_vec(),
                });
            }
            if self.audio_drag.is_none() {
                clip_hover_cursor(ui, &response);
            }
            let keyboard = response.has_focus()
                && ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::F10));
            let mut menu = egui::Popup::context_menu(&response);
            let menu_id = egui::Popup::default_response_id(&response);
            let restore_focus = egui::Popup::is_id_open(ui.ctx(), menu_id)
                && ui.input(|i| i.key_pressed(egui::Key::Escape));
            if keyboard {
                menu = menu
                    .open_memory(Some(egui::SetOpenCommand::Bool(true)))
                    .at_position(rect.center());
            }
            menu.show(|ui| {
                self.selected_audio = Some(clip.id);
                self.library_preview = false;
                let split = ui
                    .button("Split audio")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                self.controls.insert("audio-item-split", split.rect);
                if split.clicked() {
                    self.split();
                    ui.close();
                }
                if ui
                    .button("Duplicate audio")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                    && let Some(source) = &source
                {
                    self.apply_audio(Command::Duplicate {
                        clip: clip.id,
                        rate: source.rate,
                    });
                    ui.close();
                }
                let delete = ui
                    .button("Delete audio")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                self.controls.insert("audio-item-delete", delete.rect);
                if delete.clicked() {
                    self.apply_audio(Command::Delete { clip: clip.id });
                    ui.close();
                }
            });
            if restore_focus && !egui::Popup::is_id_open(ui.ctx(), menu_id) {
                ui.memory_mut(|m| m.request_focus(response.id));
            }
        }
        if ui.is_enabled()
            && let Some(drag) = &self.audio_drag
            && ui.input(|i| i.pointer.primary_down())
        {
            ui.ctx().set_cursor_icon(clip_drag_cursor(drag.mode, true));
        }
        hovered || self.audio_drag.is_some()
    }
}
