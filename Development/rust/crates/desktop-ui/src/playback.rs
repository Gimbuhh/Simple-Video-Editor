use media_engine::{Tools, direct_preview, preview_mpv_filter};
use native_mpv::{Mode, Player};
use project_store::{ClipSettings, MediaSource, OutputSettings};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Request {
    pub generation: u64,
    pub presentation_key: String,
    pub source: Option<MediaSource>,
    pub adjustment: ClipSettings,
    pub output: OutputSettings,
    pub position: f64,
    pub start: f64,
    pub end: f64,
    pub playing: bool,
    pub scrubbing: bool,
    pub volume: f64,
    pub hdr_preview: bool,
    pub steps: i32,
}
impl Request {
    // Finish a frame during a continuing gesture, then take the newest target.
    // Release, a different presentation, and frame-step commands interrupt immediately.
    fn interrupts(&self, active: &Self) -> bool {
        self.generation != active.generation
            && (!self.scrubbing
                || self.presentation_key != active.presentation_key
                || active.steps != 0)
    }
}
#[derive(Clone, Default)]
pub struct Status {
    pub generation: u64,
    pub presentation_key: Option<String>,
    pub presentation_serial: u64,
    pub ready: bool,
    pub position: f64,
    pub ended: bool,
    // Actual frame timestamp before an optional step; keep it distinct from
    // the requested seek and the next frame's exclusive trim boundary.
    pub seek_position: f64,
    pub paused: bool,
    pub software: bool,
    pub frame: Option<Arc<native_mpv::Frame>>,
    pub gamma: Option<String>,
    pub decoder: Option<String>,
    // Only present once a real audio output has negotiated its sample format.
    pub audio_output: Option<String>,
    pub audio_stream: Option<u32>,
    pub audio_warning: Option<String>,
    pub audio_recovery_attempts: u64,
    pub error: Option<String>,
    pub display: Option<platform_windows::DisplayColor>,
}
#[derive(Default)]
pub(crate) struct AudioRecovery {
    missing_since: Option<Instant>,
    last_attempt: Option<Instant>,
    attempts: u64,
}
impl AudioRecovery {
    pub(crate) fn update(&mut self, needed: bool, available: bool, now: Instant) -> bool {
        if !needed || available {
            self.missing_since = None;
            self.last_attempt = None;
            return false;
        }
        let missing = *self.missing_since.get_or_insert(now);
        if now.duration_since(missing) < Duration::from_secs(1)
            || self
                .last_attempt
                .is_some_and(|last| now.duration_since(last) < Duration::from_secs(2))
        {
            return false;
        }
        self.last_attempt = Some(now);
        self.attempts += 1;
        true
    }
    pub(crate) fn warning(&self) -> Option<String> {
        self.last_attempt.map(|_| {
            "Audio output unavailable. Check the Windows output device; preview will retry.".into()
        })
    }
}

type SubmissionKey = (u64, bool, u64, i32, bool);

pub struct Playback {
    request: Arc<Mutex<Option<Request>>>,
    status: Arc<Mutex<Status>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    submitted: Mutex<Option<SubmissionKey>>,
}
impl Playback {
    pub fn new(tools: Tools, window: isize, forced: bool, context: eframe::egui::Context) -> Self {
        let request = Arc::new(Mutex::new(None::<Request>));
        let status = Arc::new(Mutex::new(Status::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, state, stopped) = (request.clone(), status.clone(), stop.clone());
        let fail_native = std::env::args_os()
            .nth(1)
            .is_some_and(|a| a == "--verify-desktop")
            && std::env::args_os().any(|a| a == "--fail-native-preview");
        let fail_audio = std::env::args_os()
            .nth(1)
            .is_some_and(|a| a == "--verify-desktop")
            && std::env::args_os().any(|a| a == "--verify-audio-recovery");
        let worker = std::thread::spawn(move || {
            for attempt in 0..if forced { 1 } else { 2 } {
                let software = forced || attempt > 0;
                let mut retry: Option<Request> = None;
                let result = (|| -> media_engine::Result<()> {
                    let player = Player::open_embedded(
                        &tools.mpv,
                        if software {
                            Mode::SoftwareSdr
                        } else {
                            Mode::NativeAuto
                        },
                        window,
                    )?;
                    let mut current: Option<Request> = None;
                    let mut audio_track = None;
                    let mut audio_recovery = AudioRecovery::default();
                    let mut audio_fault_injected = false;
                    let mut display = None;
                    let mut display_tick = Instant::now() - Duration::from_secs(2);
                    while !stopped.load(Ordering::Acquire) {
                        let previous = state.lock().unwrap().clone();
                        let mut changed = false;
                        if display_tick.elapsed() >= Duration::from_secs(1) {
                            let updated = platform_windows::display_color(window).ok();
                            changed = display != updated;
                            display = updated;
                            state.lock().unwrap().display = display.clone();
                            display_tick = Instant::now();
                        }
                        let next = requests.lock().unwrap().take();
                        if let Some(next) = next {
                            retry = Some(next.clone());
                            if !software && fail_native {
                                return Err(
                                    "Injected native preview failure after dequeuing request"
                                        .into(),
                                );
                            }
                            changed = true;
                            let replace = current
                                .as_ref()
                                .is_none_or(|c| c.generation != next.generation);
                            if replace {
                                let reuse = current.as_ref().is_some_and(|c| {
                                    c.source == next.source
                                        && c.source.is_some()
                                        && c.adjustment == next.adjustment
                                        && c.output == next.output
                                        && c.hdr_preview == next.hdr_preview
                                        && c.start == next.start
                                        && c.end == next.end
                                });
                                *state.lock().unwrap() = Status {
                                    generation: next.generation,
                                    presentation_serial: previous.presentation_serial,
                                    frame: if reuse { previous.frame.clone() } else { None },
                                    gamma: if reuse { previous.gamma.clone() } else { None },
                                    decoder: if reuse {
                                        previous.decoder.clone()
                                    } else {
                                        None
                                    },
                                    presentation_key: if reuse {
                                        previous.presentation_key.clone()
                                    } else {
                                        None
                                    },
                                    software,
                                    display: display.clone(),
                                    ..Default::default()
                                };
                                player.set_paused(true)?;
                                if !reuse {
                                    current = None;
                                    player.command(&["stop"])?;
                                    audio_track = None;
                                    for _ in 0..1024 {
                                        if player.poll_event(0.0)? == 0 {
                                            break;
                                        }
                                    }
                                    if let Some(source) = &next.source {
                                        let path = media_path::RecordingPath::authorize(
                                            Path::new(&source.path),
                                        )?;
                                        // Use the shared color/framing graph when processing is needed;
                                        // ordinary SDR can stay on mpv's direct presentation path.
                                        let mut output = next.output.clone();
                                        if software || !next.hdr_preview {
                                            output.profile = project_store::ColorProfile::Sdr;
                                        }
                                        player.set(
                                            "hwdec",
                                            if !software
                                                && direct_preview(source, &next.adjustment, &output)
                                            {
                                                "auto"
                                            } else if !software && source.codec == "av1" {
                                                "auto-copy"
                                            } else {
                                                "no"
                                            },
                                        )?;
                                        if software {
                                            let scale = (1280.0 / f64::from(output.width))
                                                .min(720.0 / f64::from(output.height))
                                                .min(1.0);
                                            player.resize_software(
                                                (f64::from(output.width) * scale).round().max(1.0)
                                                    as usize,
                                                (f64::from(output.height) * scale).round().max(1.0)
                                                    as usize,
                                            )?;
                                        }
                                        if !software {
                                            player.set(
                                                "target-trc",
                                                if next.hdr_preview { "auto" } else { "srgb" },
                                            )?;
                                            player.set(
                                                "target-prim",
                                                if next.hdr_preview { "auto" } else { "bt.709" },
                                            )?;
                                            player.set(
                                                "target-peak",
                                                if next.hdr_preview { "auto" } else { "203" },
                                            )?;
                                        }
                                        player.set(
                                            "vf",
                                            &preview_mpv_filter(source, &next.adjustment, &output),
                                        )?;
                                        player.set("start", &next.position.to_string())?;
                                        player.set("end", &next.end.to_string())?;
                                        player.command(&[
                                            "loadfile",
                                            path.as_path()
                                                .to_str()
                                                .ok_or("Invalid recording path")?,
                                            "replace",
                                        ])?;
                                        let deadline = Instant::now() + Duration::from_secs(12);
                                        let mut loaded = false;
                                        while Instant::now() < deadline
                                            && !stopped.load(Ordering::Acquire)
                                        {
                                            if requests
                                                .lock()
                                                .unwrap()
                                                .as_ref()
                                                .is_some_and(|r| r.interrupts(&next))
                                            {
                                                break;
                                            }
                                            if player.poll_event(0.01)? == 8 {
                                                loaded = true;
                                                break;
                                            }
                                        }
                                        if !loaded {
                                            if stopped.load(Ordering::Acquire)
                                                || requests.lock().unwrap().is_some()
                                            {
                                                continue;
                                            }
                                            return Err(
                                                "Timed out preparing the recording preview".into(),
                                            );
                                        }
                                        let stream = next
                                            .adjustment
                                            .stream
                                            .or_else(|| source.audio.first().map(|a| a.index));
                                        // ffprobe's stream index and mpv's track id are different namespaces.
                                        let tracks = player
                                            .property("track-list/count")
                                            .and_then(|s| s.parse::<usize>().ok())
                                            .unwrap_or(0);
                                        let mut aid = None;
                                        let mut vid = None;
                                        for i in 0..tracks {
                                            if player
                                                .property(&format!("track-list/{i}/type"))
                                                .as_deref()
                                                == Some("audio")
                                                && player
                                                    .property(&format!("track-list/{i}/ff-index"))
                                                    .and_then(|s| s.parse::<u32>().ok())
                                                    == stream
                                            {
                                                aid =
                                                    player.property(&format!("track-list/{i}/id"));
                                            }
                                            if player
                                                .property(&format!("track-list/{i}/type"))
                                                .as_deref()
                                                == Some("video")
                                                && player
                                                    .property(&format!("track-list/{i}/ff-index"))
                                                    .and_then(|s| s.parse::<u32>().ok())
                                                    == Some(source.video_stream)
                                            {
                                                vid =
                                                    player.property(&format!("track-list/{i}/id"));
                                            }
                                        }
                                        let vid = vid.ok_or(
                                            "The probed video stream is unavailable in the preview",
                                        )?;
                                        if player.property("vid").as_deref() != Some(&vid) {
                                            player.set("vid", &vid)?;
                                        }
                                        if stream.is_some() && aid.is_none() {
                                            return Err("The selected audio stream is unavailable in the preview".into());
                                        }
                                        player.set("aid", aid.as_deref().unwrap_or("no"))?;
                                        audio_track = aid;
                                        let duration = next.end - next.start;
                                        let mut audio = format!(
                                            "lavfi=[volume={}dB",
                                            if next.adjustment.muted
                                                || next.adjustment.audio_separated
                                            {
                                                -120.0
                                            } else {
                                                next.adjustment.gain_db
                                            }
                                        );
                                        if next.adjustment.fade_in > 0.0 {
                                            audio.push_str(&format!(
                                                ",afade=t=in:st={}:d={}",
                                                next.start, next.adjustment.fade_in
                                            ));
                                        }
                                        if next.adjustment.fade_out > 0.0 {
                                            audio.push_str(&format!(
                                                ",afade=t=out:st={}:d={}",
                                                next.end - next.adjustment.fade_out.min(duration),
                                                next.adjustment.fade_out.min(duration)
                                            ));
                                        }
                                        audio.push(']');
                                        player.set("af", &audio)?;
                                    }
                                }
                                if next.source.is_some()
                                    && !player.seek_interruptible(next.position, || {
                                        stopped.load(Ordering::Acquire)
                                            || requests
                                                .lock()
                                                .unwrap()
                                                .as_ref()
                                                .is_some_and(|r| r.interrupts(&next))
                                    })?
                                {
                                    continue;
                                }
                                let mut position = player
                                    .property("time-pos")
                                    .and_then(|p| p.parse::<f64>().ok())
                                    .filter(|p| p.is_finite())
                                    .unwrap_or(next.position);
                                state.lock().unwrap().seek_position = position;
                                if next.steps != 0 && next.source.is_some() {
                                    player.wait_for(5.0, |p| {
                                        p.property("seeking").as_deref() == Some("no")
                                            && p.property("time-pos").is_some()
                                    })?;
                                    let Some(stepped) = player.step_frames(next.steps, || {
                                        stopped.load(Ordering::Acquire)
                                            || requests
                                                .lock()
                                                .unwrap()
                                                .as_ref()
                                                .is_some_and(|r| r.interrupts(&next))
                                    })?
                                    else {
                                        continue;
                                    };
                                    position = stepped;
                                }
                                state.lock().unwrap().position = position;
                                if software && next.source.is_some() {
                                    player.wait_for(5.0, |p| {
                                        p.property("seeking").as_deref() == Some("no")
                                            && p.property("video-out-params/w").is_some()
                                    })?;
                                    let serial = player.refresh_frame();
                                    let deadline = Instant::now() + Duration::from_secs(5);
                                    loop {
                                        if stopped.load(Ordering::Acquire)
                                            || requests
                                                .lock()
                                                .unwrap()
                                                .as_ref()
                                                .is_some_and(|r| r.interrupts(&next))
                                        {
                                            break;
                                        }
                                        player.poll_event(0.005)?;
                                        if let Some(frame) = player.take_frame()
                                            && frame.serial == serial
                                        {
                                            state.lock().unwrap().frame = Some(Arc::new(frame));
                                            break;
                                        }
                                        if Instant::now() > deadline {
                                            return Err(
                                                "Timed out presenting the requested preview frame"
                                                    .into(),
                                            );
                                        }
                                    }
                                    if stopped.load(Ordering::Acquire)
                                        || requests
                                            .lock()
                                            .unwrap()
                                            .as_ref()
                                            .is_some_and(|r| r.interrupts(&next))
                                    {
                                        continue;
                                    }
                                }
                                state.lock().unwrap().ended =
                                    player.property("eof-reached").as_deref() == Some("yes");
                                let mut updated = state.lock().unwrap();
                                updated.ready = true;
                                updated.presentation_serial =
                                    updated.presentation_serial.wrapping_add(1);
                                updated.presentation_key =
                                    next.source.as_ref().map(|_| next.presentation_key.clone());
                            }
                            player.set("volume", &next.volume.to_string())?;
                            player.set_paused(!next.playing || next.source.is_none())?;
                            current = Some(next);
                        }
                        player.poll_event(0.0)?;
                        let needs_audio = current.as_ref().is_some_and(|r| {
                            r.playing
                                && !r.adjustment.muted
                                && !r.adjustment.audio_separated
                                && r.volume > 0.0
                                && r.source.as_ref().is_some_and(|s| !s.audio.is_empty())
                        });
                        if fail_audio && needs_audio && !audio_fault_injected {
                            // Isolated verifier only: fail this player's endpoint, never Windows settings.
                            player.set(
                                "audio-device",
                                "wasapi/{00000000-0000-0000-0000-000000000000}",
                            )?;
                            audio_fault_injected = true;
                        }
                        let retry_audio = {
                            // Publish one coherent snapshot rather than many separately locked fields.
                            let mut updated = state.lock().unwrap();
                            if let Some(position) =
                                player.property("time-pos").and_then(|s| s.parse().ok())
                            {
                                updated.position = position;
                            }
                            updated.ended =
                                player.property("eof-reached").as_deref() == Some("yes");
                            updated.paused = player.property("pause").as_deref() == Some("yes");
                            if let Some(frame) = player.take_frame() {
                                updated.frame = Some(Arc::new(frame));
                                changed = true;
                            }
                            updated.gamma = player.property("video-out-params/gamma");
                            updated.decoder = player.property("hwdec-current");
                            updated.audio_output = player.property("current-ao").filter(|_| {
                                player.property("audio-out-params/samplerate").is_some()
                            });
                            updated.audio_stream = player
                                .property("current-tracks/audio/ff-index")
                                .and_then(|s| s.parse().ok());
                            let retry = audio_recovery.update(
                                needs_audio && !updated.ended,
                                updated
                                    .audio_output
                                    .as_deref()
                                    .is_some_and(|ao| ao != "null"),
                                Instant::now(),
                            );
                            updated.audio_warning = audio_recovery.warning();
                            updated.audio_recovery_attempts = audio_recovery.attempts;
                            changed |= previous.position != updated.position
                                || previous.ended != updated.ended
                                || previous.paused != updated.paused
                                || previous.gamma != updated.gamma
                                || previous.audio_warning != updated.audio_warning;
                            retry
                        };
                        if retry_audio && let Some(track) = audio_track.as_deref() {
                            if fail_audio {
                                player.set("audio-device", "auto")?;
                            }
                            // A missing endpoint is recoverable. Re-select a disabled audio track
                            // and re-open only AO, preserving the video clock and project state.
                            // Failed attempts leave the warning visible; actual output is checked again.
                            let _ = player.set("aid", track);
                            let _ = player.command(&["ao-reload"]);
                        }
                        let playing = current
                            .as_ref()
                            .is_some_and(|r| r.playing && r.source.is_some());
                        if changed || playing {
                            context.request_repaint();
                        }
                        // Submissions and shutdown unpark immediately. A paused player
                        // needs only a slow display/HDR check, not continuous polling.
                        if requests.lock().unwrap().is_some() {
                            continue;
                        }
                        std::thread::park_timeout(if playing {
                            Duration::from_millis(16)
                        } else {
                            Duration::from_secs(1)
                        });
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    if !software && !stopped.load(Ordering::Acquire) {
                        if let Some(request) = &mut retry {
                            let latest = state.lock().unwrap();
                            if latest.ready && latest.generation == request.generation {
                                request.position = latest.position;
                            }
                        }
                        let mut queued = requests.lock().unwrap();
                        if queued.is_none() {
                            *queued = retry;
                        }
                        continue;
                    }
                    state.lock().unwrap().error = Some(error.to_string());
                    context.request_repaint();
                }
                break;
            }
        });
        Self {
            request,
            status,
            stop,
            worker: Some(worker),
            submitted: Mutex::new(None),
        }
    }
    pub fn submit(&self, request: Request) {
        let key = (
            request.generation,
            request.playing,
            request.volume.to_bits(),
            request.steps,
            request.scrubbing,
        );
        let mut submitted = self.submitted.lock().unwrap();
        if *submitted == Some(key) {
            return;
        }
        *submitted = Some(key);
        *self.request.lock().unwrap() = Some(request);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}

/// Bound only the preview transform. Project settings and export stay exact.
pub fn preview_output(
    mut output: OutputSettings,
    width: f32,
    height: f32,
    software: bool,
) -> OutputSettings {
    let needed = (f64::from(width.max(1.0)) / f64::from(output.width))
        .min(f64::from(height.max(1.0)) / f64::from(output.height));
    // Eighth-size tiers avoid rebuilding the graph for every resize pixel.
    let mut scale = ((needed * 8.0).ceil() / 8.0).min(1.0);
    if software {
        scale = scale
            .min(1280.0 / f64::from(output.width))
            .min(720.0 / f64::from(output.height));
    }
    output.width = ((f64::from(output.width) * scale).round() as u32)
        .max(2)
        .next_multiple_of(2);
    output.height = ((f64::from(output.height) * scale).round() as u32)
        .max(2)
        .next_multiple_of(2);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_recovery_is_delayed_throttled_and_clears_on_recovery_or_intentional_silence() {
        let now = Instant::now();
        let mut health = AudioRecovery::default();
        assert!(!health.update(true, false, now));
        assert!(health.warning().is_none());
        assert!(health.update(true, false, now + Duration::from_secs(1)));
        assert!(health.warning().is_some());
        assert!(!health.update(true, false, now + Duration::from_secs(2)));
        assert!(health.update(true, false, now + Duration::from_secs(3)));
        assert_eq!(health.attempts, 2);
        assert!(!health.update(true, true, now + Duration::from_secs(4)));
        assert!(health.warning().is_none());
        assert!(!health.update(true, false, now + Duration::from_secs(5)));
        assert!(!health.update(false, false, now + Duration::from_secs(6)));
        assert!(health.warning().is_none());
        assert!(!health.update(true, false, now + Duration::from_secs(7)));
        assert_eq!(health.attempts, 2);
    }

    #[test]
    fn preview_sizes_follow_display_preserve_profile_rate_and_leave_export_unchanged() {
        let output = OutputSettings {
            width: 3840,
            height: 2160,
            profile: project_store::ColorProfile::Hdr10,
            ..Default::default()
        };
        let preview = preview_output(output.clone(), 700.0, 394.0, false);
        assert_eq!((preview.width, preview.height), (960, 540));
        assert_eq!(preview.profile, output.profile);
        assert_eq!(preview.rate, output.rate);
        assert_eq!((output.width, output.height), (3840, 2160));
        assert_eq!(preview_output(output.clone(), 710.0, 400.0, false), preview);
        assert_eq!(
            preview_output(output.clone(), 3840.0, 2160.0, false),
            output
        );
        let software = preview_output(output, 3840.0, 2160.0, true);
        assert_eq!((software.width, software.height), (1280, 720));
        let portrait = preview_output(
            OutputSettings {
                width: 2160,
                height: 3840,
                ..Default::default()
            },
            400.0,
            700.0,
            false,
        );
        assert_eq!((portrait.width, portrait.height), (540, 960));
    }
}
