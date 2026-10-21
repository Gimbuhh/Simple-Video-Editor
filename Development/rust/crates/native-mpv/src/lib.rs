//! Narrow, thread-confined libmpv owner. Scheduling belongs to the editor actor.
mod ffi;
mod software;
use sha2::{Digest, Sha256};
pub use software::Frame;
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::Read,
    marker::PhantomData,
    path::Path,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    SoftwareSdr,
    NativeSdr,
    NativeAuto,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::SoftwareSdr => "Software SDR reference (HDR inputs rejected)",
            Self::NativeSdr => "Native D3D11 SDR target (HDR inputs tone-mapped)",
            Self::NativeAuto => "Native D3D11 automatic target (HDR presentation unverified)",
        }
    }
}

pub struct Player {
    handle: NonNull<std::ffi::c_void>,
    api: Arc<ffi::Api>,
    renderer: Option<software::Renderer>,
    mode: Mode,
    // Do not allow a handle to escape the thread that owns its control API.
    _thread_confined: PhantomData<Rc<()>>,
}

impl Player {
    pub fn open(library: &Path, mode: Mode) -> Result<Self> {
        Self::open_window(library, mode, None, false)
    }

    pub fn open_embedded(library: &Path, mode: Mode, window: isize) -> Result<Self> {
        if window == 0 {
            return Err("A live video child window is required".into());
        }
        Self::open_window(library, mode, Some(window), true)
    }

    fn open_window(library: &Path, mode: Mode, window: Option<isize>, audio: bool) -> Result<Self> {
        if !library.is_absolute() {
            return Err("libmpv path must be absolute".into());
        }
        let pins: serde_json::Value =
            serde_json::from_str(include_str!("../../../../scripts/dependencies.json"))?;
        let expected = pins["MpvLibrarySha256"]
            .as_str()
            .ok_or("Missing native hash pin")?;
        let mut input = File::open(library)?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected) {
            return Err("libmpv checksum does not match the pinned native bundle".into());
        }
        let api = Arc::new(ffi::Api::load(library)?);
        // SAFETY: symbol resolved against the verified library, no arguments.
        let version = unsafe { (api.version)() };
        if version >> 16 != 2 {
            return Err("Unsupported libmpv client API major version".into());
        }
        // SAFETY: creating a new, uniquely-owned handle from the loaded API.
        let handle = NonNull::new(unsafe { (api.create)() }).ok_or("mpv_create failed")?;
        let mut player = Self {
            handle,
            api,
            renderer: None,
            mode,
            _thread_confined: PhantomData,
        };
        for (key, value) in [
            ("config", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("access-references", "no"),
            ("audio-file-auto", "no"),
            ("sub-auto", "no"),
            ("idle", "yes"),
            ("keep-open", "yes"),
            ("pause", "yes"),
            ("terminal", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("osc", "no"),
            ("osd-level", "0"),
            ("hwdec", "no"),
            ("ao", if audio { "auto" } else { "null" }),
        ] {
            player.option(key, value)?;
        }
        if let Some(window) = window {
            player.option("video-rotate", "no")?;
            player.option("wid", &window.to_string())?;
        }
        match mode {
            Mode::SoftwareSdr => {
                player.option("vo", "libmpv")?;
                player.option("profile", "sw-fast")?;
            }
            Mode::NativeSdr | Mode::NativeAuto => {
                for (key, value) in [
                    ("vo", "gpu-next"),
                    ("gpu-api", "d3d11"),
                    ("gpu-context", "d3d11"),
                    ("force-window", if window.is_some() { "no" } else { "yes" }),
                    ("geometry", "960x540"),
                    ("title", mode.label()),
                    ("target-colorspace-hint", "auto"),
                ] {
                    player.option(key, value)?;
                }
                if mode == Mode::NativeSdr {
                    player.option("target-trc", "srgb")?;
                    player.option("target-prim", "bt.709")?;
                    player.option("target-peak", "203")?;
                }
            }
        }
        // SAFETY: live, uniquely-owned handle, options initialized above.
        player.check(unsafe { (player.api.initialize)(player.handle.as_ptr()) })?;
        if mode == Mode::SoftwareSdr {
            player.renderer = Some(software::Renderer::start(
                player.handle.as_ptr(),
                player.api.clone(),
            )?);
        }
        Ok(player)
    }

    fn option(&self, key: &str, value: &str) -> Result<()> {
        let (key, value) = (CString::new(key)?, CString::new(value)?);
        // SAFETY: NUL-terminated strings and live handle; libmpv copies values.
        self.check(unsafe { (self.api.option)(self.handle.as_ptr(), key.as_ptr(), value.as_ptr()) })
    }

    pub fn property(&self, key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        // SAFETY: live handle; returned allocation is released exactly once.
        unsafe {
            let value = (self.api.get)(self.handle.as_ptr(), key.as_ptr());
            if value.is_null() {
                return None;
            }
            let text = CStr::from_ptr(value).to_string_lossy().into_owned();
            (self.api.free)(value.cast());
            Some(text)
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let (key, value) = (CString::new(key)?, CString::new(value)?);
        // SAFETY: live handle; NUL-terminated strings retained for this call.
        self.check(unsafe { (self.api.set)(self.handle.as_ptr(), key.as_ptr(), value.as_ptr()) })
    }

    pub fn command(&self, arguments: &[&str]) -> Result<()> {
        let strings = arguments
            .iter()
            .map(|s| CString::new(*s))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut pointers: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
        pointers.push(std::ptr::null());
        // SAFETY: null-terminated argument vector; strings remain alive until
        // the synchronous API returns. Filenames are arguments, never commands.
        self.check(unsafe { (self.api.command)(self.handle.as_ptr(), pointers.as_ptr()) })
    }

    pub fn load(&self, path: &media_path::RecordingPath) -> Result<()> {
        self.set_paused(true)?;
        self.command(&["stop"])?;
        // Serialized lab loads: discard old events/frames before replacement.
        // The editor will need generation-tagged asynchronous coordination.
        for _ in 0..256 {
            if self.poll_event(0.0)? == 0 {
                break;
            }
        }
        let _ = self.take_frame();
        self.command(&[
            "loadfile",
            path.as_path()
                .to_str()
                .ok_or("Recording path is not UTF-8")?,
            "replace",
        ])?;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if Instant::now() >= deadline {
                return Err("Timed out loading recording".into());
            }
            if self.poll_event(0.01)? == 8 {
                break;
            }
        }
        self.wait_for(5.0, |p| p.property("video-params/w").is_some())?;
        if self.mode == Mode::SoftwareSdr
            && matches!(
                self.property("video-params/gamma").as_deref(),
                Some("pq" | "hlg")
            )
        {
            self.command(&["stop"])?;
            return Err(
                "Software SDR reference cannot tone-map HDR; use the native SDR candidate".into(),
            );
        }
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) -> Result<()> {
        self.set("pause", if paused { "yes" } else { "no" })
    }

    pub fn seek(&self, time: f64) -> Result<()> {
        self.seek_interruptible(time, || false).map(|_| ())
    }
    pub fn seek_interruptible(
        &self,
        time: f64,
        mut interrupted: impl FnMut() -> bool,
    ) -> Result<bool> {
        let duration: f64 = self
            .property("duration")
            .ok_or("No media loaded")?
            .parse()?;
        if !time.is_finite() || time < 0.0 || time >= duration {
            return Err("Seek must be finite and inside the recording".into());
        }
        for _ in 0..1024 {
            if self.poll_event(0.0)? == 0 {
                break;
            }
        }
        self.command(&["seek", &time.to_string(), "absolute+exact"])?;
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut seeking = false;
        while Instant::now() < deadline {
            if interrupted() {
                return Ok(false);
            }
            let event = self.poll_event(0.005)?;
            // time-pos can report the requested timestamp before a frame is
            // presented. Wait for the SEEK -> PLAYBACK_RESTART completion.
            if event == 20 {
                seeking = true;
            }
            if event == 21 && seeking && self.property("seeking").as_deref() == Some("no") {
                return Ok(true);
            }
        }
        Err("Timed out completing the recording seek".into())
    }

    /// Step decoded frames; reverse stepping uses mpv's exact previous-frame seek.
    /// The caller can interrupt between polls when a newer generation supersedes it.
    pub fn step_frames(
        &self,
        count: i32,
        mut interrupted: impl FnMut() -> bool,
    ) -> Result<Option<f64>> {
        self.set_paused(true)?;
        let position = |player: &Self| {
            player
                .property("time-pos")
                .and_then(|v| v.parse::<f64>().ok())
        };
        let mut current = position(self).ok_or("No video frame available")?;
        for _ in 0..count.unsigned_abs().min(120) {
            if interrupted() {
                return Ok(None);
            }
            let before = current;
            if count > 0 {
                self.command(&["frame-step", "1", "mute"])?;
            } else {
                self.command(&["frame-back-step"])?;
            }
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                if interrupted() {
                    self.set_paused(true)?;
                    return Ok(None);
                }
                self.poll_event(0.005)?;
                current = position(self).unwrap_or(before);
                let changed = if count > 0 {
                    current > before + 0.000001
                } else {
                    current < before - 0.000001
                };
                if self.property("pause").as_deref() == Some("yes")
                    && self.property("seeking").as_deref() == Some("no")
                    && (changed
                        || count < 0 && current <= 0.000001
                        || self.property("eof-reached").as_deref() == Some("yes"))
                {
                    break;
                }
                if Instant::now() > deadline {
                    return Err("Timed out stepping a video frame".into());
                }
            }
            if (current - before) * f64::from(count.signum()) <= 0.000001 {
                break;
            }
        }
        Ok(Some(current))
    }

    pub fn wait_for(&self, seconds: f64, predicate: impl Fn(&Self) -> bool) -> Result<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("Invalid playback timeout".into());
        }
        let deadline = Instant::now() + Duration::from_secs_f64(seconds.clamp(0.0, 30.0));
        while Instant::now() < deadline {
            self.pump()?;
            if predicate(self) {
                return Ok(());
            }
        }
        Err("Timed out waiting for playback state".into())
    }

    pub fn pump(&self) -> Result<()> {
        self.poll_event(0.01)?;
        Ok(())
    }

    pub fn poll_event(&self, timeout: f64) -> Result<i32> {
        let mut id = 0;
        // SAFETY: event is borrowed only until the next wait; copy its fields
        // immediately. Never expose native pointers outside this module.
        unsafe {
            let event = (self.api.wait)(self.handle.as_ptr(), timeout);
            if !event.is_null() {
                let event = &*event;
                id = event.id;
                self.check(event.error)?;
                if event.id == 1 {
                    return Err("Native video window closed".into());
                }
                if event.id == 7 && !event.data.is_null() {
                    let end = &*event.data.cast::<ffi::EndFile>();
                    self.check(end.error)?;
                    if matches!(end.reason, 4 | 5) {
                        return Err(
                            "Recording failed to load, or attempted a playlist redirect".into()
                        );
                    }
                }
            }
        }
        if let Some(renderer) = &self.renderer {
            renderer.check()?;
        }
        Ok(id)
    }

    pub fn take_frame(&self) -> Option<Frame> {
        self.renderer.as_ref().and_then(|r| r.take_frame())
    }
    pub fn refresh_frame(&self) -> u64 {
        self.renderer.as_ref().map_or(0, |r| r.refresh())
    }
    pub fn resize_software(&self, width: usize, height: usize) -> Result<()> {
        if let Some(renderer) = &self.renderer {
            renderer.resize(width, height)?;
        }
        Ok(())
    }

    pub(crate) fn check(&self, status: i32) -> Result<()> {
        check(&self.api, status)
    }
}

pub(crate) fn check(api: &ffi::Api, status: i32) -> Result<()> {
    if status >= 0 {
        return Ok(());
    }
    // SAFETY: libmpv returns a static NUL-terminated error string.
    let message = unsafe { CStr::from_ptr((api.error)(status)) }.to_string_lossy();
    Err(format!("libmpv error {status}: {message}").into())
}

impl Drop for Player {
    fn drop(&mut self) {
        // Join render worker and free its context before releasing the core or
        // unloading API symbols. No detached callbacks may retain a dead handle.
        drop(self.renderer.take());
        // SAFETY: handle was created once, is thread-confined, and is freed once.
        unsafe {
            (self.api.destroy)(self.handle.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_layout_matches_windows_x64_headers() {
        assert_eq!(std::mem::size_of::<ffi::Event>(), 24);
        assert_eq!(std::mem::size_of::<ffi::Param>(), 16);
        assert_eq!(std::mem::size_of::<std::ffi::c_ulong>(), 4);
    }
    #[test]
    fn dll_requires_an_explicit_absolute_path() {
        let error = Player::open(Path::new("libmpv-2.dll"), Mode::SoftwareSdr)
            .err()
            .unwrap();
        assert!(error.to_string().contains("absolute"));
    }
    #[test]
    fn unpinned_bytes_are_rejected_before_dll_execution() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/native-boundary-tests");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("invalid-{}.dll", std::process::id()));
        std::fs::write(&path, b"not a native library").unwrap();
        let error = Player::open(&path, Mode::SoftwareSdr).err().unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(error.to_string().contains("checksum"));
    }
}
