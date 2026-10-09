using System.IO;
using System.Runtime.InteropServices;

namespace SimpleVideoEditor.Services;

public sealed class NativePlayer : IDisposable
{
    private IntPtr handle;
    private Thread? eventThread;
    private volatile bool stopping;
    private string lastWarning = "";
    private bool eofNotified;
    private int disposed;
    private long activeEntryId = -1;
    private SoftwarePreviewRenderer? renderer;
    public long RequestedEntryId { get; private set; } = -1;
    public event Action<string>? PlaybackError;
    public event Action<long>? FileLoaded;
    public event Action<long, string?>? PlaybackEnded;
    public event Action? PreviewFrameAvailable;

    static NativePlayer()
    {
        NativeLibrary.SetDllImportResolver(typeof(NativePlayer).Assembly, (name, _, _) =>
        {
            if (name != "libmpv-2.dll") return IntPtr.Zero;
            var binary = Path.Combine(AppContext.BaseDirectory, "bin", name);
            if (File.Exists(binary)) return NativeLibrary.Load(binary);
            var bundled = Path.Combine(AppContext.BaseDirectory, name);
            if (File.Exists(bundled)) return NativeLibrary.Load(bundled);
            var folder = new DirectoryInfo(AppContext.BaseDirectory);
            while (folder != null)
            {
                var local = Path.Combine(folder.FullName, "vendor", "mpv", name);
                if (File.Exists(local)) return NativeLibrary.Load(local);
                var app = Path.Combine(folder.FullName, "App", name);
                if (File.Exists(app)) return NativeLibrary.Load(app);
                var portable = Path.Combine(folder.FullName, "App", "bin", name);
                if (File.Exists(portable)) return NativeLibrary.Load(portable);
                folder = folder.Parent;
            }
            throw new FileNotFoundException("The playback library is missing. Extract the complete portable app folder.");
        });
    }

    public void Initialize(bool headless = false)
    {
        handle = mpv_create();
        if (handle == IntPtr.Zero) throw new InvalidOperationException("The video player could not be created.");
        try
        {
            Option("config", "no"); Option("idle", "yes"); Option("keep-open", "yes"); Option("pause", "yes");
            Option("terminal", "no"); Option("input-default-bindings", "no"); Option("input-vo-keyboard", "no");
            Option("osc", "no"); Option("osd-level", "0"); Option("hwdec", "no");
            Option("profile", "sw-fast");
            if (headless) { Option("vo", "null"); Option("ao", "null"); }
            else Option("vo", "libmpv");
            Check(mpv_initialize(handle));
            if (!headless)
            {
                renderer = new(handle);
                renderer.FrameAvailable += () => PreviewFrameAvailable?.Invoke();
                renderer.Failed += message => PlaybackError?.Invoke(message);
            }
            mpv_request_log_messages(handle, "warn");
            Check(mpv_observe_property(handle, 1, "eof-reached", 3));
            eventThread = new Thread(Pump) { IsBackground = true, Name = "Video player events" };
            eventThread.Start();
        }
        catch { renderer?.Dispose(); renderer = null; if (handle != IntPtr.Zero) mpv_terminate_destroy(handle); handle = IntPtr.Zero; throw; }
    }
    public void ResizePreview(int width, int height) => renderer?.Resize(width, height);
    public PreviewFrame? TakePreviewFrame() => renderer?.TakeFrame();

    private void Pump()
    {
        while (!stopping)
        {
            var pointer = mpv_wait_event(handle, .1);
            if (pointer == IntPtr.Zero) continue;
            var ev = Marshal.PtrToStructure<MpvEvent>(pointer);
            if (ev.Id == 6)
            {
                eofNotified = false;
                activeEntryId = ev.Data == IntPtr.Zero ? -1 : Marshal.ReadInt64(ev.Data);
            }
            if (ev.Id == 8) FileLoaded?.Invoke(activeEntryId);
            if (ev.Id == 7 && ev.Data != IntPtr.Zero)
            {
                var reason = Marshal.ReadInt32(ev.Data);
                var error = Marshal.ReadInt32(ev.Data, 4);
                if (reason == 4) PlaybackError?.Invoke(error < 0 ? Error(error) : lastWarning);
                else if (reason == 0) NotifyEnd();
            }
            if (ev.Id == 2 && ev.Data != IntPtr.Zero)
            {
                var log = Marshal.PtrToStructure<MpvLog>(ev.Data);
                lastWarning = Marshal.PtrToStringUTF8(log.Text)?.Trim() ?? "Playback failed.";
            }
            if (ev.Id == 22 && ev.Userdata == 1 && ev.Data != IntPtr.Zero)
            {
                var property = Marshal.PtrToStructure<MpvProperty>(ev.Data);
                if (property.Format == 3 && property.Data != IntPtr.Zero)
                {
                    if (Marshal.ReadInt32(property.Data) != 0) NotifyEnd();
                    else eofNotified = false;
                }
            }
        }
    }
    private void NotifyEnd()
    {
        if (eofNotified) return;
        eofNotified = true;
        PlaybackEnded?.Invoke(activeEntryId, Get("path"));
    }

    private void Option(string name, string value) => Check(mpv_set_option_string(handle, name, value));
    public void Set(string name, string value) { if (handle != IntPtr.Zero) Check(mpv_set_property_string(handle, name, value)); }
    public string? Get(string name)
    {
        if (handle == IntPtr.Zero) return null;
        var pointer = mpv_get_property_string(handle, name);
        if (pointer == IntPtr.Zero) return null;
        try { return Marshal.PtrToStringUTF8(pointer); } finally { mpv_free(pointer); }
    }
    public double Position => double.TryParse(Get("time-pos"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var value) ? value : 0;
    public bool Paused => Get("pause") != "no";
    public void Load(string path)
    {
        Set("pause", "yes");
        Command("loadfile", path, "replace");
        RequestedEntryId = long.TryParse(Get("playlist/0/id"), out var id) ? id : -1;
    }
    public void Seek(double time) => Command("seek", time.ToString("0.#########", System.Globalization.CultureInfo.InvariantCulture), "absolute+exact");
    public void TogglePause() => Set("pause", Paused ? "no" : "yes");
    public void Command(params string[] args)
    {
        if (handle == IntPtr.Zero) return;
        var strings = args.Select(Marshal.StringToCoTaskMemUTF8).ToArray();
        var list = Marshal.AllocHGlobal(IntPtr.Size * (strings.Length + 1));
        try
        {
            for (var i = 0; i < strings.Length; i++) Marshal.WriteIntPtr(list, i * IntPtr.Size, strings[i]);
            Marshal.WriteIntPtr(list, strings.Length * IntPtr.Size, IntPtr.Zero);
            Check(mpv_command(handle, list));
        }
        finally { foreach (var value in strings) Marshal.FreeCoTaskMem(value); Marshal.FreeHGlobal(list); }
    }
    private static void Check(int result) { if (result < 0) throw new InvalidOperationException(Error(result)); }
    private static string Error(int result) => Marshal.PtrToStringUTF8(mpv_error_string(result)) ?? "Playback failed.";
    public void Dispose()
    {
        if (Interlocked.Exchange(ref disposed, 1) != 0) return;
        stopping = true;
        eventThread?.Join();
        renderer?.Dispose(); renderer = null;
        if (handle != IntPtr.Zero) { mpv_terminate_destroy(handle); handle = IntPtr.Zero; }
    }

    [StructLayout(LayoutKind.Sequential)] private struct MpvEvent { public int Id; public int Error; public ulong Userdata; public IntPtr Data; }
    [StructLayout(LayoutKind.Sequential)] private struct MpvLog { public IntPtr Prefix; public IntPtr Level; public IntPtr Text; public int LogLevel; }
    [StructLayout(LayoutKind.Sequential)] private struct MpvProperty { public IntPtr Name; public int Format; public IntPtr Data; }
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_create();
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_initialize(IntPtr context);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_terminate_destroy(IntPtr context);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_wait_event(IntPtr context, double timeout);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_set_option_string(IntPtr context, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string value);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_set_property_string(IntPtr context, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string value);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_get_property_string(IntPtr context, [MarshalAs(UnmanagedType.LPUTF8Str)] string name);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_command(IntPtr context, IntPtr arguments);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_request_log_messages(IntPtr context, [MarshalAs(UnmanagedType.LPUTF8Str)] string level);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_observe_property(IntPtr context, ulong userdata, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int format);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_free(IntPtr data);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_error_string(int error);
}
