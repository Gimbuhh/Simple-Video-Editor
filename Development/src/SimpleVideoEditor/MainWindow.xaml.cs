using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using Microsoft.Win32;
using SimpleVideoEditor.Services;
using SimpleVideoEditor.Controls;

namespace SimpleVideoEditor;

public partial class MainWindow : Window
{
    public ObservableCollection<MediaClip> Clips { get; } = [];
    public ObservableCollection<MediaClip> Sources { get; } = [];
    public ICollectionView RecordingView { get; }
    private sealed record EditorSnapshot(List<MediaClip> Clips, List<MediaClip> Sources, Guid? SelectedId);
    private EditorSnapshot Snapshot() => new(Clips.Select(c => c.Clone()).ToList(), Sources.Select(c => c.Clone()).ToList(), selected?.SectionId);
    private ProjectDocument CaptureProject() => ProjectStore.Capture(Clips, Sources);
    private bool gapPreview, gapPlaying;
    private readonly System.Diagnostics.Stopwatch gapClock = new();
    private double gapAnchor;
    private bool TimelineSelected => selected != null && Clips.Contains(selected);
    private double pendingSeek;
    private bool updatingSeek;
    private bool scrubbing, settlingScrub, scrubWasPlaying, scrubSeekQueued;
    private MediaClip? scrubClip;
    private readonly System.Diagnostics.Stopwatch scrubSettleClock = new();
    private NativePlayer? player;
    private readonly DispatcherTimer playbackTimer = new() { Interval = TimeSpan.FromMilliseconds(33) };
    private readonly DispatcherTimer recoveryTimer = new() { Interval = TimeSpan.FromSeconds(1) };
    private readonly Stack<EditorSnapshot> undo = new();
    private readonly Stack<EditorSnapshot> redo = new();
    private readonly RecoverySession recovery = new(ProjectStore.DataDirectory);
    private MediaClip? selected;
    private bool syncing;
    private bool busy;
    private bool dirty;
    private bool sequencePlayback;
    private bool resumeAfterLoad;
    private bool playerReady;
    private bool closing;
    private EditorSnapshot? pendingTrimEdit;
    private double pendingEditSeek;
    private bool pendingEditWasDirty;
    private string displayedStart = "";
    private string displayedEnd = "";
    private string? projectPath;
    private Point dragOrigin;
    private MediaClip? dragClip;
    private bool sourceKeyboardNavigation;
    private CancellationTokenSource? importCancellation;
    private CancellationTokenSource waveformCancellation = new();
    private readonly SemaphoreSlim waveformSlots = new(1);
    private readonly HashSet<Task> waveformTasks = [];

    public MainWindow()
    {
        RecordingView = new ListCollectionView(Sources);
        RecordingView.Filter = item => item is MediaClip clip && clip.Name.Contains(SearchInput?.Text.Trim() ?? "", StringComparison.OrdinalIgnoreCase);
        InitializeComponent(); DataContext = this;
        Timeline.Clips = Clips;
        Video.Ready += () =>
        {
            try
            {
                player = new();
                player.FileLoaded += entryId => Dispatcher.BeginInvoke(() =>
                {
                    if (closing || selected == null || player == null || entryId != player.RequestedEntryId || !string.Equals(player.Get("path"), selected.Path, StringComparison.OrdinalIgnoreCase)) return;
                    playerReady = true;
                    player.Set("volume", VolumeSlider.Value.ToString(System.Globalization.CultureInfo.InvariantCulture));
                    player.Seek(pendingSeek);
                    player.Set("pause", !gapPreview && resumeAfterLoad ? "no" : "yes");
                    if (!busy) StatusText.Text = "";
                    RefreshSummary();
                });
                player.VideoReconfigured += entryId => Dispatcher.BeginInvoke(() =>
                {
                    if (closing || selected == null || player == null || entryId != player.RequestedEntryId || !string.Equals(player.Get("path"), selected.Path, StringComparison.OrdinalIgnoreCase)) return;
                    UpdatePreviewAspectRatio();
                });
                player.PlaybackError += message => Dispatcher.BeginInvoke(() =>
                {
                    if (closing) return;
                    ResetScrub();
                    sequencePlayback = resumeAfterLoad = false;
                    playerReady = false;
                    RefreshSummary();
                    StatusText.Text = "Playback failed · " + message;
                    MessageBox.Show(this, message, "Playback failed", MessageBoxButton.OK, MessageBoxImage.Error);
                });
                player.PlaybackEnded += (entryId, path) => Dispatcher.BeginInvoke(() =>
                {
                    if (playerReady && selected != null && player != null && entryId == player.RequestedEntryId && string.Equals(path, selected.Path, StringComparison.OrdinalIgnoreCase)) CompletePreview();
                });
                player.Initialize(); Video.Attach(player);
                Dispatcher.BeginInvoke(LoadSelected);
            }
            catch (Exception ex) { StatusText.Text = "Playback unavailable"; MessageBox.Show(this, ex.Message, "Playback unavailable", MessageBoxButton.OK, MessageBoxImage.Error); }
        };
        Timeline.SeekRequested += SeekTimeline;
        Timeline.ScrubStarted += BeginScrub;
        Timeline.ScrubCompleted += EndScrub;
        PreviewSeek.AddHandler(Thumb.DragStartedEvent, new DragStartedEventHandler((_, _) => BeginScrub()), handledEventsToo: true);
        PreviewSeek.AddHandler(Thumb.DragCompletedEvent, new DragCompletedEventHandler((_, _) => EndScrub()), handledEventsToo: true);
        Timeline.SelectionRequested += clip => { if (selected != clip) Select(clip, gapPreview ? gapPlaying : playerReady ? player?.Paused == false : resumeAfterLoad); };
        Timeline.EditStarted += BeginTrimEdit;
        Timeline.RangeChanged += UpdateTrimEdit;
        Timeline.EditCompleted += CompleteTrimEdit;
        Timeline.EditCanceled += CancelTrimEdit;
        Timeline.SourceDropped += InsertSourceClip;
        Timeline.PositionChanged += position =>
        {
            if (selected == null || pendingTrimEdit == null) return;
            selected.TimelineStart = Timeline.PlaceClip(selected, position);
            Timeline.Position = selected.TimelineStart.Value + pendingEditSeek - selected.Start;
            PositionText.Text = Timecode.Format(Timeline.Position);
            RefreshSummary();
        };
        Timeline.GapSeekRequested += time => SeekGap(time, !scrubbing && (gapPreview ? gapPlaying : playerReady ? player?.Paused == false : resumeAfterLoad));
        Timeline.FilesDropped += async (files, dropTime) =>
        {
            if (busy) return;
            if (files.Length == 1 && files[0].EndsWith(".sveproject", StringComparison.OrdinalIgnoreCase))
            { if (await ConfirmDiscardAsync()) await OpenProjectAsync(files[0]); return; }
            var recordings = files.Where(File.Exists).Where(p => !p.EndsWith(".sveproject", StringComparison.OrdinalIgnoreCase)).Select(Path.GetFullPath).ToArray();
            await ImportAsync(recordings);
            var imported = recordings.Select(p => Sources.FirstOrDefault(c => string.Equals(c.Path, p, StringComparison.OrdinalIgnoreCase))).OfType<MediaClip>().ToArray();
            foreach (var source in imported) { InsertSourceClip(source, dropTime); dropTime = Timeline.ClipOffset(selected!) + selected!.KeptDuration; }
        };
        Timeline.MenuRequested += OpenTimelineMenu;
        playbackTimer.Tick += (_, _) => PlaybackTick(); playbackTimer.Start();
        recoveryTimer.Tick += async (_, _) =>
        {
            recoveryTimer.Stop();
            try { if (dirty) await recovery.SaveAsync(CaptureProject()); }
            catch (Exception ex) { StatusText.Text = "Recovery save failed · " + ex.Message; }
        };
        Loaded += Window_Loaded;
        RefreshSummary();
    }

    private async void Window_Loaded(object sender, RoutedEventArgs e)
    {
        var args = Environment.GetCommandLineArgs().Skip(1).ToArray();
        if (args.Length == 3 && args[0] == "--verify-package")
        {
            // Internal portable-package check: wait for real project/preview readiness
            // and use the normal asynchronous close path instead of timing WM_CLOSE.
            try
            {
                if (!await OpenProjectAsync(args[1])) throw new InvalidOperationException("The verification project did not open.");
                for (var attempt = 0; attempt < 150 && (!playerReady || Video.PreviewBitmap == null); attempt++) await Task.Delay(100);
                if (!playerReady || Video.PreviewBitmap == null) throw new InvalidOperationException("The packaged preview did not render.");
                using var process = System.Diagnostics.Process.GetCurrentProcess();
                var playbackLibrary = process.Modules.Cast<System.Diagnostics.ProcessModule>().Single(m => m.ModuleName.Equals("libmpv-2.dll", StringComparison.OrdinalIgnoreCase)).FileName;
                var result = new { PreviewWidth = Video.PreviewBitmap.PixelWidth, PreviewHeight = Video.PreviewBitmap.PixelHeight, PlaybackLibrary = playbackLibrary, ProbeTool = MediaTools.Find("ffprobe") };
                if (File.Exists(args[2])) throw new IOException("The verification result already exists.");
                await File.WriteAllTextAsync(args[2], System.Text.Json.JsonSerializer.Serialize(result));
                Close();
            }
            catch { Application.Current.Shutdown(1); }
            return;
        }
        if (args.Length > 0)
        {
            if (args.Length == 1 && args[0].EndsWith(".sveproject", StringComparison.OrdinalIgnoreCase)) await OpenProjectAsync(args[0]);
            else await ImportAsync(args.Where(File.Exists));
            return;
        }
        using var interrupted = recovery.TryClaimLatest();
        if (interrupted != null)
        {
            if (MessageBox.Show(this, "An interrupted editing session was found. Restore it?", "Recover session", MessageBoxButton.YesNo, MessageBoxImage.Question) == MessageBoxResult.Yes)
            {
                if (await OpenProjectAsync(interrupted.Path, true)) interrupted.Discard();
            }
            else interrupted.Discard();
        }
    }

    private void SetBusy(bool value)
    {
        busy = value; NewButton.IsEnabled = OpenButton.IsEnabled = CacheButton.IsEnabled = SearchInput.IsEnabled = AddButton.IsEnabled = SourceList.IsEnabled = Timeline.IsEnabled = !value;
        RefreshSummary();
    }
    private void RefreshSummary()
    {
        var visibleSources = RecordingView.Cast<MediaClip>().Count();
        var searching = !string.IsNullOrWhiteSpace(SearchInput.Text);
        ClipCountText.Text = searching ? $"{visibleSources} / {Sources.Count}" : Sources.Count.ToString();
        SearchEmpty.Visibility = searching && visibleSources == 0 ? Visibility.Visible : Visibility.Collapsed;
        SaveButton.IsEnabled = !busy && (Sources.Count > 0 || dirty || projectPath != null);
        TotalText.Text = $"{Timecode.Format(Timeline.Duration)} total";
        ExportButton.IsEnabled = Clips.Count > 0 && !busy;
        AddToTimelineButton.IsEnabled = SourceList.SelectedItem != null && !busy;
        AddAllToTimelineButton.Visibility = Sources.Count > 1 ? Visibility.Visible : Visibility.Collapsed;
        AddAllToTimelineButton.IsEnabled = Sources.Count > 1 && !busy;
        Grid.SetColumnSpan(AddToTimelineButton, Sources.Count > 1 ? 1 : 2);
        UndoButton.IsEnabled = undo.Count > 0 && !busy; RedoButton.IsEnabled = redo.Count > 0 && !busy;
        RemoveButton.IsEnabled = TimelineSelected && !busy;
        SplitButton.IsEnabled = TimelineSelected && !gapPreview && !busy && selected!.KeptDuration >= 2 / selected.FrameRate - .0001;
        TrimControls.IsEnabled = TimelineSelected && !busy;
        UpdateQuickTrimAvailability();
        PlayButton.IsEnabled = BackButton.IsEnabled = ForwardButton.IsEnabled = selected != null && playerReady && !busy;
        UpdatePlaybackButton();
        Title = $"{(dirty ? "* " : "")}{(projectPath == null ? "Untitled" : Path.GetFileNameWithoutExtension(projectPath))} — Simple Video Editor";
    }
    private void ArrangeClips()
    {
        foreach (var clip in Clips) clip.TimelineStart ??= Timeline.ClipOffset(clip);
        var ordered = Clips.OrderBy(c => c.TimelineStart).ToArray();
        syncing = true;
        for (int i = 0; i < ordered.Length; i++) if (Clips[i] != ordered[i]) Clips.Move(Clips.IndexOf(ordered[i]), i);
        syncing = false;
        Timeline.Refresh();
    }
    private void MarkDirty() { ArrangeClips(); dirty = true; recoveryTimer.Stop(); recoveryTimer.Start(); RefreshSummary(); }
    private void Checkpoint(EditorSnapshot? snapshot = null)
    {
        undo.Push(snapshot ?? Snapshot()); redo.Clear();
        if (undo.Count > 100) { var keep = undo.Take(100).Reverse().ToArray(); undo.Clear(); foreach (var previous in keep) undo.Push(previous); }
        RefreshSummary();
    }
    private void BeginTrimEdit()
    {
        if (!TimelineSelected || busy) return;
        ResetScrub();
        pendingEditSeek = Math.Clamp(selected!.Start + Timeline.Position - Timeline.ClipOffset(selected), selected.Start, selected.End);
        gapPreview = gapPlaying = false; gapClock.Reset(); Video.ShowBlank = false;
        sequencePlayback = resumeAfterLoad = false;
        player?.Set("pause", "yes");
        pendingTrimEdit = Snapshot();
        pendingEditWasDirty = dirty;
        recoveryTimer.Stop();
    }
    private void UpdateTrimEdit(double start, double end)
    {
        if (selected == null || busy || pendingTrimEdit == null) return;
        SetTrim(start, end, pendingTrimEdit, final: false, constrain: true);
        UpdateTrimFields();
        Timeline.Position = Timeline.ClipOffset(selected) + EditPreviewPosition(selected) - selected.Start;
        PositionText.Text = Timecode.Format(Timeline.Position);
    }
    private double EditPreviewPosition(MediaClip clip) => Math.Clamp(pendingEditSeek, clip.Start, Math.Max(clip.Start, clip.End - 1 / clip.FrameRate));
    private void CompleteTrimEdit()
    {
        if (pendingTrimEdit == null) return;
        var original = pendingTrimEdit;
        pendingTrimEdit = null;
        var previous = original.Clips.FirstOrDefault(c => c.SectionId == selected?.SectionId);
        if (selected != null && previous != null && (selected.Start != previous.Start || selected.End != previous.End))
            SetTrim(selected.Start, selected.End, original, final: true, constrain: false);
        ArrangeClips();
        if (!ProjectStore.Capture(original.Clips).Clips.SequenceEqual(ProjectStore.Capture(Clips).Clips))
        {
            Checkpoint(original);
            MarkDirty();
        }
        if (selected != null) SeekTimeline(selected, EditPreviewPosition(selected));
        if (dirty) { recoveryTimer.Stop(); recoveryTimer.Start(); }
    }
    private void CancelTrimEdit()
    {
        if (pendingTrimEdit == null) return;
        var original = pendingTrimEdit; pendingTrimEdit = null;
        Restore(original); dirty = pendingEditWasDirty;
        if (TimelineSelected) SeekTimeline(selected!, pendingEditSeek);
        if (!dirty) recoveryTimer.Stop();
        RefreshSummary();
    }
    private void Restore(EditorSnapshot snapshot)
    {
        var selectedId = selected?.SectionId;
        var selectedPath = selected?.Path;
        var selectedStart = selected?.Start;
        syncing = true; Clips.Clear(); Sources.Clear(); foreach (var source in snapshot.Sources) Sources.Add(source.Clone()); foreach (var clip in snapshot.Clips) Clips.Add(clip.Clone()); syncing = false;
        var restoredSelection = Clips.Concat(Sources).FirstOrDefault(c => c.SectionId == snapshot.SelectedId)
            ?? Clips.FirstOrDefault(c => c.SectionId == selectedId)
            ?? Clips.FirstOrDefault(c => string.Equals(c.Path, selectedPath, StringComparison.OrdinalIgnoreCase) && c.Start <= selectedStart && c.End > selectedStart)
            ?? Clips.FirstOrDefault() ?? Sources.FirstOrDefault();
        Select(restoredSelection); foreach (var source in Sources) QueueWaveform(source); MarkDirty();
    }
    private void Undo_Click(object sender, RoutedEventArgs e) { if (busy || undo.Count == 0) return; redo.Push(Snapshot()); Restore(undo.Pop()); }
    private void Redo_Click(object sender, RoutedEventArgs e) { if (busy || redo.Count == 0) return; undo.Push(Snapshot()); Restore(redo.Pop()); }

    private void Select(MediaClip? clip, bool keepSequence = false)
    {
        if (syncing) return;
        if (!scrubbing) ResetScrub();
        gapPreview = gapPlaying = false; gapClock.Reset(); Video.ShowBlank = false;
        TrimHint.Visibility = Visibility.Collapsed;
        syncing = true; selected = clip;
        SourceList.SelectedItem = clip == null ? null : Sources.FirstOrDefault(c => string.Equals(c.Path, clip.Path, StringComparison.OrdinalIgnoreCase));
        Timeline.SelectedClip = TimelineSelected ? clip : null;
        if (SourceList.SelectedItem != null)
        {
            var item = SourceList.ItemContainerGenerator.ContainerFromItem(SourceList.SelectedItem) as ListBoxItem;
            var bounds = item?.TransformToAncestor(SourceList).TransformBounds(new Rect(item.RenderSize));
            if (bounds == null || bounds.Value.Bottom <= 0 || bounds.Value.Top >= SourceList.ActualHeight) SourceList.ScrollIntoView(SourceList.SelectedItem);
        }
        syncing = false;
        if (!keepSequence) sequencePlayback = false;
        resumeAfterLoad = keepSequence;
        if (clip == null)
        {
            playerReady = false;
            player?.Command("stop"); Video.Clear(); Video.Visibility = Visibility.Collapsed; EmptyPreview.Visibility = Visibility.Visible;
            SelectedName.Text = "Preview"; SelectedDetails.Text = ""; Timeline.Position = 0;
            PreviewViewport.AspectRatio = 16d / 9;
            Video.LayoutTransform = Transform.Identity;
            PreviewSeek.Visibility = Visibility.Collapsed;
            StartInput.Text = EndInput.Text = PositionText.Text = "00:00:00.000";
        }
        else
        {
            SelectedName.Text = clip.Name; SelectedDetails.Text = $"{(TimelineSelected ? "Timeline" : "Recording")} · {clip.Height}p · {clip.FrameRate:0.##} fps";
            PreviewViewport.AspectRatio = clip.Width > 0 && clip.Height > 0 ? (double)clip.Width / clip.Height : 16d / 9;
            Video.LayoutTransform = Transform.Identity;
            pendingSeek = clip.Start;
            if (TimelineSelected) Timeline.Position = Timeline.ClipOffset(clip);
            updatingSeek = true; PreviewSeek.Maximum = clip.Duration; PreviewSeek.Value = clip.Start; updatingSeek = false;
            PreviewSeek.Visibility = TimelineSelected ? Visibility.Collapsed : Visibility.Visible;
            UpdateTrimFields(); EmptyPreview.Visibility = Visibility.Collapsed; Video.Visibility = Visibility.Visible; LoadSelected();
        }
        RefreshSummary();
    }
    private void LoadSelected()
    {
        if (selected == null || player == null) return;
        if (playerReady && string.Equals(player.Get("path"), selected.Path, StringComparison.OrdinalIgnoreCase))
        {
            UpdatePreviewAspectRatio();
            player.Seek(pendingSeek); player.Set("pause", resumeAfterLoad ? "no" : "yes"); return;
        }
        playerReady = false; RefreshSummary(); StatusText.Text = "Loading preview…";
        try { player.Load(selected.Path); } catch (Exception ex) { StatusText.Text = ex.Message; }
    }
    private void UpdatePreviewAspectRatio()
    {
        if (player == null || !double.TryParse(player.Get("video-out-params/aspect"), System.Globalization.NumberStyles.Float,
            System.Globalization.CultureInfo.InvariantCulture, out var ratio) || !double.IsFinite(ratio) || ratio <= 0) return;
        // The software render backend supplies unrotated pixels. WPF rotates the
        // fitted bitmap so both the render buffer and displayed frame keep their aspect.
        int.TryParse(player.Get("video-out-params/rotate"), out var rotation);
        rotation = (rotation % 360 + 360) % 360;
        Video.LayoutTransform = rotation == 0 ? Transform.Identity : new RotateTransform(rotation);
        var radians = rotation * Math.PI / 180;
        var cosine = Math.Abs(Math.Cos(radians)); var sine = Math.Abs(Math.Sin(radians));
        var displayRatio = (ratio * cosine + sine) / (ratio * sine + cosine);
        if (double.IsFinite(displayRatio) && displayRatio > 0) PreviewViewport.AspectRatio = displayRatio;
    }
    private void Source_SelectionChanged(object sender, SelectionChangedEventArgs e) { if (!syncing) Select(SourceList.SelectedItem as MediaClip); }
    private void Search_Changed(object sender, TextChangedEventArgs e)
    {
        if (SourceList == null) return;
        var wasSyncing = syncing; syncing = true;
        try
        {
            RecordingView.Refresh();
            var source = Sources.FirstOrDefault(c => string.Equals(c.Path, selected?.Path, StringComparison.OrdinalIgnoreCase));
            SourceList.SelectedItem = source != null && RecordingView.Contains(source) ? source : null;
        }
        finally { syncing = wasSyncing; }
        RefreshSummary();
    }
    private void ClearSearch_Click(object sender, RoutedEventArgs e) { SearchInput.Clear(); SearchInput.Focus(); }
    private void UpdateTrimFields()
    {
        if (selected == null) return;
        displayedStart = StartInput.Text = Timecode.Format(selected.Start);
        displayedEnd = EndInput.Text = Timecode.Format(selected.End);
        RefreshSummary();
    }
    private void PlaybackTick()
    {
        if (scrubbing) { FlushScrubSeek(); return; }
        if (settlingScrub)
        {
            if (selected != scrubClip || closing || busy) { ResetScrub(); return; }
            if (!playerReady || player == null) return;
            if (player.Get("seeking") == "yes" || Math.Abs(player.Position - pendingSeek) > 1 / selected!.FrameRate + .001)
            {
                // Keep the requested position visible while decoding. A failed seek
                // must not unexpectedly restart playback from an earlier frame.
                if (scrubSettleClock.Elapsed.TotalSeconds < 5) return;
                ResetScrub(); StatusText.Text = "Preview seek timed out · scrub again to retry"; return;
            }
            var resume = scrubWasPlaying;
            ResetScrub();
            sequencePlayback = TimelineSelected && resume; resumeAfterLoad = resume;
            player.Set("pause", resume ? "no" : "yes");
        }
        // The active gesture owns the playhead until its final seek. The paused
        // preview can still report the frame grabbed before a clip was moved.
        if (pendingTrimEdit != null) return;
        if (gapPreview)
        {
            if (gapPlaying)
            {
                Timeline.Position = Math.Min(Timeline.Duration, gapAnchor + gapClock.Elapsed.TotalSeconds);
                if (Timeline.Locate(Timeline.Position) is { } at) { SeekTimeline(at.Clip, at.SourcePosition); return; }
                if (Timeline.Position >= Timeline.Duration) { gapPlaying = false; gapClock.Reset(); }
            }
            PositionText.Text = Timecode.Format(Timeline.Position); UpdatePlaybackButton(); UpdateQuickTrimAvailability(); return;
        }
        if (player == null || selected == null || closing || !playerReady) return;
        var position = player.Position;
        var paused = player.Paused;
        pendingSeek = position;
        if (TimelineSelected) { Timeline.Position = Timeline.ClipOffset(selected) + Math.Clamp(position - selected.Start, 0, selected.KeptDuration); if (!paused) Timeline.RevealPlayhead(); }
        else { updatingSeek = true; PreviewSeek.Value = position; updatingSeek = false; }
        resumeAfterLoad = !paused;
        PositionText.Text = Timecode.Format(TimelineSelected ? Timeline.Position : position); UpdatePlaybackButton();
        UpdateQuickTrimAvailability();
        if (!paused && position >= selected.End - .006) CompletePreview();
    }
    private void CompletePreview()
    {
        if (closing || scrubbing || settlingScrub || gapPreview || selected == null || player == null || !playerReady) return;
        player.Set("pause", "yes");
        if (sequencePlayback && TimelineSelected && Clips.IndexOf(selected) < Clips.Count - 1)
        {
            var end = Timeline.ClipOffset(selected) + selected.KeptDuration;
            if (Timeline.ClipOffset(Clips[Clips.IndexOf(selected) + 1]) > end + .00001) { SeekGap(end, true); return; }
            playerReady = false;
            Select(Clips[Clips.IndexOf(selected) + 1], true);
        }
        else { sequencePlayback = resumeAfterLoad = false; if (TimelineSelected) Timeline.Position = Timeline.ClipOffset(selected) + selected.KeptDuration; StatusText.Text = "Preview finished"; }
        UpdatePlaybackButton();
    }
    private void Play_Click(object sender, RoutedEventArgs e)
    {
        if (scrubbing || settlingScrub) { scrubWasPlaying = !scrubWasPlaying; UpdatePlaybackButton(); return; }
        if (gapPreview) { gapAnchor = Timeline.Position; gapPlaying = !gapPlaying; if (gapPlaying) gapClock.Restart(); else gapClock.Reset(); UpdatePlaybackButton(); return; }
        if (selected == null || player == null || !playerReady) return;
        if (TimelineSelected && player.Paused && Timeline.Position >= Timeline.Duration - .006)
        { sequencePlayback = true; if (Timeline.ClipOffset(Clips[0]) > 0) SeekGap(0, true); else Select(Clips[0], true); return; }
        sequencePlayback = TimelineSelected;
        if (player.Paused && (player.Position < selected.Start || player.Position >= selected.End - .01)) player.Seek(selected.Start);
        player.TogglePause();
        resumeAfterLoad = !player.Paused; UpdatePlaybackButton();
    }
    private void UpdatePlaybackButton()
    {
        var playing = scrubbing || settlingScrub ? scrubWasPlaying : gapPreview ? gapPlaying : selected != null && (playerReady ? player?.Paused == false : resumeAfterLoad);
        PlayGlyph.Visibility = playing ? Visibility.Collapsed : Visibility.Visible;
        PauseGlyph.Visibility = playing ? Visibility.Visible : Visibility.Collapsed;
        PlayButton.ToolTip = playing ? "Pause (Space)" : "Play (Space)";
        System.Windows.Automation.AutomationProperties.SetName(PlayButton, playing ? "Pause" : "Play");
    }
    private void Back_Click(object sender, RoutedEventArgs e) => StepFrame(-1);
    private void Forward_Click(object sender, RoutedEventArgs e) => StepFrame(1);
    private void StepFrame(int direction)
    {
        if (!playerReady || selected == null || busy) return;
        if (gapPreview) { SeekGap(Math.Clamp(Timeline.Position + direction / selected.FrameRate, 0, Timeline.Duration), false); return; }
        sequencePlayback = resumeAfterLoad = false; player?.Set("pause", "yes");
        if (TimelineSelected)
        {
            // mpv formats time-pos to six decimal places; round before stepping across a cut.
            var frame = Math.Round((player!.Position - selected.Start) * selected.FrameRate);
            var time = Timeline.ClipOffset(selected) + (frame + direction) / selected.FrameRate;
            if (Timeline.Locate(time) is { } next) SeekTimeline(next.Clip, next.SourcePosition); else SeekGap(Math.Max(0, time), false);
        }
        else player?.Command(direction < 0 ? "frame-back-step" : "frame-step");
    }
    private void Volume_Changed(object sender, RoutedPropertyChangedEventArgs<double> e) => player?.Set("volume", e.NewValue.ToString(System.Globalization.CultureInfo.InvariantCulture));

    private void CommitTrim(double start, double end) => ApplyTrim(start, end, null);
    private void ApplyTrim(double start, double end, double? previewPosition)
    {
        if (!TimelineSelected || selected == null || busy) return;
        if (!double.IsFinite(start) || !double.IsFinite(end) || start < 0 || end > selected.Duration + .001 || end - start < 1 / selected.FrameRate - .0001)
        {
            UpdateTrimFields(); ShowTrimError("Keep at least one frame within the recording."); return;
        }
        TrimHint.Visibility = Visibility.Collapsed;
        if (start == selected.Start && Math.Min(end, selected.Duration) == selected.End) return;
        var original = Snapshot();
        if (!SetTrim(start, Math.Min(end, selected.Duration), original, final: true, constrain: false))
        { UpdateTrimFields(); ShowTrimError("The trim would overlap another clip or extend before the timeline."); return; }
        Checkpoint(original); sequencePlayback = resumeAfterLoad = false; player?.Set("pause", "yes");
        UpdateTrimFields(); SeekTimeline(selected, previewPosition ?? selected.Start); MarkDirty();
    }
    private bool SetTrim(double start, double end, EditorSnapshot snapshot, bool final, bool constrain)
    {
        if (selected == null) return false;
        var original = snapshot.Clips.First(c => c.SectionId == selected.SectionId);
        var index = snapshot.Clips.IndexOf(original);
        var offset = TimelineLayout.Offset(snapshot.Clips, original);
        var previousEnd = index == 0 ? 0 : TimelineLayout.Offset(snapshot.Clips, snapshot.Clips[index - 1]) + snapshot.Clips[index - 1].KeptDuration;
        var followers = TimelineLayout.JoinedFollowing(snapshot.Clips, original);
        var joined = followers.Count > 0 || index > 0 && Math.Abs(previousEnd - offset) < .00001;
        var last = followers.LastOrDefault() ?? original;
        var next = snapshot.Clips.Skip(snapshot.Clips.IndexOf(last) + 1).FirstOrDefault();
        var nextStart = next == null ? double.PositiveInfinity : TimelineLayout.Offset(snapshot.Clips, next);
        var frame = 1 / original.FrameRate;
        if (joined)
        {
            var extra = Math.Floor((nextStart - TimelineLayout.Offset(snapshot.Clips, last) - last.KeptDuration) / frame + 1e-8) * frame;
            var maximumLength = original.KeptDuration + extra;
            if (constrain)
            {
                if (start != original.Start) start = Math.Max(start, end - maximumLength);
                else end = Math.Min(end, start + maximumLength);
            }
            else if (end - start > maximumLength + .00001) return false;
        }
        else
        {
            var minimumStart = Math.Max(0, original.Start + Math.Ceiling((previousEnd - offset) / frame - 1e-8) * frame);
            var maximumEnd = original.End + Math.Floor((nextStart - offset - original.KeptDuration) / frame + 1e-8) * frame;
            if (constrain) { start = Math.Max(start, minimumStart); end = Math.Min(end, maximumEnd); }
            else if (start < minimumStart - .00001 || end > maximumEnd + .00001) return false;
        }
        var position = joined && final ? offset : Math.Max(previousEnd, offset + start - original.Start);
        selected.TimelineStart = position; selected.Start = start; selected.End = end;
        var shift = position + end - start - offset - original.KeptDuration;
        foreach (var follower in followers)
            Clips.First(c => c.SectionId == follower.SectionId).TimelineStart = TimelineLayout.Offset(snapshot.Clips, follower) + shift;
        return true;
    }
    private void ShowTrimError(string message) { TrimHint.Text = message; TrimHint.Visibility = Visibility.Visible; }
    private void SetStart_Click(object sender, RoutedEventArgs e) { if (TimelineSelected) CommitTrim(playerReady ? player!.Position : pendingSeek, selected!.End); }
    private void SetEnd_Click(object sender, RoutedEventArgs e) { if (TimelineSelected) CommitTrim(selected!.Start, playerReady ? player!.Position : pendingSeek); }
    private bool TryCurrentFrame(out double frameStart)
    {
        frameStart = 0;
        if (!TimelineSelected || selected == null || busy || gapPreview) return false;
        var sourcePosition = selected.Start + Timeline.Position - Timeline.ClipOffset(selected);
        if (sourcePosition < selected.Start - .0001 || sourcePosition >= selected.End - .00001) return false;
        frameStart = Math.Clamp(selected.Start + Math.Round((sourcePosition - selected.Start) * selected.FrameRate) / selected.FrameRate, selected.Start, Math.Max(selected.Start, selected.End - 1 / selected.FrameRate));
        return true;
    }
    private void UpdateQuickTrimAvailability()
    {
        var valid = TryCurrentFrame(out var frame);
        TrimBeforeButton.IsEnabled = valid && frame > selected!.Start + .00001;
        TrimAfterButton.IsEnabled = valid && frame + 1 / selected!.FrameRate < selected.End - .00001;
    }
    private void TrimBefore_Click(object sender, RoutedEventArgs e)
    {
        if (TryCurrentFrame(out var frame)) ApplyTrim(frame, selected!.End, frame);
    }
    private void TrimAfter_Click(object sender, RoutedEventArgs e)
    {
        if (TryCurrentFrame(out var frame)) ApplyTrim(selected!.Start, Math.Min(selected.End, frame + 1 / selected.FrameRate), frame);
    }
    private void ResetTrim_Click(object sender, RoutedEventArgs e) { if (selected != null) CommitTrim(0, selected.Duration); }
    private void TrimInput_Commit(object sender, KeyboardFocusChangedEventArgs e) => CommitTrimInputs();
    private void TrimInput_KeyDown(object sender, KeyEventArgs e) { if (e.Key == Key.Enter) { CommitTrimInputs(); Keyboard.ClearFocus(); e.Handled = true; } if (e.Key == Key.Escape) { UpdateTrimFields(); Keyboard.ClearFocus(); e.Handled = true; } }
    private void CommitTrimInputs()
    {
        if (!TimelineSelected || selected == null || busy) return;
        var start = selected.Start;
        var end = selected.End;
        // Keep the original presentation timestamps when the displayed field wasn't edited.
        if (StartInput.Text.Trim() != displayedStart && !Timecode.TryParse(StartInput.Text, out start) ||
            EndInput.Text.Trim() != displayedEnd && !Timecode.TryParse(EndInput.Text, out end))
        {
            ShowTrimError("Enter a timestamp or seconds.");
            return;
        }
        CommitTrim(start, end);
    }

    private async void Add_Click(object sender, RoutedEventArgs e)
    {
        if (busy) return;
        var dialog = new OpenFileDialog { Title = "Add recordings", Filter = "Video files|*.mp4;*.mkv;*.mov;*.webm;*.avi;*.m4v;*.ts|All files|*.*", Multiselect = true };
        if (dialog.ShowDialog(this) == true) await ImportAsync(dialog.FileNames);
    }
    private async Task ImportAsync(IEnumerable<string> paths)
    {
        if (busy) return;
        var candidates = paths.Select(Path.GetFullPath).Distinct(StringComparer.OrdinalIgnoreCase).Where(p => !Sources.Any(c => string.Equals(c.Path, p, StringComparison.OrdinalIgnoreCase))).ToArray();
        if (candidates.Length == 0) { StatusText.Text = "Those recordings are already in the project."; return; }
        var beforeImport = Snapshot();
        SetBusy(true); importCancellation = new(); var failures = new List<string>(); var added = 0;
        try
        {
            for (var i = 0; i < candidates.Length; i++)
            {
                importCancellation.Token.ThrowIfCancellationRequested(); StatusText.Text = $"Importing {i + 1} of {candidates.Length} · {Path.GetFileName(candidates[i])}";
                try
                {
                    var clip = await MediaTools.ProbeAsync(candidates[i], importCancellation.Token);
                    if (clip.IsHdr) throw new InvalidOperationException("HDR is not supported in this SDR editor yet.");
                    clip.Thumbnail = await MediaTools.ThumbnailAsync(clip, importCancellation.Token);
                    if (added == 0) Checkpoint(beforeImport);
                    Sources.Add(clip); added++;
                    QueueWaveform(clip);
                    if (selected == null) Select(clip); MarkDirty();
                }
                catch (OperationCanceledException) { throw; }
                catch (Exception ex) { failures.Add($"{Path.GetFileName(candidates[i])}: {ex.Message}"); }
            }
            StatusText.Text = $"Added {added} {(added == 1 ? "clip" : "clips")}";
            if (failures.Count > 0) MessageBox.Show(this, string.Join("\n\n", failures.Take(8)), "Some clips could not be imported", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
        catch (OperationCanceledException) { StatusText.Text = "Import canceled"; }
        finally { importCancellation.Dispose(); importCancellation = null; SetBusy(false); }
    }
    private async void Window_Drop(object sender, DragEventArgs e)
    {
        if (e.Handled || busy || !e.Data.GetDataPresent(DataFormats.FileDrop)) return;
        var files = (string[])e.Data.GetData(DataFormats.FileDrop); e.Handled = true;
        if (files.Length == 1 && files[0].EndsWith(".sveproject", StringComparison.OrdinalIgnoreCase)) { if (await ConfirmDiscardAsync()) await OpenProjectAsync(files[0]); }
        else await ImportAsync(files.Where(File.Exists));
    }
    private void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (!TimelineSelected || selected == null || busy) return;
        Checkpoint(); var index = Clips.IndexOf(selected);
        foreach (var follower in TimelineLayout.JoinedFollowing(Clips, selected)) follower.TimelineStart = Timeline.ClipOffset(follower) - selected.KeptDuration;
        syncing = true; Clips.Remove(selected); syncing = false;
        Select(Clips.ElementAtOrDefault(Math.Min(index, Clips.Count - 1)) ?? Sources.FirstOrDefault()); MarkDirty();
    }
    private void SectionMenu_Opened(object sender, RoutedEventArgs e)
    {
        var menu = (ContextMenu)sender;
        if (busy || menu.DataContext is not MediaClip clip || !Clips.Contains(clip))
        {
            menu.IsOpen = false;
            return;
        }
        menu.DataContext = clip;
        if (selected != clip) Select(clip);
        foreach (var item in menu.Items.OfType<MenuItem>())
            item.IsEnabled = (item.Tag as string) switch
            {
                "split" => SplitButton.IsEnabled,
                "before" => TrimBeforeButton.IsEnabled,
                "after" => TrimAfterButton.IsEnabled,
                "earlier" => Clips.IndexOf(clip) > 0,
                "later" => Clips.IndexOf(clip) < Clips.Count - 1,
                _ => true
            };
    }
    private void SectionMenu_Click(object sender, RoutedEventArgs e)
    {
        if (busy || sender is not MenuItem item || ItemsControl.ItemsControlFromItemContainer(item) is not ContextMenu menu || menu.DataContext is not MediaClip clip || !Clips.Contains(clip)) return;
        if (selected != clip) Select(clip);
        switch (item.Tag as string)
        {
            case "split": Split_Click(sender, e); break;
            case "before": TrimBefore_Click(sender, e); break;
            case "after": TrimAfter_Click(sender, e); break;
            case "reset": ResetTrim_Click(sender, e); break;
            case "duplicate": DuplicateSection(); break;
            case "earlier": Earlier_Click(sender, e); break;
            case "later": Later_Click(sender, e); break;
            case "remove": Remove_Click(sender, e); break;
        }
    }
    private void DuplicateSection()
    {
        if (!TimelineSelected || selected == null || busy) return;
        Checkpoint();
        var duplicate = selected.Clone(newSection: true);
        var followers = TimelineLayout.JoinedFollowing(Clips, selected);
        var last = followers.LastOrDefault() ?? selected;
        var next = Clips.Skip(Clips.IndexOf(last) + 1).FirstOrDefault();
        if (next == null || Timeline.ClipOffset(next) - Timeline.ClipOffset(last) - last.KeptDuration >= duplicate.KeptDuration - .00001)
        {
            duplicate.TimelineStart = Timeline.ClipOffset(selected) + selected.KeptDuration;
            foreach (var follower in followers) follower.TimelineStart = Timeline.ClipOffset(follower) + duplicate.KeptDuration;
        }
        else duplicate.TimelineStart = Timeline.PlaceClip(duplicate, Timeline.ClipOffset(selected) + selected.KeptDuration);
        syncing = true; Clips.Insert(Clips.IndexOf(selected) + 1, duplicate); syncing = false;
        Select(duplicate); MarkDirty();
    }
    private void Split_Click(object sender, RoutedEventArgs e)
    {
        if (!TimelineSelected || gapPreview) return;
        SplitAt(playerReady && player != null ? player.Position : pendingSeek);
    }
    private void SplitAt(double position)
    {
        if (!TimelineSelected || selected == null || busy) return;
        var frame = 1 / selected.FrameRate;
        if (!double.IsFinite(position) || position - selected.Start < frame - .0001 || selected.End - position < frame - .0001)
        {
            ShowTrimError("Split inside the section, with at least one frame on each side.");
            return;
        }
        Checkpoint();
        sequencePlayback = resumeAfterLoad = false;
        player?.Set("pause", "yes");
        var following = selected.Clone(newSection: true);
        following.Start = position;
        following.TimelineStart = Timeline.ClipOffset(selected) + position - selected.Start;
        selected.End = position;
        var index = Clips.IndexOf(selected);
        syncing = true;
        Clips.Insert(index + 1, following);
        syncing = false;
        Select(following);
        MarkDirty();
        StatusText.Text = $"Split at {Timecode.Format(position)}";
    }
    private void MoveClip(MediaClip clip, int target)
    {
        var index = Clips.IndexOf(clip);
        if (busy || index < 0 || target < 0 || target >= Clips.Count || Math.Abs(index - target) != 1) return;
        Checkpoint();
        var left = Clips[Math.Min(index, target)]; var right = Clips[Math.Max(index, target)];
        var start = Timeline.ClipOffset(left);
        var gap = Timeline.ClipOffset(right) - start - left.KeptDuration;
        right.TimelineStart = start; left.TimelineStart = start + right.KeptDuration + gap;
        Select(clip); MarkDirty();
    }
    private void Earlier_Click(object sender, RoutedEventArgs e) { if (selected != null) MoveClip(selected, Clips.IndexOf(selected) - 1); }
    private void Later_Click(object sender, RoutedEventArgs e) { if (selected != null) MoveClip(selected, Clips.IndexOf(selected) + 1); }
    private void Source_MouseDown(object sender, MouseButtonEventArgs e)
    {
        sourceKeyboardNavigation = false;
        dragOrigin = e.GetPosition(SourceList);
        dragClip = (ItemsControl.ContainerFromElement(SourceList, e.OriginalSource as DependencyObject) as ListBoxItem)?.DataContext as MediaClip;
        if (dragClip != null && !busy) Select(dragClip);
    }
    private void Source_MouseMove(object sender, MouseEventArgs e)
    {
        if (busy || dragClip == null || e.LeftButton != MouseButtonState.Pressed) return;
        var point = e.GetPosition(SourceList);
        if (Math.Abs(point.X - dragOrigin.X) < SystemParameters.MinimumHorizontalDragDistance && Math.Abs(point.Y - dragOrigin.Y) < SystemParameters.MinimumVerticalDragDistance) return;
        var clip = dragClip; dragClip = null; ClipDragPreview.Run(SourceList, clip, EditorTimeline.LibraryDragFormat, DragDropEffects.Copy);
    }
    private void Source_KeyDown(object sender, KeyEventArgs e)
    {
        sourceKeyboardNavigation = e.Key is Key.Up or Key.Down or Key.PageUp or Key.PageDown or Key.Home or Key.End;
        Dispatcher.BeginInvoke(() => sourceKeyboardNavigation = false, DispatcherPriority.Background);
    }
    private void Source_RequestBringIntoView(object sender, RequestBringIntoViewEventArgs e)
    {
        if (sourceKeyboardNavigation || e.TargetObject is not DependencyObject target || ItemsControl.ContainerFromElement(SourceList, target) is not ListBoxItem item) return;
        // A visible recording stays under the pointer; keyboard navigation can still reveal the whole item.
        var bounds = item.TransformToAncestor(SourceList).TransformBounds(new Rect(item.RenderSize));
        if (bounds.Bottom > 0 && bounds.Top < SourceList.ActualHeight) e.Handled = true;
    }
    private void AppendSources(IEnumerable<MediaClip> sources)
    {
        if (busy) return;
        var additions = sources.Where(Sources.Contains).Select(c => c.Clone(newSection: true)).ToArray();
        if (additions.Length == 0) return;
        var wasEmpty = Clips.Count == 0;
        Checkpoint();
        foreach (var clip in additions) { clip.TimelineStart = Timeline.Duration; Clips.Add(clip); }
        Select(additions[0]); MarkDirty();
        if (wasEmpty) Timeline.Fit();
        Timeline.RevealPlayhead();
    }
    private void AddToTimeline_Click(object sender, RoutedEventArgs e)
    {
        var source = SourceList.SelectedItem as MediaClip;
        if (source != null) AppendSources([source]);
    }
    private void AddAllToTimeline_Click(object sender, RoutedEventArgs e) => AppendSources(Sources);
    private void Source_DoubleClick(object sender, MouseButtonEventArgs e)
    {
        if ((ItemsControl.ContainerFromElement(SourceList, e.OriginalSource as DependencyObject) as ListBoxItem)?.DataContext is MediaClip source) AppendSources([source]);
    }
    private void SourceMenu_Opened(object sender, RoutedEventArgs e)
    {
        var menu = (ContextMenu)sender;
        if (busy || (menu.PlacementTarget as FrameworkElement)?.DataContext is not MediaClip source) { menu.IsOpen = false; return; }
        Select(source);
    }
    private void RemoveSource_Click(object sender, RoutedEventArgs e)
    {
        if (busy || SourceList.SelectedItem is not MediaClip source) return;
        Checkpoint(); syncing = true;
        foreach (var clip in Clips.Where(c => string.Equals(c.Path, source.Path, StringComparison.OrdinalIgnoreCase)).ToArray()) Clips.Remove(clip);
        Sources.Remove(source); syncing = false;
        Select(Clips.FirstOrDefault() ?? Sources.FirstOrDefault()); MarkDirty();
    }
    private void SeekTimeline(MediaClip clip, double sourcePosition)
    {
        if (busy || !Clips.Contains(clip)) return;
        if (!scrubbing) ResetScrub();
        var playing = !scrubbing && (gapPreview ? gapPlaying : playerReady ? player?.Paused == false : resumeAfterLoad);
        gapPreview = gapPlaying = false; gapClock.Reset(); Video.ShowBlank = false;
        if (selected != clip) Select(clip, playing);
        sequencePlayback = resumeAfterLoad = playing;
        pendingSeek = Math.Clamp(clip.Start + Math.Round((sourcePosition - clip.Start) * clip.FrameRate) / clip.FrameRate, clip.Start, clip.End);
        if (scrubbing) scrubSeekQueued = true;
        else if (playerReady) player?.Seek(pendingSeek);
        if (playerReady && !scrubbing) player?.Set("pause", playing ? "no" : "yes");
        Timeline.Position = Timeline.ClipOffset(clip) + (scrubbing ? Math.Clamp(sourcePosition - clip.Start, 0, clip.KeptDuration) : pendingSeek - clip.Start);
        PositionText.Text = Timecode.Format(Timeline.Position);
        RefreshSummary();
    }
    private void PreviewSeek_Changed(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (updatingSeek || TimelineSelected || selected == null || busy) return;
        pendingSeek = e.NewValue;
        if (scrubbing) scrubSeekQueued = true;
        else { ResetScrub(); if (playerReady) player?.Seek(pendingSeek); }
        PositionText.Text = Timecode.Format(pendingSeek);
    }
    private void BeginScrub()
    {
        if (scrubbing || busy || selected == null || player == null) return;
        var playing = settlingScrub ? scrubWasPlaying : gapPreview ? gapPlaying : playerReady ? !player.Paused : resumeAfterLoad;
        ResetScrub(); scrubbing = true; scrubWasPlaying = playing;
        gapAnchor = Timeline.Position; gapPlaying = false; gapClock.Reset();
        resumeAfterLoad = false; player.Set("pause", "yes");
    }
    private void FlushScrubSeek()
    {
        if (!scrubSeekQueued || !playerReady || player == null || selected == null || gapPreview) return;
        player.Seek(Math.Min(pendingSeek, Math.Max(selected.Start, selected.End - 1 / selected.FrameRate)));
        scrubSeekQueued = false;
    }
    private void EndScrub()
    {
        if (!scrubbing) return;
        scrubbing = false;
        if (gapPreview)
        {
            var resume = scrubWasPlaying; ResetScrub(); SeekGap(Timeline.Position, resume); return;
        }
        if (selected == null || player == null) { ResetScrub(); return; }
        pendingSeek = Math.Clamp(selected.Start + Math.Round((pendingSeek - selected.Start) * selected.FrameRate) / selected.FrameRate,
            selected.Start, Math.Max(selected.Start, selected.End - 1 / selected.FrameRate));
        if (TimelineSelected) Timeline.Position = Timeline.ClipOffset(selected) + pendingSeek - selected.Start;
        else { updatingSeek = true; PreviewSeek.Value = pendingSeek; updatingSeek = false; }
        PositionText.Text = Timecode.Format(TimelineSelected ? Timeline.Position : pendingSeek);
        scrubClip = selected; settlingScrub = true; scrubSettleClock.Restart();
        // Always submit the final position, even if the last preview seek was already sent.
        scrubSeekQueued = true; FlushScrubSeek();
    }
    private void ResetScrub()
    {
        scrubbing = settlingScrub = scrubWasPlaying = scrubSeekQueued = false;
        scrubClip = null; scrubSettleClock.Reset();
    }
    private void Fit_Click(object sender, RoutedEventArgs e) => Timeline.Fit();
    private void SeekGap(double time, bool playing)
    {
        if (busy || Clips.Count == 0) return;
        if (!scrubbing) ResetScrub();
        if (Timeline.Locate(time) is { } at) { gapPlaying = playing; gapPreview = true; SeekTimeline(at.Clip, at.SourcePosition); return; }
        if (!TimelineSelected) Select(Clips.FirstOrDefault(c => Timeline.ClipOffset(c) > time) ?? Clips[^1]);
        gapPreview = true; gapPlaying = playing; sequencePlayback = playing; resumeAfterLoad = false;
        player?.Set("pause", "yes"); Video.ShowBlank = true;
        Timeline.Position = gapAnchor = time; if (playing) gapClock.Restart(); else gapClock.Reset();
        PositionText.Text = Timecode.Format(time); RefreshSummary();
    }
    private void InsertSourceClip(MediaClip source, double time)
    {
        if (busy || !Sources.Contains(source)) return;
        var addition = source.Clone(newSection: true);
        var position = Timeline.PlaceClip(addition, time);
        Checkpoint(); addition.TimelineStart = position;
        Clips.Add(addition);
        Select(addition); MarkDirty(); Timeline.RevealPlayhead();
    }
    private void ZoomIn_Click(object sender, RoutedEventArgs e) => Timeline.Zoom(1.25);
    private void ZoomOut_Click(object sender, RoutedEventArgs e) => Timeline.Zoom(.8);
    private void OpenTimelineMenu(MediaClip clip)
    {
        if (busy) return;
        var menu = (ContextMenu)FindResource("SectionMenu");
        menu.DataContext = clip; menu.PlacementTarget = Timeline; menu.IsOpen = true;
    }

    private async Task<bool> SaveProjectAsync(bool saveAs = false)
    {
        if (busy || Sources.Count == 0 && !dirty && projectPath == null) return false;
        var destination = projectPath;
        if (saveAs || destination == null)
        {
            var dialog = new SaveFileDialog { Title = "Save editing project", Filter = "Simple Video Editor project|*.sveproject", FileName = destination == null ? "Highlights.sveproject" : Path.GetFileName(destination), DefaultExt = ".sveproject", AddExtension = true };
            if (dialog.ShowDialog(this) != true) return false; destination = dialog.FileName;
        }
        try
        {
            if (Sources.Any(c => string.Equals(Path.GetFullPath(c.Path), Path.GetFullPath(destination), StringComparison.OrdinalIgnoreCase))) throw new InvalidOperationException("A project cannot overwrite a source recording. Choose a .sveproject filename.");
            var document = CaptureProject();
            await ProjectStore.SaveAsync(destination, document); projectPath = destination;
            var current = CaptureProject();
            dirty = !current.Clips.SequenceEqual(document.Clips) || !current.Sources!.SequenceEqual(document.Sources!);
            if (!dirty) { recoveryTimer.Stop(); await ClearRecoveryAsync(); }
            RefreshSummary(); StatusText.Text = dirty ? "Project saved · newer edits remain unsaved" : "Project saved"; return !dirty;
        }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, "Could not save project", MessageBoxButton.OK, MessageBoxImage.Error); return false; }
    }
    private async void Save_Click(object sender, RoutedEventArgs e) => await SaveProjectAsync(Keyboard.Modifiers.HasFlag(ModifierKeys.Shift));
    private async Task<bool> ConfirmDiscardAsync()
    {
        if (!dirty) return true;
        var result = MessageBox.Show(this, "Save changes to your project first?", "Unsaved project", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
        return result == MessageBoxResult.No || result == MessageBoxResult.Yes && await SaveProjectAsync();
    }
    private async void New_Click(object sender, RoutedEventArgs e)
    {
        if (busy || !await ConfirmDiscardAsync()) return;
        await ResetWaveformJobsAsync();
        syncing = true; Clips.Clear(); Sources.Clear(); SearchInput.Clear(); syncing = false; undo.Clear(); redo.Clear(); projectPath = null; dirty = false; recoveryTimer.Stop(); await ClearRecoveryAsync(); Select(null); StatusText.Text = "";
    }
    private async void Open_Click(object sender, RoutedEventArgs e)
    {
        if (busy) return;
        var dialog = new OpenFileDialog { Title = "Open editing project", Filter = "Simple Video Editor project|*.sveproject" };
        if (dialog.ShowDialog(this) == true && await ConfirmDiscardAsync()) await OpenProjectAsync(dialog.FileName);
    }
    private async Task<bool> OpenProjectAsync(string path, bool recovery = false)
    {
        if (busy) return false; SetBusy(true); importCancellation = new();
        try
        {
            var document = await ProjectStore.ReadAsync(path); var restored = new List<MediaClip>();
            var relinked = false;
            if (ProjectStore.SourcePaths(document).Any(p => !LocalRecordingPath.Exists(p)))
            {
                var locate = new RelinkWindow(document) { Owner = this };
                if (locate.ShowDialog() != true) { StatusText.Text = "Opening canceled"; return false; }
                document = locate.ResolvedProject!; relinked = true;
            }
            var sources = new Dictionary<string, MediaClip>(StringComparer.OrdinalIgnoreCase);
            foreach (var sourcePath in ProjectStore.SourcePaths(document))
            {
                StatusText.Text = "Opening · " + Path.GetFileName(sourcePath);
                importCancellation.Token.ThrowIfCancellationRequested();
                var source = await MediaTools.ProbeAsync(sourcePath, importCancellation.Token);
                if (source.IsHdr) throw new InvalidDataException("This project contains an HDR clip, which is not supported yet.");
                source.Thumbnail = await MediaTools.ThumbnailAsync(source, importCancellation.Token);
                sources.Add(sourcePath, source);
            }
            foreach (var entry in document.Clips)
            {
                var clip = sources[entry.Path].Clone(newSection: true);
                clip.Start = entry.Start; clip.End = entry.End; clip.TimelineStart = entry.TimelineStart; ProjectStore.ValidateTrim(clip); restored.Add(clip);
            }
            await ResetWaveformJobsAsync();
            syncing = true; Clips.Clear(); Sources.Clear(); SearchInput.Clear(); foreach (var source in sources.Values) Sources.Add(source); foreach (var clip in restored) Clips.Add(clip); syncing = false;
            foreach (var source in Sources) QueueWaveform(source);
            foreach (var clip in Clips) clip.TimelineStart ??= Timeline.ClipOffset(clip);
            undo.Clear(); redo.Clear(); projectPath = recovery ? null : path; dirty = recovery || relinked; Select(Clips.FirstOrDefault() ?? Sources.FirstOrDefault()); Timeline.Fit();
            if (dirty) { await this.recovery.SaveAsync(CaptureProject()); MarkDirty(); }
            else { recoveryTimer.Stop(); await ClearRecoveryAsync(); }
            StatusText.Text = relinked ? "Recordings located · save the project to keep the new paths" : recovery ? "Session recovered" : "Project opened";
            return true;
        }
        catch (OperationCanceledException) { StatusText.Text = "Opening canceled"; }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, "Could not open project", MessageBoxButton.OK, MessageBoxImage.Error); }
        finally { importCancellation.Dispose(); importCancellation = null; SetBusy(false); }
        return false;
    }
    private async Task ClearRecoveryAsync()
    {
        try { await recovery.ClearAsync(); } catch (IOException) { } catch (UnauthorizedAccessException) { }
    }
    private void QueueWaveform(MediaClip clip)
    {
        if (!clip.HasAudio || clip.Waveform.Data != null || clip.Waveform.IsLoading || closing) return;
        clip.Waveform.Begin();
        var task = LoadWaveformAsync(clip, waveformCancellation.Token);
        waveformTasks.Add(task);
        _ = ForgetWaveformTaskAsync(task);
    }
    private async Task ForgetWaveformTaskAsync(Task task) { await task; waveformTasks.Remove(task); }
    private async Task LoadWaveformAsync(MediaClip clip, CancellationToken token)
    {
        bool entered = false;
        try
        {
            await waveformSlots.WaitAsync(token); entered = true;
            var data = await Task.Run(() => AudioWaveforms.LoadAsync(clip, token), token);
            clip.Waveform.Complete(data);
        }
        catch (OperationCanceledException) { clip.Waveform.Complete(null); }
        catch (Exception ex) { clip.Waveform.Complete(null, ex.Message); }
        finally { if (entered) waveformSlots.Release(); }
    }
    private async Task ResetWaveformJobsAsync()
    {
        waveformCancellation.Cancel();
        await Task.WhenAll(waveformTasks.ToArray());
        waveformCancellation.Dispose(); waveformCancellation = new();
    }
    private void Export_Click(object sender, RoutedEventArgs e)
    {
        if (busy || Clips.Count == 0) return;
        gapPlaying = false; gapClock.Reset();
        sequencePlayback = resumeAfterLoad = false; player?.Set("pause", "yes"); UpdatePlaybackButton(); var window = new ExportWindow(Clips.Select(c => c.Clone()).ToArray()) { Owner = this }; window.ShowDialog();
    }
    private void Cache_Click(object sender, RoutedEventArgs e)
    {
        if (busy) return;
        SetBusy(true);
        try
        {
            new CacheWindow(async () => { await ResetWaveformJobsAsync(); return await Task.Run(PreviewCache.Clear); }) { Owner = this }.ShowDialog();
        }
        finally { SetBusy(false); foreach (var source in Sources) QueueWaveform(source); }
    }
    private void Window_KeyDown(object sender, KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (busy) { if (key == Key.Escape) importCancellation?.Cancel(); return; }
        var ctrl = Keyboard.Modifiers.HasFlag(ModifierKeys.Control);
        if (ctrl)
        {
            if (key == Key.F) { SearchInput.Focus(); SearchInput.SelectAll(); e.Handled = true; return; }
            if (SearchInput.IsKeyboardFocusWithin && key is Key.Z or Key.Y) return;
            if (key == Key.B)
            {
                if (Keyboard.FocusedElement is TextBox) return;
                Split_Click(sender, e); e.Handled = true; return;
            }
            switch (key) { case Key.I: Add_Click(sender, e); break; case Key.S: Save_Click(sender, e); break; case Key.O: Open_Click(sender, e); break; case Key.N: New_Click(sender, e); break; case Key.Z: Undo_Click(sender, e); break; case Key.Y: Redo_Click(sender, e); break; default: return; }
            e.Handled = true; return;
        }
        if (key == Key.Escape && SearchInput.IsKeyboardFocusWithin) { SearchInput.Clear(); e.Handled = true; return; }
        if (Keyboard.FocusedElement is TextBox || RecordingsSplitter.IsKeyboardFocusWithin && (key is Key.Left or Key.Right)) return;
        if (key == Key.Escape && (pendingTrimEdit != null || scrubbing))
        { Timeline.CancelGesture(); if (scrubbing) { Mouse.Capture(null); EndScrub(); } e.Handled = true; return; }
        if (key == Key.Apps || key == Key.F10 && Keyboard.Modifiers.HasFlag(ModifierKeys.Shift))
        { if (TimelineSelected) OpenTimelineMenu(selected!); e.Handled = true; return; }
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Alt) && selected != null && TimelineSelected)
        {
            if (key == Key.Left) Earlier_Click(sender, e);
            else if (key == Key.Right) Later_Click(sender, e);
            else return;
            e.Handled = true; return;
        }
        switch (key) { case Key.Space: Play_Click(sender, e); break; case Key.Left: Back_Click(sender, e); break; case Key.Right: Forward_Click(sender, e); break; case Key.Q: TrimBefore_Click(sender, e); break; case Key.W: TrimAfter_Click(sender, e); break; case Key.I: SetStart_Click(sender, e); break; case Key.O: SetEnd_Click(sender, e); break; case Key.Delete: Remove_Click(sender, e); break; default: return; }
        e.Handled = true;
    }
    private async void Window_Closing(object? sender, CancelEventArgs e)
    {
        if (closing) return;
        e.Cancel = true;
        if (busy) { importCancellation?.Cancel(); StatusText.Text = "Canceling import… Close again once it finishes."; return; }
        if (!await ConfirmDiscardAsync()) return;
        closing = true; playerReady = false;
        await ResetWaveformJobsAsync(); waveformCancellation.Dispose(); waveformSlots.Dispose();
        recoveryTimer.Stop(); playbackTimer.Stop();
        await ClearRecoveryAsync(); recovery.Dispose();
        Video.Detach();
        // Keep the dispatcher responsive while libmpv and its render thread shut down.
        await Task.Run(() => player?.Dispose());
        _ = Dispatcher.BeginInvoke(Close);
    }
}
