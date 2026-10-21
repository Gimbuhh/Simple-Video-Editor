use crate::{Result, check, ffi};
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct Frame {
    pub serial: u64,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub pixels: Vec<u8>,
}
pub(super) struct Renderer {
    stop: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<Frame>>>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    size: Arc<Mutex<(usize, usize)>>,
    refresh: Arc<AtomicU64>,
}

impl Renderer {
    pub fn start(player: ffi::Handle, api: Arc<ffi::Api>) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let frame = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (worker_stop, worker_frame, worker_error) =
            (stop.clone(), frame.clone(), error.clone());
        let size = Arc::new(Mutex::new((320, 180)));
        let worker_size = size.clone();
        let refresh = Arc::new(AtomicU64::new(0));
        let worker_refresh = refresh.clone();
        // Transfer the render-only pointer value, never control API ownership.
        // Player::drop always joins this worker before destroying the core.
        let player_address = player as usize;
        let thread = thread::Builder::new()
            .name("Rust software preview".into())
            .spawn(move || {
                let result = run(
                    player_address as ffi::Handle,
                    &api,
                    &worker_stop,
                    &worker_frame,
                    &worker_size,
                    &worker_refresh,
                    &ready_tx,
                );
                if let Err(message) = result {
                    *worker_error.lock().unwrap() = Some(message.to_string());
                }
            })?;
        let renderer = Self {
            stop,
            frame,
            error,
            thread: Some(thread),
            size,
            refresh,
        };
        ready_rx.recv_timeout(Duration::from_secs(5))??;
        Ok(renderer)
    }
    pub fn take_frame(&self) -> Option<Frame> {
        self.frame.lock().unwrap().take()
    }
    pub fn refresh(&self) -> u64 {
        self.refresh.fetch_add(1, Ordering::AcqRel) + 1
    }
    pub fn resize(&self, width: usize, height: usize) -> Result<()> {
        if !(1..=1920).contains(&width) || !(1..=1080).contains(&height) {
            return Err("Software preview size is outside its bounded surface".into());
        }
        *self.size.lock().unwrap() = (width, height);
        Ok(())
    }
    pub fn check(&self) -> Result<()> {
        if let Some(error) = self.error.lock().unwrap().as_ref() {
            return Err(error.clone().into());
        }
        Ok(())
    }
}

unsafe extern "C" fn updated(state: *mut c_void) {
    // SAFETY: registered pointer refers to the boxed AtomicBool below, retained
    // through callback removal and context destruction. No libmpv call or lock.
    unsafe { &*state.cast::<AtomicBool>() }.store(true, Ordering::Release);
}

struct Context<'a> {
    handle: ffi::Handle,
    api: &'a ffi::Api,
    _signal: Box<AtomicBool>,
}
impl Drop for Context<'_> {
    fn drop(&mut self) {
        // SAFETY: this thread exclusively owns render APIs; callback storage is
        // alive until after unregister and render-context destruction return.
        unsafe {
            (self.api.render_update_callback)(self.handle, None, std::ptr::null_mut());
            (self.api.render_free)(self.handle);
        }
    }
}

fn run(
    player: ffi::Handle,
    api: &ffi::Api,
    stop: &AtomicBool,
    frame: &Mutex<Option<Frame>>,
    size: &Mutex<(usize, usize)>,
    refresh: &AtomicU64,
    ready: &mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    let mut handle = std::ptr::null_mut();
    let mut parameters = [
        ffi::Param {
            kind: 1,
            data: c"sw".as_ptr().cast_mut().cast(),
        },
        ffi::Param {
            kind: 0,
            data: std::ptr::null_mut(),
        },
    ];
    // SAFETY: core is retained by Player until worker joins. Parameters match
    // MPV_RENDER_PARAM_API_TYPE; render context remains on this worker thread.
    let status = unsafe { (api.render_create)(&mut handle, player, parameters.as_mut_ptr()) };
    if let Err(error) = check(api, status) {
        let _ = ready.send(Err(error));
        return Ok(());
    }
    let context = Context {
        handle,
        api,
        _signal: Box::new(AtomicBool::new(true)),
    };
    // SAFETY: context owns stable signal allocation until unregister/free.
    unsafe {
        (api.render_update_callback)(
            handle,
            Some(updated),
            (&*context._signal as *const AtomicBool).cast_mut().cast(),
        );
    }
    ready.send(Ok(()))?;
    let (mut width, mut height, mut stride) = (320_usize, 180_usize, 1280_usize);
    let mut dimensions = [width as i32, height as i32];
    let mut stride_value = stride;
    let mut memory = vec![0_u8; stride * height + 63];
    let mut offset = (64 - memory.as_ptr() as usize % 64) % 64;
    let mut rendered_serial = 0;
    while !stop.load(Ordering::Acquire) {
        let desired = *size.lock().unwrap();
        let resized = desired != (width, height);
        let serial = refresh.load(Ordering::Acquire);
        let refreshed = serial != rendered_serial;
        if !context._signal.swap(false, Ordering::AcqRel) && !resized && !refreshed {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        if resized {
            (width, height) = desired;
            stride = (width * 4).next_multiple_of(64);
            stride_value = stride;
            dimensions = [width as i32, height as i32];
            memory = vec![0; stride * height + 63];
            offset = (64 - memory.as_ptr() as usize % 64) % 64;
        }
        // SAFETY: single render owner; allocation and stride have 64-byte
        // alignment and sufficient space for the bounded bgr0 surface.
        unsafe {
            if (api.render_update)(handle) & 1 == 0 && !resized && !refreshed {
                continue;
            }
            let mut parameters = [
                ffi::Param {
                    kind: 17,
                    data: dimensions.as_mut_ptr().cast(),
                },
                ffi::Param {
                    kind: 18,
                    data: c"bgr0".as_ptr().cast_mut().cast(),
                },
                ffi::Param {
                    kind: 19,
                    data: (&mut stride_value as *mut usize).cast(),
                },
                ffi::Param {
                    kind: 20,
                    data: memory.as_mut_ptr().add(offset).cast(),
                },
                ffi::Param {
                    kind: 0,
                    data: std::ptr::null_mut(),
                },
            ];
            check(api, (api.render)(handle, parameters.as_mut_ptr()))?;
            *frame.lock().unwrap() = Some(Frame {
                serial,
                width,
                height,
                stride,
                pixels: memory[offset..offset + stride * height].to_vec(),
            });
            (api.render_swap)(handle);
            rendered_serial = serial;
        }
    }
    Ok(())
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
