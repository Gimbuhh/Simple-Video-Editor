use media_engine::{Tools, video_filters};
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
    pub source: Option<MediaSource>,
    pub adjustment: ClipSettings,
    pub output: OutputSettings,
    pub position: f64,
    pub start: f64,
    pub end: f64,
    pub playing: bool,
    pub volume: f64,
    pub hdr_preview: bool,
}
#[derive(Clone, Default)]
pub struct Status {
    pub generation: u64,
    pub ready: bool,
    pub position: f64,
    pub ended: bool,
    pub software: bool,
    pub frame: Option<Arc<native_mpv::Frame>>,
    pub gamma: Option<String>,
    pub error: Option<String>,
}
pub struct Playback {
    request: Arc<Mutex<Option<Request>>>,
    status: Arc<Mutex<Status>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Playback {
    pub fn new(tools: Tools, window: isize) -> Self {
        let request = Arc::new(Mutex::new(None::<Request>));
        let status = Arc::new(Mutex::new(Status::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, state, stopped) = (request.clone(), status.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            let forced = std::env::args_os().any(|a| a == "--software-preview");
            for attempt in 0..if forced { 1 } else { 2 } {
                let software = forced || attempt > 0;
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
                    while !stopped.load(Ordering::Acquire) {
                        let next = requests.lock().unwrap().take();
                        if let Some(next) = next {
                            let replace = current
                                .as_ref()
                                .is_none_or(|c| c.generation != next.generation);
                            if replace {
                                *state.lock().unwrap() = Status {
                                    generation: next.generation,
                                    software,
                                    ..Default::default()
                                };
                                player.set_paused(true)?;
                                let reuse = current.as_ref().is_some_and(|c| {
                                    c.source == next.source
                                        && c.source.is_some()
                                        && c.adjustment == next.adjustment
                                        && c.output == next.output
                                        && c.hdr_preview == next.hdr_preview
                                        && c.start == next.start
                                        && c.end == next.end
                                });
                                if reuse {
                                    player.command(&[
                                        "seek",
                                        &next.position.to_string(),
                                        "absolute+exact",
                                    ])?;
                                    let deadline = Instant::now() + Duration::from_secs(8);
                                    loop {
                                        if stopped.load(Ordering::Acquire)
                                            || requests
                                                .lock()
                                                .unwrap()
                                                .as_ref()
                                                .is_some_and(|r| r.generation != next.generation)
                                        {
                                            break;
                                        }
                                        player.poll_event(0.005)?;
                                        let position = player
                                            .property("time-pos")
                                            .and_then(|p| p.parse::<f64>().ok());
                                        if player.property("seeking").as_deref() == Some("no")
                                            && position.is_some_and(|p| {
                                                (p - next.position).abs()
                                                    <= next.output.rate.frame_seconds().max(0.05)
                                            })
                                        {
                                            break;
                                        }
                                        if Instant::now() > deadline {
                                            return Err("Timed out seeking the recording".into());
                                        }
                                    }
                                    if stopped.load(Ordering::Acquire)
                                        || requests
                                            .lock()
                                            .unwrap()
                                            .as_ref()
                                            .is_some_and(|r| r.generation != next.generation)
                                    {
                                        continue;
                                    }
                                } else {
                                    current = None;
                                    player.command(&["stop"])?;
                                    for _ in 0..1024 {
                                        if player.poll_event(0.0)? == 0 {
                                            break;
                                        }
                                    }
                                    if let Some(source) = &next.source {
                                        let path = media_path::RecordingPath::authorize(
                                            Path::new(&source.path),
                                        )?;
                                        // All preview frames use the same color/framing graph as export.
                                        // Software always receives an explicit SDR transform.
                                        let mut output = next.output.clone();
                                        if software || !next.hdr_preview {
                                            output.profile = project_store::ColorProfile::Sdr;
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
                                            &format!(
                                                "lavfi=[{}]",
                                                video_filters(
                                                    source,
                                                    &next.adjustment,
                                                    &output,
                                                    false
                                                )
                                            ),
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
                                                .is_some_and(|r| r.generation != next.generation)
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
                                                break;
                                            }
                                        }
                                        player.set("aid", aid.as_deref().unwrap_or("no"))?;
                                        let duration = next.end - next.start;
                                        let mut audio = format!(
                                            "lavfi=[volume={}dB",
                                            if next.adjustment.muted {
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
                                state.lock().unwrap().position = next.position;
                                state.lock().unwrap().ready = true;
                            }
                            player.set("volume", &next.volume.to_string())?;
                            player.set_paused(!next.playing || next.source.is_none())?;
                            current = Some(next);
                        }
                        player.poll_event(0.005)?;
                        if let Some(position) =
                            player.property("time-pos").and_then(|s| s.parse().ok())
                        {
                            state.lock().unwrap().position = position;
                        }
                        state.lock().unwrap().ended =
                            player.property("eof-reached").as_deref() == Some("yes");
                        if let Some(frame) = player.take_frame() {
                            state.lock().unwrap().frame = Some(Arc::new(frame));
                        }
                        state.lock().unwrap().gamma = player.property("video-out-params/gamma");
                        std::thread::sleep(Duration::from_millis(3));
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    if !software && !stopped.load(Ordering::Acquire) {
                        continue;
                    }
                    state.lock().unwrap().error = Some(error.to_string());
                }
                break;
            }
        });
        Self {
            request,
            status,
            stop,
            worker: Some(worker),
        }
    }
    pub fn submit(&self, request: Request) {
        *self.request.lock().unwrap() = Some(request);
    }
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
