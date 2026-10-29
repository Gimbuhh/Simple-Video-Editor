use super::*;
impl EditorApp {
    pub(super) fn verify_audio_items(
        &mut self,
        test: &mut crate::verification::Verification,
        ctx: &egui::Context,
    ) -> media_engine::Result<bool> {
        if test.phase < 223 {
            return Ok(false);
        }
        let original = test
            .audio_original
            .as_ref()
            .ok_or("Missing audio test baseline")?;
        if self.editor.timeline().clips() != original.clips {
            return Err("Independent audio edited the video lane".into());
        }
        match test.phase {
            223 => {
                test.pointer(
                    self.controls["inspector-separate-audio"].center(),
                    Some(true),
                );
                test.phase = 224;
            }
            224 => {
                test.pointer(
                    self.controls["inspector-separate-audio"].center(),
                    Some(false),
                );
                test.phase = 225;
            }
            225 => {
                let item = self
                    .editor
                    .metadata()
                    .audio_items
                    .first()
                    .ok_or("Separate audio did not create an item")?;
                let video = &original.clips[0];
                if item.clip.source_in.to_bits() != video.source_in.to_bits()
                    || item.clip.source_out.to_bits() != video.source_out.to_bits()
                    || item.clip.timeline_start.to_bits() != video.timeline_start.to_bits()
                    || self.selected_audio != Some(item.clip.id)
                {
                    return Err(
                        "Separation changed exact source trim/position or did not select audio"
                            .into(),
                    );
                }
                let Some(volume) = self.controls.get("audio-volume").copied() else {
                    return Ok(true);
                };
                test.pointer(
                    Pos2::new(volume.left() + 15.0, volume.center().y),
                    Some(true),
                );
                test.phase = 226;
            }
            226 => {
                let r = self.controls["audio-volume"];
                test.pointer(Pos2::new(r.left() + 15.0, r.center().y), Some(false));
                test.phase = 227;
            }
            227 => {
                let item = &self.editor.metadata().audio_items[0];
                if item.settings.gain_db >= -1.0 {
                    return Err("Audio item volume slider did not lower gain".into());
                }
                test.audio_gain = item.settings.gain_db;
                let start = item.clip.timeline_start;
                self.seek(start + 0.2);
                self.playing = true;
                test.phase = 228;
            }
            228 => {
                let status = self
                    .audio_playback
                    .as_ref()
                    .ok_or("Missing independent audio actor")?
                    .status();
                if let Some(error) = status.error {
                    return Err(error.into());
                }
                if status.ready && !status.paused && status.output.is_some() {
                    let item = &self.editor.metadata().audio_items[0];
                    let source = self
                        .editor
                        .metadata()
                        .sources
                        .iter()
                        .find(|s| s.id == item.clip.source)
                        .unwrap();
                    if status.item != Some(item.clip.id)
                        || status.stream
                            != item
                                .settings
                                .stream
                                .or_else(|| source.audio.first().map(|a| a.index))
                        || status.gain_db != test.audio_gain
                        || (status.position
                            - (item.clip.source_in + self.playhead - item.clip.timeline_start))
                            .abs()
                            > 0.2
                    {
                        return Err(
                            "Independent preview lost the track, gain or video clock".into()
                        );
                    }
                    test.checks.push("Separated audio reaches Windows output on the selected stream at item volume and follows the video clock".into());
                    self.playing = false;
                    let position = item.clip.timeline_start + 0.4;
                    self.seek(position);
                    test.phase = 229;
                }
            }
            229 => {
                let item = &self.editor.metadata().audio_items[0];
                let status = self.audio_playback.as_ref().unwrap().status();
                if status.ready
                    && status.paused
                    && (status.position - item.clip.source_in - 0.4).abs() < 0.05
                {
                    let saved = SavedProject::capture(&self.editor);
                    let path = test.directory.join("separated-audio.sveproject");
                    save_document(&path, &saved)?;
                    let loaded = self
                        .tools
                        .as_ref()
                        .unwrap()
                        .load_project(&path, &uncancelled())?;
                    if loaded.clips != saved.clips
                        || loaded.metadata.audio_items != saved.metadata.audio_items
                        || loaded.version != 3
                    {
                        return Err("Separated audio failed save/reopen".into());
                    }
                    test.keyboard_position = item.clip.timeline_start;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    test.phase = 230;
                }
            }
            230 => {
                for event in ctx.input(|i| i.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        test.screenshot(&image)?;
                        std::fs::copy(
                            test.directory.join("desktop.bmp"),
                            test.directory.join("audio-items.bmp"),
                        )?;
                        let p = self.controls["first-audio-item"].center();
                        test.anchor = Some(p);
                        test.pointer(p, Some(true));
                        test.phase = 231;
                    }
                }
            }
            231 => {
                test.pointer(test.anchor.unwrap() + Vec2::new(self.zoom * 0.5, 0.0), None);
                test.phase = 232;
            }
            232 => {
                test.pointer(test.anchor.unwrap() + Vec2::new(self.zoom * 0.5, 0.0), None);
                test.phase = 233;
            }
            233 => {
                test.pointer(
                    test.anchor.unwrap() + Vec2::new(self.zoom * 0.5, 0.0),
                    Some(false),
                );
                test.phase = 234;
            }
            234 => {
                if (self.editor.metadata().audio_items[0].clip.timeline_start
                    - test.keyboard_position
                    - 0.5)
                    .abs()
                    > 0.04
                {
                    return Err("Audio item did not move independently".into());
                }
                test.key(egui::Key::Z, true);
                test.phase = 235;
            }
            235 => {
                let item = &self.editor.metadata().audio_items[0];
                if (item.clip.timeline_start - test.keyboard_position).abs() > 0.000001 {
                    return Err("Independent move undo failed".into());
                }
                let rect = self.controls["first-audio-item"];
                if self
                    .timeline_drop
                    .is_none_or(|(canvas, _, _)| !canvas.contains_rect(rect))
                {
                    return Err("Audio lane is clipped".into());
                }
                let edge = Pos2::new(rect.right() - 2.0, rect.center().y);
                test.anchor = Some(edge);
                test.pointer(edge, Some(true));
                test.phase = 242;
            }
            242 => {
                test.pointer(test.anchor.unwrap() - Vec2::new(self.zoom * 0.2, 0.0), None);
                test.phase = 243;
            }
            243 => {
                test.pointer(test.anchor.unwrap() - Vec2::new(self.zoom * 0.2, 0.0), None);
                test.phase = 244;
            }
            244 => {
                if ctx.output(|o| o.cursor_icon) != egui::CursorIcon::ResizeHorizontal {
                    return Err("Audio trim lost the resize cursor".into());
                }
                test.pointer(
                    test.anchor.unwrap() - Vec2::new(self.zoom * 0.2, 0.0),
                    Some(false),
                );
                test.phase = 245;
            }
            245 => {
                let item = &self.editor.metadata().audio_items[0];
                if (original.clips[0].source_out - item.clip.source_out - 0.2).abs() > 0.04 {
                    return Err("Audio edge did not trim independently".into());
                }
                test.key(egui::Key::Z, true);
                test.phase = 246;
            }
            246 => {
                let item = &self.editor.metadata().audio_items[0];
                if item.clip.source_out.to_bits() != original.clips[0].source_out.to_bits() {
                    return Err("Audio trim undo lost the exact boundary".into());
                }
                self.seek(item.clip.timeline_start + 0.6);
                test.key(egui::Key::B, true);
                test.phase = 236;
            }
            236 => {
                if self.editor.metadata().audio_items.len() != 2 {
                    return Err("Split did not target selected audio only".into());
                }
                let volume = self.controls["audio-volume"];
                test.pointer(
                    Pos2::new(volume.left() + 70.0, volume.center().y),
                    Some(true),
                );
                test.phase = 237;
            }
            237 => {
                let volume = self.controls["audio-volume"];
                test.pointer(
                    Pos2::new(volume.left() + 70.0, volume.center().y),
                    Some(false),
                );
                test.phase = 238;
            }
            238 => {
                let items = &self.editor.metadata().audio_items;
                if items[0].settings.gain_db != test.audio_gain || items[1].settings.gain_db <= 0.0
                {
                    return Err(
                        "Volume increase changed the other audio item or did not raise gain".into(),
                    );
                }
                test.pointer(self.controls["first-audio-item"].center(), Some(true));
                test.phase = 239;
            }
            239 => {
                test.pointer(self.controls["first-audio-item"].center(), Some(false));
                test.phase = 240;
            }
            240 => {
                test.key(egui::Key::Delete, false);
                test.phase = 241;
            }
            241 => {
                if self.editor.metadata().audio_items.len() != 1 {
                    return Err("Delete did not target selected audio only".into());
                }
                test.checks.push("Audio separation preserves exact trims; independent volume, drag, edge trim, split, delete, undo and schema-3 save/reopen leave video unchanged".into());
                self.editor = test.audio_original.take().unwrap().editor()?;
                self.selected_audio = None;
                self.audio_preview = None;
                self.audio_drag = None;
                drop(self.audio_playback.take());
                self.sync_selection();
                self.update_dirty();
                self.request_key.clear();
                test.phase = 60;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
