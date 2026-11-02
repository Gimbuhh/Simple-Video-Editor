use super::*;

impl EditorApp {
    pub(super) fn verify_clip_clicks(
        &mut self,
        test: &mut crate::verification::Verification,
    ) -> media_engine::Result<bool> {
        if test.phase != 34 || test.click_case >= 10 {
            return Ok(false);
        }
        let playing = test.click_case >= 5;
        let point = test.click_case % 5;
        match test.click_phase {
            0 => {
                self.playing = false;
                self.seek(0.2);
                test.click_phase = 1;
            }
            1 if self
                .playback
                .as_ref()
                .is_some_and(|p| p.status().ready && p.status().generation == self.generation) =>
            {
                self.playing = playing;
                let rect = self.controls["first-clip"];
                let x = match point {
                    0 | 2 => rect.left() + 2.0,
                    1 | 3 => rect.right() - 2.0,
                    _ => rect.center().x,
                };
                let y = if point == 2 || point == 3 {
                    rect.bottom() - 18.0
                } else {
                    rect.top() + 24.0
                };
                test.anchor = Some(Pos2::new(x, y));
                test.pointer(test.anchor.unwrap(), Some(true));
                test.click_phase = 2;
            }
            2 => {
                test.pointer(test.anchor.unwrap(), Some(false));
                test.click_phase = 3;
            }
            3 if self.settling.is_none()
                && self.playback.as_ref().is_some_and(|p| {
                    p.status().ready && p.status().generation == self.generation
                }) =>
            {
                if self.playing != playing
                    || self.editor.timeline().clips() != test.expected.as_ref().unwrap()
                {
                    return Err(
                        "Clip click changed playback intent or saved trim boundaries".into(),
                    );
                }
                let clip = &self.editor.timeline().clips()[0];
                let source = self
                    .editor
                    .metadata()
                    .sources
                    .iter()
                    .find(|s| s.id == clip.source)
                    .unwrap();
                let position = self.playback.as_ref().unwrap().status().seek_position;
                let expected = match point {
                    0 | 2 => clip.source_in,
                    1 | 3 => clip.source_out - source.rate.frame_seconds(),
                    _ => clip.source_in + clip.duration() / 2.0,
                };
                if (position - expected).abs() > source.rate.frame_seconds() * 1.1
                    || position >= clip.source_out
                {
                    return Err(format!(
                        "Clip click {} selected source frame {position}, expected {expected}",
                        test.click_case
                    )
                    .into());
                }
                self.playing = false;
                self.seek(0.25);
                test.click_case += 1;
                test.click_phase = 0;
                if test.click_case == 10 {
                    test.checks.push("Video/linked-audio clicks preserve playing and paused intent, and resize hit bands select first/last frames without editing or selecting the next clip".into());
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
