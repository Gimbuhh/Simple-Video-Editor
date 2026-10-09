using System.IO;

namespace SimpleVideoEditor.Services;

/// <summary>Owns one window's recovery file and prevents other windows from claiming a live session.</summary>
public sealed class RecoverySession : IDisposable
{
    private readonly string dataDirectory;
    private readonly FileStream lease;
    private readonly SemaphoreSlim writes = new(1, 1);
    private bool disposed;
    public string Path { get; }

    public RecoverySession(string dataDirectory)
    {
        this.dataDirectory = System.IO.Path.GetFullPath(dataDirectory);
        var folder = System.IO.Path.Combine(this.dataDirectory, "recovery");
        Directory.CreateDirectory(folder);
        Path = System.IO.Path.Combine(folder, Guid.NewGuid().ToString("N") + ".sveproject");
        lease = new FileStream(Path + ".lock", FileMode.CreateNew, FileAccess.ReadWrite, FileShare.None);
    }

    public async Task SaveAsync(ProjectDocument document)
    {
        await writes.WaitAsync().ConfigureAwait(false);
        try { await ProjectStore.SaveAsync(Path, document).ConfigureAwait(false); }
        finally { writes.Release(); }
    }

    public async Task ClearAsync()
    {
        await writes.WaitAsync().ConfigureAwait(false);
        try { if (File.Exists(Path)) File.Delete(Path); }
        finally { writes.Release(); }
    }

    public RecoveryClaim? TryClaimLatest()
    {
        var candidates = Directory.EnumerateFiles(System.IO.Path.GetDirectoryName(Path)!, "*.sveproject").ToList();
        var legacy = System.IO.Path.Combine(dataDirectory, "recovery.sveproject");
        if (File.Exists(legacy)) candidates.Add(legacy);
        foreach (var candidate in candidates.Where(p => p != Path).OrderByDescending(File.GetLastWriteTimeUtc))
        {
            try
            {
                var claim = new RecoveryClaim(candidate);
                if (File.Exists(candidate)) return claim;
                claim.Dispose();
            }
            catch (IOException) { } // A live window or another recovery prompt holds the lease.
            catch (UnauthorizedAccessException) { }
        }
        return null;
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        lease.Dispose();
        try { File.Delete(Path + ".lock"); } catch (IOException) { } catch (UnauthorizedAccessException) { }
        writes.Dispose();
    }
}

public sealed class RecoveryClaim : IDisposable
{
    private readonly FileStream lease;
    public string Path { get; }
    internal RecoveryClaim(string path)
    {
        Path = path;
        lease = new FileStream(path + ".lock", FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
    }
    public void Discard() { if (File.Exists(Path)) File.Delete(Path); }
    public void Dispose()
    {
        lease.Dispose();
        try { File.Delete(Path + ".lock"); } catch (IOException) { } catch (UnauthorizedAccessException) { }
    }
}
