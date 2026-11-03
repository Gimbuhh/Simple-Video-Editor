//! Windows recording authorization, independent of project parsing and libmpv.
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct RecordingPath(PathBuf);
impl RecordingPath {
    pub fn authorize(path: &Path) -> io::Result<Self> {
        authorize(path, 0).map(Self)
    }
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

fn rejected() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Recordings must be on a local drive; network, device and unsupported linked paths are rejected",
    )
}

fn local_syntax(path: &Path) -> io::Result<String> {
    let path = path.to_str().ok_or_else(rejected)?;
    let bytes = path.as_bytes();
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'/' | b'\\')
        || path.chars().any(char::is_control)
    {
        return Err(rejected());
    }
    Ok(path.replace('/', "\\"))
}

#[cfg(windows)]
fn authorize(path: &Path, links: usize) -> io::Result<PathBuf> {
    use std::os::windows::fs::MetadataExt;
    let text = local_syntax(path)?; // Reject unsafe namespaces before any query.
    let root = &text[..3];
    let wide = [u16::from(text.as_bytes()[0]), b':' as u16, b'\\' as u16, 0];
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDriveTypeW(root: *const u16) -> u32;
    }
    // SAFETY: root is a fixed-size, NUL-terminated UTF-16 local-drive root.
    if !matches!(unsafe { GetDriveTypeW(wide.as_ptr()) }, 2 | 3 | 5 | 6) {
        return Err(rejected());
    }
    let mut parts = Vec::new();
    for part in text[3..].split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part if part.contains(':') => return Err(rejected()),
            part => parts.push(part),
        }
    }
    let mut current = PathBuf::from(root);
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => {
                if links >= 16 {
                    return Err(rejected());
                }
                // read_link reads the immediate reparse target, not its contents.
                let mut target = std::fs::read_link(&current)?;
                if target.is_relative() {
                    target = current.parent().ok_or_else(rejected)?.join(target);
                }
                // std::fs::read_link may express a local link target as \\?\C:.
                // Strip that prefix ONLY for the already-read link, never input.
                if let Some(suffix) = target.to_str().and_then(|s| s.strip_prefix("\\\\?\\")) {
                    local_syntax(Path::new(suffix))?;
                    target = PathBuf::from(suffix);
                }
                for rest in &parts[index + 1..] {
                    target.push(rest);
                }
                return authorize(&target, links + 1);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                for rest in &parts[index + 1..] {
                    current.push(rest);
                }
                return Ok(current); // Preserve missing paths for relinking.
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

#[cfg(not(windows))]
fn authorize(_: &Path, _: usize) -> io::Result<PathBuf> {
    Err(rejected())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_namespaces_fail_before_drive_or_file_access() {
        for path in [
            "\\\\server\\clip.mp4",
            "\\\\?\\C:\\clip.mp4",
            "\\\\.\\C:\\clip.mp4",
            "\\??\\C:\\clip.mp4",
            "https://invalid/clip.mp4",
            "C:clip.mp4",
            "clip.mp4",
            "C:/bad\0.mp4",
        ] {
            assert!(RecordingPath::authorize(Path::new(path)).is_err(), "{path}");
        }
    }
    #[test]
    #[cfg(windows)]
    fn missing_local_recordings_and_lexical_normalization_work() {
        let path =
            RecordingPath::authorize(Path::new("C:/missing-parent/../missing-recording.mp4"))
                .unwrap();
        assert_eq!(path.as_path(), Path::new("C:\\missing-recording.mp4"));
        assert!(RecordingPath::authorize(Path::new("C:/clip.mp4:stream")).is_err());
    }
    #[test]
    #[cfg(windows)]
    fn current_executable_is_local_and_optional_junction_is_resolved() {
        let executable = std::env::current_exe().unwrap();
        assert_eq!(
            RecordingPath::authorize(&executable).unwrap().as_path(),
            executable
        );
        // Existing generated fixture; integration runner creates the junction.
        if let Some(root) = std::env::var_os("SVE_PATH_FIXTURES") {
            let root = PathBuf::from(root);
            let resolved =
                RecordingPath::authorize(&root.join("local-recording-link/source B.mp4")).unwrap();
            assert_eq!(resolved.as_path(), root.join("source B.mp4"));
        }
    }
}
