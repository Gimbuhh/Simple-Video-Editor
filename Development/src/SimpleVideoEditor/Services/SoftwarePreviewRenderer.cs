using System.Buffers;
using System.Runtime.InteropServices;

namespace SimpleVideoEditor.Services;

/// <summary>CPU presentation with no graphics device, swap chain, or GPU fallback.</summary>
internal sealed class SoftwarePreviewRenderer : IDisposable
{
    private IntPtr context;
    private readonly AutoResetEvent wake = new(false);
    private readonly UpdateCallback callback;
    private readonly Thread thread;
    private volatile bool stopping;
    private long requestedSize = (540L << 32) | 960;
    private PreviewFrame? latest;
    private int disposed;
    public event Action? FrameAvailable;
    public event Action<string>? Failed;

    public SoftwarePreviewRenderer(IntPtr player)
    {
        callback = _ => wake.Set(); // Never call libmpv from its update callback.
        var api = Marshal.StringToCoTaskMemUTF8("sw");
        try { Check(mpv_render_context_create(out context, player, [new(1, api), new(0, IntPtr.Zero)])); }
        finally { Marshal.FreeCoTaskMem(api); }
        mpv_render_context_set_update_callback(context, callback, IntPtr.Zero);
        thread = new(RenderLoop) { IsBackground = true, Name = "Software video rendering" };
        thread.Start();
    }
    public void Resize(int width, int height)
    {
        var size = ((long)Math.Clamp(height, 1, 720) << 32) | (uint)Math.Clamp(width, 1, 1280);
        if (Interlocked.Exchange(ref requestedSize, size) != size && !stopping) wake.Set();
    }
    public PreviewFrame? TakeFrame() => Interlocked.Exchange(ref latest, null);
    private void RenderLoop()
    {
        IntPtr allocation = IntPtr.Zero;
        var format = Marshal.StringToCoTaskMemUTF8("bgr0");
        var dimensions = Marshal.AllocHGlobal(8);
        var strideValue = Marshal.AllocHGlobal(IntPtr.Size);
        long currentSize = 0;
        int width = 0, height = 0, stride = 0;
        IntPtr pixels = IntPtr.Zero;
        try
        {
            while (!stopping)
            {
                wake.WaitOne();
                if (stopping) break;
                var size = Interlocked.Read(ref requestedSize);
                var resized = size != currentSize;
                var flags = mpv_render_context_update(context);
                if (!resized && (flags & 1) == 0) continue;
                if (resized)
                {
                    width = (int)(size & uint.MaxValue); height = (int)(size >> 32);
                    stride = (width * 4 + 63) & ~63;
                    if (allocation != IntPtr.Zero) Marshal.FreeHGlobal(allocation);
                    allocation = Marshal.AllocHGlobal(stride * height + 63);
                    pixels = new IntPtr((allocation.ToInt64() + 63) & ~63L);
                    Marshal.WriteInt32(dimensions, width); Marshal.WriteInt32(dimensions, 4, height);
                    Marshal.WriteIntPtr(strideValue, new IntPtr(stride));
                    currentSize = size;
                }
                Check(mpv_render_context_render(context, [new(17, dimensions), new(18, format), new(19, strideValue), new(20, pixels), new(0, IntPtr.Zero)]));
                var frame = new PreviewFrame(width, height, stride);
                Marshal.Copy(pixels, frame.Pixels, 0, stride * height);
                Interlocked.Exchange(ref latest, frame)?.Dispose();
                FrameAvailable?.Invoke();
                mpv_render_context_report_swap(context);
            }
        }
        catch (Exception ex) { Failed?.Invoke("Software preview failed: " + ex.Message); }
        finally
        {
            if (allocation != IntPtr.Zero) Marshal.FreeHGlobal(allocation);
            Marshal.FreeHGlobal(dimensions); Marshal.FreeHGlobal(strideValue); Marshal.FreeCoTaskMem(format);
        }
    }
    public void Dispose()
    {
        if (Interlocked.Exchange(ref disposed, 1) != 0) return;
        stopping = true; wake.Set(); thread.Join();
        mpv_render_context_set_update_callback(context, null, IntPtr.Zero);
        mpv_render_context_free(context); context = IntPtr.Zero;
        Interlocked.Exchange(ref latest, null)?.Dispose(); wake.Dispose();
        GC.KeepAlive(callback);
    }
    private static void Check(int result) { if (result < 0) throw new InvalidOperationException(Marshal.PtrToStringUTF8(mpv_error_string(result)) ?? "libmpv rendering error"); }
    [StructLayout(LayoutKind.Sequential)] private readonly struct Parameter(int type, IntPtr data) { public readonly int Type = type; public readonly IntPtr Data = data; }
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate void UpdateCallback(IntPtr state);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_render_context_create(out IntPtr context, IntPtr player, [In] Parameter[] parameters);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_render_context_set_update_callback(IntPtr context, UpdateCallback? callback, IntPtr state);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern ulong mpv_render_context_update(IntPtr context);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_render_context_render(IntPtr context, [In] Parameter[] parameters);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_render_context_report_swap(IntPtr context);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_render_context_free(IntPtr context);
    [DllImport("libmpv-2.dll", CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_error_string(int error);
}

public sealed class PreviewFrame : IDisposable
{
    public int Width { get; }
    public int Height { get; }
    public int Stride { get; }
    public byte[] Pixels { get; }
    private int disposed;
    internal PreviewFrame(int width, int height, int stride) { Width = width; Height = height; Stride = stride; Pixels = ArrayPool<byte>.Shared.Rent(stride * height); }
    public void Dispose() { if (Interlocked.Exchange(ref disposed, 1) == 0) ArrayPool<byte>.Shared.Return(Pixels); }
}
