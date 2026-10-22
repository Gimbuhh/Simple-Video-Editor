use super::*;
impl EditorApp {
    pub(super) fn verify(&mut self, ctx: &egui::Context) {
        let Some(mut test) = self.verification.take() else {
            return;
        };
        let result = (|| -> media_engine::Result<()> {
            std::fs::write(test.directory.join("phase.txt"), test.phase.to_string())?;
            if test.graphics_only {
                if test.started.elapsed() > Duration::from_millis(500) && test.phase != 7 {
                    test.checks
                        .push("Desktop graphics startup and rendered frames".into());
                    test.report();
                    self.close_approved = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    test.phase = 7;
                }
                return Ok(());
            }
            if test.started.elapsed()
                > Duration::from_secs(if test.capture_hold {
                    300
                } else if test.scale_layouts {
                    240
                } else {
                    90
                })
            {
                return Err(format!(
                    "Desktop verification timed out at phase {}: {}",
                    test.phase, self.message
                )
                .into());
            }
            if let Some(error) = self.playback.as_ref().and_then(|p| p.status().error) {
                return Err(error.into());
            }
            if test.capture_hold && test.phase == 80 {
                if test.directory.join("capture-stop").exists() {
                    test.checks
                        .push("Preview held until capture stop request".into());
                    test.report();
                    self.close_approved = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    test.phase = 7;
                }
                return Ok(());
            }
            if test.vfr && !test.capture_hold && test.phase != 6 && test.phase != 7 {
                self.verify_vfr(&mut test, ctx)?;
            } else {
                match test.phase {
                    0 if self.tools.is_some() => {
                        let mut paths = vec![test.source.clone()];
                        paths.extend(test.secondary.clone());
                        self.import(paths);
                        test.phase = 1;
                    }
                    1 if !self.busy => {
                        if test.capture_hold && test.capture_hdr {
                            let output = &mut self.editor.metadata_mut().output;
                            output.profile = ColorProfile::Hdr10;
                            output.codec = Codec::Hevc;
                        }
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
                        let status = self.playback.as_ref().unwrap().status();
                        if test.capture_hold && test.capture_hdr && !status.software {
                            let Some(display) = &status.display else {
                                return Ok(());
                            };
                            if !display.hdr_active {
                                return Err(
                                    "Capture test requires active HDR on the preview display"
                                        .into(),
                                );
                            }
                            if status.gamma.as_deref() != Some("pq") {
                                return Ok(());
                            }
                        }
                        test.checks
                            .push("Source load and shared preview filter".into());
                        if test.capture_hold {
                            std::fs::write(
                                test.directory.join("capture-ready.json"),
                                serde_json::to_vec_pretty(&serde_json::json!({
                                    "software": status.software,
                                    "hdrOutputRequested": test.capture_hdr,
                                    "generation": status.generation,
                                    "position": status.position,
                                    "outputGamma": status.gamma,
                                    "displayHdrActive": status.display.as_ref().map(|d| d.hdr_active),
                                    "displayStateKnown": status.display.as_ref().map(|d| d.state_known),
                                }))?,
                            )?;
                            test.phase = 80;
                            return Ok(());
                        }
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
                        if test.startup_only {
                            test.report();
                            self.close_approved = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            test.phase = 7;
                            return Ok(());
                        }
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
                            if test.source.file_name().is_some_and(|s| s == "multiple.mkv") {
                                let center = frame.height / 2 * frame.stride + frame.width / 2 * 4;
                                if frame.pixels[center + 2] < 180 || frame.pixels[center] > 80 {
                                    return Err("Preview selected a different video stream than probing and export".into());
                                }
                                test.checks.push("Preview selects the probed video stream even when another video is the default".into());
                            }
                            if matches!(status.gamma.as_deref(), Some("pq" | "hlg")) {
                                return Err(
                                    "Software fallback retained an HDR transfer function".into()
                                );
                            }
                            test.checks.push(
                                "HDR-capable import rendered into a nonblack SDR software frame"
                                    .into(),
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
                            "Playback uses the media clock and pauses through keyboard input"
                                .into(),
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
                                "Pointer drag did not move the linked section into free space"
                                    .into(),
                            );
                        }
                        if (self.playhead - clip.timeline_start).abs() > 0.000001 {
                            return Err("Dragging a newly selected section did not preview its first retained frame".into());
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
                        if self.editor.metadata().sources.len() > 1 {
                            self.add(vec![self.editor.metadata().sources[1].id]);
                            test.scrub_target = 3.6031;
                        }
                        self.seek(0.0);
                        test.key(egui::Key::Space, false);
                        test.phase = 24;
                    }
                    24 if self.playing
                        && self.playback.as_ref().is_some_and(|p| {
                            let status = p.status();
                            status.ready && status.generation == self.generation && !status.paused
                        }) =>
                    {
                        let ruler = *self
                            .controls
                            .get("ruler")
                            .ok_or("Missing ruler input surface")?;
                        test.anchor = Some(Pos2::new(ruler.left(), ruler.center().y));
                        test.pointer(
                            test.anchor.unwrap() + Vec2::new(self.zoom * 0.3031, 0.0),
                            Some(true),
                        );
                        test.phase = 25;
                    }
                    25 => {
                        test.pointer(
                            test.anchor.unwrap()
                                + Vec2::new(self.zoom * test.scrub_target as f32, 0.0),
                            None,
                        );
                        test.phase = 26;
                    }
                    26..=28 => {
                        if self.scrub.is_none()
                            || self.playing
                            || (self.playhead - test.scrub_target).abs() > 0.00001
                        {
                            return Err(format!(
                                "Held ruler lost pointer ownership: {} versus {}",
                                self.playhead, test.scrub_target
                            )
                            .into());
                        }
                        let status = self.playback.as_ref().unwrap().status();
                        if status.ready && status.generation == self.generation && status.paused {
                            test.phase += 1;
                        }
                    }
                    29 => {
                        test.checks.push(
                            "Held ruler remains continuous across recording loads and native ticks"
                                .into(),
                        );
                        test.pointer(
                            test.anchor.unwrap()
                                + Vec2::new(self.zoom * test.scrub_target as f32, 0.0),
                            Some(false),
                        );
                        test.phase = 30;
                    }
                    30 if self.settling.is_none()
                        && self.playing
                        && self.playback.as_ref().is_some_and(|p| !p.status().paused) =>
                    {
                        test.checks.push(
                            "Final seek settles before restoring the prior playing state".into(),
                        );
                        test.key(egui::Key::Space, false);
                        test.phase = 31;
                    }
                    31 if !self.playing => {
                        test.pointer(
                            test.anchor.unwrap()
                                + Vec2::new(
                                    (self.preview_duration() as f32 + 1.0) * self.zoom,
                                    0.0,
                                ),
                            Some(true),
                        );
                        test.phase = 32;
                    }
                    32 => {
                        if (self.playhead - self.preview_duration()).abs() > 1e-6 {
                            return Err(
                                "Held outside ruler did not clamp to the sequence end".into()
                            );
                        }
                        test.pointer(
                            test.anchor.unwrap()
                                + Vec2::new(
                                    (self.preview_duration() as f32 + 1.0) * self.zoom,
                                    0.0,
                                ),
                            Some(false),
                        );
                        test.phase = 33;
                    }
                    33 if self.settling.is_none()
                        && self.playback.as_ref().is_some_and(|p| {
                            p.status().ready && p.status().generation == self.generation
                        }) =>
                    {
                        let last = self.editor.timeline().clips().last().unwrap();
                        let source = self
                            .editor
                            .metadata()
                            .sources
                            .iter()
                            .find(|s| s.id == last.source)
                            .unwrap();
                        let expected = last.source_out - source.rate.frame_seconds();
                        let position = self.playback.as_ref().unwrap().status().position;
                        if (position - expected).abs() > source.rate.frame_seconds().max(0.05)
                            || self.playing
                        {
                            return Err(
                                "Endpoint scrub did not retain the final frame paused".into()
                            );
                        }
                        test.checks.push(
                            "Endpoint scrub displays the last retained frame and stays paused"
                                .into(),
                        );
                        let rect = self.controls["preview"];
                        let viewport = self.controls["preview-viewport"];
                        let output = self.preview_output();
                        if (rect.aspect_ratio() - output.width as f32 / output.height as f32).abs()
                            > 0.001
                            || !viewport.expand(0.1).contains_rect(rect)
                        {
                            return Err("Preview aspect or viewport containment failed".into());
                        }
                        test.checks
                            .push("Preview remains fitted with its output aspect ratio".into());
                        self.seek(0.25);
                        test.expected = Some(self.editor.timeline().clips().to_vec());
                        test.phase = 34;
                    }
                    34 => {
                        let rect = self.controls["first-clip"];
                        test.anchor = Some(Pos2::new(rect.left() + 2.0, rect.center().y));
                        test.pointer(test.anchor.unwrap(), Some(true));
                        test.phase = 35;
                    }
                    35 => {
                        test.pointer(test.anchor.unwrap() + Vec2::new(self.zoom * 0.1, 0.0), None);
                        test.phase = 36;
                    }
                    36 => {
                        let clips = self.timeline().clips();
                        let original = test.expected.as_ref().unwrap();
                        if self.drag.as_ref().is_none_or(|d| d.mode != 1)
                            || clips[0].timeline_start <= original[0].timeline_start
                            || (clips[0].timeline_end() - original[0].timeline_end()).abs() > 1e-6
                            || clips[1].timeline_start != original[1].timeline_start
                            || (self.playhead - 0.25).abs() > 1e-6
                        {
                            return Err("Left trim preview did not anchor the right edge, followers and retained frame".into());
                        }
                        test.pointer(
                            test.anchor.unwrap() + Vec2::new(self.zoom * 0.1, 0.0),
                            Some(false),
                        );
                        test.phase = 37;
                    }
                    37 => {
                        let clips = self.editor.timeline().clips();
                        let original = test.expected.as_ref().unwrap();
                        let delta = clips[0].source_in - original[0].source_in;
                        if self.preview_timeline.is_some()
                            || clips[0].timeline_start != original[0].timeline_start
                            || (clips[1].timeline_start - original[1].timeline_start + delta).abs()
                                > 1e-6
                            || (self.playhead - 0.25 + delta).abs() > 1e-6
                        {
                            return Err(
                            "Trim commit did not ripple once and retain the preview source frame"
                                .into(),
                        );
                        }
                        test.checks.push("Trim pointer preview anchors the right edge and commit ripples with the retained frame".into());
                        test.key(egui::Key::Z, true);
                        test.phase = 38;
                    }
                    38 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Trim was not one undoable edit".into());
                        }
                        self.seek(0.25);
                        test.pointer(test.anchor.unwrap(), Some(true));
                        test.phase = 39;
                    }
                    39 => {
                        test.pointer(test.anchor.unwrap() + Vec2::new(self.zoom * 0.3, 0.0), None);
                        test.phase = 40;
                    }
                    40 => {
                        if self.playhead < 0.3 {
                            return Err("Trimming past the preview frame failed to clamp it".into());
                        }
                        test.key(egui::Key::Escape, false);
                        test.phase = 41;
                    }
                    41 => {
                        test.pointer(
                            test.anchor.unwrap() + Vec2::new(self.zoom * 0.3, 0.0),
                            Some(false),
                        );
                        test.phase = 42;
                    }
                    42 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                            || self.preview_timeline.is_some()
                            || (self.playhead - 0.25).abs() > 1e-6
                        {
                            return Err(
                                "Escape failed to restore trim bounds and original preview frame"
                                    .into(),
                            );
                        }
                        test.checks.push(
                            "Trim is one undo step; Escape restores both bounds and preview frame"
                                .into(),
                        );
                        self.editor.metadata_mut().output.rate =
                            editor_core::FrameRate::new(120, 1)?;
                        self.seek(0.2);
                        test.phase = 43;
                    }
                    43 if self.playback.as_ref().is_some_and(|p| {
                        p.status().ready && p.status().generation == self.generation
                    }) =>
                    {
                        test.key(egui::Key::ArrowRight, false);
                        test.phase = 44;
                    }
                    44 if self.stepping.is_none() => {
                        let frame = self.editor.metadata().sources[0].rate.frame_seconds();
                        if (self.playhead - 0.2 - frame).abs() > 0.002 || self.playing {
                            return Err(format!("Forward frame key used the output rate instead of source timestamps: {}",self.playhead).into());
                        }
                        test.key(egui::Key::ArrowLeft, false);
                        test.phase = 45;
                    }
                    45 if self.stepping.is_none() => {
                        if (self.playhead - 0.2).abs() > 0.002 || self.playing {
                            return Err("Backward frame key did not return to the previous decoded frame paused".into());
                        }
                        test.checks.push("Frame keys use decoded source timestamps independently of the export frame rate".into());
                        test.expected = Some(self.editor.timeline().clips().to_vec());
                        test.pointer(self.controls["recording-search"].center(), Some(true));
                        test.phase = 46;
                    }
                    46 => {
                        test.pointer(self.controls["recording-search"].center(), Some(false));
                        test.phase = 47;
                    }
                    47 => {
                        test.input
                            .push(egui::Event::Text("qw no matching recordings".into()));
                        test.key(egui::Key::Q, false);
                        test.key(egui::Key::W, false);
                        test.phase = 48;
                    }
                    48 => {
                        if self.search != "qw no matching recordings"
                            || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        {
                            return Err("Search text input triggered timeline shortcuts".into());
                        }
                        test.pointer(self.controls["add-selected"].center(), Some(true));
                        test.phase = 49;
                    }
                    49 => {
                        test.pointer(self.controls["add-selected"].center(), Some(false));
                        test.phase = 50;
                    }
                    50 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Search-hidden selection was added to the timeline".into());
                        }
                        test.pointer(self.controls["add-all"].center(), Some(true));
                        test.phase = 51;
                    }
                    51 => {
                        test.pointer(self.controls["add-all"].center(), Some(false));
                        test.phase = 52;
                    }
                    52 => {
                        let original = test.expected.as_ref().unwrap();
                        let added = &self.editor.timeline().clips()[original.len()..];
                        let expected: Vec<_> = self
                            .editor
                            .metadata()
                            .sources
                            .iter()
                            .map(|s| s.id)
                            .collect();
                        if added.iter().map(|c| c.source).collect::<Vec<_>>() != expected {
                            return Err(
                                "Add all omitted or reordered search-hidden recordings".into()
                            );
                        }
                        test.key(egui::Key::Z, true);
                        test.phase = 53;
                    }
                    53 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Filtered Add all was not one undo step".into());
                        }
                        test.pointer(self.controls["recording-search"].center(), Some(true));
                        test.phase = 54;
                    }
                    54 => {
                        test.pointer(self.controls["recording-search"].center(), Some(false));
                        test.phase = 55;
                    }
                    55 => {
                        test.key(egui::Key::Escape, false);
                        test.phase = 56;
                    }
                    56 => {
                        if !self.search.is_empty()
                            || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        {
                            return Err(
                                "Escape did not clear search without editing the timeline".into()
                            );
                        }
                        test.checks.push("Search owns text shortcuts; hidden selections stay disabled; Add all includes the full library as one undo step".into());
                        self.seek(0.2);
                        test.key(egui::Key::W, false);
                        test.phase = 57;
                    }
                    57 if self.stepping.is_none() => {
                        let frame = self.editor.metadata().sources[0].rate.frame_seconds();
                        let clip = &self.editor.timeline().clips()[0];
                        if (clip.source_out - 0.2 - frame).abs() > 0.002
                            || (self.playhead - 0.2).abs() > 0.002
                        {
                            return Err(format!("Trim after a pending seek used out={} preview={} instead of out={} preview=0.2",clip.source_out,self.playhead,0.2+frame).into());
                        }
                        test.key(egui::Key::Z, true);
                        test.phase = 58;
                    }
                    58 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Quick trim was not one undoable edit".into());
                        }
                        self.seek(0.2);
                        test.key(egui::Key::B, true);
                        test.phase = 59;
                    }
                    59 => {
                        if self.editor.timeline().clips().len()
                            != test.expected.as_ref().unwrap().len() + 1
                        {
                            return Err("Legacy Ctrl+B split shortcut failed".into());
                        }
                        test.key(egui::Key::Z, true);
                        test.phase = 64;
                    }
                    64 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Ctrl+B split was not one undo step".into());
                        }
                        test.key(egui::Key::F, true);
                        test.phase = 65;
                    }
                    65 => {
                        test.input.push(egui::Event::Text("focused".into()));
                        test.phase = 66;
                    }
                    66 => {
                        if self.search != "focused" {
                            return Err("Ctrl+F did not focus recording search".into());
                        }
                        test.key(egui::Key::Escape, false);
                        test.phase = 67;
                    }
                    67 => {
                        if !self.search.is_empty() {
                            return Err("Escape failed after Ctrl+F".into());
                        }
                        test.checks.push("Legacy Ctrl+B/Ctrl+F shortcuts and frame-inclusive quick trim remain undoable".into());
                        test.was_software = self.software_preview;
                        test.pointer(self.controls["software-preview"].center(), Some(true));
                        test.phase = 80;
                    }
                    80 => {
                        test.pointer(self.controls["software-preview"].center(), Some(false));
                        test.phase = 81;
                    }
                    81 if self.playback.as_ref().is_some_and(|p| {
                        p.status().ready && p.status().generation == self.generation
                    }) =>
                    {
                        if self.software_preview == test.was_software
                            || self.software_preview
                                && !self.playback.as_ref().unwrap().status().software
                            || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        {
                            return Err(
                                "Preview mode change lost its setting or edited the timeline"
                                    .into(),
                            );
                        }
                        test.pointer(self.controls["software-preview"].center(), Some(true));
                        test.phase = 82;
                    }
                    82 => {
                        test.pointer(self.controls["software-preview"].center(), Some(false));
                        test.phase = 83;
                    }
                    83 if self.playback.as_ref().is_some_and(|p| {
                        p.status().ready && p.status().generation == self.generation
                    }) =>
                    {
                        if self.software_preview != test.was_software
                            || self.software_preview
                                && !self.playback.as_ref().unwrap().status().software
                            || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        {
                            return Err(
                                "Preview mode restoration changed the project or failed to reload"
                                    .into(),
                            );
                        }
                        test.checks.push("Preview modes switch and restore through real input without changing the project".into());
                        test.phase = 84;
                    }
                    84 => {
                        if self.timeline_focus.is_some() {
                            test.key(egui::Key::Enter, false);
                            test.phase = 85;
                        } else {
                            test.focus_steps += 1;
                            if test.focus_steps > 80 {
                                return Err(format!(
                                    "Tab navigation could not reach a timeline section (window focused: {}, widget: {:?}, busy: {})",
                                    ctx.input(|i| i.focused), ctx.memory(|m| m.focused()), self.busy
                                ).into());
                            }
                            test.key(egui::Key::Tab, false);
                        }
                    }
                    85 => {
                        let (_, focused) = self
                            .timeline_focus
                            .ok_or("Timeline focus was lost on Enter")?;
                        if self.library_preview || self.selected != Some(focused) {
                            return Err(
                                "Enter did not select the keyboard-focused timeline section".into(),
                            );
                        }
                        let clip = self
                            .selected_clip()
                            .ok_or("Keyboard selection has no section")?;
                        self.seek(clip.timeline_start + clip.duration() * 0.5);
                        test.phase = 86;
                    }
                    86 if self.playback.as_ref().is_some_and(|p| {
                        let status = p.status();
                        status.ready && status.generation == self.generation
                    }) =>
                    {
                        test.keyboard_position = self.playhead;
                        test.key(egui::Key::ArrowRight, false);
                        test.phase = 97;
                    }
                    97 if self.stepping.is_none()
                        && self.playback.as_ref().is_some_and(|p| {
                            let status = p.status();
                            status.ready && status.generation == self.generation
                        }) =>
                    {
                        if self.timeline_focus.is_none() || self.playhead <= test.keyboard_position
                        {
                            return Err(
                                "Right frame key lost timeline focus or failed to advance".into()
                            );
                        }
                        test.key(egui::Key::ArrowLeft, false);
                        test.phase = 98;
                    }
                    98 if self.stepping.is_none()
                        && self.playback.as_ref().is_some_and(|p| {
                            let status = p.status();
                            status.ready && status.generation == self.generation
                        }) =>
                    {
                        let clip = self
                            .selected_clip()
                            .ok_or("Frame step lost its selected section")?;
                        let frame = self
                            .editor
                            .metadata()
                            .sources
                            .iter()
                            .find(|s| s.id == clip.source)
                            .ok_or("Frame step lost its recording")?
                            .rate
                            .frame_seconds();
                        if self.timeline_focus.is_none()
                            || (self.playhead - test.keyboard_position).abs() > frame + 0.000001
                        {
                            return Err(
                                "Left frame key lost timeline focus or failed to return".into()
                            );
                        }
                        test.key(egui::Key::S, false);
                        test.phase = 87;
                    }
                    87 => {
                        if self.editor.timeline().clips().len()
                            != test.expected.as_ref().unwrap().len() + 1
                        {
                            return Err(
                                "Split shortcut was blocked by timeline keyboard focus".into()
                            );
                        }
                        test.key(egui::Key::Z, true);
                        test.phase = 88;
                    }
                    88 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Undo shortcut failed with timeline keyboard focus".into());
                        }
                        test.key(egui::Key::Space, false);
                        test.phase = 89;
                    }
                    89 => {
                        if !self.playing {
                            return Err("Space did not play the keyboard-focused timeline".into());
                        }
                        test.key(egui::Key::Space, false);
                        test.phase = 90;
                    }
                    90 => {
                        if self.playing {
                            return Err("Space did not pause the keyboard-focused timeline".into());
                        }
                        test.key(egui::Key::Delete, false);
                        test.phase = 91;
                    }
                    91 => {
                        if self.editor.timeline().clips().len() + 1
                            != test.expected.as_ref().unwrap().len()
                        {
                            return Err(
                                "Delete shortcut failed with timeline keyboard focus".into()
                            );
                        }
                        test.key(egui::Key::Z, true);
                        test.phase = 92;
                    }
                    92 => {
                        if self.editor.timeline().clips() != test.expected.as_ref().unwrap() {
                            return Err("Keyboard deletion was not restored by one undo".into());
                        }
                        test.key(egui::Key::F, true);
                        test.phase = 93;
                    }
                    93 => {
                        test.key(egui::Key::S, false);
                        test.input.push(egui::Event::Text("s".into()));
                        test.phase = 94;
                    }
                    94 => {
                        if self.search != "s"
                            || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                        {
                            return Err("Search typing triggered a timeline edit".into());
                        }
                        test.key(egui::Key::Escape, false);
                        test.phase = 95;
                    }
                    95 => {
                        if !self.search.is_empty() {
                            return Err(
                                "Escape did not release search after keyboard editing".into()
                            );
                        }
                        test.checks.push("Tab/Enter timeline selection, frame keys, split/undo/delete and Space transport preserve text-entry focus".into());
                        test.phase = 60;
                    }
                    60 => {
                        ctx.set_zoom_factor(test.layout_scale());
                        // Zoom applies on the next pass. Resize only after its
                        // new point-to-pixel conversion is active.
                        test.phase = 96;
                    }
                    96 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(test.layout_size()));
                        test.phase = 61;
                    }
                    61 => {
                        if (ctx.screen_rect().size() - test.layout_size()).length() < 3.0 {
                            test.phase = 62;
                        }
                    }
                    62 => {
                        let rect = self.controls["preview"];
                        let viewport = self.controls["preview-viewport"];
                        let screen = ctx.screen_rect();
                        let output = self.preview_output();
                        let ppp = ctx.pixels_per_point();
                        let expected_bounds = (
                            (rect.left() * ppp).round() as i32,
                            (rect.top() * ppp).round() as i32,
                            (rect.width() * ppp).round() as i32,
                            (rect.height() * ppp).round() as i32,
                        );
                        if self.video.bounds()? != expected_bounds {
                            return Err(
                                "Native child bounds did not match the scaled preview".into()
                            );
                        }
                        if rect.width() < 1.0
                            || rect.height() < 1.0
                            || !viewport.expand(0.1).contains_rect(rect)
                            || !screen.expand(1.0).contains_rect(viewport)
                            || (rect.aspect_ratio() - output.width as f32 / output.height as f32)
                                .abs()
                                > 0.001
                            || self.controls["scrubber"].bottom() > self.controls["ruler"].top()
                            || self.controls["position-label"]
                                .intersects(self.controls["frame-forward"])
                            || self.controls["frame-forward"].right() > viewport.right() + 0.1
                        {
                            return Err(format!(
                                "Preview or transport escaped its layout at resize {}, scale {} (screen {:?}, viewport {:?}, preview {:?}, ruler {:?}, scrubber {:?}, label {:?}, next {:?})",
                                test.layout, test.layout_scale(), screen, viewport, rect,
                                self.controls["ruler"], self.controls["scrubber"],
                                self.controls["position-label"], self.controls["frame-forward"]
                            )
                            .into());
                        }
                        ctx.send_viewport_cmd(
                            egui::ViewportCommand::Screenshot(Default::default()),
                        );
                        test.phase = 63;
                    }
                    63 => {
                        for event in ctx.input(|i| i.events.clone()) {
                            if let egui::Event::Screenshot { image, .. } = event {
                                test.screenshot(&image)?;
                                std::fs::copy(
                                    test.directory.join("desktop.bmp"),
                                    test.directory.join(if test.scale_layouts {
                                        format!("scale-{}-layout-{}.bmp", test.scale, test.layout)
                                    } else {
                                        format!("layout-{}.bmp", test.layout)
                                    }),
                                )?;
                                test.layout += 1;
                                if test.layout == 4 && test.scale_layouts && test.scale < 4 {
                                    test.scale += 1;
                                    test.layout = 0;
                                }
                                if test.layout < 4 {
                                    test.phase = 60;
                                } else {
                                    test.checks.push(if test.scale_layouts {
                                        "Preview, actual native bounds and transport fit four sizes at 100/125/150/175/200% UI scale".into()
                                    } else {
                                        "Preview, actual native bounds and transport fit four window sizes with the correct aspect".into()
                                    });
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                                        Default::default(),
                                    ));
                                    test.phase = 6;
                                }
                            }
                        }
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
    fn verify_vfr(
        &mut self,
        test: &mut crate::verification::Verification,
        ctx: &egui::Context,
    ) -> media_engine::Result<()> {
        let ready = self
            .playback
            .as_ref()
            .is_some_and(|p| p.status().ready && p.status().generation == self.generation);
        match test.phase {
            0 if self.tools.is_some() => {
                self.import(vec![test.source.clone()]);
                test.phase = 1;
            }
            1 if !self.busy => {
                self.add(vec![self.editor.metadata().sources[0].id]);
                self.seek(0.14);
                test.phase = 2;
            }
            2 if ready => {
                test.key(egui::Key::ArrowRight, false);
                test.phase = 3;
            }
            3 if ready && self.stepping.is_none() => {
                if (self.playhead - 0.18).abs() > 0.002 {
                    return Err("VFR frame key did not reach the next PTS".into());
                }
                test.checks
                    .push("VFR frame key follows decoded timestamps".into());
                self.seek(0.2);
                test.phase = 4;
            }
            4 if ready => {
                if (self.playback.as_ref().unwrap().status().position - 0.38).abs() > 0.002 {
                    return Err(
                        "Between-frame seek was acknowledged before the actual frame presentation"
                            .into(),
                    );
                }
                test.key(egui::Key::W, false);
                test.phase = 5;
            }
            5 if ready && self.stepping.is_none() => {
                if (self.editor.timeline().clips()[0].source_out - 0.42).abs() > 0.002
                    || (self.playhead - 0.38).abs() > 0.002
                {
                    return Err(
                        "VFR trim after did not retain the complete current frame interval".into(),
                    );
                }
                test.checks.push(
                    "VFR trim after excludes the actual next PTS and restores the retained frame"
                        .into(),
                );
                test.key(egui::Key::Z, true);
                test.phase = 8;
            }
            8 => {
                if self.editor.timeline().clips()[0].source_out < 0.8 {
                    return Err("VFR quick trim undo failed".into());
                }
                self.seek(0.2);
                test.phase = 9;
            }
            9 => {
                test.key(egui::Key::Q, false);
                test.phase = 10;
            }
            10 if ready => {
                if (self.editor.timeline().clips()[0].source_in - 0.38).abs() > 0.002 {
                    return Err("VFR trim before ignored the actual preview timestamp".into());
                }
                test.checks
                    .push("VFR trim before and undo preserve decoded frame boundaries".into());
                self.recovery.save(&SavedProject::capture(&self.editor))?;
                use std::os::windows::fs::OpenOptionsExt;
                test.recovery_lock = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(0)
                        .open(&self.recovery.path)?,
                );
                let clip = self.editor.timeline().clips()[0].id;
                let rate = self.editor.metadata().sources[0].rate;
                self.apply(Command::Duplicate { clip, rate });
                self.recovery_saved = None;
                self.busy = true;
                self.exporting = true;
                self.recovery_tick = Instant::now() - Duration::from_secs(3);
                test.phase = 11;
            }
            11 if self.recovery_saved.is_none()
                && self.message.starts_with("Recovery save failed") =>
            {
                drop(test.recovery_lock.take());
                self.recovery_tick = Instant::now() - Duration::from_secs(3);
                test.phase = 12;
            }
            12 => {
                let written = read_document(&self.recovery.path)
                    .ok()
                    .and_then(|v| serde_json::from_value::<SavedProject>(v).ok());
                if written.as_ref() == Some(&SavedProject::capture(&self.editor)) {
                    test.checks.push("Recovery retries a locked-file failure while export is busy and preserves the latest edit".into());
                    self.busy = false;
                    self.exporting = false;
                    self.path = Some(test.directory.join("verified-save.sveproject"));
                    self.save(false);
                    test.phase = 13;
                }
            }
            13 if !self.busy => {
                if self.dirty() || self.recovery.path.exists() || self.recovery_saved.is_some() {
                    return Err("Successful project save left a stale recovery session".into());
                }
                let document: SavedProject =
                    serde_json::from_value(read_document(self.path.as_ref().unwrap())?)?;
                if document != SavedProject::capture(&self.editor) {
                    return Err("Asynchronous save changed the current project".into());
                }
                test.checks.push("Successful asynchronous save clears only its own obsolete recovery and marks the exact snapshot saved".into());
                self.selected = self.editor.timeline().clips().first().map(|c| c.id);
                self.trim(Some(0.38123456789), Some(0.82987654321));
                test.precision = Some(SavedProject::capture(&self.editor));
                test.phase = 14;
            }
            14 => {
                test.pointer(self.controls["trim-in"].center(), Some(true));
                test.phase = 15;
            }
            15 => {
                test.pointer(self.controls["trim-in"].center(), Some(false));
                test.phase = 16;
            }
            16 => {
                test.key(egui::Key::Enter, false);
                test.phase = 17;
            }
            17 => {
                if SavedProject::capture(&self.editor) != *test.precision.as_ref().unwrap() {
                    return Err(
                        "Unchanged in-point field rounded the original trim timestamp".into(),
                    );
                }
                test.pointer(self.controls["trim-out"].center(), Some(true));
                test.phase = 18;
            }
            18 => {
                test.pointer(self.controls["trim-out"].center(), Some(false));
                test.phase = 19;
            }
            19 => {
                test.key(egui::Key::Enter, false);
                test.phase = 20;
            }
            20 => {
                if SavedProject::capture(&self.editor) != *test.precision.as_ref().unwrap() {
                    return Err(
                        "Unchanged out-point field rounded the original trim timestamp".into(),
                    );
                }
                test.checks.push("Focusing and accepting unchanged in/out fields preserves full timestamp precision".into());
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                test.phase = 6;
            }
            _ => {}
        }
        Ok(())
    }
}
