using System.IO;
using System.Text.Json;
using SimpleVideoEditor;
using SimpleVideoEditor.Services;

var useGpu = args.Contains("--gpu");
var clips = args.Where(a => a != "--ui" && a != "--gpu").ToArray();
if (clips.Any(a => a.StartsWith("--") || !File.Exists(a))) { Console.Error.WriteLine("Unknown option or missing test recording."); Environment.ExitCode = 1; return; }
if (args.Contains("--ui")) { Environment.ExitCode = UiVerification.Run(clips); return; }

var root = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../artifacts/verification"));
Directory.CreateDirectory(root);
var checks = new List<string>();
void Assert(bool condition, string description) { if (!condition) throw new Exception(description); checks.Add(description); Console.WriteLine("PASS " + description); }
async Task ExpectFailure(Func<Task> task, string description) { try { await task(); } catch { Assert(true, description); return; } throw new Exception(description + " did not fail"); }

try
{
    Assert(Timecode.TryParse("01:02:03.456", out var seconds) && Math.Abs(seconds - 3723.456) < .000001, "Timestamp parsing preserves milliseconds");
    Assert(!Timecode.TryParse("00:61", out _) && !Timecode.TryParse("NaN", out _) && !Timecode.TryParse("-1", out _), "Invalid timestamps are rejected");
    Assert(Timecode.Format(59.9996) == "00:01:00.000", "Timestamp rounding carries into the next minute");
    var sourceA = Path.Combine(root, "source A's clip.mp4");
    var sourceB = Path.Combine(root, "source B.mp4");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=60:duration=4", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=4", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "10", "-g", "240", "-c:a", "aac", "-shortest", sourceA]);
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-f", "lavfi", "-i", "color=blue:size=640x360:rate=30:duration=3", "-c:v", "libx264", "-preset", "ultrafast", sourceB]);
    var a = await MediaTools.ProbeAsync(sourceA); var b = await MediaTools.ProbeAsync(sourceB);
    Assert(a.HasAudio && !b.HasAudio && a.FrameRate == 60 && b.Width == 640, "Probe handles audio and video-only inputs with different formats");
    ProjectStore.DataDirectory = Path.Combine(root, "waveform-tests", Guid.NewGuid().ToString("N"));
    var waveformSource = Path.Combine(root, "waveform-pattern.mkv");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-f", "lavfi", "-i", "color=black:size=64x64:rate=30:duration=3", "-itsoffset", "0.4", "-f", "lavfi", "-i", "aevalsrc=if(between(t\\,0.5\\,1)\\,0.5*sin(2*PI*440*t)\\,0)|if(between(t\\,0.5\\,1)\\,-0.5*sin(2*PI*440*t)\\,0):s=48000:d=2", "-f", "lavfi", "-i", "sine=frequency=800:sample_rate=48000:duration=3", "-map", "0:v", "-map", "1:a", "-map", "2:a", "-c:v", "libx264", "-preset", "ultrafast", "-c:a", "pcm_f32le", waveformSource]);
    var patterned = await MediaTools.ProbeAsync(waveformSource);
    var wave = (await AudioWaveforms.LoadAsync(patterned))!;
    Assert(wave.PeakBetween(0, .8) == 0 && wave.PeakBetween(1.5, 2.9) == 0 && wave.PeakBetween(.95, 1.2) > .49f,
        "Waveform preserves delayed audio, real silence, and opposite-phase stereo peaks from the first audio track");
    Assert(wave.PeakBetween(.95, 1.2) < .51f && Math.Abs(wave.BinDuration - .01) < .000001,
        "Waveform stores measured audio amplitude at ten-millisecond resolution");
    patterned.Waveform.Complete(wave);
    Assert(ReferenceEquals(patterned.Waveform, patterned.Clone(true).Waveform) && wave.PeakBetween(-2, -.1) == 0 && wave.PeakBetween(4, 5) == 0,
        "Split and undo copies share source waveform data; out-of-source ranges are silent");
    Assert(await AudioWaveforms.LoadAsync(b) == null, "Video-only recordings do not generate fictitious audio peaks");
    var cache = Directory.GetFiles(Path.Combine(ProjectStore.DataDirectory, "waveforms"), "*.peaks").Single();
    var written = File.GetLastWriteTimeUtc(cache);
    var cachedWave = (await AudioWaveforms.LoadAsync(patterned))!;
    Assert(cachedWave.Peaks.Span.SequenceEqual(wave.Peaks.Span) && File.GetLastWriteTimeUtc(cache) == written,
        "Reopening a source uses the existing waveform cache without rewriting it");
    await File.WriteAllTextAsync(cache, "incomplete");
    var rebuilt = (await AudioWaveforms.LoadAsync(patterned))!;
    Assert(rebuilt.Peaks.Span.SequenceEqual(wave.Peaks.Span), "A corrupt waveform cache is rebuilt from the audio");
    var originalTimestamp = File.GetLastWriteTimeUtc(waveformSource);
    try
    {
        File.SetLastWriteTimeUtc(waveformSource, originalTimestamp.AddSeconds(2));
        await AudioWaveforms.LoadAsync(patterned);
        Assert(Directory.GetFiles(Path.Combine(ProjectStore.DataDirectory, "waveforms"), "*.peaks").Length == 2,
            "Replacing or changing a recording invalidates its waveform cache");
    }
    finally { File.SetLastWriteTimeUtc(waveformSource, originalTimestamp); }
    using (var canceledWave = new CancellationTokenSource())
    {
        canceledWave.Cancel();
        await ExpectFailure(() => AudioWaveforms.LoadAsync(patterned, canceledWave.Token), "Canceled waveform work exits without modifying cached data");
    }
    Assert(!Directory.GetFiles(Path.Combine(ProjectStore.DataDirectory, "waveforms"), "*.tmp").Any(), "Waveform generation leaves no incomplete cache files");
    a.Start = 1.15; a.End = 2.15; b.Start = .5; b.End = 1.5;
    var projectPath = Path.Combine(root, "roundtrip.sveproject");
    await ProjectStore.SaveAsync(projectPath, ProjectStore.Capture([b, a]));
    var project = await ProjectStore.ReadAsync(projectPath);
    Assert(project.Clips[0].Path == b.Path && project.Clips[1].Start == a.Start && project.Clips[1].End == a.End, "Project roundtrip preserves order, filenames, and trim boundaries");
    var movedA = Path.Combine(root, "missing-folder", "a.mp4"); var movedB = Path.Combine(root, "missing-folder", "b.mp4");
    var missingProject = new ProjectDocument(1, [new(movedA, .5, 1.5, 1), new(movedA, 2, 3, 4)], [movedA, movedB]);
    var linkedProject = ProjectStore.Relink(missingProject, new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) { [movedA] = sourceA, [movedB] = sourceB });
    Assert(linkedProject.Clips.All(c => c.Path == sourceA) && linkedProject.Clips[0].Start == .5 && linkedProject.Clips[1].TimelineStart == 4 && linkedProject.Sources!.SequenceEqual(new[] { sourceA, sourceB }),
        "Relinking changes recording paths while preserving repeated cuts, gaps, and unused library recordings");
    Assert(missingProject.Clips[0].Path == movedA && missingProject.Sources![1] == movedB, "Relinking stages a new document without changing the original project");
    await ExpectFailure(() => { ProjectStore.Relink(missingProject, new Dictionary<string, string> { [movedA] = sourceA }); return Task.CompletedTask; }, "Relinking refuses an unresolved missing recording");
    var previousDataDirectory = ProjectStore.DataDirectory;
    ProjectStore.DataDirectory = Path.Combine(root, "cache-tests", Guid.NewGuid().ToString("N"));
    var thumbnailFolder = Path.Combine(ProjectStore.DataDirectory, "thumbnails"); var waveformFolder = Path.Combine(ProjectStore.DataDirectory, "waveforms");
    Directory.CreateDirectory(thumbnailFolder); Directory.CreateDirectory(waveformFolder);
    var cachedThumbnail = Path.Combine(thumbnailFolder, new string('A', 64) + ".jpg");
    var cachedPeak = Path.Combine(waveformFolder, new string('B', 64) + ".peaks");
    await File.WriteAllBytesAsync(cachedThumbnail, new byte[1024]); await File.WriteAllBytesAsync(cachedPeak, new byte[2048]);
    var cacheSentinels = new[] { Path.Combine(ProjectStore.DataDirectory, "project.sveproject"), Path.Combine(ProjectStore.DataDirectory, "recovery.sveproject"), Path.Combine(thumbnailFolder, "notes.txt"), Path.Combine(waveformFolder, "recording.mp4") };
    foreach (var sentinel in cacheSentinels) await File.WriteAllTextAsync(sentinel, "preserve");
    var cacheUsage = PreviewCache.Inspect();
    Assert(cacheUsage.ThumbnailBytes == 1024 && cacheUsage.WaveformBytes == 2048 && cacheUsage.Files == 2, "Cache usage counts only generated thumbnail and waveform files");
    using (var lockedCache = new FileStream(cachedPeak, FileMode.Open, FileAccess.Read, FileShare.None))
    {
        var cleared = PreviewCache.Clear();
        Assert(cleared.Deleted == 1 && cleared.FreedBytes == 1024 && cleared.InUse == 1 && File.Exists(cachedPeak), "Cache cleanup reports locked files while freeing the available cache");
    }
    var partialPeak = Path.Combine(waveformFolder, new string('C', 64) + ".peaks." + new string('D', 32) + ".tmp"); await File.WriteAllBytesAsync(partialPeak, new byte[16]);
    var clearedRemaining = PreviewCache.Clear();
    Assert(clearedRemaining.Deleted == 2 && PreviewCache.Inspect().Files == 0 && cacheSentinels.All(p => File.ReadAllText(p) == "preserve"), "Cache cleanup removes generated partial files and preserves projects, recovery, recordings, and unrelated files");
    var regeneratedThumbnail = await MediaTools.ThumbnailAsync(a); var regeneratedWave = await AudioWaveforms.LoadAsync(a);
    Assert(regeneratedThumbnail != null && File.Exists(regeneratedThumbnail) && regeneratedWave != null && PreviewCache.Inspect().Files == 2, "Cleared thumbnails and waveforms are regenerated from the unchanged recording");
    ProjectStore.DataDirectory = previousDataDirectory;
    var recoveryRoot = Path.Combine(root, "recovery-tests", Guid.NewGuid().ToString("N"));
    using (var first = new RecoverySession(recoveryRoot))
    using (var second = new RecoverySession(recoveryRoot))
    using (var third = new RecoverySession(recoveryRoot))
    {
        await Task.WhenAll(first.SaveAsync(ProjectStore.Capture([a])), second.SaveAsync(ProjectStore.Capture([b])));
        Assert(first.Path != second.Path && (await ProjectStore.ReadAsync(first.Path)).Clips[0].Path == a.Path &&
            (await ProjectStore.ReadAsync(second.Path)).Clips[0].Path == b.Path, "Concurrent editor sessions save separate recovery projects");
        Assert(third.TryClaimLatest() == null, "Recovery ignores sessions still owned by running windows");
        first.Dispose(); // Releasing the lease without clearing simulates an interrupted window.
        using var claim = third.TryClaimLatest();
        Assert(claim?.Path == first.Path && second.TryClaimLatest() == null, "Only one window can claim an interrupted recovery session");
        await second.ClearAsync();
        Assert(File.Exists(first.Path) && !File.Exists(second.Path), "Clearing one session leaves other recovery projects intact");
        claim!.Discard();
        Assert(!File.Exists(first.Path), "Discarding a claimed recovery removes only that interrupted session");
    }
    var legacyRecovery = Path.Combine(recoveryRoot, "recovery.sveproject");
    await ProjectStore.SaveAsync(legacyRecovery, ProjectStore.Capture([a]));
    using (var session = new RecoverySession(recoveryRoot))
    using (var claim = session.TryClaimLatest())
    {
        Assert(claim?.Path == legacyRecovery, "Recovery can restore the previous version's single-session file");
        claim!.Discard();
    }
    var invalidPath = Path.Combine(root, "invalid.sveproject"); await File.WriteAllTextAsync(invalidPath, "{\"Version\":1,\"Clips\":[{\"Path\":\"x\",\"Start\":2,\"End\":1}]}");
    await ExpectFailure(async () => { await ProjectStore.ReadAsync(invalidPath); }, "Invalid project ranges are rejected");
    var settings = new ExportSettings(ExportCodec.Av1, ExportQuality.High, 320, 180, 60, UseGpu: useGpu);
    var export = new ExportService();
    await ExpectFailure(() => export.ExportAsync([a], sourceA, settings, null, CancellationToken.None), "Export refuses to overwrite a source recording");
    var output = Path.Combine(root, "combined.mp4");
    await export.ExportAsync([a, b], output, settings, new Progress<ExportProgress>(p => { }), CancellationToken.None);
    var result = await MediaTools.ProbeAsync(output);
    Assert(result.Codec == "av1" && result.Width == 320 && result.Height == 180 && Math.Abs(result.Duration - 2) < .08, $"{(useGpu ? "NVIDIA" : "CPU")} AV1 export normalizes mixed clips and preserves edited duration");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-i", output, "-f", "null", "-"]);
    Assert(true, "Combined AV1 video and AAC audio fully decode without errors");
    using var frames = JsonDocument.Parse(await MediaTools.RunAsync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries", "stream=nb_read_frames", "-of", "json", output]));
    Assert(frames.RootElement.GetProperty("streams")[0].GetProperty("nb_read_frames").GetString() == "120", "Two one-second trims export exactly 120 frames");
    // Compare the first lossy output frame against neighboring original frames.
    // The intended cut is frame 69, well away from the source's only keyframe.
    var actualFrame = Path.Combine(root, "actual.gray");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-i", output, "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", actualFrame]);
    var actual = await File.ReadAllBytesAsync(actualFrame); var best = -1; var bestError = double.MaxValue;
    for (var frame = 67; frame <= 71; frame++)
    {
        var referencePath = Path.Combine(root, $"reference-{frame}.gray");
        await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-i", sourceA, "-vf", $"select=eq(n\\,{frame})", "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", referencePath]);
        var reference = await File.ReadAllBytesAsync(referencePath);
        var error = actual.Zip(reference).Average(pair => Math.Abs(pair.First - pair.Second));
        if (error < bestError) { bestError = error; best = frame; }
    }
    Assert(best == 69, $"Non-keyframe trim starts at the selected frame (69, actual {best})");
    var firstSection = a.Clone(); firstSection.End = 1.65;
    var followingSection = a.Clone(newSection: true); followingSection.Start = 2.15; followingSection.End = 2.65;
    var sectionProjectPath = Path.Combine(root, "multiple-sections.sveproject");
    await ProjectStore.SaveAsync(sectionProjectPath, ProjectStore.Capture([firstSection, followingSection]));
    var sectionProject = await ProjectStore.ReadAsync(sectionProjectPath);
    Assert(sectionProject.Clips.Count == 2 && sectionProject.Clips[0].Path == sectionProject.Clips[1].Path &&
        sectionProject.Clips[0].End == 1.65 && sectionProject.Clips[1].Start == 2.15,
        "Version 1 projects retain multiple sections and gaps from a single recording");
    var sectionOutput = Path.Combine(root, "removed-middle.mp4");
    await export.ExportAsync([firstSection, followingSection], sectionOutput, settings, null, CancellationToken.None);
    using var sectionFrames = JsonDocument.Parse(await MediaTools.RunAsync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries", "stream=nb_read_frames", "-of", "json", sectionOutput]));
    Assert(sectionFrames.RootElement.GetProperty("streams")[0].GetProperty("nb_read_frames").GetString() == "60" &&
        Math.Abs((await MediaTools.ProbeAsync(sectionOutput)).Duration - 1) < .08, "Export joins two kept sections without including the removed middle or adding frames");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-i", sectionOutput, "-vf", "select=eq(n\\,30)", "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", actualFrame]);
    actual = await File.ReadAllBytesAsync(actualFrame); best = -1; bestError = double.MaxValue;
    for (var frame = 127; frame <= 131; frame++)
    {
        var referencePath = Path.Combine(root, $"reference-{frame}.gray");
        await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-i", sourceA, "-vf", $"select=eq(n\\,{frame})", "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", referencePath]);
        var reference = await File.ReadAllBytesAsync(referencePath);
        var error = actual.Zip(reference).Average(pair => Math.Abs(pair.First - pair.Second));
        if (error < bestError) { bestError = error; best = frame; }
    }
    Assert(best == 129, $"The join resumes at the selected source frame after the removed middle (129, actual {best})");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-i", sectionOutput, "-f", "null", "-"]);
    Assert(true, "Export with a removed middle fully decodes with video and audio");
    // Verify the later silent clip's audio stays silent after the join.
    var audioPath = Path.Combine(root, "audio.raw");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-ss", "1.2", "-i", output, "-t", "0.5", "-vn", "-ac", "1", "-f", "s16le", audioPath]);
    var audio = await File.ReadAllBytesAsync(audioPath); var maxSample = 0;
    for (var i = 0; i + 1 < audio.Length; i += 2) maxSample = Math.Max(maxSample, Math.Abs((int)BitConverter.ToInt16(audio, i)));
    Assert(maxSample < 10, "Video-only clip receives correctly aligned silent audio");
    var protectedOutput = Path.Combine(root, "cancel-protected.mp4"); await File.WriteAllTextAsync(protectedOutput, "keep-existing-output");
    using var cancellation = new CancellationTokenSource();
    var cancelProgress = new InlineProgress(p => { if (p.Fraction > 0) cancellation.Cancel(); });
    await ExpectFailure(() => export.ExportAsync([a, b], protectedOutput, settings, cancelProgress, cancellation.Token), "Cancellation interrupts an active export");
    Assert(await File.ReadAllTextAsync(protectedOutput) == "keep-existing-output" && !Directory.GetDirectories(root, ".sve-export-*").Any(), "Canceled export preserves existing output and removes temporary files");
    var h264 = Path.Combine(root, "cpu-h264.mp4");
    await export.ExportAsync([a], h264, settings with { Codec = ExportCodec.H264, UseGpu = false }, null, CancellationToken.None);
    Assert((await MediaTools.ProbeAsync(h264)).Codec == "h264", "CPU H.264 fallback exports successfully");
    var cpuAv1 = Path.Combine(root, "cpu-av1.mp4");
    await export.ExportAsync([a], cpuAv1, settings with { UseGpu = false }, null, CancellationToken.None);
    Assert((await MediaTools.ProbeAsync(cpuAv1)).Codec == "av1", "CPU AV1 fallback exports successfully");
    var gapA = a.Clone(true); gapA.Start = 0; gapA.End = .5; gapA.TimelineStart = .25;
    var gapB = b.Clone(true); gapB.Start = 0; gapB.End = .5; gapB.TimelineStart = 1.25;
    Assert(TimelineLayout.AvailablePosition([gapA], gapB, .76, .02) == .75, "Automatic snapping joins exact edges even when the clips have different frame rates");
    Assert(TimelineLayout.AvailablePosition([gapB], gapA, .74, .02) == .75, "Automatic snapping can join a dragged clip's end to a following clip's start");
    Assert(Math.Abs(TimelineLayout.AvailablePosition([gapA], gapB, .79, .02) - .8) < .000001, "Placement outside the snap tolerance keeps the requested frame-aligned gap");
    var gapProject = Path.Combine(root, "free-roundtrip.sveproject");
    await ProjectStore.SaveAsync(gapProject, ProjectStore.Capture([gapA, gapB], [a, b]));
    var freeProject = await ProjectStore.ReadAsync(gapProject);
    Assert(freeProject.Clips[0].TimelineStart == .25 && freeProject.Clips[1].TimelineStart == 1.25, "Project serialization preserves a leading gap and a gap between clips");
    var savedPositions = await File.ReadAllTextAsync(gapProject);
    Assert(!savedPositions.Contains("Magnetic"), "Projects store timeline positions without a mode flag");
    var previousModeProject = Path.Combine(root, "previous-mode.sveproject");
    await File.WriteAllTextAsync(previousModeProject, savedPositions.Replace("\"Version\": 1", "\"Version\": 1, \"Magnetic\": false"));
    Assert((await ProjectStore.ReadAsync(previousModeProject)).Clips.SequenceEqual(freeProject.Clips), "Projects saved by the previous two-mode build retain their positions and gaps");
    var gapOutput = Path.Combine(root, "free-gaps.mp4");
    await export.ExportAsync([gapA, gapB], gapOutput, settings with { Codec = ExportCodec.H264, UseGpu = false }, null, CancellationToken.None);
    using (var gapFrames = JsonDocument.Parse(await MediaTools.RunAsync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries", "stream=nb_read_frames", "-of", "json", gapOutput])))
        Assert(gapFrames.RootElement.GetProperty("streams")[0].GetProperty("nb_read_frames").GetString() == "105" && Math.Abs((await MediaTools.ProbeAsync(gapOutput)).Duration - 1.75) < .05,
            "Free timeline export preserves leading and middle gaps with an exact output frame count");
    foreach (var gapFrame in new[] { 0, 60 })
    {
        await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-i", gapOutput, "-vf", $"select=eq(n\\,{gapFrame})", "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", actualFrame]);
        Assert((await File.ReadAllBytesAsync(actualFrame)).Max() <= 2, $"Exported gap frame {gapFrame} is black");
    }
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-y", "-ss", "0.9", "-i", gapOutput, "-t", "0.2", "-vn", "-ac", "1", "-f", "s16le", audioPath]);
    var gapAudio = await File.ReadAllBytesAsync(audioPath);
    Assert(gapAudio.Length > 0 && Enumerable.Range(0, gapAudio.Length / 2).Max(i => Math.Abs((int)BitConverter.ToInt16(gapAudio, i * 2))) < 10, "Exported free gaps contain silence without leaking source audio");
    await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-i", gapOutput, "-f", "null", "-"]);
    Assert(true, "Free-gap export fully decodes across gap and recording boundaries");
    gapB.TimelineStart = .5;
    await ExpectFailure(() => export.ExportAsync([gapA, gapB], gapOutput, settings, null, CancellationToken.None), "Overlapping free positions are rejected before replacing an output");
    Assert(!Directory.GetDirectories(root, ".sve-export-*").Any(), "Invalid free positions leave no temporary export folders");
    await ProjectStore.SaveAsync(invalidPath, ProjectStore.Capture([gapA, gapB]));
    await ExpectFailure(() => ProjectStore.ReadAsync(invalidPath), "Projects with overlapping free clips are rejected");
    using (var native = new NativePlayer())
    {
        var loaded = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        native.FileLoaded += _ => loaded.TrySetResult();
        native.PlaybackError += message => loaded.TrySetException(new Exception(message));
        native.Initialize(true); native.Load(output); await loaded.Task.WaitAsync(TimeSpan.FromSeconds(15));
        native.Seek(.5); native.Set("pause", "no"); await Task.Delay(400); native.Set("pause", "yes");
        Assert(native.Position > .7 && native.Position < 1.2, "Bundled libmpv decodes AV1, seeks, and plays");
        var ends = 0;
        var ended = new TaskCompletionSource<string?>(TaskCreationOptions.RunContinuationsAsynchronously);
        native.PlaybackEnded += (_, path) => { Interlocked.Increment(ref ends); ended.TrySetResult(path); };
        native.Seek(1.8); native.Set("pause", "no");
        var endedPath = await ended.Task.WaitAsync(TimeSpan.FromSeconds(5));
        await Task.Delay(150);
        Assert(ends == 1 && string.Equals(endedPath, output, StringComparison.OrdinalIgnoreCase) && native.Paused, "Keep-open playback reports end-of-file once with the correct source path");
        ended = new(TaskCreationOptions.RunContinuationsAsynchronously);
        native.Seek(.2); await Task.Delay(100); native.Seek(1.8); native.Set("pause", "no");
        await ended.Task.WaitAsync(TimeSpan.FromSeconds(5));
        Assert(ends == 2, "Seeking back and replaying produces a new end-of-file notification");
    }
    for (var cycle = 0; cycle < 3; cycle++)
    {
        using var software = new NativePlayer();
        var loaded = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        software.FileLoaded += _ => loaded.TrySetResult();
        software.PlaybackError += message => loaded.TrySetException(new Exception(message));
        software.Initialize(); software.ResizePreview(153, 89); software.Load(output);
        await loaded.Task.WaitAsync(TimeSpan.FromSeconds(10)); software.Set("pause", "no"); await Task.Delay(250);
        using var frame = software.TakePreviewFrame();
        Assert(frame is { Width: 153, Height: 89 } && frame.Stride % 64 == 0 && frame.Pixels.Length >= frame.Stride * frame.Height && software.Get("hwdec-current") == "no" && software.Get("current-vo") == "libmpv", $"Software renderer cycle {cycle + 1} presents aligned memory frames without GPU decoding");
        software.Dispose(); software.Dispose();
    }
    Assert(true, "Repeated software-renderer shutdown is safe during active playback");
    if (clips.Length > 0)
    {
        var real = await MediaTools.ProbeAsync(clips[0]); real.Start = 12.35; real.End = 13.35;
        var real2 = await MediaTools.ProbeAsync(clips.Length > 1 ? clips[1] : clips[0]); real2.Start = 20.5; real2.End = 21.5;
        var realOutput = Path.Combine(root, "shadowplay-check.mp4");
        await export.ExportAsync([real, real2], realOutput, new(ExportCodec.Av1, ExportQuality.High, real.Width, real.Height, real.FrameRate, UseGpu: useGpu), null, CancellationToken.None);
        await MediaTools.RunAsync("ffmpeg", ["-v", "error", "-i", realOutput, "-f", "null", "-"]);
        Assert(Math.Abs((await MediaTools.ProbeAsync(realOutput)).Duration - 2) < .08, "Actual 1440p Shadowplay clips trim, combine, and fully decode");
    }
    await File.WriteAllTextAsync(Path.Combine(root, "results.json"), JsonSerializer.Serialize(new { passed = checks.Count, checks }, new JsonSerializerOptions { WriteIndented = true }));
    Console.WriteLine($"\nAll {checks.Count} checks passed.");
}
catch (Exception exception) { Console.Error.WriteLine(exception); Environment.ExitCode = 1; }

sealed class InlineProgress(Action<ExportProgress> callback) : IProgress<ExportProgress> { public void Report(ExportProgress value) => callback(value); }
