using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace SimpleVideoEditor.Controls;

/// <summary>An in-window drag preview; it never creates another rendering window.</summary>
public sealed class ClipDragPreview : IDisposable
{
    private readonly FrameworkElement root;
    private readonly AdornerLayer layer;
    public ClipDragAdorner Preview { get; }
    private bool disposed;

    private ClipDragPreview(FrameworkElement root, AdornerLayer layer, MediaClip clip)
    {
        this.root = root; this.layer = layer;
        Preview = new(root, clip); layer.Add(Preview);
    }
    public static ClipDragPreview? Attach(FrameworkElement source, MediaClip clip)
    {
        if (Window.GetWindow(source)?.Content is not FrameworkElement root || AdornerLayer.GetAdornerLayer(root) is not { } layer) return null;
        return new(root, layer, clip);
    }
    public void MoveTo(Point cursor)
    {
        if (disposed) return;
        Preview.Visibility = cursor.X >= 0 && cursor.Y >= 0 && cursor.X < root.ActualWidth && cursor.Y < root.ActualHeight ? Visibility.Visible : Visibility.Hidden;
        Preview.Position = new(cursor.X + 14, cursor.Y + 16); Preview.InvalidateVisual();
    }
    public static DragDropEffects Run(FrameworkElement source, MediaClip clip, string format, DragDropEffects effects)
    {
        using var preview = Attach(source, clip);
        void FollowCursor(object sender, GiveFeedbackEventArgs e)
        {
            // WPF's cached mouse position can stay at the drag origin during the native drag loop.
            if (preview != null && GetCursorPos(out var point)) preview.MoveTo(preview.root.PointFromScreen(new(point.X, point.Y)));
        }
        void FollowDrag(object sender, DragEventArgs e) { if (preview != null) preview.MoveTo(e.GetPosition(preview.root)); }
        source.GiveFeedback += FollowCursor;
        if (preview != null) { preview.MoveTo(Mouse.GetPosition(preview.root)); preview.root.PreviewDragOver += FollowDrag; }
        try { return DragDrop.DoDragDrop(source, new DataObject(format, clip), effects); }
        finally { source.GiveFeedback -= FollowCursor; if (preview != null) preview.root.PreviewDragOver -= FollowDrag; }
    }
    public void Dispose() { if (disposed) return; disposed = true; layer.Remove(Preview); }
    [StructLayout(LayoutKind.Sequential)] private struct CursorPoint { public int X, Y; }
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool GetCursorPos(out CursorPoint point);
}

public sealed class ClipDragAdorner : Adorner
{
    private readonly MediaClip clip;
    private readonly BitmapSource? thumbnail;
    public Point Position { get; internal set; }
    public bool HasThumbnail => thumbnail != null;
    public ClipDragAdorner(UIElement root, MediaClip clip) : base(root)
    {
        this.clip = clip; IsHitTestVisible = false; Opacity = .94;
        if (clip.Thumbnail is { } path)
        {
            try
            {
                var image = new BitmapImage(); image.BeginInit(); image.CacheOption = BitmapCacheOption.OnLoad;
                image.UriSource = new Uri(path, UriKind.Absolute); image.DecodePixelWidth = 180; image.EndInit(); image.Freeze(); thumbnail = image;
            }
            catch { /* The clip is still draggable when its thumbnail is unavailable. */ }
        }
    }
    protected override void OnRender(DrawingContext dc)
    {
        dc.PushTransform(new TranslateTransform(Position.X, Position.Y));
        dc.DrawRoundedRectangle(new SolidColorBrush(Color.FromRgb(32, 54, 78)), new Pen(new SolidColorBrush(Color.FromRgb(37, 217, 233)), 1), new(0, 0, 180, 136), 6, 6);
        var bounds = new Rect(8, 8, 164, 92);
        dc.DrawRectangle(Brushes.Black, null, bounds);
        if (thumbnail != null)
        {
            var scale = Math.Max(bounds.Width / thumbnail.PixelWidth, bounds.Height / thumbnail.PixelHeight);
            var width = thumbnail.PixelWidth * scale; var height = thumbnail.PixelHeight * scale;
            dc.PushClip(new RectangleGeometry(bounds));
            dc.DrawImage(thumbnail, new(bounds.X + (bounds.Width - width) / 2, bounds.Y + (bounds.Height - height) / 2, width, height)); dc.Pop();
        }
        Label(dc, clip.Name, 105, 12, Brushes.White);
        Label(dc, Timecode.Format(clip.KeptDuration), 122, 10, new SolidColorBrush(Color.FromRgb(180, 200, 220)));
        dc.Pop();
    }
    private void Label(DrawingContext dc, string value, double y, double size, Brush color)
    {
        var text = new FormattedText(value, CultureInfo.CurrentUICulture, FlowDirection.LeftToRight, new Typeface("Segoe UI"), size, color, VisualTreeHelper.GetDpi(this).PixelsPerDip)
        { MaxTextWidth = 164, MaxTextHeight = size * 1.6, Trimming = TextTrimming.CharacterEllipsis };
        dc.DrawText(text, new(8, y));
    }
}
