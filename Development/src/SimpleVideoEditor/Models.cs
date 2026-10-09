using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;

namespace SimpleVideoEditor;

public sealed class MediaClip : INotifyPropertyChanged
{
    public Guid SectionId { get; init; } = Guid.NewGuid();
    public required string Path { get; init; }
    public string Name => System.IO.Path.GetFileNameWithoutExtension(Path);
    public double Duration { get; init; }
    public int Width { get; init; }
    public int Height { get; init; }
    public double FrameRate { get; init; }
    public bool HasAudio { get; init; }
    public string Codec { get; init; } = "";
    public string? Thumbnail { get; set; }
    public bool IsHdr { get; init; }
    public AudioWaveform Waveform { get; init; } = new();
    private double start;
    private double end;
    private double? timelineStart;
    public double? TimelineStart { get => timelineStart; set { if (timelineStart == value) return; timelineStart = value; Changed(); } }
    public double Start { get => start; set { if (start == value) return; start = value; Changed(); Changed(nameof(TrimLabel)); Changed(nameof(KeptDuration)); } }
    public double End { get => end; set { if (end == value) return; end = value; Changed(); Changed(nameof(TrimLabel)); Changed(nameof(KeptDuration)); } }
    public double KeptDuration => Math.Max(0, End - Start);
    public string TrimLabel => $"{Timecode.Format(Start)} → {Timecode.Format(End)}  ·  {Timecode.Format(KeptDuration)}";
    public string DurationLabel => Timecode.Format(Duration);
    public MediaClip Clone(bool newSection = false) => new() { SectionId = newSection ? Guid.NewGuid() : SectionId, Path = Path, Duration = Duration, Width = Width, Height = Height, FrameRate = FrameRate, HasAudio = HasAudio, Codec = Codec, Thumbnail = Thumbnail, Start = Start, End = End, TimelineStart = TimelineStart, IsHdr = IsHdr, Waveform = Waveform };
    public event PropertyChangedEventHandler? PropertyChanged;
    private void Changed([CallerMemberName] string? name = null) => PropertyChanged?.Invoke(this, new(name));
}

/// <summary>Shared by all sections of one source, including undo snapshots.</summary>
public sealed class AudioWaveform : INotifyPropertyChanged
{
    public WaveformData? Data { get; private set; }
    public bool IsLoading { get; private set; }
    public string? Error { get; private set; }
    public void Begin() { IsLoading = true; Error = null; Changed(); }
    public void Complete(WaveformData? data, string? error = null) { Data = data; Error = error; IsLoading = false; Changed(); }
    public event PropertyChangedEventHandler? PropertyChanged;
    private void Changed() => PropertyChanged?.Invoke(this, new(null));
}

public sealed class WaveformData(double binDuration, float[] peaks)
{
    public double BinDuration { get; } = binDuration;
    public ReadOnlyMemory<float> Peaks => peaks;
    public float PeakBetween(double start, double end)
    {
        if (peaks.Length == 0 || end <= start || end <= 0 || start >= peaks.Length * BinDuration) return 0;
        var first = (int)Math.Clamp(Math.Floor(start / BinDuration), 0, peaks.Length - 1);
        var last = (int)Math.Clamp(Math.Ceiling(end / BinDuration) - 1, first, peaks.Length - 1);
        float maximum = 0;
        for (var i = first; i <= last; i++) maximum = Math.Max(maximum, peaks[i]);
        return maximum;
    }
}

public static class Timecode
{
    public static string Format(double seconds)
    {
        var milliseconds = (long)Math.Round(Math.Max(0, seconds) * 1000);
        return $"{milliseconds / 3600000:00}:{milliseconds / 60000 % 60:00}:{milliseconds / 1000 % 60:00}.{milliseconds % 1000:000}";
    }
    public static bool TryParse(string? text, out double seconds)
    {
        seconds = 0;
        if (string.IsNullOrWhiteSpace(text)) return false;
        var pieces = text.Trim().Split(':');
        if (pieces.Length is < 1 or > 3) return false;
        for (var i = 0; i < pieces.Length; i++)
        {
            if (!double.TryParse(pieces[i], NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out var value) || !double.IsFinite(value) || value < 0) return false;
            if (i < pieces.Length - 1 && value != Math.Floor(value)) return false;
            if (i > 0 && value >= 60) return false;
            seconds = seconds * 60 + value;
        }
        return true;
    }
}

public sealed record ProjectEntry(string Path, double Start, double End, double? TimelineStart = null);
public sealed record ProjectDocument(int Version, List<ProjectEntry> Clips, List<string>? Sources = null);
public enum ExportCodec { Av1, H264 }
public enum ExportQuality { High, Balanced, Smaller }
public sealed record ExportSettings(ExportCodec Codec, ExportQuality Quality, int Width, int Height, double FrameRate, bool UseGpu = true);
public sealed record ExportProgress(double Fraction, string Message);
