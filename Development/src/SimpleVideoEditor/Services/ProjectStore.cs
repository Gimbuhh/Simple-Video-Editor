using System.IO;
using System.Text.Json;

namespace SimpleVideoEditor.Services;

public static class ProjectStore
{
    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true, PropertyNameCaseInsensitive = true };
    public static string DataDirectory { get; set; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SimpleVideoEditor");
    public static ProjectDocument Capture(IEnumerable<MediaClip> clips, IEnumerable<MediaClip>? sources = null) => new(1, clips.Select(c => new ProjectEntry(c.Path, c.Start, c.End, c.TimelineStart)).ToList(), sources?.Select(c => c.Path).Distinct(StringComparer.OrdinalIgnoreCase).ToList());
    public static IEnumerable<string> SourcePaths(ProjectDocument document) => (document.Sources ?? []).Concat(document.Clips.Select(c => c.Path)).Distinct(StringComparer.OrdinalIgnoreCase);
    public static ProjectDocument Relink(ProjectDocument document, IReadOnlyDictionary<string, string> replacements)
    {
        var paths = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var original in SourcePaths(document))
        {
            var resolved = replacements.TryGetValue(original, out var replacement) ? Path.GetFullPath(replacement) : original;
            if (!File.Exists(resolved)) throw new FileNotFoundException("Locate all missing recordings before opening the project.", resolved);
            paths.Add(original, resolved);
        }
        return document with { Clips = document.Clips.Select(c => c with { Path = paths[c.Path] }).ToList(), Sources = SourcePaths(document).Select(p => paths[p]).Distinct(StringComparer.OrdinalIgnoreCase).ToList() };
    }
    public static async Task SaveAsync(string path, ProjectDocument document)
    {
        var full = Path.GetFullPath(path);
        Directory.CreateDirectory(Path.GetDirectoryName(full)!);
        var temporary = full + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try { await File.WriteAllTextAsync(temporary, JsonSerializer.Serialize(document, Options)).ConfigureAwait(false); File.Move(temporary, full, true); }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
    public static async Task<ProjectDocument> ReadAsync(string path)
    {
        var document = JsonSerializer.Deserialize<ProjectDocument>(await File.ReadAllTextAsync(path), Options) ?? throw new InvalidDataException("The project file is empty.");
        if (document.Version != 1 || document.Clips == null || document.Clips.Count > 10000) throw new InvalidDataException("This project format is not supported.");
        if (document.Sources is { Count: > 10000 } || document.Sources?.Any(string.IsNullOrWhiteSpace) == true) throw new InvalidDataException("The project contains an invalid recording library.");
        foreach (var clip in document.Clips)
            if (string.IsNullOrWhiteSpace(clip.Path) || !double.IsFinite(clip.Start) || !double.IsFinite(clip.End) || clip.Start < 0 || clip.End <= clip.Start) throw new InvalidDataException("The project contains an invalid trim range.");
        double previousEnd = 0;
        foreach (var clip in document.Clips)
        {
            var position = clip.TimelineStart ?? previousEnd;
            if (!double.IsFinite(position) || position < previousEnd - .00001) throw new InvalidDataException("The project contains overlapping or invalid timeline positions.");
            previousEnd = position + clip.End - clip.Start;
        }
        return document;
    }
    public static void ValidateTrim(MediaClip clip)
    {
        if (!double.IsFinite(clip.Start) || !double.IsFinite(clip.End) || clip.Start < 0 || clip.End > clip.Duration + 0.001 || clip.End - clip.Start < 1 / clip.FrameRate - 0.0001) throw new InvalidDataException($"Invalid trim for {clip.Name}. Keep at least one frame within the recording.");
    }
}
