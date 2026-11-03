use std::{
    io,
    mem::{size_of, zeroed},
};
use windows_sys::Win32::{Devices::Display::*, Foundation::*, Graphics::Gdi::*};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayColor {
    pub name: String,
    pub hdr_supported: bool,
    pub hdr_active: bool,
    pub bits: u32,
    pub state_known: bool,
}
// Windows SDK wingdi.h: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2 (request 15).
// windows-sys 0.52 predates this structure. Keep the exact C layout here.
// https://github.com/microsoft/win32metadata/blob/main/generation/WinSDK/RecompiledIdlHeaders/um/wingdi.h
#[repr(C)]
struct AdvancedColor2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    flags: u32,
    encoding: i32,
    bits: u32,
    mode: i32,
}
fn hdr_active(flags: u32, mode: i32) -> bool {
    mode == 2 && flags & 2 != 0 && flags & 8 == 0
}

/// Read the display containing this window. This never changes Windows settings.
/// Call on the playback worker; display enumeration is not UI-frame work.
pub fn display_color(window: isize) -> io::Result<DisplayColor> {
    // SAFETY: buffers have SDK sizes, zeroed POD fields and bounded lengths.
    // Win32 borrows them only during each call. No native pointer escapes.
    unsafe {
        let mut monitor: MONITORINFOEXW = zeroed();
        monitor.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(
            MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
            &mut monitor.monitorInfo,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        for _ in 0..3 {
            let (mut paths, mut modes) = (0, 0);
            let error = GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths, &mut modes);
            if error != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            if paths > 128 || modes > 1024 {
                return Err(io::Error::other("Display configuration exceeds bounds"));
            }
            let mut path_buffer = vec![zeroed::<DISPLAYCONFIG_PATH_INFO>(); paths as usize];
            let mut mode_buffer = vec![zeroed::<DISPLAYCONFIG_MODE_INFO>(); modes as usize];
            let error = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths,
                path_buffer.as_mut_ptr(),
                &mut modes,
                mode_buffer.as_mut_ptr(),
                std::ptr::null_mut(),
            );
            if error == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if error != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            let end = monitor
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(monitor.szDevice.len());
            let mut result = DisplayColor {
                name: String::from_utf16_lossy(&monitor.szDevice[..end]),
                hdr_supported: true,
                hdr_active: true,
                bits: u32::MAX,
                state_known: true,
            };
            let mut found = false;
            for path in path_buffer.iter().take(paths as usize) {
                let mut name: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
                name.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                };
                if DisplayConfigGetDeviceInfo(&mut name.header) != 0
                    || name.viewGdiDeviceName != monitor.szDevice
                {
                    continue;
                }
                found = true;
                let mut color = AdvancedColor2 {
                    header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                        r#type: 15,
                        size: size_of::<AdvancedColor2>() as u32,
                        adapterId: path.targetInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                    flags: 0,
                    encoding: 0,
                    bits: 0,
                    mode: 0,
                };
                if DisplayConfigGetDeviceInfo(&mut color.header) == 0 {
                    result.hdr_supported &= color.flags & 16 != 0;
                    result.hdr_active &= hdr_active(color.flags, color.mode);
                    result.bits = result.bits.min(color.bits);
                } else {
                    // Older systems expose Advanced Color, which can mean WCG.
                    // Do not label that ambiguous state as confirmed HDR.
                    result.state_known = false;
                    result.hdr_supported = false;
                    result.hdr_active = false;
                    result.bits = 0;
                }
            }
            if found {
                return Ok(result);
            }
            return Err(io::Error::other(
                "The preview display has no active desktop path",
            ));
        }
        Err(io::Error::other(
            "Display configuration changed while reading it",
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hdr_requires_active_hdr_mode_not_wide_color() {
        assert_eq!(size_of::<AdvancedColor2>(), 36);
        assert!(hdr_active(0b110010, 2));
        assert!(!hdr_active(0b110010, 1));
        assert!(!hdr_active(0b110000, 2));
        assert!(!hdr_active(0b111010, 2));
    }
}
