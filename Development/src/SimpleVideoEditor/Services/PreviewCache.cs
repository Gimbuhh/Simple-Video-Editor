using System.IO;
using System.Text.RegularExpressions;

namespace SimpleVideoEditor.Services;

public sealed record CacheUsage(long ThumbnailBytes, long WaveformBytes, int Files)
{
    public long TotalBytes => ThumbnailBytes + WaveformBytes;
}
public sealed record CacheClearResult(int Deleted, long FreedBytes, int InUse);

public static class PreviewCache
{
    private static IEnumerable<(string Path, bool Waveform)> Files()
    {
        foreach (var (folder, waveform, pattern) in new[] {
            ("thumbnails", false, @"^[a-fA-F0-9]{64}(\.[a-fA-F0-9]{32})?\.jpg$"),
            ("waveforms", true, @"^[a-fA-F0-9]{64}\.peaks(\.[a-fA-F0-9]{32}\.tmp)?$") })
        {
            var root = Path.Combine(ProjectStore.DataDirectory, folder);
            if (!Directory.Exists(root)) continue;
            if (File.GetAttributes(root).HasFlag(FileAttributes.ReparsePoint)) throw new IOException("The preview cache folder is a link. Its files were left untouched.");
            foreach (var file in Directory.EnumerateFiles(root, "*", SearchOption.TopDirectoryOnly))
                if (Regex.IsMatch(Path.GetFileName(file), pattern) && !File.GetAttributes(file).HasFlag(FileAttributes.ReparsePoint)) yield return (file, waveform);
        }
    }
    public static CacheUsage Inspect()
    {
        long thumbnails = 0, waveforms = 0; int count = 0;
        foreach (var (path, waveform) in Files())
        {
            try { var size = new FileInfo(path).Length; if (waveform) waveforms += size; else thumbnails += size; count++; }
            catch (FileNotFoundException) { } // Another editor window can clear the same cache.
        }
        return new(thumbnails, waveforms, count);
    }
    public static CacheClearResult Clear()
    {
        // Enumerate only known generated files in the two cache folders. Never recurse
        // into recovery, projects, recordings, links, or unrelated files in these folders.
        var files = Files().ToArray(); int deleted = 0, inUse = 0; long freed = 0;
        foreach (var (path, _) in files)
        {
            try { var size = new FileInfo(path).Length; File.Delete(path); deleted++; freed += size; }
            catch (FileNotFoundException) { }
            catch (IOException) { inUse++; }
            catch (UnauthorizedAccessException) { inUse++; }
        }
        return new(deleted, freed, inUse);
    }
    public static string SizeLabel(long bytes) => bytes >= 1024 * 1024 ? $"{bytes / (1024d * 1024):0.#} MB" : bytes >= 1024 ? $"{bytes / 1024d:0.#} KB" : $"{bytes} B";
}
