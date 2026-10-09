using System.Globalization;
using System.IO;

namespace SimpleVideoEditor.Services;

public sealed class ExportService
{
    private static string N(double value) => value.ToString("0.#########", CultureInfo.InvariantCulture);
    public async Task ExportAsync(IReadOnlyList<MediaClip> clips, string destination, ExportSettings settings, IProgress<ExportProgress>? progress, CancellationToken token)
    {
        if (clips.Count == 0) throw new InvalidOperationException("Add at least one clip before exporting.");
        destination = Path.GetFullPath(destination);
        if (clips.Any(c => string.Equals(Path.GetFullPath(c.Path), destination, StringComparison.OrdinalIgnoreCase))) throw new InvalidOperationException("Choose an output filename different from every source recording.");
        if (settings.Width < 2 || settings.Height < 2 || settings.Width % 2 != 0 || settings.Height % 2 != 0 || settings.FrameRate <= 0 || settings.FrameRate > 240 || !double.IsFinite(settings.FrameRate)) throw new InvalidOperationException("Invalid output resolution or frame rate.");
        foreach (var clip in clips)
        {
            ProjectStore.ValidateTrim(clip);
            if (!File.Exists(clip.Path)) throw new FileNotFoundException("A source recording is missing.", clip.Path);
            if (clip.IsHdr) throw new InvalidOperationException("HDR recordings need a color conversion workflow. This version exports SDR recordings only.");
        }
        Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
        var work = Path.Combine(Path.GetDirectoryName(destination)!, ".sve-export-" + Guid.NewGuid().ToString("N"));
        var temporary = Path.Combine(work, "output.mp4");
        var segments = new List<(MediaClip? Clip, double Duration)>();
        double sourceEnd = 0; long outputFrames = 0;
        foreach (var clip in clips)
        {
            var start = clip.TimelineStart ?? sourceEnd;
            if (!double.IsFinite(start) || start < sourceEnd - .00001) throw new InvalidOperationException("Timeline clips must not overlap.");
            var gapFrames = clip.TimelineStart == null ? 0 : Math.Max(0, (long)Math.Round(start * settings.FrameRate) - outputFrames);
            if (gapFrames > 0) { segments.Add((null, gapFrames / settings.FrameRate)); outputFrames += gapFrames; }
            var frames = Math.Max(1, (long)Math.Round(clip.KeptDuration * settings.FrameRate));
            segments.Add((clip, frames / settings.FrameRate)); outputFrames += frames;
            sourceEnd = start + clip.KeptDuration;
        }
        var durations = segments.Select(s => s.Duration).ToArray();
        var total = durations.Sum();
        double completed = 0;
        Directory.CreateDirectory(work);
        try
        {
            for (var index = 0; index < segments.Count; index++)
            {
                token.ThrowIfCancellationRequested();
                var clip = segments[index].Clip;
                var duration = durations[index];
                var args = new List<string> { "-hide_banner", "-loglevel", "error", "-nostdin", "-y" };
                if (clip == null) args.AddRange(["-f", "lavfi", "-i", $"color=c=black:s={settings.Width}x{settings.Height}:r={N(settings.FrameRate)}"]);
                else args.AddRange(["-ss", N(clip.Start), "-i", clip.Path]);
                if (clip?.HasAudio != true) args.AddRange(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]);
                args.AddRange(["-map", "0:v:0", "-map", clip?.HasAudio == true ? "0:a:0" : "1:a:0", "-t", N(duration), "-vf", $"scale={settings.Width}:{settings.Height}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={settings.Width}:{settings.Height}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={N(settings.FrameRate)},format=yuv420p", "-af", $"aresample=48000:async=1:first_pts=0,apad,atrim=duration={N(duration)},asetpts=PTS-STARTPTS"]);
                args.AddRange(EncoderArguments(settings));
                args.AddRange(["-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-c:a", "pcm_s16le", "-ar", "48000", "-ac", "2", "-progress", "pipe:1", "-nostats", Path.Combine(work, $"part-{index:00000}.mkv")]);
                var baseTime = completed;
                var label = clip == null ? "Empty timeline gap" : clip.Name;
                progress?.Report(new(completed / total * .92, $"Encoding part {index + 1} of {segments.Count} · {label}"));
                await MediaTools.RunAsync("ffmpeg", args, token, line =>
                {
                    if (line.StartsWith("out_time_us=") && long.TryParse(line[12..], out var microseconds)) progress?.Report(new(Math.Clamp((baseTime + Math.Min(duration, microseconds / 1e6)) / total * .92, 0, .92), $"Encoding part {index + 1} of {segments.Count} · {label}"));
                });
                completed += duration;
            }
            var list = Path.Combine(work, "join.ffconcat");
            await File.WriteAllLinesAsync(list, new[] { "ffconcat version 1.0" }.Concat(Enumerable.Range(0, segments.Count).SelectMany(i => new[] { $"file 'part-{i:00000}.mkv'", $"duration {N(durations[i])}" })), token);
            progress?.Report(new(.93, "Combining sections and finishing audio…"));
            await MediaTools.RunAsync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-f", "concat", "-safe", "1", "-i", list, "-map", "0:v:0", "-map", "0:a:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-af", "aresample=48000:async=1:first_pts=0", "-movflags", "+faststart", "-progress", "pipe:1", "-nostats", temporary], token, line =>
            {
                if (line.StartsWith("out_time_us=") && long.TryParse(line[12..], out var microseconds)) progress?.Report(new(.93 + Math.Clamp(microseconds / 1e6 / total, 0, 1) * .06, "Combining sections and finishing audio…"));
            });
            var result = await MediaTools.ProbeAsync(temporary, token);
            if (Math.Abs(result.Duration - total) > Math.Max(.2, clips.Count * .04)) throw new InvalidOperationException("The output duration did not match the edited sequence. The export was not saved.");
            token.ThrowIfCancellationRequested();
            File.Move(temporary, destination, true);
            progress?.Report(new(1, "Export complete"));
        }
        finally
        {
            // This directory is a freshly created, unique child of the output folder.
            try { Directory.Delete(work, true); } catch (IOException) { } catch (UnauthorizedAccessException) { }
        }
    }

    public static string[] EncoderArguments(ExportSettings settings)
    {
        var quality = settings.Quality switch { ExportQuality.High => 22, ExportQuality.Balanced => 28, _ => 34 };
        if (settings.UseGpu) return ["-c:v", settings.Codec == ExportCodec.Av1 ? "av1_nvenc" : "h264_nvenc", "-preset", "p6", "-tune", "hq", "-rc", "vbr", "-cq", quality.ToString(), "-b:v", "0"];
        return settings.Codec == ExportCodec.Av1 ? ["-c:v", "libsvtav1", "-preset", "8", "-crf", quality.ToString(), "-svtav1-params", "lp=4"] : ["-c:v", "libx264", "-preset", "medium", "-crf", quality.ToString()];
    }
}
