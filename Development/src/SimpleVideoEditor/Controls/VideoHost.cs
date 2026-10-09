using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SimpleVideoEditor.Services;

namespace SimpleVideoEditor.Controls;

/// <summary>Ordinary WPF bitmap presentation; no native video window or swap chain.</summary>
public sealed class VideoHost : FrameworkElement
{
    private NativePlayer? player;
    private WriteableBitmap? bitmap;
    private int queued;
    private bool initialized;
    public event Action? Ready;
    public long PresentedFrames { get; private set; }
    public BitmapSource? PreviewBitmap => bitmap;
    private bool showBlank;
    public bool ShowBlank { get => showBlank; set { showBlank = value; InvalidateVisual(); } }

    public VideoHost()
    {
        Loaded += (_, _) => { if (!initialized) { initialized = true; Ready?.Invoke(); } };
        SizeChanged += (_, _) => ResizePreview();
    }
    public void Attach(NativePlayer value)
    {
        Detach(); player = value;
        player.PreviewFrameAvailable += QueueFrame;
        ResizePreview(); QueueFrame();
    }
    public void Detach()
    {
        if (player != null) player.PreviewFrameAvailable -= QueueFrame;
        player = null;
    }
    public void Clear() { bitmap = null; InvalidateVisual(); }
    private void ResizePreview()
    {
        var dpi = VisualTreeHelper.GetDpi(this);
        var width = Math.Max(2, ActualWidth * dpi.DpiScaleX);
        var height = Math.Max(2, ActualHeight * dpi.DpiScaleY);
        var scale = Math.Min(1, Math.Min(1280 / width, 720 / height));
        player?.ResizePreview(Math.Max(2, (int)(width * scale)), Math.Max(2, (int)(height * scale)));
    }
    private void QueueFrame()
    {
        if (Dispatcher.HasShutdownStarted || Interlocked.Exchange(ref queued, 1) != 0) return;
        Dispatcher.BeginInvoke(DispatcherPriority.Render, () =>
        {
            Interlocked.Exchange(ref queued, 0);
            using var frame = player?.TakePreviewFrame();
            if (frame == null) return;
            if (bitmap == null || bitmap.PixelWidth != frame.Width || bitmap.PixelHeight != frame.Height)
                bitmap = new(frame.Width, frame.Height, 96, 96, PixelFormats.Bgr32, null);
            bitmap.WritePixels(new Int32Rect(0, 0, frame.Width, frame.Height), frame.Pixels, frame.Stride, 0);
            PresentedFrames++; InvalidateVisual();
        });
    }
    protected override void OnRender(DrawingContext context)
    {
        var bounds = new Rect(0, 0, ActualWidth, ActualHeight);
        context.DrawRectangle(Brushes.Black, null, bounds);
        if (!showBlank && bitmap != null) context.DrawImage(bitmap, bounds);
    }
}
