//! UI-independent timeline foundation. Imported seconds retain their original
//! f64 value: frame alignment is an edit policy, never an import conversion.
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_TIMELINE_SECONDS: f64 = 922_337_203_685.477_5;
pub const MAX_CLIPS: usize = 10_000;
const POSITION_TOLERANCE: f64 = 0.00001;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SourceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ClipId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn fps(self) -> f64 {
        1.0 / self.frame_seconds()
    }
    pub fn rational(self) -> String {
        format!("{}/{}", self.numerator, self.denominator)
    }
    pub fn parse(value: &str) -> Result<Self, EditError> {
        let (a, b) = value.split_once('/').unwrap_or((value, "1"));
        Self::new(
            a.parse().map_err(|_| EditError("Invalid frame rate"))?,
            b.parse().map_err(|_| EditError("Invalid frame rate"))?,
        )
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
    Replace {
        clips: Vec<Clip>,
    },
    Move {
        clip: ClipId,
        position: f64,
        rate: FrameRate,
        snap: f64,
    },
    Trim {
        clip: ClipId,
        start: f64,
        end: f64,
        source_duration: f64,
        rate: FrameRate,
    },
    Duplicate {
        clip: ClipId,
        rate: FrameRate,
    },
    Swap {
        clip: ClipId,
        direction: i32,
    },
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
pub struct Editor<M: Clone = ()> {
    timeline: Timeline,
    metadata: M,
    undo: Vec<(Timeline, M)>,
    redo: Vec<(Timeline, M)>,
    next_id: u64,
}

impl Editor<()> {
    pub fn new(timeline: Timeline) -> Result<Self, EditError> {
        Self::with_metadata(timeline, ())
    }
}
impl<M: Clone> Editor<M> {
    pub fn with_metadata(timeline: Timeline, metadata: M) -> Result<Self, EditError> {
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
            metadata,
            undo: Vec::new(),
            redo: Vec::new(),
            next_id,
        })
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }
    pub fn metadata(&self) -> &M {
        &self.metadata
    }
    pub fn metadata_mut(&mut self) -> &mut M {
        &mut self.metadata
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn apply(&mut self, command: Command) -> Result<(), EditError> {
        self.apply_with_metadata(command, self.metadata.clone())
    }
    pub fn apply_with_metadata(&mut self, command: Command, metadata: M) -> Result<(), EditError> {
        self.apply_checked(command, metadata, |_, _| Ok(()))
    }
    pub fn apply_checked(
        &mut self,
        command: Command,
        mut metadata: M,
        validate: impl FnOnce(&Timeline, &mut M) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
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
            Command::Replace { clips: replacement } => {
                clips = replacement;
            }
            Command::Move {
                clip,
                position,
                rate,
                snap,
            } => {
                if !position.is_finite() || !snap.is_finite() || snap < 0.0 {
                    return Err(EditError("Invalid clip position"));
                }
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                clips[index].timeline_start =
                    available_position(&clips, clip, position, rate, snap)?;
            }
            Command::Trim {
                clip,
                start,
                end,
                source_duration,
                rate,
            } => {
                if !start.is_finite()
                    || !end.is_finite()
                    || !source_duration.is_finite()
                    || start < 0.0
                    || end > source_duration + 0.001
                    || end - start < rate.frame_seconds() - 0.0001
                {
                    return Err(EditError("Keep at least one frame within the recording"));
                }
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                let original = clips[index].clone();
                let last = joined_end(&clips, index);
                let previous = if index == 0 {
                    0.0
                } else {
                    clips[index - 1].timeline_end()
                };
                let next = clips
                    .get(last + 1)
                    .map_or(f64::INFINITY, |c| c.timeline_start);
                let joined = last > index
                    || index > 0 && (previous - original.timeline_start).abs() < POSITION_TOLERANCE;
                let frame = rate.frame_seconds();
                if joined {
                    let maximum = original.duration()
                        + ((next - clips[last].timeline_end()) / frame + 1e-8).floor() * frame;
                    if end - start > maximum + POSITION_TOLERANCE {
                        return Err(EditError("Trim overlaps the next group"));
                    }
                } else {
                    let minimum = (original.source_in
                        + ((previous - original.timeline_start) / frame - 1e-8).ceil() * frame)
                        .max(0.0);
                    let maximum = original.source_out
                        + ((next - original.timeline_end()) / frame + 1e-8).floor() * frame;
                    if start < minimum - POSITION_TOLERANCE || end > maximum + POSITION_TOLERANCE {
                        return Err(EditError("Trim overlaps another clip"));
                    }
                }
                let position = if joined {
                    original.timeline_start
                } else {
                    (original.timeline_start + start - original.source_in).max(previous)
                };
                let shift = position + end.min(source_duration) - start - original.timeline_end();
                clips[index].timeline_start = position;
                clips[index].source_in = start;
                clips[index].source_out = end.min(source_duration);
                for following in clips.iter_mut().take(last + 1).skip(index + 1) {
                    following.timeline_start += shift;
                }
            }
            Command::Duplicate { clip, rate } => {
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                let mut duplicate = clips[index].clone();
                duplicate.id = allocate_id()?;
                let last = joined_end(&clips, index);
                let next = clips
                    .get(last + 1)
                    .map_or(f64::INFINITY, |c| c.timeline_start);
                if next - clips[last].timeline_end() >= duplicate.duration() - POSITION_TOLERANCE {
                    duplicate.timeline_start = clips[index].timeline_end();
                    for following in clips.iter_mut().take(last + 1).skip(index + 1) {
                        following.timeline_start += duplicate.duration();
                    }
                } else {
                    let temporary = Clip {
                        timeline_start: self.timeline.duration(),
                        ..duplicate.clone()
                    };
                    let mut ranges = clips.clone();
                    ranges.push(temporary);
                    duplicate.timeline_start = available_position(
                        &ranges,
                        duplicate.id,
                        clips[index].timeline_end(),
                        rate,
                        0.0,
                    )?;
                }
                clips.push(duplicate);
            }
            Command::Swap { clip, direction } => {
                if direction == 0 {
                    return Ok(());
                }
                let index = clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(EditError("Clip not found"))?;
                let target = index as i64 + i64::from(direction.signum());
                if target < 0 || target >= clips.len() as i64 {
                    return Ok(());
                }
                let left = index.min(target as usize);
                let right = index.max(target as usize);
                let position = clips[left].timeline_start;
                let gap = clips[right].timeline_start - clips[left].timeline_end();
                clips[left].timeline_start = position + clips[right].duration() + gap;
                clips[right].timeline_start = position;
            }
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
        clips.sort_by(|a, b| a.timeline_start.total_cmp(&b.timeline_start));
        let updated = Timeline::new(clips)?;
        validate(&updated, &mut metadata)?;
        let next_id = next_id.max(
            updated
                .clips
                .iter()
                .map(|c| c.id.0)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(EditError("Clip identity exhausted"))?,
        );
        if self.undo.len() == 100 {
            self.undo.remove(0);
        }
        self.undo.push((
            std::mem::replace(&mut self.timeline, updated),
            std::mem::replace(&mut self.metadata, metadata),
        ));
        self.redo.clear();
        self.next_id = next_id;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push((
            std::mem::replace(&mut self.timeline, previous.0),
            std::mem::replace(&mut self.metadata, previous.1),
        ));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push((
            std::mem::replace(&mut self.timeline, next.0),
            std::mem::replace(&mut self.metadata, next.1),
        ));
        true
    }
}

fn joined_end(clips: &[Clip], index: usize) -> usize {
    let mut last = index;
    let mut end = clips[index].timeline_end();
    while last + 1 < clips.len()
        && (clips[last + 1].timeline_start - end).abs() < POSITION_TOLERANCE
    {
        last += 1;
        end += clips[last].duration();
    }
    last
}

pub fn available_position(
    clips: &[Clip],
    moving: ClipId,
    requested: f64,
    rate: FrameRate,
    snap: f64,
) -> Result<f64, EditError> {
    let clip = clips
        .iter()
        .find(|c| c.id == moving)
        .ok_or(EditError("Clip not found"))?;
    let mut ranges: Vec<_> = clips
        .iter()
        .filter(|c| c.id != moving)
        .map(|c| (c.timeline_start, c.timeline_end()))
        .collect();
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut boundaries = vec![0.0];
    for range in &ranges {
        boundaries.extend([range.1, range.0 - clip.duration()]);
    }
    boundaries.retain(|p| {
        *p >= 0.0
            && (*p - requested).abs() <= snap
            && ranges
                .iter()
                .all(|c| *p + clip.duration() <= c.0 + 1e-8 || *p >= c.1 - 1e-8)
    });
    if let Some(value) = boundaries
        .into_iter()
        .min_by(|a, b| (a - requested).abs().total_cmp(&(b - requested).abs()))
    {
        return Ok(value);
    }
    let fps = rate.fps();
    let requested = (requested * fps).round_ties_even().max(0.0) / fps;
    let mut end: f64 = 0.0;
    let mut candidates = Vec::new();
    for range in ranges {
        let first = ((end - 1e-8) * fps).ceil() / fps;
        let last = ((range.0 - clip.duration() + 1e-8) * fps).floor() / fps;
        if last >= first {
            candidates.push(requested.clamp(first, last));
        }
        end = end.max(range.1);
    }
    candidates.push(requested.max(((end - 1e-8) * fps).ceil() / fps));
    candidates
        .into_iter()
        .min_by(|a, b| (a - requested).abs().total_cmp(&(b - requested).abs()))
        .ok_or(EditError("No valid position"))
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
    #[test]
    fn trim_connected_group_preserves_anchor_and_separate_gap() {
        let mut editor = Editor::new(
            Timeline::new(vec![section(1, 1.0), section(2, 2.0), section(3, 5.0)]).unwrap(),
        )
        .unwrap();
        editor
            .apply(Command::Trim {
                clip: ClipId(1),
                start: 1.4,
                end: 2.15,
                source_duration: 8.0,
                rate: FrameRate::new(60, 1).unwrap(),
            })
            .unwrap();
        let clips = editor.timeline().clips();
        assert_eq!(clips[0].timeline_start, 1.0);
        assert!((clips[1].timeline_start - 1.75).abs() < 1e-10);
        assert_eq!(clips[2].timeline_start, 5.0);
    }
    #[test]
    fn free_trim_anchors_source_time_and_failed_overlap_is_atomic() {
        let initial = Timeline::new(vec![section(1, 2.0), section(2, 5.0)]).unwrap();
        let mut editor = Editor::new(initial.clone()).unwrap();
        editor
            .apply(Command::Trim {
                clip: ClipId(1),
                start: 1.4,
                end: 2.15,
                source_duration: 8.0,
                rate: FrameRate::new(60, 1).unwrap(),
            })
            .unwrap();
        assert!((editor.timeline().clips()[0].timeline_start - 2.25).abs() < 1e-10);
        assert!(editor.undo());
        assert!(
            editor
                .apply(Command::Trim {
                    clip: ClipId(1),
                    start: 0.0,
                    end: 7.0,
                    source_duration: 8.0,
                    rate: FrameRate::new(60, 1).unwrap()
                })
                .is_err()
        );
        assert_eq!(editor.timeline(), &initial);
        assert!(editor.redo());
    }
    #[test]
    fn duplicate_swap_and_metadata_share_one_bounded_history() {
        let mut editor = Editor::with_metadata(
            Timeline::new(vec![section(1, 0.0), section(2, 1.0), section(3, 5.0)]).unwrap(),
            String::from("old audio"),
        )
        .unwrap();
        editor
            .apply_with_metadata(
                Command::Duplicate {
                    clip: ClipId(1),
                    rate: FrameRate::new(30, 1).unwrap(),
                },
                "new audio".into(),
            )
            .unwrap();
        assert_eq!(editor.timeline().clips()[1].timeline_start, 1.0);
        assert_eq!(editor.timeline().clips()[2].timeline_start, 2.0);
        assert_eq!(editor.timeline().clips()[3].timeline_start, 5.0);
        assert!(editor.undo());
        assert_eq!(editor.metadata(), "old audio");
        assert!(editor.redo());
        assert_eq!(editor.metadata(), "new audio");
        editor
            .apply(Command::Swap {
                clip: ClipId(3),
                direction: -1,
            })
            .unwrap();
        assert_eq!(editor.timeline().clips()[2].id, ClipId(3));
        assert!(editor.undo());
        for _ in 0..120 {
            editor
                .apply(Command::Replace {
                    clips: editor.timeline().clips().to_vec(),
                })
                .unwrap();
        }
        let mut count = 0;
        while editor.undo() {
            count += 1;
        }
        assert_eq!(count, 100);
    }
}
