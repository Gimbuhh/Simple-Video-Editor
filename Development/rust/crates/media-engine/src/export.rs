use super::*;
use project_store::{Codec, unique_id};
#[derive(Clone, Debug)]
pub struct ExportProgress {
    pub fraction: f64,
    pub message: String,
}
#[derive(Clone, Debug)]
pub struct Segment {
    pub clip: Option<usize>,
    pub frames: u64,
    pub duration: f64,
}
pub fn segments(document: &SavedProject) -> Result<Vec<Segment>> {
    document.validate()?;
    if document.clips.is_empty() {
        return Err("Add clips before exporting".into());
    }
    let fps = document.metadata.output.rate.fps();
    let mut frames = 0_u64;
    let mut result = Vec::new();
    for (index, clip) in document.clips.iter().enumerate() {
        let start = (clip.timeline_start * fps).round_ties_even() as u64;
        if start > frames {
            let gap = start - frames;
            result.push(Segment {
                clip: None,
                frames: gap,
                duration: gap as f64 / fps,
            });
            frames += gap;
        }
        let count = (clip.duration() * fps).round_ties_even().max(1.0) as u64;
        result.push(Segment {
            clip: Some(index),
            frames: count,
            duration: count as f64 / fps,
        });
        frames = frames.checked_add(count).ok_or("Export is too long")?;
    }
    Ok(result)
}
pub fn encoder_arguments(output: &OutputSettings) -> Result<Vec<String>> {
    if output.profile == ColorProfile::Hdr10 && output.codec == Codec::H264 {
        return Err("HDR10 requires AV1 or HEVC".into());
    }
    let quality = output.quality.to_string();
    let codec = match (output.codec, output.gpu) {
        (Codec::Av1, true) => "av1_nvenc",
        (Codec::H264, true) => "h264_nvenc",
        (Codec::Hevc, true) => "hevc_nvenc",
        (Codec::Av1, false) => "libsvtav1",
        (Codec::H264, false) => "libx264",
        (Codec::Hevc, false) => "libx265",
    };
    let mut result = vec!["-c:v".into(), codec.into()];
    if output.gpu {
        result.extend(
            [
                "-preset", "p6", "-tune", "hq", "-rc", "vbr", "-cq", &quality, "-b:v", "0",
            ]
            .map(str::to_owned),
        );
    } else {
        result.extend(
            [
                "-preset",
                if output.codec == Codec::Av1 {
                    "8"
                } else {
                    "medium"
                },
                "-crf",
                &quality,
            ]
            .map(str::to_owned),
        );
        if output.codec == Codec::Av1 {
            result.extend(["-svtav1-params", "lp=4"].map(str::to_owned));
        }
        if output.codec == Codec::Hevc {
            result.extend(["-x265-params", "pools=4:log-level=error"].map(str::to_owned));
        }
    }
    result.extend([
        "-pix_fmt".into(),
        if output.profile == ColorProfile::Hdr10 {
            if output.gpu { "p010le" } else { "yuv420p10le" }
        } else {
            "yuv420p"
        }
        .into(),
    ]);
    Ok(result)
}
impl Tools {
    pub fn supports_gpu(&self, output: &OutputSettings, cancel: &Cancel) -> Result<bool> {
        check_cancel(cancel)?;
        let mut args = [
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=s=256x144:r=30:d=0.1",
            "-frames:v",
            "1",
        ]
        .map(str::to_owned)
        .to_vec();
        args.extend(encoder_arguments(output)?);
        args.extend(["-f", "null", "-"].map(str::to_owned));
        let supported = run(&self.ffmpeg, &args, cancel, |_| {}).is_ok();
        check_cancel(cancel)?;
        Ok(supported)
    }
    pub fn export_adaptive(
        &self,
        document: &SavedProject,
        destination: &Path,
        cancel: &Cancel,
        mut progress: impl FnMut(ExportProgress),
    ) -> Result<bool> {
        let mut snapshot = document.clone();
        if snapshot.metadata.output.gpu && !self.supports_gpu(&snapshot.metadata.output, cancel)? {
            snapshot.metadata.output.gpu = false;
            progress(ExportProgress {
                fraction: 0.0,
                message: "NVIDIA encoding is unavailable · using the CPU encoder".into(),
            });
        }
        self.export(&snapshot, destination, cancel, progress)?;
        Ok(snapshot.metadata.output.gpu)
    }
    pub fn export(
        &self,
        document: &SavedProject,
        destination: &Path,
        cancel: &Cancel,
        mut progress: impl FnMut(ExportProgress),
    ) -> Result<()> {
        let planned = segments(document)?;
        let output = &document.metadata.output;
        let destination = if destination.is_absolute() {
            destination.to_path_buf()
        } else {
            std::env::current_dir()?.join(destination)
        };
        for source in &document.metadata.sources {
            let authorized = media_path::RecordingPath::authorize(Path::new(&source.path))?;
            if platform_windows::ordinal_ignore_case(
                &destination.to_string_lossy(),
                &authorized.as_path().to_string_lossy(),
            ) || destination.exists()
                && platform_windows::same_file(&destination, authorized.as_path())?
            {
                return Err("Choose an output different from every source recording".into());
            }
        }
        let total: f64 = planned.iter().map(|s| s.duration).sum();
        let parent = destination.parent().ok_or("Invalid destination")?;
        std::fs::create_dir_all(parent)?;
        let work = parent.join(format!(".sve-export-{}", unique_id()));
        std::fs::create_dir(&work)?;
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for attempt in 0..5 {
                    if std::fs::remove_dir_all(&self.0).is_ok() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100 * (attempt + 1)));
                }
            }
        }
        let _cleanup = Cleanup(work.clone());
        let mut completed = 0.0;
        for (index, segment) in planned.iter().enumerate() {
            check_cancel(cancel)?;
            let mut args = ["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]
                .map(str::to_owned)
                .to_vec();
            let (source, adjustment, clip) = if let Some(position) = segment.clip {
                let clip = &document.clips[position];
                let source = document
                    .metadata
                    .sources
                    .iter()
                    .find(|s| s.id == clip.source)
                    .ok_or("Missing source")?
                    .clone();
                let path = media_path::RecordingPath::authorize(Path::new(&source.path))?;
                args.extend([
                    "-ss".into(),
                    format!("{:.9}", clip.source_in),
                    "-i".into(),
                    path.as_path().to_string_lossy().into_owned(),
                ]);
                (
                    source,
                    document
                        .metadata
                        .adjustments
                        .get(&clip.id)
                        .cloned()
                        .unwrap_or_default(),
                    Some(clip),
                )
            } else {
                args.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    format!(
                        "color=c=black:s={}x{}:r={}",
                        output.width,
                        output.height,
                        output.rate.rational()
                    ),
                ]);
                (
                    MediaSource {
                        id: SourceId(0),
                        path: String::new(),
                        duration: segment.duration,
                        width: output.width,
                        height: output.height,
                        display_width: f64::from(output.width),
                        display_height: f64::from(output.height),
                        rate: output.rate,
                        codec: "rawvideo".into(),
                        audio: Vec::new(),
                        transfer: "bt709".into(),
                        primaries: "bt709".into(),
                        matrix: "bt709".into(),
                        range: "tv".into(),
                        pixel_format: "yuv420p".into(),
                    },
                    ClipSettings::default(),
                    None,
                )
            };
            let audio = adjustment
                .stream
                .or_else(|| source.audio.first().map(|s| s.index));
            if audio.is_none() {
                args.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(str::to_owned));
            }
            let mut af = format!(
                "aresample=48000:async=1:first_pts=0,apad,atrim=duration={:.9},asetpts=PTS-STARTPTS,volume={}dB",
                segment.duration,
                if adjustment.muted {
                    -120.0
                } else {
                    adjustment.gain_db
                }
            );
            if adjustment.fade_in > 0.0 {
                af.push_str(&format!(
                    ",afade=t=in:st=0:d={}",
                    adjustment.fade_in.min(segment.duration)
                ));
            }
            if adjustment.fade_out > 0.0 {
                let fade = adjustment.fade_out.min(segment.duration);
                af.push_str(&format!(
                    ",afade=t=out:st={}:d={fade}",
                    segment.duration - fade
                ));
            }
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                audio.map(|i| format!("0:{i}")).unwrap_or("1:a:0".into()),
                "-t".into(),
                format!("{:.9}", segment.duration),
                "-frames:v".into(),
                segment.frames.to_string(),
                "-vf".into(),
                video_filters(&source, &adjustment, output, true),
                "-af".into(),
                af,
            ]);
            args.extend(encoder_arguments(output)?);
            let hdr = output.profile == ColorProfile::Hdr10;
            args.extend(
                [
                    "-map_metadata",
                    "-1",
                    "-color_primaries",
                    if hdr { "bt2020" } else { "bt709" },
                    "-color_trc",
                    if hdr { "smpte2084" } else { "bt709" },
                    "-colorspace",
                    if hdr { "bt2020nc" } else { "bt709" },
                    "-color_range",
                    "tv",
                    "-c:a",
                    "pcm_s16le",
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                    "-progress",
                    "pipe:1",
                    "-nostats",
                ]
                .map(str::to_owned),
            );
            args.push(
                work.join(format!("part-{index:05}.mkv"))
                    .to_string_lossy()
                    .into_owned(),
            );
            let message = format!(
                "Encoding section {} of {}{}",
                index + 1,
                planned.len(),
                if clip.is_none() {
                    " · empty timeline gap"
                } else {
                    ""
                }
            );
            run(&self.ffmpeg, &args, cancel, |line| {
                if let Some(value) = line
                    .strip_prefix("out_time_us=")
                    .and_then(|v| v.parse::<f64>().ok())
                {
                    progress(ExportProgress {
                        fraction: ((completed + (value / 1e6).min(segment.duration)) / total
                            * 0.92)
                            .clamp(0.0, 0.92),
                        message: message.clone(),
                    });
                }
            })?;
            completed += segment.duration;
        }
        let list = work.join("join.ffconcat");
        let mut file = std::fs::File::create(&list)?;
        writeln!(file, "ffconcat version 1.0")?;
        for (index, segment) in planned.iter().enumerate() {
            writeln!(
                file,
                "file 'part-{index:05}.mkv'\nduration {:.9}",
                segment.duration
            )?;
        }
        drop(file);
        let temporary = work.join("output.mp4");
        let mut args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-f",
            "concat",
            "-safe",
            "1",
            "-i",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(list.to_string_lossy().into_owned());
        args.extend(
            [
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-af",
                "aresample=48000:async=1:first_pts=0",
                "-movflags",
                "+faststart",
                "-progress",
                "pipe:1",
                "-nostats",
            ]
            .map(str::to_owned),
        );
        if output.codec == Codec::Hevc {
            args.extend(["-tag:v", "hvc1"].map(str::to_owned));
        }
        args.push(temporary.to_string_lossy().into_owned());
        progress(ExportProgress {
            fraction: 0.93,
            message: "Combining sections and finishing audio".into(),
        });
        run(&self.ffmpeg, &args, cancel, |_| {})?;
        let bytes = run(
            &self.ffprobe,
            &[
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ]
            .map(str::to_owned)
            .into_iter()
            .chain([temporary.to_string_lossy().into_owned()])
            .collect::<Vec<_>>(),
            cancel,
            |_| {},
        )?;
        let verified = parse_probe(
            &bytes,
            temporary.to_string_lossy().into_owned(),
            SourceId(0),
        )?;
        if (verified.duration - total).abs() > 0.2_f64.max(document.clips.len() as f64 * 0.04) {
            return Err("Encoded duration does not match the edited sequence".into());
        }
        if output.profile == ColorProfile::Hdr10
            && (!verified.pixel_format.contains("10")
                || verified.transfer != "smpte2084"
                || verified.primaries != "bt2020"
                || verified.matrix != "bt2020nc")
        {
            return Err("HDR export failed pixel-format or color verification".into());
        }
        run(
            &self.ffmpeg,
            &["-v", "error", "-i"]
                .map(str::to_owned)
                .into_iter()
                .chain([temporary.to_string_lossy().into_owned()])
                .chain(["-f", "null", "-"].map(str::to_owned))
                .collect::<Vec<_>>(),
            cancel,
            |_| {},
        )?;
        check_cancel(cancel)?;
        platform_windows::replace_file(&temporary, &destination)?;
        progress(ExportProgress {
            fraction: 1.0,
            message: "Export complete".into(),
        });
        Ok(())
    }
}
