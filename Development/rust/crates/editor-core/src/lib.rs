//! UI-independent timeline foundation. Imported seconds retain their original
//! f64 value: frame alignment is an edit policy, never an import conversion.
use std::fmt;

pub const MAX_TIMELINE_SECONDS: f64 = 922_337_203_685.477_5;
pub const MAX_CLIPS: usize = 10_000;
const POSITION_TOLERANCE: f64 = 0.00001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: ClipId,
    pub source: SourceId,
    pub source_in: f64,
    pub source_out: f64,
    pub timeline_start: f64,
}

impl Clip {
    pub fn duration(&self) -> f64 {
        self.source_out - self.source_in
    }

    pub fn timeline_end(&self) -> f64 {
        self.timeline_start + self.duration()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

impl FrameRate {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, EditError> {
        if numerator == 0 || denominator == 0 {
            return Err(EditError("Frame rate must be positive"));
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub fn frame_seconds(self) -> f64 {
        f64::from(self.denominator) / f64::from(self.numerator)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Timeline {
    clips: Vec<Clip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError(pub &'static str);

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for EditError {}

impl Timeline {
    pub fn new(clips: Vec<Clip>) -> Result<Self, EditError> {
        if clips.len() > MAX_CLIPS {
            return Err(EditError("Too many clips"));
        }
        let mut end = 0.0;
        let mut ids = std::collections::HashSet::new();
        for clip in &clips {
            if !ids.insert(clip.id.0) {
                return Err(EditError("Duplicate clip identity"));
            }
            if !clip.source_in.is_finite()
                || !clip.source_out.is_finite()
                || clip.source_in < 0.0
                || clip.source_out <= clip.source_in
            {
                return Err(EditError("Invalid source trim range"));
            }
            if !clip.timeline_start.is_finite()
                || clip.timeline_start < 0.0
                || clip.timeline_start < end - POSITION_TOLERANCE
            {
                return Err(EditError("Overlapping or invalid timeline positions"));
            }
            end = clip.timeline_end();
            if !end.is_finite() || end > MAX_TIMELINE_SECONDS {
                return Err(EditError("Timeline exceeds the supported time range"));
            }
        }
        Ok(Self { clips })
    }

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    pub fn duration(&self) -> f64 {
        self.clips
            .iter()
            .map(Clip::timeline_end)
            .fold(0.0, f64::max)
    }

    /// End points are exclusive. A gap or the final endpoint maps to no source.
    pub fn source_at(&self, timeline_time: f64) -> Option<(ClipId, SourceId, f64)> {
        if !timeline_time.is_finite() {
            return None;
        }
        self.clips
            .iter()
            .find(|c| timeline_time >= c.timeline_start && timeline_time < c.timeline_end())
            .map(|c| {
                (
                    c.id,
                    c.source,
                    c.source_in + (timeline_time - c.timeline_start),
                )
            })
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    Split {
        clip: ClipId,
        source_time: f64,
        rate: FrameRate,
    },
    Delete {
        clip: ClipId,
    },
    /// One command for the entire library batch, independent of UI filtering.
    Append {
        sections: Vec<Clip>,
    },
}

/// Owns timeline edits and bounded undo history. Video and audio refer to the
/// same section; no independently mutable audio positions exist.
pub struct Editor {
    timeline: Timeline,
    undo: Vec<Timeline>,
    redo: Vec<Timeline>,
    next_id: u64,
}

impl Editor {
    pub fn new(timeline: Timeline) -> Result<Self, EditError> {
        let next_id = timeline
            .clips
            .iter()
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EditError("Clip identity space exhausted"))?;
        Ok(Self {
            timeline,
            undo: Vec::new(),
            redo: Vec::new(),
            next_id,
        })
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn apply(&mut self, command: Command) -> Result<(), EditError> {
        let mut clips = self.timeline.clips.clone();
        let mut next_id = self.next_id;
        let mut allocate_id = || {
            let id = ClipId(next_id);
            next_id = next_id
                .checked_add(1)
                .ok_or(EditError("Clip identity space exhausted"))?;
            Ok(id)
        };
        match command {
            Command::Split {
                clip,
                source_time,
                rate,
            } => {
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                let original = &clips[index];
                if !source_time.is_finite()
                    || source_time <= original.source_in
                    || source_time >= original.source_out
                    || source_time - original.source_in < rate.frame_seconds() - 0.0001
                    || original.source_out - source_time < rate.frame_seconds() - 0.0001
                {
                    return Err(EditError("Keep at least one frame on each side of a split"));
                }
                let following = Clip {
                    id: allocate_id()?,
                    source: original.source,
                    source_in: source_time,
                    source_out: original.source_out,
                    timeline_start: original.timeline_start + source_time - original.source_in,
                };
                clips[index].source_out = source_time;
                clips.insert(index + 1, following);
            }
            Command::Delete { clip } => {
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                let removed_duration = clips[index].duration();
                let mut end = clips[index].timeline_end();
                // Close only the connected run. Existing gaps separate groups
                // and must never pull later clips earlier on the timeline.
                for following in clips.iter_mut().skip(index + 1) {
                    if (following.timeline_start - end).abs() >= POSITION_TOLERANCE {
                        break;
                    }
                    end += following.duration();
                    following.timeline_start -= removed_duration;
                }
                clips.remove(index);
            }
            Command::Append { sections } => {
                if sections.is_empty() {
                    return Ok(());
                }
                if sections.len() > MAX_CLIPS - clips.len() {
                    return Err(EditError("Too many clips"));
                }
                let mut end = self.timeline.duration();
                for section in sections {
                    let added = Clip {
                        id: allocate_id()?,
                        timeline_start: end,
                        ..section
                    };
                    end = added.timeline_end();
                    clips.push(added);
                }
            }
        }
        let updated = Timeline::new(clips)?;
        if self.undo.len() == 100 {
            self.undo.remove(0);
        }
        self.undo
            .push(std::mem::replace(&mut self.timeline, updated));
        self.redo.clear();
        self.next_id = next_id;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.timeline, previous));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.timeline, next));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn section(id: u64, position: f64) -> Clip {
        Clip {
            id: ClipId(id),
            source: SourceId(1),
            source_in: 1.15,
            source_out: 2.15,
            timeline_start: position,
        }
    }
    #[test]
    fn mapping_preserves_gaps_and_exclusive_endpoints() {
        let timeline = Timeline::new(vec![section(1, 1.0), section(2, 4.0)]).unwrap();
        assert_eq!(timeline.duration(), 5.0);
        assert_eq!(timeline.source_at(0.0), None);
        assert_eq!(timeline.source_at(2.0), None);
        assert_eq!(timeline.source_at(5.0), None);
        assert_eq!(
            timeline.source_at(4.0),
            Some((ClipId(2), SourceId(1), 1.15))
        );
    }
    #[test]
    fn split_is_one_undoable_linked_edit_without_moving_other_clips() {
        let initial = Timeline::new(vec![section(1, 1.0), section(2, 4.0)]).unwrap();
        let mut editor = Editor::new(initial.clone()).unwrap();
        editor
            .apply(Command::Split {
                clip: ClipId(1),
                source_time: 1.5,
                rate: FrameRate::new(60_000, 1001).unwrap(),
            })
            .unwrap();
        let result = editor.timeline.clone();
        assert_eq!(result.clips[0].source_out, result.clips[1].source_in);
        assert_ne!(result.clips[0].id, result.clips[1].id);
        assert_eq!(result.clips[0].source, result.clips[1].source);
        assert_eq!(result.clips[2], initial.clips[1]);
        assert_eq!(result.duration(), initial.duration());
        assert!(editor.undo());
        assert_eq!(editor.timeline(), &initial);
        assert!(editor.redo());
        assert_eq!(editor.timeline(), &result);
    }
    #[test]
    fn failed_command_preserves_state_and_redo() {
        let mut editor = Editor::new(Timeline::new(vec![section(1, 0.0)]).unwrap()).unwrap();
        editor.apply(Command::Delete { clip: ClipId(1) }).unwrap();
        assert!(editor.undo());
        let original = editor.timeline.clone();
        for position in [1.15, 2.15, f64::NAN, f64::INFINITY, 1.151] {
            assert!(
                editor
                    .apply(Command::Split {
                        clip: ClipId(1),
                        source_time: position,
                        rate: FrameRate::new(60, 1).unwrap()
                    })
                    .is_err()
            );
            assert_eq!(editor.timeline(), &original);
        }
        assert!(editor.redo());
        assert!(editor.timeline.clips.is_empty());
    }
    #[test]
    fn delete_leaves_gap_and_append_batch_is_one_undo_step() {
        let mut editor =
            Editor::new(Timeline::new(vec![section(1, 1.0), section(2, 4.0)]).unwrap()).unwrap();
        editor.apply(Command::Delete { clip: ClipId(1) }).unwrap();
        assert_eq!(editor.timeline.clips[0].timeline_start, 4.0);
        let before = editor.timeline.clone();
        editor
            .apply(Command::Append {
                sections: vec![section(1, 0.0), section(1, 0.0)],
            })
            .unwrap();
        assert_eq!(editor.timeline.duration(), 7.0);
        assert_ne!(editor.timeline.clips[1].id, editor.timeline.clips[2].id);
        assert!(editor.undo());
        assert_eq!(editor.timeline, before);
    }
    #[test]
    fn delete_closes_only_connected_group_and_keeps_existing_gap() {
        let initial = Timeline::new(vec![
            section(1, 1.0),
            section(2, 2.0),
            section(3, 3.0),
            section(4, 7.0),
        ])
        .unwrap();
        let mut editor = Editor::new(initial.clone()).unwrap();
        editor.apply(Command::Delete { clip: ClipId(1) }).unwrap();
        assert_eq!(editor.timeline.clips[0].timeline_start, 1.0);
        assert_eq!(editor.timeline.clips[1].timeline_start, 2.0);
        assert_eq!(editor.timeline.clips[2].timeline_start, 7.0);
        assert!(editor.undo());
        assert_eq!(editor.timeline, initial);
    }

    #[test]
    fn invalid_timeline_cannot_enter_editor() {
        for start in [-1.0, f64::NAN, f64::INFINITY, 1e20] {
            assert!(Timeline::new(vec![section(1, start)]).is_err());
        }
        assert!(Timeline::new(vec![section(1, 1.0), section(2, 1.5)]).is_err());
        assert!(Timeline::new(vec![section(1, 1.0), section(1, 3.0)]).is_err());
        assert!(FrameRate::new(0, 1).is_err());
        assert!(FrameRate::new(1, 0).is_err());
    }
}
