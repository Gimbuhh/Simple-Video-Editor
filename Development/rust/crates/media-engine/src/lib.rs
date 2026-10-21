//! Native tools, color transforms and export jobs. No UI or C# dependency.
mod cache;
mod export;
use editor_core::{FrameRate, SourceId};
pub use export::*;
use project_store::{
    AudioStream, ClipSettings, ColorProfile, Framing, MediaSource, OutputSettings, ProjectMetadata,
    SavedProject,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type Cancel = Arc<AtomicBool>;
pub fn uncancelled() -> Cancel {
    Arc::new(AtomicBool::new(false))
}
pub fn check_cancel(cancel: &Cancel) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err("Operation canceled".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub mpv: PathBuf,
    pub cache: PathBuf,
}
impl Tools {
    pub fn discover(cache: PathBuf) -> Result<Self> {
        let executable = std::env::current_exe()?;
        let mut roots = vec![executable.parent().unwrap().to_path_buf()];
        if cfg!(debug_assertions) {
            roots.push(std::env::current_dir()?);
            roots.extend(executable.ancestors().map(Path::to_path_buf));
        }
        for root in roots {
            for (ff, mpv) in [
                (root.join("bin"), root.join("bin/libmpv-2.dll")),
                (
                    root.join("Development/vendor/ffmpeg"),
                    root.join("Development/vendor/mpv/libmpv-2.dll"),
                ),
                (
                    root.join("vendor/ffmpeg"),
                    root.join("vendor/mpv/libmpv-2.dll"),
                ),
            ] {
                if !cfg!(debug_assertions) && ff != root.join("bin") {
                    continue;
                }
                if ff.join("ffmpeg.exe").is_file()
                    && ff.join("ffprobe.exe").is_file()
                    && mpv.is_file()
                {
                    let result = Self {
                        ffmpeg: ff.join("ffmpeg.exe"),
                        ffprobe: ff.join("ffprobe.exe"),
                        mpv,
                        cache: cache.clone(),
                    };
                    let pins: serde_json::Value = serde_json::from_str(include_str!(
                        "../../../../scripts/dependencies.json"
                    ))?;
                    for (name, pin) in pins["FfmpegFiles"]
                        .as_object()
                        .ok_or("Missing native pins")?
                    {
                        if !file_hash(&ff.join(name))?
                            .eq_ignore_ascii_case(pin.as_str().ok_or("Invalid native pin")?)
                        {
                            return Err(format!("Native tool checksum mismatch: {name}").into());
                        }
                    }
                    return Ok(result);
                }
            }
        }
        Err("The bundled video tools are missing. Extract the complete app folder or run setup.ps1.".into())
    }
    pub fn probe(&self, path: &Path, id: SourceId, cancel: &Cancel) -> Result<MediaSource> {
        let path = media_path::RecordingPath::authorize(path)?;
        let bytes = run(
            &self.ffprobe,
            &[
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                path.as_path().to_str().ok_or("Invalid path")?,
            ]
            .map(str::to_owned),
            cancel,
            |_| {},
        )?;
        parse_probe(&bytes, path.as_path().to_string_lossy().into_owned(), id)
    }
    pub fn load_project(&self, path: &Path, cancel: &Cancel) -> Result<SavedProject> {
        self.load_project_relinked(path, &[], cancel)
    }
    pub fn project_sources(&self, path: &Path) -> Result<Vec<String>> {
        let value = project_store::read_document(path)?;
        let version = value
            .as_object()
            .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case("Version")))
            .and_then(|(_, v)| v.as_u64())
            .ok_or("Missing schema version")?;
        if version == 1 {
            return Ok(project_store::parse_project(&serde_json::to_vec(&value)?)?
                .sources
                .into_iter()
                .map(|s| s.path)
                .collect());
        }
        let document: SavedProject = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document
            .metadata
            .sources
            .into_iter()
            .map(|s| s.path)
            .collect())
    }
    pub fn load_project_relinked(
        &self,
        path: &Path,
        replacements: &[(String, PathBuf)],
        cancel: &Cancel,
    ) -> Result<SavedProject> {
        let resolve = |path: &str| {
            replacements
                .iter()
                .find(|(old, _)| platform_windows::ordinal_ignore_case(old, path))
                .map_or_else(
                    || PathBuf::from(path),
                    |(_, replacement)| replacement.clone(),
                )
        };
        let value = project_store::read_document(path)?;
        let version = value
            .as_object()
            .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case("Version")))
            .and_then(|(_, v)| v.as_u64())
            .ok_or("Missing project schema version")?;
        let mut document = if version == 1 {
            let legacy = project_store::parse_project(&serde_json::to_vec(&value)?)?;
            let mut metadata = ProjectMetadata::default();
            for source in legacy.sources {
                check_cancel(cancel)?;
                metadata
                    .sources
                    .push(self.probe(&resolve(&source.path), source.id, cancel)?);
            }
            if let Some(first) = metadata.sources.first() {
                metadata.output.width = first.width.next_multiple_of(2);
                metadata.output.height = first.height.next_multiple_of(2);
                metadata.output.rate = first.rate;
            }
            SavedProject {
                version: 2,
                application_version: project_store::APPLICATION_VERSION.into(),
                clips: legacy.timeline.clips().to_vec(),
                metadata,
            }
        } else if version == 2 {
            serde_json::from_value::<SavedProject>(value)?
        } else {
            return Err("This project uses a newer unsupported schema".into());
        };
        document.validate()?;
        if version == 2 {
            for source in &mut document.metadata.sources {
                check_cancel(cancel)?;
                *source = self.probe(&resolve(&source.path), source.id, cancel)?;
            }
        }
        document.validate()?;
        Ok(document)
    }
    pub fn thumbnail(&self, source: &MediaSource, cancel: &Cancel) -> Result<PathBuf> {
        let key = self.cache_key(source)?;
        let root = self.cache_directory("thumbnails")?;
        let output = root.join(format!("{key}.bmp"));
        if output.is_file() {
            return Ok(output);
        }
        let temp = root.join(format!("{key}-{}.bmp", project_store::unique_id()));
        let filters = video_filters(
            source,
            &ClipSettings::default(),
            &OutputSettings {
                width: 160,
                height: 90,
                profile: ColorProfile::Sdr,
                ..Default::default()
            },
            false,
        );
        let result = run(
            &self.ffmpeg,
            &[
                "-v".into(),
                "error".into(),
                "-nostdin".into(),
                "-ss".into(),
                (source.duration / 4.0).min(5.0).to_string(),
                "-noautorotate".into(),
                "-i".into(),
                source.path.clone(),
                "-map".into(),
                format!("0:{}", source.video_stream),
                "-vf".into(),
                filters,
                "-frames:v".into(),
                "1".into(),
                "-y".into(),
                temp.to_string_lossy().into_owned(),
            ],
            cancel,
            |_| {},
        );
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
        platform_windows::replace_file(&temp, &output)?;
        Ok(output)
    }
    pub fn waveform(&self, source: &MediaSource, cancel: &Cancel) -> Result<Vec<f32>> {
        if source.audio.is_empty() {
            return Ok(Vec::new());
        }
        let cache = self
            .cache_directory("waveforms")?
            .join(format!("{}.peaks", self.cache_key(source)?));
        if cache.is_file() {
            let mut data = Vec::new();
            File::open(&cache)?
                .take(40_000_009)
                .read_to_end(&mut data)?;
            if data.starts_with(b"SVEPEAK1")
                && (data.len() - 8).is_multiple_of(4)
                && data.len() <= 40_000_008
            {
                let peaks: Vec<f32> = data[8..]
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect();
                if peaks.iter().all(|v| v.is_finite() && *v >= 0.0) {
                    return Ok(peaks);
                }
            }
        }
        let path = media_path::RecordingPath::authorize(Path::new(&source.path))?;
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(&self.ffmpeg)
            .creation_flags(0x08000000)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-i",
                path.as_path().to_str().ok_or("Invalid path")?,
                "-map",
                "0:a:0",
                "-vn",
                "-af",
                "aresample=48000:async=1:first_pts=0",
                "-ac",
                "2",
                "-ar",
                "48000",
                "-f",
                "f32le",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut output = child.stdout.take().unwrap();
        let flag = cancel.clone();
        let result = std::thread::spawn(move || -> Result<Vec<f32>> {
            let mut peaks = Vec::new();
            let mut buffer = [0_u8; 3840];
            loop {
                check_cancel(&flag)?;
                let mut count = 0;
                while count < buffer.len() {
                    let n = output.read(&mut buffer[count..])?;
                    if n == 0 {
                        break;
                    }
                    count += n;
                }
                if count == 0 {
                    break;
                }
                let peak = buffer[..count]
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()).abs())
                    .filter(|v| v.is_finite())
                    .fold(0.0, f32::max);
                peaks.push(peak);
                if peaks.len() > 10_000_000 {
                    return Err("Recording waveform exceeds the cache limit".into());
                }
            }
            Ok(peaks)
        });
        loop {
            if cancel.load(Ordering::Acquire) {
                let _ = child.kill();
            }
            if child.try_wait()?.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = child.wait()?;
        let peaks = result.join().map_err(|_| "Waveform worker failed")??;
        check_cancel(cancel)?;
        if !status.success() {
            return Err("Waveform decoding failed".into());
        }
        let temporary = cache.with_extension(format!("{}.tmp", project_store::unique_id()));
        let cached = (|| -> Result<()> {
            let mut file = File::create(&temporary)?;
            file.write_all(b"SVEPEAK1")?;
            for peak in &peaks {
                file.write_all(&peak.to_le_bytes())?;
            }
            drop(file);
            platform_windows::replace_file(&temporary, &cache)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(temporary);
        cached?;
        Ok(peaks)
    }
}
pub fn file_hash(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn run(
    executable: &Path,
    arguments: &[String],
    cancel: &Cancel,
    mut progress: impl FnMut(&str),
) -> Result<Vec<u8>> {
    use std::os::windows::process::CommandExt;
    check_cancel(cancel)?;
    let mut child = Command::new(executable)
        .creation_flags(0x08000000)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let overflow = Arc::new(AtomicBool::new(false));
    let output_overflow = overflow.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel::<String>(64);
    let out = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        let mut line = Vec::new();
        loop {
            let count = stdout.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if bytes.len() + count > 8 * 1024 * 1024 {
                output_overflow.store(true, Ordering::Release);
                return Err(std::io::Error::other("Tool output exceeds 8 MiB"));
            }
            bytes.extend_from_slice(&buffer[..count]);
            for byte in &buffer[..count] {
                if *byte == b'\n' {
                    let _ = tx.try_send(String::from_utf8_lossy(&line).into_owned());
                    line.clear();
                } else if line.len() < 4096 {
                    line.push(*byte);
                }
            }
        }
        Ok(bytes)
    });
    let errors = std::thread::spawn(move || {
        let mut data = Vec::new();
        let mut buffer = [0_u8; 8192];
        while let Ok(count) = stderr.read(&mut buffer) {
            if count == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..count]);
            if data.len() > 65536 {
                data.drain(..data.len() - 65536);
            }
        }
        data
    });
    loop {
        while let Ok(line) = rx.try_recv() {
            progress(&line);
        }
        if cancel.load(Ordering::Acquire) || overflow.load(Ordering::Acquire) {
            let _ = child.kill();
        }
        if child.try_wait()?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = child.wait()?;
    while let Ok(line) = rx.try_recv() {
        progress(&line);
    }
    let bytes = out.join().map_err(|_| "Native stdout reader failed")??;
    let errors = errors.join().map_err(|_| "Native stderr reader failed")?;
    check_cancel(cancel)?;
    if !status.success() {
        return Err(format!("Video tool failed: {}", String::from_utf8_lossy(&errors)).into());
    }
    Ok(bytes)
}
pub fn parse_probe(bytes: &[u8], path: String, id: SourceId) -> Result<MediaSource> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let streams = value["streams"].as_array().ok_or("No media streams")?;
    let video = streams
        .iter()
        .find(|s| {
            s["codec_type"] == "video" && s["disposition"]["attached_pic"].as_i64() != Some(1)
        })
        .ok_or("The recording has no video stream")?;
    let duration: f64 = value["format"]["duration"]
        .as_str()
        .ok_or("Recording duration is missing")?
        .parse()?;
    if !duration.is_finite() || duration <= 0.0 || duration > editor_core::MAX_TIMELINE_SECONDS {
        return Err("Unsupported recording duration".into());
    }
    let width = video["width"].as_u64().ok_or("Missing video width")? as u32;
    let height = video["height"].as_u64().ok_or("Missing video height")? as u32;
    if width == 0 || height == 0 || width > 32768 || height > 32768 {
        return Err("Unsupported recording size".into());
    }
    let rate = video["r_frame_rate"]
        .as_str()
        .and_then(|s| FrameRate::parse(s).ok())
        .or_else(|| {
            video["avg_frame_rate"]
                .as_str()
                .and_then(|s| FrameRate::parse(s).ok())
        })
        .unwrap_or(FrameRate::new(30, 1)?);
    let sar = video["sample_aspect_ratio"]
        .as_str()
        .and_then(|s| s.split_once(':'))
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .filter(|s| s.is_finite() && *s > 0.0 && *s <= 100.0)
        .unwrap_or(1.0);
    let rotation = video["side_data_list"]
        .as_array()
        .and_then(|a| a.iter().find_map(|v| v["rotation"].as_f64()))
        .filter(|v| v.is_finite())
        .unwrap_or(0.0);
    let rotation = (-rotation).rem_euclid(360.0);
    let angle = rotation.to_radians();
    let (w, h) = (f64::from(width) * sar, f64::from(height));
    let (dw, dh) = (
        w * angle.cos().abs() + h * angle.sin().abs(),
        w * angle.sin().abs() + h * angle.cos().abs(),
    );
    let field = |name: &str| video[name].as_str().unwrap_or("unknown").to_owned();
    Ok(MediaSource {
        id,
        path,
        duration,
        width,
        height,
        display_width: dw,
        display_height: dh,
        rotation,
        sample_aspect_ratio: sar,
        rate,
        codec: field("codec_name"),
        video_stream: video["index"]
            .as_u64()
            .ok_or("Missing video stream identity")?
            .try_into()?,
        audio: streams
            .iter()
            .filter(|s| s["codec_type"] == "audio")
            .map(|s| AudioStream {
                index: s["index"].as_u64().unwrap_or(1) as u32,
                label: s["tags"]["title"].as_str().unwrap_or("Audio").to_owned(),
                channels: s["channels"].as_u64().unwrap_or(2) as u32,
            })
            .collect(),
        transfer: field("color_transfer"),
        primaries: field("color_primaries"),
        matrix: field("color_space"),
        range: field("color_range"),
        pixel_format: field("pix_fmt"),
    })
}

pub fn color_transform(source: &MediaSource, profile: ColorProfile) -> String {
    let rgb = source.pixel_format.starts_with("gbr")
        || source.pixel_format.starts_with("rgb")
        || source.pixel_format.starts_with("bgr");
    let trc = match source.transfer.as_str() {
        "smpte2084" => "smpte2084",
        "arib-std-b67" => "arib-std-b67",
        "iec61966-2-1" => "iec61966-2-1",
        "linear" => "linear",
        "smpte170m" => "smpte170m",
        "smpte240m" => "smpte240m",
        "gamma22" => "bt470m",
        "gamma28" => "bt470bg",
        "bt2020-10" => "bt2020-10",
        "bt2020-12" => "bt2020-12",
        "unknown" | "unspecified" if rgb => "iec61966-2-1",
        _ => "bt709",
    };
    let prim = match source.primaries.as_str() {
        "bt709" => "bt709",
        "bt2020" => "bt2020",
        "smpte170m" => "smpte170m",
        "bt470bg" => "bt470bg",
        "smpte432" => "smpte432",
        "smpte431" => "smpte431",
        _ if source.hdr() => "bt2020",
        _ => "bt709",
    };
    let matrix = if rgb {
        "gbr"
    } else {
        match source.matrix.as_str() {
            "bt709" => "bt709",
            "gbr" => "gbr",
            "bt2020nc" => "bt2020nc",
            "bt2020c" => "bt2020c",
            "smpte170m" | "bt470bg" => "smpte170m",
            _ => {
                if source.hdr() {
                    "bt2020nc"
                } else {
                    "bt709"
                }
            }
        }
    };
    let range = if source.range == "pc" || rgb && source.range != "tv" {
        "full"
    } else {
        "limited"
    };
    let linear = format!(
        "zscale=pin={prim}:tin={trc}:min={matrix}:rin={range}:p={prim}:t=linear:m=gbr:r=full:npl=100,format=gbrpf32le"
    );
    match profile {
        ColorProfile::Sdr if source.hdr() => format!(
            "{linear},tonemap=tonemap=mobius:desat=0,zscale=pin={prim}:tin=linear:min=gbr:rin=full:p=bt709:t=bt709:m=bt709:r=limited,format=yuv420p"
        ),
        ColorProfile::Sdr => format!(
            "zscale=pin={prim}:tin={trc}:min={matrix}:rin={range}:p=bt709:t=bt709:m=bt709:r=limited,format=yuv420p"
        ),
        ColorProfile::Hdr10 => format!(
            "{linear},zscale=pin={prim}:tin=linear:min=gbr:rin=full:p=bt2020:t=smpte2084:m=bt2020nc:r=limited,format=yuv420p10le,sidedata=mode=delete:type=MASTERING_DISPLAY_METADATA,sidedata=mode=delete:type=CONTENT_LIGHT_LEVEL"
        ),
    }
}
pub fn video_filters(
    source: &MediaSource,
    adjustment: &ClipSettings,
    output: &OutputSettings,
    frame_rate: bool,
) -> String {
    let mut filters = vec!["scale=-2:ceil(ih/2)*2:reset_sar=1".into()];
    let rotation = if (source.rotation - source.rotation.round()).abs() < 0.00001 {
        source.rotation.round() as i32
    } else {
        -1
    };
    match rotation {
        0 => {}
        90 => filters.push("transpose=clock".into()),
        180 => filters.extend(["hflip".into(), "vflip".into()]),
        270 => filters.push("transpose=cclock".into()),
        _ => {
            let angle = source.rotation.to_radians();
            filters.push(format!(
                "rotate={angle}:ow=ceil(rotw({angle})/2)*2:oh=ceil(roth({angle})/2)*2:c=black"
            ));
        }
    }
    filters.push("sidedata=mode=delete:type=DISPLAYMATRIX".into());
    filters.push(color_transform(source, output.profile));
    match adjustment.rotation {
        90 => filters.push("transpose=clock".into()),
        180 => filters.extend(["hflip".into(), "vflip".into()]),
        270 => filters.push("transpose=cclock".into()),
        _ => {}
    }
    if adjustment.framing == Framing::Crop {
        let [x, y, w, h] = adjustment.crop;
        filters.push(format!(
            "crop=max(2\\,trunc(iw*{w}/2)*2):max(2\\,trunc(ih*{h}/2)*2):iw*{x}:ih*{y}"
        ));
    }
    let (w, h) = (output.width, output.height);
    if adjustment.framing == Framing::Fit {
        filters.push(format!("scale={w}:{h}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"));
    } else {
        filters.push(format!(
            "scale={w}:{h}:force_original_aspect_ratio=increase:force_divisible_by=2,crop={w}:{h}"
        ));
    }
    filters.push("setsar=1".into());
    if frame_rate {
        filters.push(format!("fps={}", output.rate.rational()));
    }
    filters.join(",")
}
