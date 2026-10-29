use std::{
    path::Path,
    process::{Command, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

/// Auto uses the scene-aware processor when the verified native bundle can
/// initialize it. CPU remains available without Vulkan, including headless CI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SdrToneMapping {
    #[default]
    Auto,
    Adaptive,
    Cpu,
}

// Disable temporal peak smoothing so a sought frame, thumbnail and exported
// frame use the same brightness without depending on previously decoded frames.
// Ignore the brightest 0.01% when estimating scene exposure: captured HDR
// cursors can reach 5,000 nits and should not dim otherwise normal UI white.
pub(crate) const ADAPTIVE_SDR: &str = "libplacebo=colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:format=yuv420p:tonemapping=auto:peak_detect=1:percentile=99.99:smoothing_period=0:extra_opts=allow_delayed_peak=no";

pub(crate) fn available(ffmpeg: &Path) -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| probe(ffmpeg, Duration::from_secs(8)))
}

fn probe(ffmpeg: &Path, timeout: Duration) -> bool {
    use std::os::windows::process::CommandExt;
    let Ok(mut child) = Command::new(ffmpeg)
        .creation_flags(0x08000000)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=s=16x16:r=25:d=0.04",
            "-vf",
            &format!("format=yuv420p10le,setparams=range=limited:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc,{ADAPTIVE_SDR}"),
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_native_processor_retains_cpu_support() {
        assert!(!probe(
            Path::new("missing-sve-processor.exe"),
            Duration::from_millis(1)
        ));
    }
}
