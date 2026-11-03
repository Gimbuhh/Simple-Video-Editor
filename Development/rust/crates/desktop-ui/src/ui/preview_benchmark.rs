use super::*;
impl EditorApp {
    pub(super) fn verify_preview_benchmark(
        &mut self,
        test: &mut crate::verification::Verification,
        ctx: &egui::Context,
    ) -> media_engine::Result<()> {
        const TARGETS: [f64; 8] = [0.75, 8.4, 2.2, 9.1, 4.6, 7.3, 1.2, 6.4];
        if test.phase == 3
            && let Some(playback) = &self.playback
        {
            let status = playback.status();
            if !status.ready {
                if status.presentation_key.as_ref() != Some(&self.request_key)
                    || status.software && status.frame.is_none()
                {
                    return Err(
                        "A same-presentation seek discarded its last valid preview frame".into(),
                    );
                }
                test.retained_frame_checks += 1;
            }
        }
        match test.phase {
            0 if self.tools.is_some() => {
                self.import(vec![test.source.clone()]);
                test.phase = 1;
            }
            1 if !self.busy => {
                let source = self
                    .editor
                    .metadata()
                    .sources
                    .first()
                    .ok_or("Benchmark import failed")?;
                self.library_selected = Some(source.id);
                self.library_preview = true;
                self.seek(0.0);
                test.preview_started = Some(Instant::now());
                test.phase = 2;
            }
            2 | 3
                if self.playback.as_ref().is_some_and(|p| {
                    let status = p.status();
                    status.ready
                        && status.generation == self.generation
                        && (!status.software || status.frame.is_some())
                }) =>
            {
                let status = self.playback.as_ref().unwrap().status();
                let elapsed = test.preview_started.take().unwrap().elapsed().as_secs_f64() * 1000.0;
                test.preview_samples.push(serde_json::json!({"kind":if test.phase==2 {"load"} else {"seek"},"target":self.playhead,"decodedPosition":status.position,"ms":elapsed,"software":status.software,"decoder":status.decoder}));
                if test.preview_samples.len() == 25 {
                    test.scrub_start_serial = status.presentation_serial;
                    test.preview_started = Some(Instant::now());
                    let scrubber = self.controls["scrubber"];
                    test.pointer(
                        Pos2::new(
                            scrubber.left() + scrubber.width() * 0.15,
                            scrubber.center().y,
                        ),
                        Some(true),
                    );
                    test.phase = 4;
                } else {
                    let target = TARGETS[(test.preview_samples.len() - 1) % TARGETS.len()]
                        .min((self.preview_duration() - 0.1).max(0.0));
                    self.seek(target);
                    test.preview_started = Some(Instant::now());
                    test.phase = 3;
                }
            }
            4 => {
                if self.scrub.is_none() || self.playing {
                    return Err("Continuous preview gesture lost ownership".into());
                }
                let elapsed = test.preview_started.unwrap().elapsed().as_secs_f64();
                if elapsed < 2.0 {
                    let scrubber = self.controls["scrubber"];
                    test.pointer(
                        Pos2::new(
                            scrubber.left()
                                + scrubber.width() * (0.15 + 0.65 * (elapsed / 2.0) as f32),
                            scrubber.center().y,
                        ),
                        None,
                    );
                } else {
                    let status = self.playback.as_ref().unwrap().status();
                    test.scrub_frames = status
                        .presentation_serial
                        .wrapping_sub(test.scrub_start_serial);
                    if test.scrub_frames < 2 {
                        return Err(format!(
                            "Continuous scrubbing starved the preview: {} completed frames",
                            test.scrub_frames
                        )
                        .into());
                    }
                    test.scrub_target = self.playhead;
                    test.pointer(
                        ctx.input(|i| i.pointer.latest_pos())
                            .ok_or("Missing scrub pointer")?,
                        Some(false),
                    );
                    test.preview_started = Some(Instant::now());
                    test.phase = 5;
                }
            }
            5 if self.scrub.is_none()
                && self.settling.is_none()
                && self.playback.as_ref().is_some_and(|p| {
                    let status = p.status();
                    status.ready && status.generation == self.generation && status.paused
                }) =>
            {
                let status = self.playback.as_ref().unwrap().status();
                let rate = self.editor.metadata().sources[0].rate;
                if (status.position - self.playhead).abs() > rate.frame_seconds() + 0.002
                    || (self.playhead - test.scrub_target).abs() > rate.frame_seconds() + 0.002
                {
                    return Err("Continuous scrub release did not settle on the final frame".into());
                }
                test.scrub_release_ms =
                    test.preview_started.take().unwrap().elapsed().as_secs_f64() * 1000.0;
                std::fs::write(
                    test.directory.join("preview-latency.json"),
                    serde_json::to_vec_pretty(&serde_json::json!({
                        "samples":test.preview_samples,"retainedFrameChecks":test.retained_frame_checks,
                        "continuousScrubFrames":test.scrub_frames,"continuousScrubSeconds":2.0,"finalScrubReleaseMs":test.scrub_release_ms,
                        "scope":"UI request to production playback-ready acknowledgement; software requires a rendered frame; OS/filesystem caching uncontrolled"
                    }))?,
                )?;
                test.checks
                    .push("Production preview load and 24 exact seeks completed".into());
                test.checks.push("Continuous actual-pointer scrubbing presents new frames and release settles exactly".into());
                test.report();
                self.close_approved = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                test.phase = 7;
            }
            _ => {}
        }
        Ok(())
    }
}
