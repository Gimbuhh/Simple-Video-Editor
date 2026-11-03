//! ABI declarations from official mpv v0.41.0 include/mpv/{client,render}.h.
//! Resolve every symbol before creating a handle. Keep the Library alive until
//! all handles/render contexts are destroyed. No global DLL search path changes.
use std::{
    ffi::{c_char, c_int, c_ulong, c_void},
    path::Path,
};

// Use kernel32's normal import library supplied with Rust's GNU toolchain.
// raw-dylib bindings otherwise require a separately installed dlltool.
struct Library(Handle);
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(path: *const u16, reserved: Handle, flags: u32) -> Handle;
    fn GetProcAddress(module: Handle, name: *const c_char) -> *mut c_void;
    fn FreeLibrary(module: Handle) -> i32;
}
// SAFETY: Windows module handles and immutable symbol lookups may be used
// concurrently. Arc keeps this module loaded through all renderer calls.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}
impl Library {
    #[cfg(windows)]
    fn load(path: &Path) -> std::io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "NUL in library path",
            ));
        }
        wide.push(0);
        // SAFETY: NUL-terminated absolute path; no process-global loader changes.
        let handle = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x100 | 0x800) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(handle))
    }
    #[cfg(windows)]
    fn symbol(&self, name: &[u8]) -> std::io::Result<*mut c_void> {
        // SAFETY: callers supply literal NUL-terminated ASCII export names.
        let symbol = unsafe { GetProcAddress(self.0, name.as_ptr().cast()) };
        if symbol.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(symbol)
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: exactly one owning module reference from LoadLibraryExW.
        unsafe {
            FreeLibrary(self.0);
        }
    }
}

pub type Handle = *mut c_void;
#[repr(C)]
pub struct Event {
    pub id: c_int,
    pub error: c_int,
    pub userdata: u64,
    pub data: *mut c_void,
}
#[repr(C)]
pub struct EndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}
#[repr(C)]
pub struct Param {
    pub kind: c_int,
    pub data: *mut c_void,
}
pub type UpdateCallback = unsafe extern "C" fn(*mut c_void);

macro_rules! exports {
    ($($field:ident: $signature:ty => $symbol:literal),* $(,)?) => {
        pub struct Api { $(pub $field: $signature,)* _library: Library }
        impl Api {
            pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                #[cfg(not(windows))]
                return Err("The playback prototype requires Windows x64".into());
                #[cfg(windows)]
                {
                    let library = Library::load(path)?;
                    Ok(Self {
                        // SAFETY: exact exported ABI signatures from client.h
                        // and render.h. Library retained through all calls.
                        $($field: unsafe { std::mem::transmute::<*mut c_void, $signature>(
                            library.symbol(concat!($symbol, "\0").as_bytes())?) },)*
                        _library: library,
                    })
                }
            }
        }
    }
}
exports! {
    create: unsafe extern "C" fn() -> Handle => "mpv_create",
    initialize: unsafe extern "C" fn(Handle) -> c_int => "mpv_initialize",
    destroy: unsafe extern "C" fn(Handle) -> () => "mpv_terminate_destroy",
    version: unsafe extern "C" fn() -> c_ulong => "mpv_client_api_version",
    option: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int => "mpv_set_option_string",
    set: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int => "mpv_set_property_string",
    get: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char => "mpv_get_property_string",
    command: unsafe extern "C" fn(Handle, *const *const c_char) -> c_int => "mpv_command",
    wait: unsafe extern "C" fn(Handle, f64) -> *const Event => "mpv_wait_event",
    free: unsafe extern "C" fn(*mut c_void) -> () => "mpv_free",
    error: unsafe extern "C" fn(c_int) -> *const c_char => "mpv_error_string",
    render_create: unsafe extern "C" fn(*mut Handle, Handle, *mut Param) -> c_int => "mpv_render_context_create",
    render_update_callback: unsafe extern "C" fn(Handle, Option<UpdateCallback>, *mut c_void) -> () => "mpv_render_context_set_update_callback",
    render_update: unsafe extern "C" fn(Handle) -> u64 => "mpv_render_context_update",
    render: unsafe extern "C" fn(Handle, *mut Param) -> c_int => "mpv_render_context_render",
    render_swap: unsafe extern "C" fn(Handle) -> () => "mpv_render_context_report_swap",
    render_free: unsafe extern "C" fn(Handle) -> () => "mpv_render_context_free",
}
