//! One bounded audio-only actor for the independent timeline lane. The video
//! clock remains authoritative; silence/gaps never create a GPU renderer.
use crate::playback::AudioRecovery;
use editor_core::ClipId;
use media_engine::Tools;
use native_mpv::Player;
use project_store::{AudioItem, MediaSource};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq)]
pub struct AudioRequest {
    pub item: AudioItem,
    pub source: MediaSource,
    pub position: f64,
    pub playing: bool,
    pub volume: f64,
    pub serial: u64,
}
#[derive(Clone, Default)]
pub struct AudioStatus {
    pub item: Option<ClipId>,
    pub ready: bool,
    pub paused: bool,
    pub position: f64,
    pub output: Option<String>,
    pub stream: Option<u32>,
    pub gain_db: f64,
    pub error: Option<String>,
    pub warning: Option<String>,
}
pub struct AudioPlayback {
    request: Arc<Mutex<Option<Option<AudioRequest>>>>,
    status: Arc<Mutex<AudioStatus>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl AudioPlayback {
    pub fn new(tools: Tools, ctx: eframe::egui::Context) -> Self {
        let request = Arc::new(Mutex::new(None::<Option<AudioRequest>>));
        let status = Arc::new(Mutex::new(AudioStatus::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, state, stopped) = (request.clone(), status.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            let result = (|| -> media_engine::Result<()> {
                let player = Player::open_audio(&tools.mpv)?;
                let mut current: Option<AudioRequest> = None;
                let mut loaded_source = None;
                let mut recovery = AudioRecovery::default();
                let mut audio_track = None;
                while !stopped.load(Ordering::Acquire) {
                    let next = requests.lock().unwrap().take();
                    if let Some(next) = next {
                        if let Some(next) = next {
                            let settings = &next.item.settings;
                            let load = loaded_source.as_ref() != Some(&next.source);
                            let changed = current.as_ref().is_none_or(|c| {
                                c.item != next.item
                                    || c.source != next.source
                                    || c.serial != next.serial
                            });
                            if load {
                                player.set_paused(true)?;
                                let path = media_path::RecordingPath::authorize(Path::new(
                                    &next.source.path,
                                ))?;
                                player.load(&path)?;
                                loaded_source = Some(next.source.clone());
                            }
                            if changed {
                                player.set_paused(true)?;
                                let stream = settings
                                    .stream
                                    .or_else(|| next.source.audio.first().map(|a| a.index))
                                    .ok_or("Audio item has no stream")?;
                                let count = player
                                    .property("track-list/count")
                                    .and_then(|n| n.parse::<usize>().ok())
                                    .unwrap_or(0);
                                let aid = (0..count)
                                    .find_map(|i| {
                                        (player
                                            .property(&format!("track-list/{i}/type"))
                                            .as_deref()
                                            == Some("audio")
                                            && player
                                                .property(&format!("track-list/{i}/ff-index"))
                                                .and_then(|s| s.parse::<u32>().ok())
                                                == Some(stream))
                                        .then(|| player.property(&format!("track-list/{i}/id")))
                                        .flatten()
                                    })
                                    .ok_or("Selected audio stream is unavailable")?;
                                player.set("aid", &aid)?;
                                audio_track = Some(aid);
                                let mut af = format!(
                                    "lavfi=[volume={}dB",
                                    if settings.muted {
                                        -120.0
                                    } else {
                                        settings.gain_db
                                    }
                                );
                                if settings.fade_in > 0.0 {
                                    af.push_str(&format!(
                                        ",afade=t=in:st={}:d={}",
                                        next.item.clip.source_in, settings.fade_in
                                    ));
                                }
                                if settings.fade_out > 0.0 {
                                    af.push_str(&format!(
                                        ",afade=t=out:st={}:d={}",
                                        next.item.clip.source_out - settings.fade_out,
                                        settings.fade_out
                                    ));
                                }
                                af.push(']');
                                player.set("af", &af)?;
                            }
                            let needs_seek = changed
                                || next.playing
                                    && player
                                        .property("time-pos")
                                        .and_then(|v| v.parse::<f64>().ok())
                                        .is_some_and(|p| (p - next.position).abs() > 0.10);
                            if needs_seek
                                && !player.seek_interruptible(next.position, || {
                                    stopped.load(Ordering::Acquire)
                                        || requests.lock().unwrap().as_ref().is_some_and(|r| {
                                            r.as_ref().is_none_or(|r| {
                                                r.serial != next.serial
                                                    || r.item.clip.id != next.item.clip.id
                                            })
                                        })
                                })?
                            {
                                continue;
                            }
                            player.set("volume", &next.volume.to_string())?;
                            player.set_paused(!next.playing)?;
                            let output = player.property("audio-out-params/format");
                            if recovery.update(
                                next.playing && !settings.muted && next.volume > 0.0,
                                output.as_deref().is_some_and(|ao| ao != "null"),
                                Instant::now(),
                            ) {
                                if let Some(track) = audio_track.as_deref() {
                                    let _ = player.set("aid", track);
                                }
                                let _ = player.command(&["ao-reload"]);
                            }
                            *state.lock().unwrap() = AudioStatus {
                                item: Some(next.item.clip.id),
                                ready: true,
                                paused: !next.playing,
                                position: player
                                    .property("time-pos")
                                    .and_then(|v| v.parse::<f64>().ok())
                                    .unwrap_or(next.position),
                                output,
                                stream: settings
                                    .stream
                                    .or_else(|| next.source.audio.first().map(|a| a.index)),
                                gain_db: settings.gain_db,
                                error: None,
                                warning: recovery.warning(),
                            };
                            current = Some(next);
                        } else {
                            player.set_paused(true)?;
                            current = None;
                            *state.lock().unwrap() = AudioStatus::default();
                        }
                    }
                    player.poll_event(0.0)?;
                    if current.as_ref().is_some_and(|r| r.playing) {
                        std::thread::park_timeout(Duration::from_millis(20));
                    } else {
                        std::thread::park();
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                state.lock().unwrap().error = Some(error.to_string());
                ctx.request_repaint();
            }
        });
        Self {
            request,
            status,
            stop,
            worker: Some(worker),
        }
    }
    pub fn submit(&self, request: Option<AudioRequest>) {
        *self.request.lock().unwrap() = Some(request);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
    pub fn status(&self) -> AudioStatus {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for AudioPlayback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
