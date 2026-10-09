using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace SimpleVideoEditor.Services;

public static class AudioWaveforms
{
    private const int CacheVersion = 1;
    private const int SampleRate = 48000;
    private const int MaximumBins = 1_000_000;
    public static async Task<WaveformData?> LoadAsync(MediaClip clip, CancellationToken token = default)
    {
        if (!clip.HasAudio) return null;
        var info = new FileInfo(clip.Path);
        var key = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes($"waveform-{CacheVersion}|{clip.Path}|{info.Length}|{info.LastWriteTimeUtc.Ticks}")));
        var folder = Path.Combine(ProjectStore.DataDirectory, "waveforms"); Directory.CreateDirectory(folder);
        var path = Path.Combine(folder, key + ".peaks");
        token.ThrowIfCancellationRequested();
        if (File.Exists(path))
        {
            try
            {
                using var input = new BinaryReader(File.OpenRead(path));
                var version = input.ReadInt32(); var duration = input.ReadDouble(); var count = input.ReadInt32();
                if (version != CacheVersion || !double.IsFinite(duration) || duration <= 0 || count < 1 || count > MaximumBins + 1 || input.BaseStream.Length != 16L + count * 4L) throw new InvalidDataException("Invalid waveform cache.");
                var cached = new float[count];
                for (var i = 0; i < count; i++) { cached[i] = input.ReadSingle(); if (!float.IsFinite(cached[i]) || cached[i] < 0 || cached[i] > 1) throw new InvalidDataException("Invalid waveform peak."); }
                token.ThrowIfCancellationRequested(); return new(duration, cached);
            }
            catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException) { /* Rebuild an incomplete or corrupt cache. */ }
        }
        var samplesPerBin = (int)Math.Max(480, Math.Ceiling(clip.Duration * SampleRate / MaximumBins));
        var binDuration = samplesPerBin / (double)SampleRate;
        var peaks = new float[(int)Math.Ceiling(clip.Duration / binDuration)];
        var bin = -1;
        // Keep channels separate: opposite-phase stereo must not cancel in the display.
        // Overall peak is the loudest channel. PTS preserves delayed audio relative to video.
        var filter = $"aresample={SampleRate},aformat=sample_fmts=flt,asetnsamples=n={samplesPerBin}:p=1,astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=Peak_level,ametadata=mode=print:key=lavfi.astats.Overall.Peak_level:file=-";
        await MediaTools.RunAsync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-threads", "2", "-i", clip.Path, "-map", "0:a:0", "-vn", "-sn", "-dn", "-af", filter, "-f", "null", "-"], token, line =>
        {
            var pts = line.IndexOf("pts_time:", StringComparison.Ordinal);
            if (pts >= 0 && double.TryParse(line[(pts + 9)..].Trim(), CultureInfo.InvariantCulture, out var time))
                bin = (int)Math.Floor((time + .000001) / binDuration);
            const string prefix = "lavfi.astats.Overall.Peak_level=";
            if (bin >= 0 && bin < peaks.Length && line.StartsWith(prefix, StringComparison.Ordinal) && double.TryParse(line[prefix.Length..], CultureInfo.InvariantCulture, out var db))
                peaks[bin] = Math.Max(peaks[bin], double.IsFinite(db) ? (float)Math.Clamp(Math.Pow(10, db / 20), 0, 1) : 0);
        }).ConfigureAwait(false);
        token.ThrowIfCancellationRequested();
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            using (var output = new BinaryWriter(File.Create(temporary)))
            { output.Write(CacheVersion); output.Write(binDuration); output.Write(peaks.Length); foreach (var peak in peaks) output.Write(peak); }
            token.ThrowIfCancellationRequested(); File.Move(temporary, path, true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
        return new(binDuration, peaks);
    }
}
