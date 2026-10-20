use super::*;
use std::os::windows::fs::MetadataExt;
fn unlinked(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_attributes() & 0x400 != 0 => {
            Err("The preview cache folder is a link; its files were left untouched".into())
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
impl Tools {
    pub(crate) fn cache_directory(&self, kind: &str) -> Result<PathBuf> {
        unlinked(&self.cache)?;
        let path = self.cache.join(kind);
        unlinked(&path)?;
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }
    pub(crate) fn cache_key(&self, source: &MediaSource) -> Result<String> {
        let authorized = media_path::RecordingPath::authorize(Path::new(&source.path))?;
        let metadata = std::fs::metadata(authorized.as_path())?;
        Ok(format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "v1|{}|{}|{:?}|{}|{}|{}|{}",
                    source.path,
                    metadata.len(),
                    metadata.modified()?,
                    source.transfer,
                    source.primaries,
                    source.matrix,
                    source.range
                )
                .as_bytes()
            )
        ))
    }
    /// Delete only our generated flat cache files, never directories or links.
    pub fn clear_cache(&self) -> Result<(usize, u64)> {
        unlinked(&self.cache)?;
        let mut count = 0;
        let mut bytes = 0;
        for (kind, extension) in [("thumbnails", "bmp"), ("waveforms", "peaks")] {
            let directory = self.cache.join(kind);
            unlinked(&directory)?;
            if !directory.exists() {
                continue;
            }
            for entry in std::fs::read_dir(directory)? {
                let path = entry?.path();
                let meta = std::fs::symlink_metadata(&path)?;
                if !meta.is_file() || meta.file_attributes() & 0x400 != 0 {
                    continue;
                }
                let valid = path.extension().is_some_and(|v| v == extension)
                    && path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()));
                if valid && std::fs::remove_file(&path).is_ok() {
                    count += 1;
                    bytes += meta.len();
                }
            }
        }
        Ok((count, bytes))
    }
}
