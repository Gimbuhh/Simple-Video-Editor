using System.IO;

namespace SimpleVideoEditor.Services;

/// <summary>Authorizes recording paths before filesystem or native media access.</summary>
public static class LocalRecordingPath
{
    public static string Validate(string path) => Validate(path, 0);
    private static string Validate(string path, int links)
    {
        const string message = "Recordings must be on a local drive. Copy network or unsupported linked recordings to a local folder first.";
        // Reject network/device namespaces syntactically, before querying the filesystem.
        if (string.IsNullOrWhiteSpace(path) || path.Length < 3 || !char.IsAsciiLetter(path[0]) || path[1] != ':' || (path[2] != '\\' && path[2] != '/'))
            throw new InvalidDataException(message);
        var full = Path.GetFullPath(path);
        var root = Path.GetPathRoot(full)!;
        if (new DriveInfo(root).DriveType is not (DriveType.Fixed or DriveType.Removable or DriveType.CDRom or DriveType.Ram))
            throw new InvalidDataException(message);
        var parts = full[root.Length..].Split(Path.DirectorySeparatorChar, StringSplitOptions.RemoveEmptyEntries);
        var current = root;
        for (var i = 0; i < parts.Length; i++)
        {
            current = Path.Combine(current, parts[i]);
            FileAttributes attributes;
            try { attributes = File.GetAttributes(current); }
            catch (FileNotFoundException) { return full; }
            catch (DirectoryNotFoundException) { return full; }
            if (!attributes.HasFlag(FileAttributes.ReparsePoint)) continue;
            // Get attributes and the immediate link target, never follow an unchecked link.
            if (links >= 16) throw new InvalidDataException("The recording path contains too many links.");
            FileSystemInfo entry = attributes.HasFlag(FileAttributes.Directory) ? new DirectoryInfo(current) : new FileInfo(current);
            var target = entry.LinkTarget;
            if (target == null) throw new InvalidDataException(message);
            if (!Path.IsPathRooted(target)) target = Path.Combine(Path.GetDirectoryName(current)!, target);
            for (var j = i + 1; j < parts.Length; j++) target = Path.Combine(target, parts[j]);
            return Validate(target, links + 1);
        }
        return full;
    }
    public static bool Exists(string path) => File.Exists(Validate(path));
}
