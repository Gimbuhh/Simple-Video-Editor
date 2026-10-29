//! Windows UI/file-system boundary. Editor rules remain outside this crate.
mod diagnostics;
mod display;
pub use diagnostics::trace_verification_crashes;
pub use display::{DisplayColor, display_color};
use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
};
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
pub fn ordinal_ignore_case(left: &str, right: &str) -> bool {
    let left: Vec<u16> = left.encode_utf16().collect();
    let right: Vec<u16> = right.encode_utf16().collect();
    unsafe {
        windows_sys::Win32::Globalization::CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            1,
        ) == 2
    }
}
pub fn same_file(left: &Path, right: &Path) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    let a = std::fs::File::open(left)?;
    let b = std::fs::File::open(right)?;
    unsafe {
        let mut ai = std::mem::zeroed();
        let mut bi = std::mem::zeroed();
        if windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(
            a.as_raw_handle() as isize,
            &mut ai,
        ) == 0
            || windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(
                b.as_raw_handle() as isize,
                &mut bi,
            ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(ai.dwVolumeSerialNumber == bi.dwVolumeSerialNumber
            && ai.nFileIndexHigh == bi.nFileIndexHigh
            && ai.nFileIndexLow == bi.nFileIndexLow)
    }
}

type VideoLayout = (i32, i32, i32, i32, bool);
pub struct VideoWindow {
    handle: HWND,
    layout: std::cell::Cell<Option<VideoLayout>>,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl VideoWindow {
    /// The owner must outlive this child. Destroy the player before this window.
    pub fn new(parent: isize) -> io::Result<Self> {
        // SAFETY: parent is the live UI HWND supplied by the window integration;
        // STATIC is a registered Windows class; all strings are terminated.
        let handle = unsafe {
            CreateWindowExW(
                0,
                wide(std::ffi::OsStr::new("STATIC")).as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent,
                0,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        if handle == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            handle,
            layout: std::cell::Cell::new(None),
            _thread: std::marker::PhantomData,
        })
    }
    pub fn handle(&self) -> isize {
        self.handle
    }
    /// Read this owned child's physical bounds relative to its live parent.
    pub fn bounds(&self) -> io::Result<(i32, i32, i32, i32)> {
        // SAFETY: the child and its parent are live on this UI thread. Windows
        // writes only to the local RECT/POINT buffers during these calls.
        unsafe {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(self.handle, &mut rect) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut origin = POINT {
                x: rect.left,
                y: rect.top,
            };
            if windows_sys::Win32::Graphics::Gdi::ScreenToClient(
                GetParent(self.handle),
                &mut origin,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok((
                origin.x,
                origin.y,
                rect.right - rect.left,
                rect.bottom - rect.top,
            ))
        }
    }
    pub fn layout(&self, x: i32, y: i32, width: i32, height: i32, visible: bool) {
        let next = (x, y, width.max(1), height.max(1), visible);
        if self.layout.replace(Some(next)) == Some(next) {
            return;
        }
        // SAFETY: called on the owning UI thread, with a live child HWND.
        unsafe {
            SetWindowPos(
                self.handle,
                0,
                x,
                y,
                width.max(1),
                height.max(1),
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            ShowWindow(self.handle, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
}
pub fn cursor_position(owner: isize) -> io::Result<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: Windows writes to this local point; owner is the live UI HWND.
    unsafe {
        if GetCursorPos(&mut point) == 0
            || windows_sys::Win32::Graphics::Gdi::ScreenToClient(owner, &mut point) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((point.x, point.y))
}
impl Drop for VideoWindow {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.handle);
        }
    }
}

pub fn choose_files(owner: isize, save: bool, projects: bool) -> io::Result<Vec<PathBuf>> {
    let mut buffer = vec![0_u16; 65536];
    let filter = if projects {
        "Editor projects\0*.sveproject\0All files\0*.*\0\0"
    } else if save {
        "MP4 video\0*.mp4\0\0"
    } else {
        "Recordings\0*.mp4;*.mkv;*.mov;*.webm;*.avi;*.m4v;*.ts\0All files\0*.*\0\0"
    };
    let filter: Vec<u16> = filter.encode_utf16().collect();
    let extension: Vec<u16> = if projects { "sveproject\0" } else { "mp4\0" }
        .encode_utf16()
        .collect();
    // SAFETY: zero initialization and cbSize follow OPENFILENAMEW's ABI. Owned
    // UTF-16 buffers remain valid throughout the modal native dialog call.
    unsafe {
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = owner;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.lpstrDefExt = extension.as_ptr();
        dialog.Flags = OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST | if projects { 0 } else { OFN_ALLOWMULTISELECT }
            };
        let accepted = if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        };
        if accepted == 0 {
            let code = CommDlgExtendedError();
            return if code == 0 {
                Ok(Vec::new())
            } else {
                Err(io::Error::other(format!("File dialog failed ({code})")))
            };
        }
    }
    let parts: Vec<_> = buffer
        .split(|c| *c == 0)
        .take_while(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect();
    if parts.len() == 1 {
        return Ok(vec![PathBuf::from(&parts[0])]);
    }
    Ok(parts[1..]
        .iter()
        .map(|s| Path::new(&parts[0]).join(s))
        .collect())
}

pub fn confirm(owner: isize, message: &str, title: &str) -> bool {
    unsafe {
        MessageBoxW(
            owner,
            wide(std::ffi::OsStr::new(message)).as_ptr(),
            wide(std::ffi::OsStr::new(title)).as_ptr(),
            MB_YESNO | MB_ICONQUESTION,
        ) == IDYES
    }
}

pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    // SAFETY: both paths are NUL-terminated and chosen by the caller; MoveFileEx
    // atomically replaces on the same volume without deleting the old file first.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            wide(source.as_os_str()).as_ptr(),
            wide(destination.as_os_str()).as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
