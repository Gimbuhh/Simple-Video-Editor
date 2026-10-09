using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace SimpleVideoEditor.Services;

public static class MediaTools
{
    public static string Find(string name)
    {
        var binary = System.IO.Path.Combine(AppContext.BaseDirectory, "bin", name + ".exe");
        if (File.Exists(binary)) return binary;
        var bundled = System.IO.Path.Combine(AppContext.BaseDirectory, "tools", name + ".exe");
        if (File.Exists(bundled)) return bundled;
        var current = new DirectoryInfo(AppContext.BaseDirectory);
        while (current != null)
        {
            var local = System.IO.Path.Combine(current.FullName, "vendor", "ffmpeg", name + ".exe");
            if (File.Exists(local)) return local;
            var app = System.IO.Path.Combine(current.FullName, "App", "tools", name + ".exe");
            if (File.Exists(app)) return app;
            var portable = System.IO.Path.Combine(current.FullName, "App", "bin", name + ".exe");
            if (File.Exists(portable)) return portable;
            current = current.Parent;
        }
        throw new FileNotFoundException($"The bundled {name} tool is missing. Extract the complete portable app folder and try again.");
    }

    public static async Task<string> RunAsync(string tool, IEnumerable<string> arguments, CancellationToken token = default, Action<string>? outputLine = null)
    {
        using var process = new Process { StartInfo = new ProcessStartInfo(Find(tool)) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true } };
        foreach (var argument in arguments) process.StartInfo.ArgumentList.Add(argument);
        process.Start();
        using var cancel = token.Register(() => { try { if (!process.HasExited) process.Kill(true); } catch (InvalidOperationException) { } });
        var errors = process.StandardError.ReadToEndAsync();
        var output = new StringBuilder();
        try
        {
            while (await process.StandardOutput.ReadLineAsync(token) is { } line) { if (outputLine != null) outputLine(line); else output.AppendLine(line); }
            await process.WaitForExitAsync(token);
        }
        catch (OperationCanceledException)
        {
            try { if (!process.HasExited) process.Kill(true); } catch (InvalidOperationException) { }
            await process.WaitForExitAsync();
            await errors;
            throw;
        }
        var error = await errors;
        token.ThrowIfCancellationRequested();
        if (process.ExitCode != 0) throw new InvalidOperationException($"{tool} failed.\n{(error.Length > 5000 ? error[^5000..] : error).Trim()}");
        return output.ToString();
    }

    public static async Task<MediaClip> ProbeAsync(string path, CancellationToken token = default)
        => await ProbeCoreAsync(LocalRecordingPath.Validate(path), token);
    // The output is generated beneath a destination explicitly chosen by the user.
    // Recording-reference restrictions must not reject authorized network exports.
    internal static Task<MediaClip> ProbeOutputAsync(string path, CancellationToken token) => ProbeCoreAsync(System.IO.Path.GetFullPath(path), token);
    private static async Task<MediaClip> ProbeCoreAsync(string path, CancellationToken token)
    {
        if (!File.Exists(path)) throw new FileNotFoundException("The source recording could not be found.", path);
        using var json = JsonDocument.Parse(await RunAsync("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", path], token));
        var root = json.RootElement;
        var streams = root.GetProperty("streams").EnumerateArray().ToArray();
        var video = streams.FirstOrDefault(s => s.GetProperty("codec_type").GetString() == "video" && (!s.TryGetProperty("disposition", out var d) || !d.TryGetProperty("attached_pic", out var a) || a.GetInt32() == 0));
        if (video.ValueKind == JsonValueKind.Undefined) throw new InvalidOperationException("This file contains no video.");
        var durationText = root.GetProperty("format").TryGetProperty("duration", out var duration) ? duration.GetString() : null;
        if (!double.TryParse(durationText, CultureInfo.InvariantCulture, out var seconds) || !double.IsFinite(seconds) || seconds <= 0) throw new InvalidOperationException("The recording's duration could not be read.");
        var rate = ParseRate(video.TryGetProperty("r_frame_rate", out var fps) ? fps.GetString() : "0/0");
        if (rate <= 0) rate = ParseRate(video.TryGetProperty("avg_frame_rate", out fps) ? fps.GetString() : "0/0");
        var transfer = video.TryGetProperty("color_transfer", out var color) ? color.GetString() : "";
        return new MediaClip { Path = path, Duration = seconds, End = seconds, Width = video.GetProperty("width").GetInt32(), Height = video.GetProperty("height").GetInt32(), FrameRate = rate > 0 ? rate : 30, Codec = video.GetProperty("codec_name").GetString() ?? "unknown", HasAudio = streams.Any(s => s.GetProperty("codec_type").GetString() == "audio"), IsHdr = transfer is "smpte2084" or "arib-std-b67" };
    }

    private static double ParseRate(string? text)
    {
        var pair = (text ?? "").Split('/');
        return pair.Length == 2 && double.TryParse(pair[0], CultureInfo.InvariantCulture, out var numerator) && double.TryParse(pair[1], CultureInfo.InvariantCulture, out var denominator) && denominator > 0 ? numerator / denominator : 0;
    }

    public static async Task<string?> ThumbnailAsync(MediaClip clip, CancellationToken token = default)
    {
        var info = new FileInfo(clip.Path);
        var key = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes($"{clip.Path}|{info.Length}|{info.LastWriteTimeUtc.Ticks}")));
        var folder = System.IO.Path.Combine(ProjectStore.DataDirectory, "thumbnails");
        Directory.CreateDirectory(folder);
        var output = System.IO.Path.Combine(folder, key + ".jpg");
        if (File.Exists(output)) return output;
        var partial = output + "." + Guid.NewGuid().ToString("N") + ".jpg";
        try
        {
            await RunAsync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-threads", "2", "-ss", Math.Min(5, clip.Duration / 4).ToString(CultureInfo.InvariantCulture), "-i", clip.Path, "-frames:v", "1", "-vf", "scale=240:-2", "-q:v", "5", "-update", "1", "-y", partial], token);
            File.Move(partial, output, true);
            return output;
        }
        catch (OperationCanceledException) { throw; }
        catch { return null; }
        finally { if (File.Exists(partial)) File.Delete(partial); }
    }
}
