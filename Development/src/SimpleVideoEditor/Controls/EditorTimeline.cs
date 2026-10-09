using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace SimpleVideoEditor.Controls;

/// <summary>Both lanes represent the same linked clip edits.</summary>
public sealed class EditorTimeline : Grid
{
    public const string LibraryDragFormat = "SimpleVideoEditor.Source";
    private readonly ScrollViewer scroll;
    private readonly TimelineSurface surface;
    private ObservableCollection<MediaClip>? clips;
    private readonly HashSet<MediaClip> subscribed = [];
    private readonly HashSet<AudioWaveform> waveforms = [];
    private double pixelsPerSecond = 10;
    private double position;
    private MediaClip? selected;
    private readonly Dictionary<MediaClip, double> offsets = [];
    public event Action<double>? GapSeekRequested;
    public event Action<double>? PositionChanged;
    public event Action<MediaClip, double>? SeekRequested;
    public event Action<MediaClip>? SelectionRequested;
    public event Action? EditStarted;
    public event Action<double, double>? RangeChanged;
    public event Action? EditCompleted;
    public event Action? EditCanceled;
    public event Action<MediaClip, double>? SourceDropped;
    public event Action<string[], double>? FilesDropped;
    public event Action<MediaClip>? MenuRequested;

    public EditorTimeline()
    {
        ColumnDefinitions.Add(new() { Width = new GridLength(76) });
        ColumnDefinitions.Add(new() { Width = new GridLength(1, GridUnitType.Star) });
        Background = Brush("#11171C");
        Focusable = true;
        var labels = new StackPanel { Margin = new Thickness(10, 28, 0, 0), IsHitTestVisible = false };
        labels.Children.Add(new TextBlock { Text = "Video", Height = 82, Padding = new Thickness(0, 26, 0, 0), FontWeight = FontWeights.SemiBold, Foreground = Brush("#B5CFE5") });
        labels.Children.Add(new TextBlock { Text = "Audio", Height = 58, Padding = new Thickness(0, 18, 0, 0), FontWeight = FontWeights.SemiBold, Foreground = Brush("#A8DADF") });
        Children.Add(labels);
        surface = new(this) { Height = 182, Focusable = true, AllowDrop = true };
        scroll = new() { Content = surface, HorizontalScrollBarVisibility = ScrollBarVisibility.Auto, VerticalScrollBarVisibility = ScrollBarVisibility.Disabled, CanContentScroll = false };
        SetColumn(scroll, 1); Children.Add(scroll);
        scroll.ScrollChanged += (_, e) => { if (e.HorizontalChange != 0 || e.ViewportWidthChange != 0) surface.InvalidateVisual(); };
        SizeChanged += (_, _) => Refresh();
        IsEnabledChanged += (_, _) => { surface.Opacity = IsEnabled ? 1 : .45; };
    }
    public ObservableCollection<MediaClip>? Clips
    {
        get => clips;
        set { if (clips != null) clips.CollectionChanged -= CollectionChanged; clips = value; if (clips != null) clips.CollectionChanged += CollectionChanged; RefreshSubscriptions(); }
    }
    public MediaClip? SelectedClip { get => selected; set { if (selected == value) return; selected = value; surface.InvalidateVisual(); } }
    public double Position { get => position; set { position = Math.Clamp(value, 0, Duration); surface.UpdatePlayhead(); } }
    public double Duration { get; private set; }
    public double PixelsPerSecond => pixelsPerSecond;
    public double ClipOffset(MediaClip clip) => offsets.GetValueOrDefault(clip);
    public double PlaceClip(MediaClip clip, double requested) => TimelineLayout.AvailablePosition(Clips ?? [], clip, requested, 10 / pixelsPerSecond);
    public (MediaClip Clip, double SourcePosition)? Locate(double time)
    {
        if (Clips == null || Clips.Count == 0) return null;
        time = Math.Clamp(time, 0, Duration);
        foreach (var clip in Clips)
        {
            var offset = ClipOffset(clip);
            if (time >= offset - 1e-9 && (time < offset + clip.KeptDuration - 1e-9 || time == Duration && clip == Clips[^1])) return (clip, clip.Start + Math.Clamp(time - offset, 0, clip.KeptDuration));
        }
        return null;
    }
    public void Fit() { pixelsPerSecond = Math.Clamp(Math.Max(1, scroll.ActualWidth - 40) / Math.Max(1, Duration), .0001, 2000); scroll.ScrollToHorizontalOffset(0); Refresh(); }
    public void Zoom(double factor)
    {
        var anchor = position * pixelsPerSecond - scroll.HorizontalOffset;
        var maximum = Math.Min(2000, 900000 / Math.Max(1, Duration));
        pixelsPerSecond = Math.Clamp(pixelsPerSecond * factor, Math.Min(.0001, maximum), maximum);
        Refresh(); scroll.ScrollToHorizontalOffset(position * pixelsPerSecond - anchor);
    }
    public void RevealPlayhead()
    {
        var x = position * pixelsPerSecond;
        if (x < scroll.HorizontalOffset || x > scroll.HorizontalOffset + scroll.ViewportWidth - 20) scroll.ScrollToHorizontalOffset(Math.Max(0, x - 40));
    }
    public void CancelGesture() => surface.CancelEdit();
    public void Refresh()
    {
        offsets.Clear(); double end = 0; Duration = 0;
        foreach (var clip in Clips ?? []) { var start = clip.TimelineStart ?? end; offsets[clip] = start; end = start + clip.KeptDuration; Duration = Math.Max(Duration, end); }
        // Avoid WPF's layout size limit for long source recordings at large zoom levels.
        pixelsPerSecond = Math.Min(pixelsPerSecond, 900000 / Math.Max(1, Duration));
        surface.Width = Math.Max(Math.Max(1, ActualWidth - 76), Duration * pixelsPerSecond + 40);
        surface.UpdatePlayhead();
        surface.InvalidateVisual();
    }
    private void CollectionChanged(object? sender, NotifyCollectionChangedEventArgs e) => RefreshSubscriptions();
    private void RefreshSubscriptions()
    {
        foreach (var clip in subscribed) clip.PropertyChanged -= ClipChanged;
        foreach (var waveform in waveforms) waveform.PropertyChanged -= WaveformChanged;
        subscribed.Clear();
        waveforms.Clear();
        foreach (var clip in Clips ?? [])
        {
            subscribed.Add(clip); clip.PropertyChanged += ClipChanged;
            if (waveforms.Add(clip.Waveform)) clip.Waveform.PropertyChanged += WaveformChanged;
        }
        surface.ClearWaveformCache();
        if (clips == null || clips.Count == 0) surface.ClearThumbnails();
        Refresh();
    }
    private void ClipChanged(object? sender, PropertyChangedEventArgs e) { if (e.PropertyName is nameof(MediaClip.Start) or nameof(MediaClip.End) or nameof(MediaClip.TimelineStart)) Refresh(); }
    private void WaveformChanged(object? sender, PropertyChangedEventArgs e) { surface.ClearWaveformCache(); surface.InvalidateVisual(); }
    private static SolidColorBrush Brush(string color) { var brush = (SolidColorBrush)new BrushConverter().ConvertFromString(color)!; brush.Freeze(); return brush; }

    private sealed class TimelineSurface(EditorTimeline owner) : FrameworkElement
    {
        private readonly Dictionary<string, BitmapImage?> thumbnails = new(StringComparer.OrdinalIgnoreCase);
        public void ClearThumbnails() => thumbnails.Clear();
        // Keep the playhead as a separate retained drawing. Playback changes only its
        // transform, without rebuilding clip labels, thumbnails, or waveform geometry.
        private readonly DrawingGroup playhead = CreatePlayhead();
        public void UpdatePlayhead() => ((TranslateTransform)playhead.Transform).X = owner.position * owner.pixelsPerSecond;
        private static DrawingGroup CreatePlayhead()
        {
            var drawing = new DrawingGroup { Transform = new TranslateTransform() };
            var brush = Brush("#F0F4F5");
            var pen = new Pen(brush, 1.5); pen.Freeze();
            var arrow = Geometry.Parse("M-5,0 L5,0 L0,8 Z"); arrow.Freeze();
            using (var context = drawing.Open()) { context.DrawLine(pen, new(0, 0), new(0, 172)); context.DrawGeometry(brush, null, arrow); }
            return drawing;
        }
        private sealed record WaveformDrawing(WaveformData Data, double Start, double End, double Scale, double Offset, int ViewStart, int ViewEnd, Geometry Geometry);
        private readonly Dictionary<Guid, WaveformDrawing> waveformDrawings = [];
        public void ClearWaveformCache() => waveformDrawings.Clear();
        private Point down;
        private MediaClip? pressed;
        private int edge;
        private bool editing;
        private bool scrubbing;
        private double originalStart, originalEnd;
        private double originalOffset;
        private double? freeInsertion;
        private static readonly Brush Cyan = Brush("#25D9E9");
        private void Text(DrawingContext dc, string text, double x, double y, double width, Brush brush, double size = 12)
        {
            if (width < 8) return;
            var formatted = new FormattedText(text, CultureInfo.CurrentCulture, FlowDirection.LeftToRight, new Typeface("Segoe UI"), size, brush, VisualTreeHelper.GetDpi(this).PixelsPerDip) { MaxTextWidth = Math.Max(1, width), MaxTextHeight = size * 1.6, Trimming = TextTrimming.CharacterEllipsis };
            dc.DrawText(formatted, new(x, y));
        }
        protected override void OnRender(DrawingContext dc)
        {
            dc.DrawRectangle(Brush("#11171C"), null, new(0, 0, ActualWidth, ActualHeight));
            dc.DrawRectangle(Brush("#172330"), null, new(0, 28, ActualWidth, 80));
            dc.DrawRectangle(Brush("#102024"), null, new(0, 112, ActualWidth, 56));
            var visibleStart = Math.Max(0, owner.scroll.HorizontalOffset - 100);
            var visibleEnd = visibleStart + Math.Max(500, owner.scroll.ViewportWidth) + 200;
            var steps = new double[] { .1, .25, .5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600 };
            if (!double.IsFinite(owner.pixelsPerSecond) || owner.pixelsPerSecond <= 0) return;
            var minimumStep = 85 / owner.pixelsPerSecond;
            var step = steps.FirstOrDefault(s => s >= minimumStep);
            if (step == 0)
            {
                var magnitude = Math.Pow(10, Math.Floor(Math.Log10(minimumStep)));
                step = new double[] { 1, 2, 5, 10 }.Select(m => m * magnitude).FirstOrDefault(s => s >= minimumStep);
            }
            if (!double.IsFinite(step) || step <= 0) return;
            var firstTick = Math.Floor(visibleStart / owner.pixelsPerSecond / step);
            for (var tick = 0; tick < 256; tick++)
            {
                var t = (firstTick + tick) * step;
                var x = t * owner.pixelsPerSecond;
                if (!double.IsFinite(x) || x >= Math.Min(ActualWidth, visibleEnd)) break;
                dc.DrawLine(new Pen(Brush("#39444C"), 1), new(x + .5, 24), new(x + .5, 168));
                Text(dc, t < 60 ? $"{t:0.##}s" : $"{Math.Floor(t / 60):0}:{t % 60:00}", x + 5, 4, 80, Brush("#9BA6AF"), 11);
            }
            foreach (var clip in owner.Clips ?? [])
            {
                var x = owner.ClipOffset(clip) * owner.pixelsPerSecond; var width = clip.KeptDuration * owner.pixelsPerSecond;
                if (x + width < visibleStart || x > visibleEnd) continue;
                var active = clip == owner.selected;
                var videoBorder = new Pen(active ? Cyan : Brush("#375368"), active ? 2 : 1);
                var audioBorder = new Pen(active ? Cyan : Brush("#254A54"), active ? 2 : 1);
                var videoRect = new Rect(x + 1, 32, Math.Max(.1, width - 2), 72);
                var audioRect = new Rect(x + 1, 116, Math.Max(.1, width - 2), 48);
                dc.DrawRoundedRectangle(Brush(active ? "#263F5C" : "#20364E"), videoBorder, videoRect, 4, 4);
                dc.DrawRoundedRectangle(Brush(active ? "#123138" : "#12282D"), audioBorder, audioRect, 4, 4);
                dc.PushClip(new RectangleGeometry(videoRect));
                if (width > 42 && clip.Thumbnail is { } path)
                {
                    if (!thumbnails.TryGetValue(path, out var thumbnail))
                    {
                        try { thumbnail = new BitmapImage(); thumbnail.BeginInit(); thumbnail.CacheOption = BitmapCacheOption.OnLoad; thumbnail.UriSource = new Uri(path, UriKind.Absolute); thumbnail.DecodePixelWidth = 140; thumbnail.EndInit(); thumbnail.Freeze(); } catch { thumbnail = null; }
                        thumbnails[path] = thumbnail;
                    }
                    if (thumbnail != null) dc.DrawImage(thumbnail, new Rect(x + 6, 39, Math.Min(82, width - 12), 44));
                }
                var textX = x + (width > 140 ? 98 : 8);
                if (width > 140) { Text(dc, clip.Name, textX, 41, width - 108, Brush("#F0F4F5")); Text(dc, Timecode.Format(clip.KeptDuration), textX, 63, width - 108, Brush("#B4C8DC"), 10); }
                dc.Pop();
                DrawWaveform(dc, clip, audioRect, x, width, visibleStart, visibleEnd);
                if (active) { DrawTrimHandles(dc, videoRect, 36); DrawTrimHandles(dc, audioRect, 26); }
            }
            if (owner.Duration == 0) Text(dc, "Drag recordings here", 22, 60, Math.Max(100, owner.scroll.ViewportWidth - 50), Brush("#9BA6AF"), 13);
            if (freeInsertion is { } dropTime) dc.DrawLine(new Pen(Cyan, 3), new(dropTime * owner.pixelsPerSecond, 28), new(dropTime * owner.pixelsPerSecond, 168));
            dc.DrawDrawing(playhead);
        }
        private static void DrawTrimHandles(DrawingContext dc, Rect bounds, double height)
        {
            // Tiny clips keep their selection outline without handles overlapping adjacent clips.
            if (bounds.Width < 10) return;
            var width = Math.Min(6, bounds.Width / 4);
            var y = bounds.Y + (bounds.Height - height) / 2;
            foreach (var x in new[] { bounds.X + 1, bounds.Right - width - 1 })
            {
                dc.DrawRoundedRectangle(Cyan, null, new(x, y, width, height), 2, 2);
                dc.DrawLine(new Pen(Brush("#F0F4F5"), 1), new(x + width / 2, y + height / 2 - 5), new(x + width / 2, y + height / 2 + 5));
            }
        }
        private void DrawWaveform(DrawingContext dc, MediaClip clip, Rect bounds, double x, double width, double visibleStart, double visibleEnd)
        {
            dc.PushClip(new RectangleGeometry(bounds));
            var baseline = new Pen(Brush(clip.HasAudio ? "#315864" : "#293D46"), 1);
            if (clip.Waveform.IsLoading) baseline.DashStyle = DashStyles.Dot;
            dc.DrawLine(baseline, new(x + 5, 140), new(x + width - 5, 140));
            if (clip.HasAudio && clip.Waveform.Data is { } data)
            {
                // Cache the waveform geometry: moving the playhead does not rescan audio peaks.
                var viewStart = (int)Math.Floor(visibleStart / 100) * 100;
                var viewEnd = (int)Math.Ceiling(visibleEnd / 100) * 100;
                if (!waveformDrawings.TryGetValue(clip.SectionId, out var cached) || cached.Data != data || cached.Start != clip.Start || cached.End != clip.End || cached.Scale != owner.pixelsPerSecond || cached.Offset != x || cached.ViewStart != viewStart || cached.ViewEnd != viewEnd)
                {
                    var geometry = new StreamGeometry();
                    using (var drawing = geometry.Open())
                    {
                        var from = Math.Max(x + 7, viewStart);
                        var to = Math.Min(x + width - 7, viewEnd);
                        for (var pixel = Math.Floor(from / 2) * 2; pixel < to; pixel += 2)
                        {
                            var start = clip.Start + Math.Max(0, pixel - x) / owner.pixelsPerSecond;
                            var end = Math.Min(clip.End, clip.Start + (pixel + 2 - x) / owner.pixelsPerSecond);
                            // Square-root display scale makes quiet detail visible; silence stays flat.
                            var amplitude = Math.Sqrt(data.PeakBetween(start, end)) * 20;
                            if (amplitude < .15) continue;
                            drawing.BeginFigure(new(pixel, 140 - amplitude), true, true);
                            drawing.LineTo(new(pixel + 1.5, 140 - amplitude), true, false);
                            drawing.LineTo(new(pixel + 1.5, 140 + amplitude), true, false);
                            drawing.LineTo(new(pixel, 140 + amplitude), true, false);
                        }
                    }
                    geometry.Freeze();
                    cached = new(data, clip.Start, clip.End, owner.pixelsPerSecond, x, viewStart, viewEnd, geometry);
                    waveformDrawings[clip.SectionId] = cached;
                }
                dc.DrawGeometry(Brush(clip == owner.selected ? "#63EFF7" : "#50D9E8"), null, cached.Geometry);
            }
            dc.Pop();
        }
        private MediaClip? Hit(Point point) => point.Y is >= 28 and <= 168 ? owner.Locate(point.X / owner.pixelsPerSecond)?.Clip is { } clip && point.X < owner.Duration * owner.pixelsPerSecond ? clip : null : null;
        private int Edge(MediaClip clip, Point point)
        {
            var x = owner.ClipOffset(clip) * owner.pixelsPerSecond;
            var end = x + clip.KeptDuration * owner.pixelsPerSecond;
            var tolerance = Math.Min(8, (end - x) / 3);
            return point.X - x <= tolerance ? -1 : end - point.X <= tolerance ? 1 : 0;
        }
        protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
        {
            BeginPointer(e.GetPosition(this)); e.Handled = true;
        }
        private void BeginPointer(Point point)
        {
            Focus(); down = point; pressed = Hit(down); edge = pressed == null ? 0 : Edge(pressed, down);
            scrubbing = point.Y < 28 && owner.Clips?.Count > 0;
            if (pressed != null) owner.SelectionRequested?.Invoke(pressed);
            if (pressed != null) { originalStart = pressed.Start; originalEnd = pressed.End; originalOffset = owner.ClipOffset(pressed); }
            if (pressed != null) CaptureMouse();
            // A clip-body press may become a drag. Seek only after a simple click
            // completes so moving a clip does not replace its current preview frame.
            if (edge == 0 && (pressed == null || scrubbing)) SeekAt(down.X);
            if (scrubbing) CaptureMouse();
        }
        protected override void OnMouseMove(MouseEventArgs e)
        {
            MovePointer(e.GetPosition(this), e.LeftButton == MouseButtonState.Pressed);
            if (editing) e.Handled = true;
        }
        private void MovePointer(Point point, bool held)
        {
            if (held && scrubbing)
            {
                SeekAt(point.X);
                return;
            }
            if (!held || pressed == null)
            {
                var hit = Hit(point); Cursor = hit != null && Edge(hit, point) != 0 ? Cursors.SizeWE : Cursors.Arrow;
                ToolTip = hit == null ? null : $"{hit.Name}\n{hit.TrimLabel}" + (point.Y >= 112 ? hit.HasAudio ? hit.Waveform.IsLoading ? "\nLoading waveform…" : hit.Waveform.Error != null ? "\nWaveform unavailable · " + hit.Waveform.Error : "\nLinked audio" : "\nNo audio" : ""); return;
            }
            if (Math.Abs(point.X - down.X) < SystemParameters.MinimumHorizontalDragDistance && !editing) return;
            if (edge != 0)
            {
                if (!editing)
                {
                    editing = true; owner.EditStarted?.Invoke();
                }
                var frame = 1 / pressed.FrameRate;
                var delta = Math.Round((point.X - down.X) / owner.pixelsPerSecond / frame) * frame;
                var start = edge < 0 ? Math.Clamp(originalStart + delta, 0, originalEnd - frame) : originalStart;
                var end = edge > 0 ? Math.Clamp(originalEnd + delta, originalStart + frame, pressed.Duration) : originalEnd;
                owner.RangeChanged?.Invoke(start, end);
            }
            else
            {
                if (!editing) { editing = true; owner.EditStarted?.Invoke(); }
                var requested = originalOffset + (point.X - down.X) / owner.pixelsPerSecond;
                freeInsertion = owner.PlaceClip(pressed, requested);
                owner.PositionChanged?.Invoke(requested);
                var viewportX = TranslatePoint(point, owner.scroll).X;
                if (viewportX < 35) owner.scroll.ScrollToHorizontalOffset(owner.scroll.HorizontalOffset - 18);
                else if (viewportX > owner.scroll.ActualWidth - 35) owner.scroll.ScrollToHorizontalOffset(owner.scroll.HorizontalOffset + 18);
            }
        }
        protected override void OnMouseLeftButtonUp(MouseButtonEventArgs e) { ReleaseMouseCapture(); FinishEdit(); e.Handled = true; }
        protected override void OnLostMouseCapture(MouseEventArgs e) { FinishEdit(); base.OnLostMouseCapture(e); }
        private void SeekAt(double x)
        {
            var time = Math.Clamp(x / owner.pixelsPerSecond, 0, owner.Duration);
            if (owner.Locate(time) is { } at) owner.SeekRequested?.Invoke(at.Clip, at.SourcePosition);
            else if (owner.Clips?.Count > 0) owner.GapSeekRequested?.Invoke(time);
        }
        private void FinishEdit()
        {
            if (pressed != null && edge == 0 && !editing && !scrubbing) SeekAt(down.X);
            freeInsertion = null;
            pressed = null; edge = 0; scrubbing = false;
            if (editing) { editing = false; owner.EditCompleted?.Invoke(); }
            owner.Refresh();
        }
        public void CancelEdit()
        {
            if (!editing) return;
            editing = false; pressed = null;
            ReleaseMouseCapture(); FinishEdit(); owner.EditCanceled?.Invoke();
        }
        protected override void OnMouseRightButtonDown(MouseButtonEventArgs e)
        {
            if (Hit(e.GetPosition(this)) is { } clip) { Focus(); owner.SelectionRequested?.Invoke(clip); owner.MenuRequested?.Invoke(clip); e.Handled = true; }
        }
        protected override void OnMouseWheel(MouseWheelEventArgs e)
        {
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control)) owner.Zoom(e.Delta > 0 ? 1.25 : .8);
            else owner.scroll.ScrollToHorizontalOffset(owner.scroll.HorizontalOffset - e.Delta);
            e.Handled = true;
        }
        protected override void OnDragOver(DragEventArgs e)
        {
            var library = e.Data.GetDataPresent(LibraryDragFormat); var files = e.Data.GetDataPresent(DataFormats.FileDrop);
            e.Effects = library || files ? DragDropEffects.Copy : DragDropEffects.None;
            freeInsertion = library && e.Data.GetData(LibraryDragFormat) is MediaClip dragged
                ? owner.PlaceClip(dragged, e.GetPosition(this).X / owner.pixelsPerSecond) : null;
            var viewportX = e.GetPosition(owner.scroll).X;
            if (viewportX < 35) owner.scroll.ScrollToHorizontalOffset(owner.scroll.HorizontalOffset - 18);
            else if (viewportX > owner.scroll.ActualWidth - 35) owner.scroll.ScrollToHorizontalOffset(owner.scroll.HorizontalOffset + 18);
            InvalidateVisual(); e.Handled = true;
        }
        protected override void OnDragLeave(DragEventArgs e) { freeInsertion = null; InvalidateVisual(); }
        protected override void OnDrop(DragEventArgs e)
        {
            var time = Math.Max(0, e.GetPosition(this).X / owner.pixelsPerSecond);
            if (e.Data.GetData(LibraryDragFormat) is MediaClip source) { owner.SourceDropped?.Invoke(source, time); e.Handled = true; }
            else if (e.Data.GetData(DataFormats.FileDrop) is string[] files) { owner.FilesDropped?.Invoke(files, time); e.Handled = true; }
            freeInsertion = null; InvalidateVisual();
        }
    }
}
