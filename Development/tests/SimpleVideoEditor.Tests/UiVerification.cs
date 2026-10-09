using System.IO;
using System.Reflection;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SimpleVideoEditor;
using SimpleVideoEditor.Controls;
using SimpleVideoEditor.Services;

public static class UiVerification
{
    public static int Run(string[] clips)
    {
        var code = 0;
        var thread = new Thread(() =>
        {
            var root = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../artifacts/verification"));
            ProjectStore.DataDirectory = Path.Combine(root, "timeline-test-data", Guid.NewGuid().ToString("N"));
            var app = new App(); app.InitializeComponent(); app.ShutdownMode = ShutdownMode.OnExplicitShutdown;
            var dispatcherErrors = new List<Exception>();
            app.DispatcherUnhandledException += (_, e) => { dispatcherErrors.Add(e.Exception); Console.Error.WriteLine(e.Exception); e.Handled = true; code = 1; };
            var window = new MainWindow { ShowActivated = false }; app.MainWindow = window;
            // The harness imports fixtures explicitly. Optional recording arguments
            // must not trigger the application's command-line import on startup.
            var startup = typeof(MainWindow).GetMethod("Window_Loaded", BindingFlags.NonPublic | BindingFlags.Instance)!;
            window.Loaded -= (RoutedEventHandler)Delegate.CreateDelegate(typeof(RoutedEventHandler), window, startup);
            // This harness dispatches pointer callbacks itself. Ignore concurrent physical
            // mouse input so test capture cannot turn the user's movement into a scrub.
            window.PreviewMouseMove += (_, e) => e.Handled = true;
            window.PreviewMouseDown += (_, e) => e.Handled = true;
            window.Show();
            window.Dispatcher.BeginInvoke(async () =>
            {
                var checks = new List<string>();
                void Assert(bool condition, string name) { if (!condition) throw new Exception(name); checks.Add(name); Console.WriteLine("PASS " + name); }
                T Control<T>(string name) where T : class => (T)window.FindName(name);
                object? Invoke(string name, params object[] args) => typeof(MainWindow).GetMethod(name, BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(window, args);
                object? Field(string name) => typeof(MainWindow).GetField(name, BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(window);
                void Click(string name) => Control<Button>(name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                async Task Ready() { for (int i = 0; i < 100 && !(bool)Field("playerReady")!; i++) await Task.Delay(30); if (!(bool)Field("playerReady")!) throw new Exception("Preview did not load"); await Task.Delay(50); }
                async Task WaveformsReady() { await Task.WhenAll(((HashSet<Task>)Field("waveformTasks")!).ToArray()).WaitAsync(TimeSpan.FromSeconds(25)); await Task.Delay(30); }
                async Task HandleDialog<T>(Func<T, Task> action) where T : Window
                {
                    for (var i = 0; i < 150; i++)
                    {
                        var dialog = app.Windows.OfType<T>().FirstOrDefault(w => w.IsVisible);
                        if (dialog != null) { try { await action(dialog); } finally { if (dialog.IsVisible) dialog.Close(); } return; }
                        await Task.Delay(20);
                    }
                    throw new Exception(typeof(T).Name + " did not open");
                }
                void Press(Key key) => Invoke("Window_KeyDown", window, new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(window)!, Environment.TickCount, key) { RoutedEvent = Keyboard.PreviewKeyDownEvent });
                var timeline = Control<EditorTimeline>("Timeline");
                var surface = (FrameworkElement)typeof(EditorTimeline).GetField("surface", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(timeline)!;
                void Pointer(string method, params object[] args) => surface.GetType().GetMethod(method, BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(surface, args);
                void DragClipTo(MediaClip clip, double time)
                {
                    var grab = clip.KeptDuration / 2;
                    Pointer("BeginPointer", new Point((timeline.ClipOffset(clip) + grab) * timeline.PixelsPerSecond, 65));
                    Pointer("MovePointer", new Point((time + grab) * timeline.PixelsPerSecond, 65), true);
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                }
                Directory.CreateDirectory(root);
                try
                {
                    await Task.Delay(150); window.UpdateLayout();
                    Assert(!Control<Button>("ExportButton").IsEnabled && window.Clips.Count == 0, "Empty timeline disables export");
                    Assert(Control<Button>("AddAllToTimelineButton").Visibility == Visibility.Collapsed && !Control<Button>("AddToTimelineButton").IsEnabled, "Empty library hides Add all and disables Add to timeline");
                    Assert(!Control<Button>("SaveButton").IsEnabled && Control<Button>("NewButton").IsEnabled && Control<Button>("OpenButton").IsEnabled, "Empty projects disable Save while keeping New and Open available");
                    Assert(Control<VideoHost>("Video").Visibility == Visibility.Collapsed && RenderOptions.ProcessRenderMode == System.Windows.Interop.RenderMode.SoftwareOnly, "Empty preview is hidden and the interface uses software rendering");
                    SaveRender(window, Path.Combine(root, "timeline-empty.png"));
                    var import = (Task)Invoke("ImportAsync", (object)new[] { Path.Combine(root, "source A's clip.mp4") })!;
                    window.UpdateLayout();
                    Assert(!timeline.IsEnabled && !Control<ListBox>("SourceList").IsEnabled && DarkAt(timeline, new Point(100, 60)) && DarkAt(Control<ListBox>("SourceList"), new Point(100, 60)), "Import keeps the disabled library and tracks dark");
                    Assert(new[] { "NewButton", "OpenButton", "SaveButton" }.All(name => !Control<Button>(name).IsEnabled), "Project actions visibly disable during import");
                    SaveRender(window, Path.Combine(root, "timeline-importing.png")); await import; await Ready();
                    Assert(Control<Button>("AddAllToTimelineButton").Visibility == Visibility.Collapsed && Control<Button>("AddToTimelineButton").IsEnabled, "One recording keeps only Add to timeline visible");
                    await (Task)Invoke("ImportAsync", (object)new[] { Path.Combine(root, "source B.mp4") })!; await Ready();
                    Assert(window.Sources.Count == 2 && window.Clips.Count == 0 && !Control<Button>("ExportButton").IsEnabled, "Import fills the library without adding unwanted clips to the timeline");
                    Assert(Control<Button>("AddAllToTimelineButton").IsVisible && Control<Button>("AddAllToTimelineButton").IsEnabled && Control<Button>("AddToTimelineButton").IsVisible, "Multiple recordings show both timeline buttons");
                    window.Width = window.MinWidth; window.Height = window.MinHeight; window.UpdateLayout();
                    var addOne = Control<Button>("AddToTimelineButton"); var addAll = Control<Button>("AddAllToTimelineButton");
                    var oneBounds = addOne.TransformToAncestor(window).TransformBounds(new Rect(addOne.RenderSize));
                    var allBounds = addAll.TransformToAncestor(window).TransformBounds(new Rect(addAll.RenderSize));
                    Assert(Math.Abs(oneBounds.Top - allBounds.Top) < 1 && oneBounds.Right <= allBounds.Left && Math.Abs(oneBounds.Height - allBounds.Height) < 1,
                        "Minimum window keeps timeline actions side by side in one row");
                    Assert(new[] { addOne, addAll }.All(button => Descendants(button).OfType<ButtonLabel>().Single().DesiredSize.Width <= button.ActualWidth - button.Padding.Left - button.Padding.Right),
                        "Both complete timeline button labels fit at minimum window size");
                    var compactItem = (ListBoxItem)Control<ListBox>("SourceList").ItemContainerGenerator.ContainerFromItem(window.Sources[0]);
                    Assert(compactItem.ActualHeight <= 60 && Control<ListBox>("SourceList").ActualHeight >= compactItem.ActualHeight * 2,
                        $"Compact rows leave room for at least two complete recordings at minimum window size (row {compactItem.ActualHeight}, list {Control<ListBox>("SourceList").ActualHeight})");
                    SaveRender(window, Path.Combine(root, "recordings-compact-small.png"));
                    window.Width = 1320; window.Height = 880; window.UpdateLayout();
                    Assert(Control<ListBox>("SourceList").ActualHeight >= compactItem.ActualHeight * 4, "Normal window has space for at least four compact recording rows");
                    foreach (var extreme in new[] { 1e20, 1e300 })
                    {
                        var renderClip = window.Sources[0].Clone(true); renderClip.Start = 0; renderClip.End = 1; renderClip.TimelineStart = extreme;
                        var isolatedTimeline = new EditorTimeline { Clips = new([renderClip]), Width = 700, Height = 220 };
                        var extremeRenderWatch = System.Diagnostics.Stopwatch.StartNew();
                        isolatedTimeline.Measure(new Size(700, 220)); isolatedTimeline.Arrange(new Rect(0, 0, 700, 220)); isolatedTimeline.Fit(); isolatedTimeline.Zoom(.5); isolatedTimeline.UpdateLayout();
                        SaveElement(isolatedTimeline, Path.Combine(root, $"extreme-ruler-{extreme:0E0}.png"));
                        Assert(extremeRenderWatch.Elapsed < TimeSpan.FromSeconds(2) && double.IsFinite(isolatedTimeline.PixelsPerSecond) && isolatedTimeline.PixelsPerSecond > 0,
                            $"Extreme finite timeline renders and zooms promptly with bounded ruler work: {extreme}");
                        isolatedTimeline.Clips = null;
                    }
                    Invoke("SetBusy", true);
                    Click("AddAllToTimelineButton");
                    Assert(!Control<Button>("AddToTimelineButton").IsEnabled && !Control<Button>("AddAllToTimelineButton").IsEnabled && window.Clips.Count == 0, "Busy state disables both timeline actions and blocks batch insertion");
                    Invoke("SetBusy", false);
                    Assert(Control<Slider>("PreviewSeek").IsVisible && !Control<Button>("SplitButton").IsEnabled, "Recording preview has its own scrubber and cannot alter timeline clips");
                    var sourceList = Control<ListBox>("SourceList");
                    var sourceScroll = Descendants(sourceList).OfType<ScrollViewer>().Single();
                    // Constrain just the list to test partial-item selection independently of card size.
                    window.UpdateLayout();
                    var firstSourceItem = (ListBoxItem)sourceList.ItemContainerGenerator.ContainerFromItem(window.Sources[0]);
                    sourceList.MaxHeight = firstSourceItem.ActualHeight * 1.5;
                    window.UpdateLayout(); sourceScroll.ScrollToTop(); await Task.Delay(30);
                    Invoke("Select", window.Sources[1], false); await Ready();
                    Assert(sourceScroll.VerticalOffset == 0 && sourceScroll.ExtentHeight > sourceScroll.ViewportHeight,
                        $"Selecting a partially visible recording preserves the list position instead of hiding the first thumbnail (offset {sourceScroll.VerticalOffset}, extent {sourceScroll.ExtentHeight}, viewport {sourceScroll.ViewportHeight})");
                    SaveElement((FrameworkElement)sourceList.Parent, Path.Combine(root, "recordings-selection.png"));
                    var secondSourceItem = (ListBoxItem)sourceList.ItemContainerGenerator.ContainerFromItem(window.Sources[1]);
                    var sourceKey = new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(window)!, Environment.TickCount, Key.Down);
                    Invoke("Source_KeyDown", sourceList, sourceKey); secondSourceItem.BringIntoView(); await Task.Delay(30);
                    Assert(sourceScroll.VerticalOffset > 0 && sourceScroll.VerticalOffset < secondSourceItem.ActualHeight,
                        "Keyboard reveal scrolls only the necessary pixels and keeps both recording thumbnails in view");
                    sourceList.MaxHeight = double.PositiveInfinity; sourceScroll.ScrollToTop(); window.Height = 880; window.UpdateLayout(); await Task.Delay(30);
                    var dragRoot = (FrameworkElement)window.Content;
                    var dragLayer = System.Windows.Documents.AdornerLayer.GetAdornerLayer(dragRoot);
                    var sourceGhost = ClipDragPreview.Attach(sourceList, window.Sources[0])!;
                    sourceGhost.MoveTo(new Point(340, 150)); await Task.Delay(30);
                    Assert(sourceGhost.Preview.HasThumbnail && !sourceGhost.Preview.IsHitTestVisible && dragLayer.GetAdorners(dragRoot).Contains(sourceGhost.Preview),
                        "Recording drag shows the correct thumbnail without intercepting drop targets");
                    SaveRender(window, Path.Combine(root, "recording-drag-preview.png"));
                    sourceGhost.MoveTo(new Point(470, 210));
                    Assert(sourceGhost.Preview.Position == new Point(484, 226), "Drag thumbnail follows the cursor with a small offset");
                    sourceGhost.MoveTo(new Point(-10, 210));
                    Assert(sourceGhost.Preview.Visibility == Visibility.Hidden, "Drag thumbnail hides when the cursor leaves the editor");
                    sourceGhost.MoveTo(new Point(340, 150)); sourceGhost.Dispose(); sourceGhost.Dispose();
                    Assert(dragLayer.GetAdorners(dragRoot)?.Contains(sourceGhost.Preview) != true, "Dropping or canceling removes the drag preview safely");
                    var blueBitmap = Control<VideoHost>("Video").PreviewBitmap!;
                    var bluePixel = new byte[4];
                    blueBitmap.CopyPixels(new Int32Rect(blueBitmap.PixelWidth / 2, blueBitmap.PixelHeight / 2, 1, 1), bluePixel, 4, 0);
                    Assert(bluePixel[0] > 220 && bluePixel[1] < 25 && bluePixel[2] < 25, "BGR software presentation preserves the blue fixture's color channels");
                    Assert(DarkAt(Control<VideoHost>("Video"), new Point(3, 3)), "Software preview preserves black letterboxing");
                    Invoke("Select", window.Sources[0], false); await Ready();
                    await (Task)Invoke("ImportAsync", (object)new[] { window.Sources[0].Path })!;
                    Assert(window.Sources.Count == 2, "Duplicate imports keep one library recording");
                    Control<TextBox>("SearchInput").Text = "no-such-recording";
                    Assert(!Control<Button>("AddToTimelineButton").IsEnabled && Control<Button>("AddAllToTimelineButton").IsEnabled, "Add all remains available without a selection or search matches");
                    Click("AddAllToTimelineButton"); await Ready();
                    Assert(window.Clips.Select(c => c.Path).SequenceEqual(window.Sources.Select(c => c.Path)) && window.Clips.Zip(window.Sources).All(pair => pair.First.SectionId != pair.Second.SectionId && pair.First.Start == pair.Second.Start && pair.First.End == pair.Second.End) && window.Clips[0].TimelineStart == 0 && window.Clips[1].TimelineStart == window.Clips[0].KeptDuration && timeline.SelectedClip == window.Clips[0],
                        "Add all includes search-hidden recordings in library order as separate linked clips on an empty timeline");
                    Click("UndoButton");
                    Assert(window.Clips.Count == 0 && window.Sources.Count == 2, "One undo removes the entire batch and preserves the recording library");
                    Click("ClearSearchButton"); Invoke("Select", window.Sources[0], false); await Ready();
                    Click("AddToTimelineButton"); await Ready();
                    var first = window.Clips[0];
                    Assert(window.Clips.Count == 1 && first != window.Sources[0] && first.SectionId != window.Sources[0].SectionId && timeline.SelectedClip == first, "Add to timeline creates a separate selected clip identity");
                    Assert(Control<Button>("ExportButton").IsEnabled && !Control<Slider>("PreviewSeek").IsVisible && Control<Grid>("TrimControls").IsEnabled, "Timeline selection enables editing and uses the shared playhead");
                    var source = window.Sources[1];
                    Invoke("InsertSourceClip", source, 4d); await Ready();
                    Assert(window.Clips[0].SectionId == first.SectionId && window.Clips[1].Path == source.Path, "Library drop joins a recording at the requested boundary");
                    DragClipTo(window.Clips[0], 7d); await Ready();
                    Assert(window.Clips[1].SectionId == first.SectionId && window.Clips[1].TimelineStart == 7 && window.Clips[0].TimelineStart == 4, "Moving a joined clip away detaches it without moving its neighbor");
                    DragClipTo(window.Clips[1], 0d);
                    Assert(window.Clips[0].SectionId == first.SectionId, "A detached clip can be placed freely before another recording");
                    var unchangedUndo = ((System.Collections.ICollection)Field("undo")!).Count;
                    DragClipTo(window.Clips[0], 0d);
                    Assert(((System.Collections.ICollection)Field("undo")!).Count == unchangedUndo, "Dropping at the current position does not create an undo entry");
                    timeline.Fit(); window.UpdateLayout();
                    Assert(Math.Abs(timeline.Duration - 7) < .1 && timeline.ClipOffset(window.Clips[1]) == 4, "Timeline scale and joined boundaries follow actual clip durations");
                    var at = timeline.Locate(4.5)!.Value;
                    Assert(at.Clip == window.Clips[1] && Math.Abs(at.SourcePosition - .5) < .001, "Global playhead maps into the correct source recording");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    var seekingPlayer = (NativePlayer)Field("player")!;
                    Invoke("SeekTimeline", window.Clips[0], .5); await Task.Delay(60);
                    Assert(seekingPlayer.Paused && Control<System.Windows.Shapes.Path>("PlayGlyph").IsVisible && !Control<System.Windows.Shapes.Path>("PauseGlyph").IsVisible,
                        "Paused timeline scrubbing stays paused and shows the play icon");
                    Click("PlayButton"); Invoke("SeekTimeline", window.Clips[0], 1d); await Task.Delay(120);
                    Assert(!seekingPlayer.Paused && seekingPlayer.Position > 1 && Control<System.Windows.Shapes.Path>("PauseGlyph").IsVisible,
                        "Scrubbing within a playing timeline keeps playback and shows the pause icon");
                    var seekSurface = (FrameworkElement)typeof(EditorTimeline).GetField("surface", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(timeline)!;
                    seekSurface.GetType().GetMethod("BeginPointer", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(seekSurface, [new Point((timeline.ClipOffset(window.Clips[1]) + .5) * timeline.PixelsPerSecond, 65)]);
                    seekSurface.GetType().GetMethod("FinishEdit", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(seekSurface, null);
                    await Ready(); await Task.Delay(100);
                    Assert(!seekingPlayer.Paused && seekingPlayer.Position > .5 && timeline.SelectedClip == window.Clips[1],
                        "Clicking another video block resumes at the requested point without losing the playing state");
                    Click("PlayButton"); Invoke("SeekTimeline", window.Clips[0], .75); await Ready();
                    Assert(seekingPlayer.Paused && Math.Abs(seekingPlayer.Position - .75) < .01,
                        "Cross-recording scrubbing while paused stays paused at the requested frame");
                    Invoke("Select", window.Sources[0], false); await Ready();
                    Control<Slider>("PreviewSeek").Value = .5; await Task.Delay(60);
                    Assert(seekingPlayer.Paused, "Paused library scrubbing stays paused");
                    Click("PlayButton"); Control<Slider>("PreviewSeek").Value = 1d; await Task.Delay(120);
                    Assert(!seekingPlayer.Paused && seekingPlayer.Position > 1, "Library scrubbing while playing continues playback");
                    Click("PlayButton");
                    Assert(seekingPlayer.Paused && System.Windows.Automation.AutomationProperties.GetName(Control<Button>("PlayButton")) == "Play" && Control<Button>("PlayButton").Content is Grid,
                        "One icon button toggles play and pause and exposes the current action to accessibility tools");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    await WaveformsReady();
                    Assert(window.Sources[0].Waveform.Data?.PeakBetween(.5, .6) > .1f && ReferenceEquals(window.Clips[0].Waveform, window.Sources[0].Waveform) && window.Sources[1].Waveform.Data == null,
                        "Background audio analysis supplies shared real peaks while video-only sources stay silent");
                    var selectedBeforeSearch = timeline.SelectedClip!.SectionId;
                    var timelineBeforeSearch = ProjectStore.Capture(window.Clips, window.Sources);
                    var undoBeforeSearch = ((System.Collections.ICollection)Field("undo")!).Count;
                    Click("PlayButton");
                    Control<TextBox>("SearchInput").Text = "SOURCE B"; window.UpdateLayout();
                    Assert(Control<ListBox>("SourceList").Items.Count == 1 && window.Sources.Count == 2 && timeline.SelectedClip.SectionId == selectedBeforeSearch && !seekingPlayer.Paused,
                        "Case-insensitive library search filters recording names without interrupting timeline selection or playback");
                    Assert(Control<TextBlock>("ClipCountText").Text == "1 / 2" && Control<Button>("ClearSearchButton").IsVisible, "Search shows the matching count and a clear button");
                    SaveRender(window, Path.Combine(root, "library-search.png"));
                    Control<TextBox>("SearchInput").Text = "no-such-recording";
                    Assert(Control<TextBlock>("SearchEmpty").IsVisible && Control<ListBox>("SourceList").Items.Count == 0 && ProjectStore.Capture(window.Clips, window.Sources).Clips.SequenceEqual(timelineBeforeSearch.Clips),
                        "An empty search result does not remove recordings or timeline edits");
                    Control<TextBox>("SearchInput").Focus(); Press(Key.Q);
                    Assert(ProjectStore.Capture(window.Clips).Clips.SequenceEqual(timelineBeforeSearch.Clips), "Typing in search cannot trigger quick trims");
                    Press(Key.Escape); window.UpdateLayout();
                    Assert(Control<TextBox>("SearchInput").Text == "" && Control<ListBox>("SourceList").Items.Count == 2 && ((System.Collections.ICollection)Field("undo")!).Count == undoBeforeSearch,
                        "Escape clears search and restores all recordings without changing undo history");
                    Control<TextBox>("SearchInput").Text = "source b"; Click("ClearSearchButton");
                    Assert(Control<ListBox>("SourceList").Items.Count == 2 && Control<TextBox>("SearchInput").Text == "", "The clear-search button restores the complete library");
                    Keyboard.ClearFocus(); Click("PlayButton");
                    var otherId = window.Clips[1].SectionId;
                    Invoke("SeekTimeline", window.Clips[0], 1.5); await Ready();
                    window.UpdateLayout();
                    Assert(new[] { "TrimBeforeButton", "TrimAfterButton" }.Select(name => MeasureButtonInk(Control<Button>(name))).All(m => ButtonInkCentered(m)), "Quick trim button labels remain centered");
                    Keyboard.ClearFocus(); Press(Key.Q);
                    Assert(window.Clips.Count == 2 && window.Clips[0].Start == 1.5 && window.Clips[0].End == 4 && timeline.Position == 0 && window.Clips[1].SectionId == otherId && window.Sources[0].Start == 0,
                        "Q trims before the current frame in one step, ripples following clips, and preserves other clips and the source");
                    Click("UndoButton"); await Ready();
                    Assert(window.Clips[0].Start == 0 && window.Clips[0].End == 4, "One undo restores the complete quick trim");
                    Invoke("SeekTimeline", window.Clips[0], 1.5); await Ready(); Press(Key.W);
                    Assert(window.Clips.Count == 2 && window.Clips[0].Start == 0 && Math.Abs(window.Clips[0].End - (1.5 + 1d / 60)) < .000001 && Math.Abs(timeline.Position - 1.5) < .000001,
                        "W trims after the playhead while retaining the current frame on both linked tracks");
                    Press(Key.Q);
                    Assert(Math.Abs(window.Clips[0].KeptDuration - 1d / 60) < .000001 && !Control<Button>("TrimBeforeButton").IsEnabled && !Control<Button>("TrimAfterButton").IsEnabled,
                        "Both quick trims can isolate a single frame without enabling an empty trim");
                    Press(Key.Q); Press(Key.W); Click("UndoButton"); await Ready();
                    Assert(window.Clips[0].Start == 0 && Math.Abs(window.Clips[0].End - (1.5 + 1d / 60)) < .000001, "Repeating a boundary trim adds no undo entry");
                    Click("RedoButton"); await Ready();
                    Assert(window.Clips[0].Start == 1.5 && ReferenceEquals(window.Clips[0].Waveform, window.Sources[0].Waveform), "Redo restores the quick trim and its aligned source waveform");
                    Click("UndoButton"); Click("UndoButton"); await Ready();
                    Invoke("SeekTimeline", window.Clips[0], 1.5); await Ready();
                    Control<TextBox>("StartInput").Focus(); Press(Key.Q); Press(Key.W); Keyboard.ClearFocus();
                    Assert(window.Clips[0].Start == 0 && window.Clips[0].End == 4, "Quick trim shortcuts do not edit clips while typing a timestamp");
                    var quickMenu = (ContextMenu)window.FindResource("SectionMenu"); quickMenu.PlacementTarget = timeline; quickMenu.DataContext = window.Clips[0]; quickMenu.IsOpen = true; await Task.Delay(30);
                    var after = quickMenu.Items.OfType<MenuItem>().Single(i => (string)i.Tag == "after");
                    Assert(after.IsEnabled && quickMenu.Items.OfType<MenuItem>().Single(i => (string)i.Tag == "before").IsEnabled, "Context menu offers both quick trim actions at an interior frame");
                    after.RaiseEvent(new RoutedEventArgs(MenuItem.ClickEvent)); quickMenu.IsOpen = false;
                    Assert(Math.Abs(window.Clips[0].End - (1.5 + 1d / 60)) < .000001, "Right-click trim after performs the same frame-inclusive edit");
                    Click("UndoButton"); await Ready();
                    Invoke("Select", window.Sources[0], false); await Ready();
                    Assert(!Control<Button>("TrimBeforeButton").IsEnabled && !Control<Button>("TrimAfterButton").IsEnabled, "Library preview disables destructive timeline trim actions");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    Control<TextBox>("StartInput").Text = "1.15"; Control<TextBox>("EndInput").Text = "2.15"; Invoke("CommitTrimInputs");
                    Assert(window.Clips[0].Start == 1.15 && window.Clips[0].End == 2.15 && Math.Abs(timeline.ClipOffset(window.Clips[1]) - 1) < .00001 && window.Sources[0].Start == 0, "Trim inputs ripple both tracks and preserve the original library recording");
                    Invoke("CommitTrim", 1.150475, 2.149525); Invoke("CommitTrimInputs");
                    Assert(window.Clips[0].Start == 1.150475 && window.Clips[0].End == 2.149525, "Unchanged trim fields preserve sub-millisecond source timestamps");
                    Click("UndoButton");
                    var beforeGesture = window.Clips[0].Start;
                    Invoke("BeginTrimEdit"); Invoke("UpdateTrimEdit", 1.3, 2.15); Invoke("UpdateTrimEdit", 1.4, 2.15); Invoke("CompleteTrimEdit");
                    Assert(window.Clips[0].Start == 1.4 && Math.Abs(timeline.ClipOffset(window.Clips[1]) - .75) < .000001, "A trim drag updates the linked lanes and following clip offsets");
                    Click("UndoButton");
                    Assert(window.Clips[0].Start == beforeGesture && window.Clips[0].End == 2.15, "A complete trim drag produces one undo step");
                    Invoke("BeginTrimEdit"); Invoke("CompleteTrimEdit"); Click("UndoButton");
                    Assert(window.Clips[0].Start == 0 && window.Clips[0].End == 4, "Clicking a trim edge without movement does not create an undo step");
                    Click("RedoButton");
                    // Run the same pointer handlers used by the native WPF events, on both linked lanes.
                    await Ready(); Invoke("SeekTimeline", window.Clips[0], 1.65); await Ready();
                    var beforeEdgeDrag = seekingPlayer.Position;
                    var edgeX = window.Clips[0].KeptDuration * timeline.PixelsPerSecond - 2;
                    Pointer("BeginPointer", new Point(edgeX, 65)); Pointer("MovePointer", new Point(edgeX + timeline.PixelsPerSecond * 5 / 60, 65), true);
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                    Assert(Math.Abs(window.Clips[0].End - (2.15 + 5d / 60)) < .000001, "Video edge pointer drag trims in whole-frame increments from the original timestamp");
                    await Ready();
                    Assert(Math.Abs(seekingPlayer.Position - beforeEdgeDrag) < .001 && Math.Abs(timeline.Position - (beforeEdgeDrag - window.Clips[0].Start)) < .001,
                        "Extending the right edge preserves the preview frame and playhead on release");
                    Click("UndoButton"); await Ready(); Invoke("SeekTimeline", window.Clips[0], beforeEdgeDrag); await Ready();
                    Pointer("BeginPointer", new Point(2, 140)); Pointer("MovePointer", new Point(2 - timeline.PixelsPerSecond * 5 / 60, 140), true);
                    Assert(Math.Abs(timeline.Position - (timeline.ClipOffset(window.Clips[0]) + beforeEdgeDrag - window.Clips[0].Start)) < .001,
                        "Extending the linked audio edge keeps the playhead on the same source frame during the gesture");
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                    Assert(Math.Abs(window.Clips[0].Start - (1.15 - 5d / 60)) < .000001, "Audio edge pointer drag edits the same linked video clip");
                    await Ready();
                    Assert(Math.Abs(seekingPlayer.Position - beforeEdgeDrag) < .001 && Math.Abs(timeline.Position - (beforeEdgeDrag - window.Clips[0].Start)) < .001,
                        "Extending the left edge preserves the preview frame after joined clips close their gap");
                    Click("UndoButton"); await Ready(); Invoke("SeekTimeline", window.Clips[0], beforeEdgeDrag); await Ready();
                    var anchoredRight = timeline.ClipOffset(window.Clips[0]) + window.Clips[0].KeptDuration;
                    Pointer("BeginPointer", new Point(2, 65)); Pointer("MovePointer", new Point(2 + timeline.PixelsPerSecond * 5 / 60, 65), true);
                    Assert(Math.Abs(timeline.ClipOffset(window.Clips[0]) - 5d / 60) < .000001 && Math.Abs(timeline.ClipOffset(window.Clips[0]) + window.Clips[0].KeptDuration - anchoredRight) < .000001 && Math.Abs(timeline.ClipOffset(window.Clips[1]) - anchoredRight) < .000001,
                        "Dragging the left trim edge right follows the pointer while anchoring the right edge and following clip");
                    await Task.Delay(30);
                    SaveElement(timeline, Path.Combine(root, "left-trim-during-drag.png"));
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                    Assert(timeline.ClipOffset(window.Clips[0]) == 0 && Math.Abs(timeline.ClipOffset(window.Clips[1]) - (anchoredRight - 5d / 60)) < .000001,
                        "Joined clips close the temporary left-trim gap when the gesture finishes");
                    await Ready();
                    Assert(Math.Abs(seekingPlayer.Position - beforeEdgeDrag) < .001 && Math.Abs(timeline.Position - (beforeEdgeDrag - window.Clips[0].Start)) < .001,
                        "Shortening the left edge retains an interior preview frame instead of jumping to the trim start");
                    Click("UndoButton"); await Ready(); Invoke("SeekTimeline", window.Clips[0], 2.05); await Ready();
                    edgeX = window.Clips[0].KeptDuration * timeline.PixelsPerSecond - 2;
                    Pointer("BeginPointer", new Point(edgeX, 65)); Pointer("MovePointer", new Point(edgeX - timeline.PixelsPerSecond * 10 / 60, 65), true);
                    var lastRetainedFrame = window.Clips[0].End - 1 / window.Clips[0].FrameRate;
                    Assert(Math.Abs(timeline.Position - (lastRetainedFrame - window.Clips[0].Start)) < .001,
                        "A right trim that removes the preview frame clamps the playhead to the last retained frame during dragging");
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Ready();
                    Assert(Math.Abs(seekingPlayer.Position - lastRetainedFrame) < .001,
                        "A right trim that removes the preview frame seeks to the last retained frame instead of the clip start");
                    Click("UndoButton"); await Ready(); Invoke("SeekTimeline", window.Clips[0], 1.2); await Ready();
                    Pointer("BeginPointer", new Point(2, 140)); Pointer("MovePointer", new Point(2 + timeline.PixelsPerSecond * 6 / 60, 140), true);
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Ready();
                    Assert(Math.Abs(window.Clips[0].Start - 1.25) < .001 && Math.Abs(seekingPlayer.Position - 1.25) < .001 && Math.Abs(timeline.Position) < .001,
                        "A left trim that removes the preview frame clamps to the first retained frame");
                    Click("UndoButton"); await Ready(); Invoke("SeekTimeline", window.Clips[0], beforeEdgeDrag); await Ready();
                    Pointer("BeginPointer", new Point(2, 65)); Pointer("MovePointer", new Point(2 + timeline.PixelsPerSecond * 5 / 60, 65), true); Press(Key.Escape); await Ready();
                    Assert(Math.Abs(window.Clips[0].Start - 1.15) < .001 && Math.Abs(seekingPlayer.Position - beforeEdgeDrag) < .001 && Math.Abs(timeline.Position - (beforeEdgeDrag - 1.15)) < .001,
                        "Canceling an edge trim restores both its boundaries and the original preview frame");
                    Pointer("BeginPointer", new Point(window.Clips[0].KeptDuration * timeline.PixelsPerSecond / 2, 20));
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Task.Delay(80);
                    for (var wait = 0; wait < 50 && Math.Abs(timeline.Position - .5) >= .001; wait++) await Task.Delay(10);
                    Assert(timeline.SelectedClip == window.Clips[0] && Math.Abs(timeline.Position - .5) < .001, $"Clicking the ruler scrubs the global playhead without trimming (position {timeline.Position}, source start {window.Clips[0].Start}, selected first {timeline.SelectedClip == window.Clips[0]}, paused {seekingPlayer.Paused})");
                    Pointer("BeginPointer", new Point(timeline.PixelsPerSecond / 2, 20));
                    Pointer("MovePointer", new Point(timeline.PixelsPerSecond * 1.5, 20), true); surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Ready();
                    Assert(timeline.SelectedClip == window.Clips[1] && Math.Abs(timeline.Position - 1.5) < .001 && window.Clips.Count == 2, "Dragging the ruler scrubs across recordings without moving clips");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    Invoke("SplitAt", double.NaN); Invoke("SplitAt", 1.15);
                    Assert(window.Clips.Count == 2 && Control<TextBlock>("TrimHint").IsVisible, "Invalid splits do not change the timeline and show an error");
                    Invoke("SeekTimeline", window.Clips[0], 1.5); await Task.Delay(100); Click("SplitButton");
                    Assert(window.Clips.Count == 3 && Math.Abs(window.Clips[0].End - 1.5) < .001 && Math.Abs(window.Clips[1].Start - 1.5) < .001 && Math.Abs(timeline.Duration - 4) < .001, "Split at the shared playhead keeps every frame on both linked tracks");
                    var rightId = window.Clips[1].SectionId;
                    Click("UndoButton"); Click("RedoButton");
                    Assert(window.Clips.Count == 3 && window.Clips[1].SectionId == rightId && timeline.SelectedClip == window.Clips[1], "Undo and redo preserve identity across a same-recording split");
                    Invoke("Select", window.Clips[1], false); Invoke("SplitAt", 2d);
                    Assert(window.Clips.Count == 4 && window.Clips[1].End == 2 && window.Clips[2].Start == 2, "A second split isolates a middle portion");
                    Invoke("Select", window.Clips[1], false); Click("RemoveButton");
                    Assert(window.Clips.Count == 3 && window.Clips[0].End == 1.5 && window.Clips[1].Start == 2 && Math.Abs(timeline.ClipOffset(window.Clips[1]) - .35) < .001 && File.Exists(first.Path), "Deleting a middle portion closes the timeline gap without deleting the recording");
                    Click("UndoButton"); Click("RedoButton");
                    Assert(window.Clips.Count == 3 && window.Sources.Count == 2, "Deleting a middle portion supports undo and redo without duplicating the library");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    Invoke("SeekTimeline", window.Clips[0], window.Clips[0].End - 1d / 60); await Task.Delay(80); Click("ForwardButton"); await Ready();
                    Assert(timeline.SelectedClip == window.Clips[1] && Math.Abs(((NativePlayer)Field("player")!).Position - 2) < .001, "Frame stepping across a cut skips the deleted middle range");
                    Click("BackButton"); await Ready();
                    Assert(timeline.SelectedClip == window.Clips[0] && Math.Abs(((NativePlayer)Field("player")!).Position - (1.5 - 1d / 60)) < .001, "Backward frame stepping returns to the previous kept frame");
                    Invoke("Select", window.Clips[1], false); await Ready();
                    var menu = (ContextMenu)window.FindResource("SectionMenu"); menu.PlacementTarget = timeline; menu.DataContext = window.Clips[1]; menu.IsOpen = true; await Task.Delay(30);
                    var duplicate = menu.Items.OfType<MenuItem>().Single(i => (string)i.Tag == "duplicate");
                    Assert(menu.IsOpen && duplicate.IsEnabled && DarkAt(menu, new Point(5, 30)), "Timeline context menu opens with the dark theme");
                    SaveElement(menu, Path.Combine(root, "timeline-context-menu.png")); duplicate.RaiseEvent(new RoutedEventArgs(MenuItem.ClickEvent)); menu.IsOpen = false;
                    Assert(window.Clips.Count == 4 && window.Clips[2].Start == 2 && window.Clips[2].SectionId != window.Clips[1].SectionId, "Context menu duplicates only the targeted timeline clip");
                    Click("UndoButton");
                    var previousScale = timeline.PixelsPerSecond; timeline.Zoom(4); window.UpdateLayout();
                    Assert(Math.Abs(timeline.PixelsPerSecond - Math.Min(2000, previousScale * 4)) < .000001 && Descendants(timeline).OfType<ScrollViewer>().Single().ScrollableWidth > 0, $"Zoom creates a horizontally scrollable timeline with proportional durations (scale {previousScale} to {timeline.PixelsPerSecond}, scroll {Descendants(timeline).OfType<ScrollViewer>().Single().ScrollableWidth})");
                    timeline.Fit();
                    var sourceItem = (ListBoxItem)Control<ListBox>("SourceList").ItemContainerGenerator.ContainerFromItem(window.Sources[1]);
                    var sourceMenu = sourceItem.ContextMenu; sourceMenu.PlacementTarget = sourceItem; sourceMenu.IsOpen = true; await Task.Delay(20);
                    Assert(Control<ListBox>("SourceList").SelectedItem == window.Sources[1], "Recording context menu selects the clicked recording");
                    sourceMenu.Items.OfType<MenuItem>().Last().RaiseEvent(new RoutedEventArgs(MenuItem.ClickEvent)); sourceMenu.IsOpen = false;
                    Assert(window.Sources.Count == 1 && window.Clips.All(c => c.Path != source.Path) && File.Exists(source.Path), "Removing a recording from the project removes its linked clips and keeps the file");
                    Click("UndoButton");
                    Assert(window.Sources.Count == 2 && window.Clips.Count == 3, "Undo restores a removed recording and its timeline clips together");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    var player = (NativePlayer)Field("player")!;
                    Assert(player.Get("current-vo") == "libmpv" && player.Get("hwdec-current") == "no" && Control<VideoHost>("Video").PreviewBitmap != null, "Preview renders CPU frames through libmpv without a GPU output backend");
                    // Exercise two short nonadjacent ranges of the same source followed by a second file.
                    Invoke("Select", window.Clips[2], false); await Ready(); Invoke("CommitTrim", .5, 1d);
                    Invoke("Select", window.Clips[0], false); await Ready(); Click("PlayButton");
                    for (int i = 0; i < 160 && Control<TextBlock>("StatusText").Text != "Preview finished"; i++) await Task.Delay(30);
                    Assert(Control<TextBlock>("StatusText").Text == "Preview finished" && timeline.SelectedClip == window.Clips[2] && player.Paused && player.Position >= 1, "Space playback crosses same-source cuts and different recordings then stops at the final out point");
                    Click("PlayButton");
                    Assert(timeline.SelectedClip == window.Clips[0], "Playing at the timeline end restarts the whole sequence");
                    await Ready(); player.Set("pause", "yes");
                    await Task.Delay(1300);
                    var session = (RecoverySession)Field("recovery")!;
                    var recovery = await ProjectStore.ReadAsync(session.Path);
                    Assert(recovery.Clips.Count == 3 && recovery.Sources?.Count == 2, "Recovery saves the recording library and multiple timeline cuts");
                    var saved = Path.Combine(root, "timeline-session.sveproject");
                    typeof(MainWindow).GetField("projectPath", BindingFlags.NonPublic | BindingFlags.Instance)!.SetValue(window, saved);
                    Assert(await (Task<bool>)Invoke("SaveProjectAsync", false)! && !File.Exists(session.Path), "Project save clears only this window's recovery");
                    await (Task<bool>)Invoke("OpenProjectAsync", saved, false)!; await Ready();
                    Assert(window.Clips.Count == 3 && window.Sources.Count == 2 && window.Clips[1].Start == 2 && window.Clips.Select(c => c.SectionId).Distinct().Count() == 3, "Reopening restores library, order, removed ranges, and separate clip identities");
                    var legacy = Path.Combine(root, "timeline-legacy.sveproject"); await ProjectStore.SaveAsync(legacy, ProjectStore.Capture(window.Clips));
                    await (Task<bool>)Invoke("OpenProjectAsync", legacy, false)!; await Ready();
                    Assert(window.Sources.Count == 2 && window.Clips.Count == 3, "Older projects reconstruct the recording library from their sections");
                    var unused = Path.Combine(root, "timeline-unused.mp4"); File.Copy(Path.Combine(root, "source B.mp4"), unused, true);
                    await (Task)Invoke("ImportAsync", (object)new[] { unused })!;
                    await (Task<bool>)Invoke("SaveProjectAsync", false)!;
                    await (Task<bool>)Invoke("OpenProjectAsync", legacy, false)!;
                    Assert(window.Sources.Count == 3 && window.Clips.Count == 3, "Unused library recordings survive saving and reopening");
                    var canceled = Path.Combine(root, "timeline-cancel.mp4"); File.Copy(unused, canceled, true);
                    var canceledImport = (Task)Invoke("ImportAsync", (object)new[] { canceled })!;
                    ((CancellationTokenSource)Field("importCancellation")!).Cancel(); await canceledImport;
                    Assert(window.Sources.Count == 3 && window.Clips.Count == 3, "Canceling import preserves timeline and library");
                    var beforeRelink = ProjectStore.Capture(window.Clips, window.Sources);
                    var restoreAfterRelink = Path.Combine(root, "before-relink-ui.sveproject"); await ProjectStore.SaveAsync(restoreAfterRelink, beforeRelink);
                    var absentA = Path.Combine(root, "moved-recordings", "source A's clip.mp4"); var absentB = Path.Combine(root, "moved-recordings", "source B.mp4");
                    var relinkPath = Path.Combine(root, "missing-recordings.sveproject");
                    var absentDocument = new ProjectDocument(1, [new(absentA, .5, 1.5, 1), new(absentA, 2, 3, 4)], [absentA, absentB]);
                    await ProjectStore.SaveAsync(relinkPath, absentDocument);
                    var cancelRelink = HandleDialog<RelinkWindow>(dialog =>
                    {
                        Assert(!((Button)dialog.FindName("OpenButton")).IsEnabled && dialog.Missing.Count == 2, "Missing-recording dialog prevents opening until every source is located");
                        dialog.LocateRecording(absentA, Path.Combine(root, "source A's clip.mp4"));
                        Assert(dialog.Missing.All(r => r.Replacement != null), "Locating one recording also finds matching names from its original folder");
                        dialog.UpdateLayout();
                        SaveRender(dialog, Path.Combine(root, "relink-recordings.png"));
                        dialog.DialogResult = false;
                        return Task.CompletedTask;
                    });
                    var canceledRelink = await (Task<bool>)Invoke("OpenProjectAsync", relinkPath, false)!; await cancelRelink;
                    Assert(!canceledRelink && ProjectStore.Capture(window.Clips, window.Sources).Clips.SequenceEqual(beforeRelink.Clips) && window.Sources.Count == 3,
                        "Canceling a partially relinked project preserves the current editing session");
                    var applyRelink = HandleDialog<RelinkWindow>(dialog =>
                    {
                        dialog.LocateRecording(absentA, Path.Combine(root, "source A's clip.mp4"));
                        ((Button)dialog.FindName("OpenButton")).RaiseEvent(new RoutedEventArgs(Button.ClickEvent)); return Task.CompletedTask;
                    });
                    var openedRelink = await (Task<bool>)Invoke("OpenProjectAsync", relinkPath, false)!; await applyRelink; await Ready(); await WaveformsReady();
                    Assert(openedRelink && window.Clips.Count == 2 && window.Sources.Count == 2 && window.Clips.All(c => c.Path == Path.Combine(root, "source A's clip.mp4")) && window.Clips[0].Start == .5 && window.Clips[1].TimelineStart == 4 && window.Title.StartsWith("*"),
                        "Opening relinked recordings preserves repeated cuts, gaps, and unused recordings and marks new paths unsaved");
                    Assert((await ProjectStore.ReadAsync(relinkPath)).Clips[0].Path == absentA, "Relinking does not overwrite the project until Save");
                    await (Task<bool>)Invoke("SaveProjectAsync", false)!;
                    Assert((await ProjectStore.ReadAsync(relinkPath)).Clips.All(c => c.Path == Path.Combine(root, "source A's clip.mp4")), "Saving a relinked project persists the replacement paths");
                    var previewBeforeCache = Control<VideoHost>("Video").PreviewBitmap;
                    var clipsBeforeCache = ProjectStore.Capture(window.Clips, window.Sources);
                    var clearCache = HandleDialog<CacheWindow>(async dialog =>
                    {
                        var clearButton = (Button)dialog.FindName("ClearButton");
                        for (var i = 0; i < 100 && !clearButton.IsEnabled; i++) await Task.Delay(20);
                        Assert(clearButton.IsEnabled && ((TextBlock)dialog.FindName("TotalSizeText")).Text != "…", "Cache dialog displays measured thumbnail and waveform sizes");
                        SaveRender(dialog, Path.Combine(root, "preview-cache.png"));
                        clearButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                        for (var i = 0; i < 100 && !((Button)dialog.FindName("CloseButton")).IsEnabled; i++) await Task.Delay(20);
                        await File.WriteAllTextAsync(Path.Combine(root, "cache-clear-ui.json"), JsonSerializer.Serialize(new { Usage = PreviewCache.Inspect(), Status = ((TextBlock)dialog.FindName("StatusText")).Text }));
                        Assert(PreviewCache.Inspect().Files == 0 && ((TextBlock)dialog.FindName("StatusText")).Text.StartsWith("Freed"), "Clear cache removes the generated disk cache and reports the freed space");
                        dialog.UpdateLayout(); SaveRender(dialog, Path.Combine(root, "preview-cache-cleared.png"));
                    });
                    Invoke("Cache_Click", window, new RoutedEventArgs()); await clearCache;
                    Assert(ReferenceEquals(previewBeforeCache, Control<VideoHost>("Video").PreviewBitmap) && ProjectStore.Capture(window.Clips, window.Sources).Clips.SequenceEqual(clipsBeforeCache.Clips) && window.Clips.All(c => c.Waveform.Data != null),
                        "Clearing disk caches preserves the active video preview, waveform data, and timeline edits");
                    await (Task<bool>)Invoke("OpenProjectAsync", restoreAfterRelink, false)!; await Ready();
                    await WaveformsReady();
                    Assert(PreviewCache.Inspect().Files > 0 && window.Sources.Count == 3, "Reopening recordings regenerates cleared preview caches");
                    Invoke("Select", window.Clips[0], false); await Ready();
                    var buttons = new[] { "AddButton", "PlayButton", "SplitButton", "RemoveButton", "ExportButton", "UndoButton" };
                    // A reopened project has no undo entry; a duplicate gives the compact Undo button its enabled state.
                    Invoke("DuplicateSection"); await Ready(); window.UpdateLayout();
                    Assert(buttons.Select(name => MeasureButtonInk(Control<Button>(name))).All(m => ButtonInkCentered(m)), "Normal, compact, and export button labels remain centered");
                    Click("UndoButton"); timeline.Fit();
                    var allButtonMeasurements = Descendants(window).OfType<Button>().Where(b => b.IsVisible && b.IsEnabled && b.Content is string).Select(b => MeasureButtonInk(b, inWindow: true)).ToArray();
                    await File.WriteAllTextAsync(Path.Combine(root, "button-window-alignment.json"), JsonSerializer.Serialize(allButtonMeasurements, new JsonSerializerOptions { WriteIndented = true }));
                    Assert(allButtonMeasurements.All(m => ButtonInkCentered(m)), "All visible button labels are centered in the full window render");
                    var projectButtons = Descendants(window).OfType<Button>().Where(b => b.Content is string label && new[] { "New", "Open", "Save" }.Contains(label)).ToArray();
                    var projectRow = (FrameworkElement)VisualTreeHelper.GetParent(projectButtons[0]);
                    SaveElement(projectRow, Path.Combine(root, "project-buttons-aligned.png"));
                    foreach (var scale in new[] { 1d, 1.25, 1.5, 2d })
                    {
                        var measurements = projectButtons.Select(b => MeasureButtonInk(b, scale)).ToArray();
                        await File.WriteAllTextAsync(Path.Combine(root, $"project-button-baselines-{scale * 100:0}.json"), JsonSerializer.Serialize(measurements, new JsonSerializerOptions { WriteIndented = true }));
                        Assert(measurements.Length == 3 && measurements.All(m => m.Found) && measurements.Max(m => m.InkTop) - measurements.Min(m => m.InkTop) <= 1,
                            $"New, Open, and Save capital letters align at {scale * 100}% raster scale despite Open's descender");
                    }
                    foreach (var scale in new[] { 1d, 1.25, 1.5, 2d })
                    {
                        var fields = new[] { "StartInput", "EndInput" }.Select(name => Control<TextBox>(name)).ToArray();
                        var measurements = fields.Select(field => MeasureTextInk(field, field.Text, scale)).ToArray();
                        await File.WriteAllTextAsync(Path.Combine(root, $"time-field-alignment-{scale * 100:0}.json"), JsonSerializer.Serialize(measurements, new JsonSerializerOptions { WriteIndented = true }));
                        Assert(measurements.All(m => m.Found && Math.Abs(m.HorizontalError) <= 1.5 && Math.Abs(m.VerticalError) <= 1.5), $"Editable time fields center visible digits at {scale * 100}% scale");
                    }
                    var scaledButtons = Descendants(window).OfType<Button>().Where(b => b.IsVisible && b.IsEnabled && b.Content is string).ToArray();
                    foreach (var scale in new[] { 1.25, 1.5, 2d })
                    {
                        var measurements = scaledButtons.Select(b => MeasureButtonInk(b, scale)).ToArray();
                        await File.WriteAllTextAsync(Path.Combine(root, $"button-alignment-{scale * 100:0}.json"), JsonSerializer.Serialize(measurements, new JsonSerializerOptions { WriteIndented = true }));
                        Assert(measurements.All(m => ButtonInkCentered(m, scale)), $"Button glyphs stay centered when rasterized at {scale * 100}% scale");
                    }
                    SaveRender(window, Path.Combine(root, "timeline-populated.png"));
                    window.Width = 1000; window.Height = 740; window.UpdateLayout(); timeline.Fit(); window.UpdateLayout();
                    foreach (var name in new[] { "SourceList", "SearchInput", "AddToTimelineButton", "AddAllToTimelineButton", "CacheButton", "Timeline", "StartInput", "EndInput", "PlayButton", "SplitButton", "TrimBeforeButton", "TrimAfterButton", "ExportButton", "VolumeSlider" })
                    {
                        var element = Control<FrameworkElement>(name); var point = element.TransformToAncestor(window).Transform(new Point());
                        Assert(element.IsVisible && element.ActualWidth > 0 && point.X >= 0 && point.Y >= 0 && point.X + element.ActualWidth <= window.ActualWidth + 1 && point.Y + element.ActualHeight <= window.ActualHeight + 1, "Minimum window contains " + name);
                    }
                    SaveRender(window, Path.Combine(root, "timeline-minimum.png"));
                    Assert(Descendants(timeline).OfType<ScrollViewer>().Single().ViewportHeight >= 168, "Minimum layout fully exposes both track lanes and trim handles");
                    var dialog = new ExportWindow(window.Clips.ToArray()) { Owner = window }; dialog.Show(); dialog.UpdateLayout();
                    Assert(((Button)dialog.FindName("StartButton")).IsVisible && DarkAt((ComboBox)dialog.FindName("CodecInput"), new Point(5, 5)), "Export dialog still offers dark codec and quality controls"); dialog.Close();
                    Assert(window.Icon != null, "Timeline editor retains the approved application icon");
                    var pattern = await MediaTools.ProbeAsync(Path.Combine(root, "waveform-pattern.mkv"));
                    pattern.Waveform.Complete(await Task.Run(() => AudioWaveforms.LoadAsync(pattern)));
                    var waveformTimeline = new EditorTimeline { Width = 900, Height = 200, Clips = new([pattern]), SelectedClip = pattern };
                    var waveformSurface = (FrameworkElement)typeof(EditorTimeline).GetField("surface", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(waveformTimeline)!;
                    waveformTimeline.Measure(new Size(900, 200)); waveformTimeline.Arrange(new Rect(0, 0, 900, 200)); waveformTimeline.UpdateLayout(); waveformTimeline.Fit(); waveformTimeline.UpdateLayout();
                    bool PeakVisible(double timelineTime)
                    {
                        var image = CaptureElement(waveformSurface);
                        var x = (int)(timelineTime * waveformTimeline.PixelsPerSecond);
                        var pixels = new byte[5 * 4]; image.CopyPixels(new Int32Rect(x - 2, 130, 5, 1), pixels, 20, 0);
                        return Enumerable.Range(0, 5).Any(i => pixels[i * 4 + 1] > 120);
                    }
                    SaveElement(waveformTimeline, Path.Combine(root, "waveform-pattern-render.png"));
                    Assert(!PeakVisible(.3) && PeakVisible(1.1) && !PeakVisible(2), "Rendered waveform places the delayed sound at its source time and draws silence flat");
                    pattern.Start = 1; pattern.End = 2; waveformTimeline.Fit(); waveformTimeline.UpdateLayout();
                    Assert(PeakVisible(.1) && !PeakVisible(.7), "Trimming redraws the waveform using the selected source range");
                    pattern.Start = 0; pattern.End = 3; waveformTimeline.Fit(); waveformTimeline.Zoom(2); waveformTimeline.UpdateLayout();
                    Assert(!PeakVisible(.3) && PeakVisible(1.1), "Zooming preserves waveform timing and updates its cached geometry");
                    waveformTimeline.Clips = null;
                    var benchmarkClips = Enumerable.Range(0, 6).Select(i => { var clip = pattern.Clone(newSection: true); clip.TimelineStart = i * 3; return clip; }).ToArray();
                    var benchmarkTimeline = new EditorTimeline { Width = 1800, Height = 200, Clips = new(benchmarkClips) };
                    benchmarkTimeline.Measure(new Size(1800, 200)); benchmarkTimeline.Arrange(new Rect(0, 0, 1800, 200)); benchmarkTimeline.UpdateLayout(); benchmarkTimeline.Fit(); benchmarkTimeline.UpdateLayout();
                    var benchmarkSurface = (FrameworkElement)typeof(EditorTimeline).GetField("surface", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(benchmarkTimeline)!;
                    bool WhitePlayhead(BitmapSource bitmap, double time)
                    {
                        var x = (int)Math.Round(time * benchmarkTimeline.PixelsPerSecond);
                        var pixels = new byte[12]; bitmap.CopyPixels(new Int32Rect(x - 1, 170, 3, 1), pixels, 12, 0);
                        // Fractional positions leave partially covered antialiased pixels.
                        // This row is below both tracks, so only the playhead can be light.
                        return Enumerable.Range(0, 3).Any(i => pixels[i * 4] > 80 && pixels[i * 4 + 1] > 80 && pixels[i * 4 + 2] > 80);
                    }
                    benchmarkTimeline.Position = .7; benchmarkTimeline.UpdateLayout();
                    Assert(WhitePlayhead(CaptureElement(benchmarkSurface), .7), "Retained playhead drawing renders at its current timeline position");
                    benchmarkTimeline.Position = 12.3; benchmarkTimeline.UpdateLayout();
                    var movedPlayhead = CaptureElement(benchmarkSurface);
                    Assert(WhitePlayhead(movedPlayhead, 12.3) && !WhitePlayhead(movedPlayhead, .7), "Moving the retained playhead draws its new location without leaving the previous line behind");
                    for (var i = 0; i < 10; i++) { benchmarkTimeline.Position = i / 10d; benchmarkTimeline.UpdateLayout(); }
                    var allocatedBefore = GC.GetAllocatedBytesForCurrentThread();
                    var renderWatch = System.Diagnostics.Stopwatch.StartNew();
                    for (var i = 0; i < 300; i++) { benchmarkTimeline.Position = i / 20d; benchmarkTimeline.UpdateLayout(); }
                    renderWatch.Stop();
                    var renderBenchmark = new { Updates = 300, Clips = 6, ElapsedMs = renderWatch.Elapsed.TotalMilliseconds, AllocatedBytes = GC.GetAllocatedBytesForCurrentThread() - allocatedBefore };
                    await File.WriteAllTextAsync(Path.Combine(root, "playhead-update-benchmark.json"), JsonSerializer.Serialize(renderBenchmark, new JsonSerializerOptions { WriteIndented = true }));
                    benchmarkTimeline.Position = 3.1; benchmarkTimeline.Zoom(4); benchmarkTimeline.UpdateLayout();
                    Assert(WhitePlayhead(CaptureElement(benchmarkSurface), 3.1), "Zooming updates the playhead drawing to the new timeline scale");
                    var benchmarkScroll = Descendants(benchmarkTimeline).OfType<ScrollViewer>().Single();
                    benchmarkScroll.ScrollToHorizontalOffset(5000); benchmarkTimeline.UpdateLayout();
                    await Dispatcher.Yield(DispatcherPriority.Background); benchmarkTimeline.UpdateLayout();
                    var scrolledWaveform = CaptureElement(benchmarkSurface); var peakPixels = new byte[20];
                    scrolledWaveform.CopyPixels(new Int32Rect((int)(16.1 * benchmarkTimeline.PixelsPerSecond) - 2, 130, 5, 1), peakPixels, 20, 0);
                    Assert(benchmarkScroll.HorizontalOffset > 4000 && Enumerable.Range(0, 5).Any(i => peakPixels[i * 4 + 1] > 120), "Scrolling a paused timeline redraws the newly visible waveform region");
                    benchmarkTimeline.Clips = null;
                    var real = Path.Combine(root, "shadowplay-check.mp4");
                    if (File.Exists(real))
                    {
                        window.Width = 1320; window.Height = 880; window.UpdateLayout();
                        await (Task)Invoke("ImportAsync", (object)new[] { real })!;
                        Invoke("AppendSources", (object)new[] { window.Sources.Last() }); await Ready();
                        Assert(player.Get("video-format") == "av1" && player.Get("video-params/w") == "2560", "New timeline plays the real 1440p AV1 Shadowplay export");
                        Click("PlayButton"); await Task.Delay(300);
                        Assert(!player.Paused && player.Position > .15, "Real AV1 timeline preview advances"); player.Set("pause", "yes");
                        player.Command("screenshot-to-file", Path.Combine(root, "timeline-shadowplay-frame.png"), "video"); timeline.Fit(); window.UpdateLayout();
                        SaveRender(window, Path.Combine(root, "timeline-shadowplay.png"));
                    }
                    if (clips.Length >= 2)
                    {
                        var gameplayA = clips[0]; var gameplayB = clips[1];
                        var demo = Path.Combine(root, "timeline-gameplay.sveproject");
                        await ProjectStore.SaveAsync(demo, new ProjectDocument(1, [new(gameplayA, 2, 15), new(gameplayA, 25, 40), new(gameplayB, 0, 20)], [gameplayA, gameplayB]));
                        await (Task<bool>)Invoke("OpenProjectAsync", demo, false)!; await Ready();
                        await WaveformsReady();
                        Assert(window.Sources.All(c => c.Waveform.Data is { } data && data.Peaks.ToArray().Any(p => p > 0)), "Original Shadowplay audio produces real cached waveforms");
                        Invoke("SeekTimeline", window.Clips[1], 30d); await Ready(); timeline.Fit(); window.UpdateLayout();
                        Assert(window.Sources.Count == 2 && window.Clips.Count == 3 && timeline.Duration == 48 && player.Get("video-format") == "av1", "Original Shadowplay recordings display multiple kept parts in the linked timeline");
                        var host = Control<VideoHost>("Video");
                        Invoke("Select", window.Clips[0], false); await Ready();
                        var framesBefore = host.PresentedFrames;
                        var process = System.Diagnostics.Process.GetCurrentProcess();
                        var cpuBefore = process.TotalProcessorTime;
                        var benchmark = System.Diagnostics.Stopwatch.StartNew();
                        var delays = new List<double>();
                        Click("PlayButton");
                        while (benchmark.Elapsed.TotalSeconds < 3)
                        {
                            var previous = benchmark.Elapsed.TotalMilliseconds; await Task.Delay(50);
                            delays.Add(benchmark.Elapsed.TotalMilliseconds - previous);
                        }
                        player.Set("pause", "yes");
                        var frames = host.PresentedFrames - framesBefore;
                        var averageFps = frames / benchmark.Elapsed.TotalSeconds;
                        var cpuPercent = (process.TotalProcessorTime - cpuBefore).TotalSeconds / benchmark.Elapsed.TotalSeconds / Environment.ProcessorCount * 100;
                        var dropped = player.Get("frame-drop-count");
                        Assert(averageFps >= 45 && player.Position >= 4.5 && delays.Max() < 500, "Software preview displays original 1440p/60fps AV1 smoothly while the UI stays responsive");
                        Assert(host.PreviewBitmap!.PixelWidth <= 1280 && host.PreviewBitmap.PixelHeight <= 720 && player.Get("hwdec-current") == "no", "Preview resolution is bounded while source and export remain full resolution");
                        await File.WriteAllTextAsync(Path.Combine(root, "software-preview-performance.json"), JsonSerializer.Serialize(new { averageFps, frames, seconds = benchmark.Elapsed.TotalSeconds, cpuPercent, maxUiDelayMilliseconds = delays.Max(), dropped, previewWidth = host.PreviewBitmap.PixelWidth, previewHeight = host.PreviewBitmap.PixelHeight, decoder = player.Get("video-codec"), graphicsBackend = player.Get("current-vo") }, new JsonSerializerOptions { WriteIndented = true }));
                        Invoke("Select", window.Clips[1], false); await Ready();
                        using (var realGhost = ClipDragPreview.Attach(sourceList, window.Clips[1])!)
                        {
                            realGhost.MoveTo(new Point(550, 390)); await Task.Delay(30);
                            SaveRender(window, Path.Combine(root, "gameplay-drag-preview.png"));
                        }
                        SaveElement((FrameworkElement)((FrameworkElement)timeline.Parent).Parent, Path.Combine(root, "timeline-gameplay-panel.png"));
                        SaveRender(window, Path.Combine(root, "timeline-gameplay-window.png"));
                        player.Command("screenshot-to-file", Path.Combine(root, "timeline-gameplay-frame.png"), "video");
                    }
                    var modePath = Path.Combine(root, "timeline-free-mode.sveproject");
                    var freeA = await MediaTools.ProbeAsync(Path.Combine(root, "source A's clip.mp4")); freeA.Start = 0; freeA.End = 1;
                    var freeB = await MediaTools.ProbeAsync(Path.Combine(root, "source B.mp4")); freeB.Start = 0; freeB.End = 1;
                    await ProjectStore.SaveAsync(modePath, ProjectStore.Capture([freeA, freeB]));
                    await (Task<bool>)Invoke("OpenProjectAsync", modePath, false)!; await Ready();
                    Assert(window.FindName("TimelineModeButton") == null && window.Clips[0].TimelineStart == 0 && window.Clips[1].TimelineStart == 1, "Existing projects use automatic placement with no mode switch");
                    var freeId = window.Clips[0].SectionId;
                    timeline.Zoom(.2); window.UpdateLayout();
                    var freeScale = timeline.PixelsPerSecond;
                    Invoke("SeekTimeline", window.Clips[0], .25); await Ready();
                    var sourceBeforeDrag = ((NativePlayer)Field("player")!).Position;
                    Pointer("BeginPointer", new Point(.5 * freeScale, 65));
                    await Task.Delay(150);
                    Assert(Math.Abs(((NativePlayer)Field("player")!).Position - sourceBeforeDrag) < .001 && Math.Abs(timeline.Position - sourceBeforeDrag) < .001,
                        "Pressing a clip body leaves the existing preview frame and playhead unchanged until click or drag is resolved");
                    Pointer("MovePointer", new Point(3.75 * freeScale, 65), true);
                    Assert(Math.Abs(window.Clips[0].TimelineStart!.Value - 3.25) < .000001,
                        "Free dragging moves the linked clip continuously from its grab point");
                    Assert(!(dragLayer.GetAdorners(dragRoot) ?? []).OfType<ClipDragAdorner>().Any(),
                        "Moving an existing timeline clip uses the moving block without a floating thumbnail");
                    foreach (var pointerTime in new[] { 4.25, 3.5, 3.75 })
                    {
                        Pointer("MovePointer", new Point(pointerTime * freeScale, 65), true);
                        var dragPosition = timeline.Position;
                        Invoke("PlaybackTick");
                        Assert(Math.Abs(timeline.Position - dragPosition) < .000001 && Math.Abs(timeline.Position - window.Clips[0].TimelineStart!.Value - sourceBeforeDrag) < .000001,
                            $"Playback updates leave the drag playhead stable at {dragPosition:0.##} seconds");
                    }
                    Assert(Control<TextBlock>("PositionText").Text == Timecode.Format(timeline.Position), "The timestamp stays synchronized with the playhead during a free drag");
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Ready();
                    Invoke("PlaybackTick");
                    Assert(Math.Abs(((NativePlayer)Field("player")!).Position - sourceBeforeDrag) < .001 && Math.Abs(timeline.Position - 3.25 - sourceBeforeDrag) < .001,
                        "Releasing a free drag preserves the same source frame instead of seeking to the clip start");
                    Assert(Math.Abs(timeline.Position - (timeline.ClipOffset(timeline.SelectedClip!) + Math.Clamp(((NativePlayer)Field("player")!).Position - timeline.SelectedClip!.Start, 0, timeline.SelectedClip.KeptDuration))) < .000001,
                        "Playback controls the playhead again after a free drag finishes");
                    Assert(window.Clips[1].SectionId == freeId && window.Clips[0].TimelineStart == 1 && Math.Abs(timeline.Duration - 4.25) < .000001 && timeline.Locate(2.5) == null,
                        "Free movement preserves gaps and other clip positions while sorting playback order");
                    var undoBeforeCancel = ((System.Collections.ICollection)Field("undo")!).Count;
                    Pointer("BeginPointer", new Point(3.75 * freeScale, 65)); Pointer("MovePointer", new Point(4.75 * freeScale, 65), true); Press(Key.Escape); await Ready();
                    Assert(window.Clips[1].TimelineStart == 3.25 && ((System.Collections.ICollection)Field("undo")!).Count == undoBeforeCancel,
                        "Escape cancels a free drag and restores positions without an undo entry");
                    Assert(Math.Abs(((NativePlayer)Field("player")!).Position - sourceBeforeDrag) < .001 && Math.Abs(timeline.Position - 3.25 - sourceBeforeDrag) < .001,
                        "Canceling a free drag restores the original preview frame and playhead");
                    Click("UndoButton"); Assert(window.Clips[0].SectionId == freeId && window.Clips[0].TimelineStart == 0, "A free drag undoes in one step");
                    Click("RedoButton"); Assert(window.Clips[1].SectionId == freeId && window.Clips[1].TimelineStart == 3.25, "Redo restores free placement and clip identity");
                    DragClipTo(window.Clips[1], 1.25);
                    Assert(window.Clips[1].TimelineStart == 2 && window.Clips[0].TimelineStart == 1, "Free placement avoids overwriting another linked clip");
                    Click("UndoButton"); await Ready();
                    timeline.Fit(); window.UpdateLayout();
                    var countBeforeFreeDrop = window.Clips.Count;
                    Invoke("InsertSourceClip", window.Sources[1], 5.5); await Ready();
                    Assert(window.Clips.Count == countBeforeFreeDrop + 1 && window.Clips[^1].TimelineStart == 5.5 && window.Clips[0].TimelineStart == 1 && window.Clips[1].TimelineStart == 3.25,
                        "Dropping a library recording in Free mode uses its requested position without moving existing clips");
                    Click("UndoButton"); await Ready();
                    Pointer("BeginPointer", new Point(2.5 * timeline.PixelsPerSecond, 20)); surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Task.Delay(80);
                    Assert((bool)Field("gapPreview")! && !(bool)Field("gapPlaying")! && Control<VideoHost>("Video").ShowBlank && ((NativePlayer)Field("player")!).Paused,
                        "Paused scrubbing into a free gap displays black and silences audio");
                    Click("SplitButton"); Assert(window.Clips.Count == countBeforeFreeDrop && !Control<Button>("SplitButton").IsEnabled,
                        "A playhead in an empty gap cannot split an unrelated selected recording");
                    Click("PlayButton"); await Task.Delay(100); Invoke("PlaybackTick");
                    Assert((bool)Field("gapPlaying")! && timeline.Position > 2.55 && Control<System.Windows.Shapes.Path>("PauseGlyph").IsVisible,
                        "The single playback button plays and pauses empty timeline gaps");
                    Click("PlayButton"); var pausedGap = timeline.Position; await Task.Delay(100); Invoke("PlaybackTick");
                    Assert(Math.Abs(timeline.Position - pausedGap) < .001, "Pausing a free gap holds the playhead still");
                    Invoke("SeekGap", 3.2, true); await Task.Delay(150); Invoke("PlaybackTick"); await Ready();
                    Assert(!(bool)Field("gapPreview")! && timeline.SelectedClip!.SectionId == freeId && !((NativePlayer)Field("player")!).Paused && !Control<VideoHost>("Video").ShowBlank,
                        "Playing a gap resumes the next recording at the matching source position");
                    ((NativePlayer)Field("player")!).Set("pause", "yes");
                    Invoke("Select", window.Clips[1], false); await Ready();
                    var freeRight = timeline.ClipOffset(window.Clips[1]) + window.Clips[1].KeptDuration;
                    var freeLeftX = timeline.ClipOffset(window.Clips[1]) * timeline.PixelsPerSecond + 2;
                    Pointer("BeginPointer", new Point(freeLeftX, 140)); Pointer("MovePointer", new Point(freeLeftX + timeline.PixelsPerSecond / 4, 140), true);
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                    Assert(window.Clips[1].TimelineStart == 3.5 && window.Clips[1].Start == .25 && Math.Abs(timeline.ClipOffset(window.Clips[1]) + window.Clips[1].KeptDuration - freeRight) < .000001 && window.Clips[0].TimelineStart == 1,
                        "Free left-edge trimming preserves the anchored right edge, linked waveform, and neighboring clip");
                    Invoke("SeekTimeline", window.Clips[1], .6); await Ready();
                    var trimmedPreview = ((NativePlayer)Field("player")!).Position;
                    DragClipTo(window.Clips[1], 4.5); await Ready(); Invoke("PlaybackTick");
                    Assert(Math.Abs(((NativePlayer)Field("player")!).Position - trimmedPreview) < .001 && Math.Abs(timeline.Position - 4.5 - trimmedPreview + .25) < .001,
                        "Moving a clip with a trimmed source start preserves its source frame and correct relative playhead position");
                    Click("UndoButton"); await Ready();
                    Click("UndoButton"); await Ready();
                    Invoke("Select", window.Clips[1], false); Invoke("SplitAt", .5);
                    Assert(window.Clips.Count == 3 && window.Clips[1].TimelineStart == 3.25 && window.Clips[2].TimelineStart == 3.75 && timeline.Duration == 4.25,
                        "Splitting in Free mode preserves both parts' absolute positions and the total duration");
                    Click("RemoveButton"); Assert(window.Clips.Count == 2 && window.Clips[0].TimelineStart == 1 && window.Clips[1].TimelineStart == 3.25,
                        "Free deletion preserves neighboring clip positions without rippling");
                    Click("UndoButton"); Click("UndoButton"); await Ready();
                    Invoke("Select", window.Clips[0], false);
                    var freeEndX = 2 * timeline.PixelsPerSecond - 2;
                    Pointer("BeginPointer", new Point(freeEndX, 65)); Pointer("MovePointer", new Point(freeEndX + 2 * timeline.PixelsPerSecond, 65), true);
                    surface.ReleaseMouseCapture(); Pointer("FinishEdit");
                    Assert(timeline.ClipOffset(window.Clips[0]) + window.Clips[0].KeptDuration <= window.Clips[1].TimelineStart + .000001 && window.Clips[1].TimelineStart == 3.25,
                        "Extending a Free trim stops at the next clip instead of overlapping or moving it");
                    Click("UndoButton"); await Ready();
                    await (Task<bool>)Invoke("SaveProjectAsync", false)!;
                    await (Task<bool>)Invoke("OpenProjectAsync", modePath, false)!; await Ready();
                    Assert(window.Clips[0].TimelineStart == 1 && window.Clips[1].TimelineStart == 3.25, "Saving and reopening preserves free positions and timeline gaps");
                    Invoke("SeekTimeline", window.Clips[1], window.Clips[1].End); await Task.Delay(80); Click("PlayButton");
                    Assert((bool)Field("gapPreview")! && (bool)Field("gapPlaying")! && timeline.Position < .1, "Restarting a Free sequence includes its leading black and silent gap");
                    Pointer("BeginPointer", new Point(1.5 * timeline.PixelsPerSecond, 65)); surface.ReleaseMouseCapture(); Pointer("FinishEdit"); await Ready();
                    Assert(timeline.SelectedClip == window.Clips[0] && !((NativePlayer)Field("player")!).Paused, "Clicking a different recording while a gap plays preserves playback");
                    ((NativePlayer)Field("player")!).Set("pause", "yes"); Invoke("SeekGap", .95, true); await Task.Delay(150); Invoke("PlaybackTick"); await Ready();
                    Assert(!(bool)Field("gapPreview")! && timeline.SelectedClip == window.Clips[0] && !((NativePlayer)Field("player")!).Paused,
                        "Leading-gap playback enters the first clip without losing the playing state");
                    ((NativePlayer)Field("player")!).Set("pause", "yes");
                    SaveRender(window, Path.Combine(root, "timeline-free-mode.png"));
                    var outsideSnap = timeline.PlaceClip(window.Clips[1], 2 + 20 / timeline.PixelsPerSecond);
                    Assert(Math.Abs(outsideSnap - 2) > 10 / timeline.PixelsPerSecond, "Clips outside the snap distance remain freely positioned");
                    Invoke("SeekTimeline", window.Clips[1], .4); await Ready();
                    var snappedPreview = ((NativePlayer)Field("player")!).Position;
                    DragClipTo(window.Clips[1], 2 + 5 / timeline.PixelsPerSecond);
                    await Ready(); Invoke("PlaybackTick");
                    Assert(Math.Abs(((NativePlayer)Field("player")!).Position - snappedPreview) < .001 && Math.Abs(timeline.Position - 2 - snappedPreview) < .001,
                        "Snapping a moved clip to its neighbor preserves the preview frame");
                    Assert(window.Clips[1].TimelineStart == 2 && window.Clips[0].TimelineStart == 1, "Dragging close to a neighboring edge automatically snaps clips together");
                    Invoke("InsertSourceClip", window.Sources[1], 4d);
                    var separatedId = window.Clips[2].SectionId;
                    Invoke("Select", window.Clips[0], false); Invoke("CommitTrim", 0d, 3d);
                    Assert(window.Clips[0].End == 1 && window.Clips[2].TimelineStart == 4 && Control<TextBlock>("TrimHint").IsVisible,
                        "A joined trim cannot push its connected group into a separated clip");
                    Invoke("Select", window.Clips[0], false); Invoke("CommitTrim", 0d, .5);
                    Assert(window.Clips[0].TimelineStart == 1 && window.Clips[1].TimelineStart == 1.5 && window.Clips[2].TimelineStart == 4, "Trimming joined clips keeps their edges together while preserving a separate group's position");
                    Click("RemoveButton");
                    Assert(window.Clips[0].TimelineStart == 1 && window.Clips[1].SectionId == separatedId && window.Clips[1].TimelineStart == 4,
                        "Deleting a joined clip closes its local gap without pulling clips across an existing gap");
                    Click("UndoButton"); Click("UndoButton"); Click("UndoButton"); Click("UndoButton"); await Ready();
                    Assert(window.Clips[1].TimelineStart == 3.25 && window.Clips[0].End == 1, "Undo restores the previous gap and trim without changing a mode");
                    var positionsBeforeM = window.Clips.Select(c => c.TimelineStart).ToArray(); Press(Key.M);
                    Assert(window.Clips.Select(c => c.TimelineStart).SequenceEqual(positionsBeforeM), "M no longer toggles or repacks the timeline");
                    var beforeBatch = ProjectStore.Capture(window.Clips, window.Sources);
                    var beforeBatchCount = window.Clips.Count;
                    var beforeBatchDuration = timeline.Duration;
                    Click("AddAllToTimelineButton"); await Ready();
                    var afterBatch = ProjectStore.Capture(window.Clips, window.Sources);
                    Assert(afterBatch.Clips.Take(beforeBatchCount).SequenceEqual(beforeBatch.Clips) && afterBatch.Sources!.SequenceEqual(beforeBatch.Sources!) && window.Clips.Skip(beforeBatchCount).Select(c => c.Path).SequenceEqual(window.Sources.Select(c => c.Path)) && window.Clips[beforeBatchCount].TimelineStart == beforeBatchDuration,
                        "Add all appends after existing edits and gaps without changing the recording library");
                    Click("UndoButton"); await Ready();
                    Assert(ProjectStore.Capture(window.Clips, window.Sources).Clips.SequenceEqual(beforeBatch.Clips), "One undo restores the complete timeline before adding all recordings");
                    Click("RedoButton"); await Ready();
                    Assert(ProjectStore.Capture(window.Clips, window.Sources).Clips.SequenceEqual(afterBatch.Clips), "One redo restores the complete batch with its clip identities and positions");
                    Click("UndoButton"); await Ready();
                    foreach (var recording in window.Sources.ToArray())
                    {
                        Control<ListBox>("SourceList").SelectedItem = recording; Invoke("RemoveSource_Click", window, new RoutedEventArgs());
                        Assert(Control<Button>("AddAllToTimelineButton").Visibility == (window.Sources.Count > 1 ? Visibility.Visible : Visibility.Collapsed), "Removing recordings updates Add all visibility");
                    }
                    Assert(window.Sources.Count == 0 && Control<Button>("SaveButton").IsEnabled && await (Task<bool>)Invoke("SaveProjectAsync", false)! && (await ProjectStore.ReadAsync(modePath)).Clips.Count == 0,
                        "Clearing the last recording keeps Save available and persists the intentionally empty project");
                    await (Task<bool>)Invoke("SaveProjectAsync", false)!;
                    var closed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously); window.Closed += (_, _) => closed.TrySetResult(); window.Close(); await closed.Task.WaitAsync(TimeSpan.FromSeconds(5));
                    Assert(!window.IsVisible && dispatcherErrors.Count == 0, "Editing and normal close finish without dispatcher errors");
                    await File.WriteAllTextAsync(Path.Combine(root, "timeline-ui-results.json"), JsonSerializer.Serialize(new { passed = checks.Count, checks, coverage = "Actual WPF controls, project operations, and libmpv. Drag dispatch and editing callbacks tested in-process; external pointer automation unavailable. WPF renders include the software preview bitmap." }, new JsonSerializerOptions { WriteIndented = true }));
                    Console.WriteLine($"All {checks.Count} timeline UI checks passed.");
                }
                catch (Exception ex) { Console.Error.WriteLine(ex); code = 1; }
                finally
                {
                    if (window.IsVisible) await (Task)Invoke("ResetWaveformJobsAsync")!;
                    typeof(MainWindow).GetField("closing", BindingFlags.NonPublic | BindingFlags.Instance)!.SetValue(window, true);
                    ((DispatcherTimer)Field("recoveryTimer")!).Stop(); ((DispatcherTimer)Field("playbackTimer")!).Stop(); ((RecoverySession)Field("recovery")!).Dispose();
                    await Task.Run(() => ((NativePlayer?)Field("player"))?.Dispose());
                    if (window.IsVisible) window.Close(); app.Shutdown(); Dispatcher.CurrentDispatcher.BeginInvokeShutdown(DispatcherPriority.Background);
                }
            });
            app.Run(); Console.WriteLine("UI dispatcher exited");
        });
        thread.IsBackground = true; thread.SetApartmentState(ApartmentState.STA); thread.Start();
        if (!thread.Join(TimeSpan.FromSeconds(60))) { Console.Error.WriteLine("UI verification timed out."); return 1; }
        return code;
    }
    private static void SaveRender(Window window, string path)
    {
        var dpi = VisualTreeHelper.GetDpi(window);
        var image = new RenderTargetBitmap((int)Math.Ceiling(window.ActualWidth * dpi.DpiScaleX), (int)Math.Ceiling(window.ActualHeight * dpi.DpiScaleY), dpi.PixelsPerInchX, dpi.PixelsPerInchY, PixelFormats.Pbgra32);
        image.Render(window); var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(image)); using var stream = File.Create(path); encoder.Save(stream);
    }
    private static bool DarkAt(FrameworkElement element, Point point)
    {
        var bitmap = CaptureElement(element);
        var pixel = new byte[4]; bitmap.CopyPixels(new Int32Rect((int)point.X, (int)point.Y, 1, 1), pixel, 4, 0);
        return pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100;
    }
    private sealed record TextInk(string? Label, int Width, int Height, bool Found, double HorizontalError, double VerticalError, int InkTop);
    // Shared baselines intentionally leave capitals above center and descenders below it.
    private static bool ButtonInkCentered(TextInk ink, double scale = 1) => ink.Found && Math.Abs(ink.HorizontalError) <= 1 &&
        Math.Abs(ink.VerticalError) <= (ink.Label?.Any(char.IsLetter) == true ? 1.5 * scale + .5 : 1);
    private static TextInk MeasureButtonInk(Button button, double scale = 1, bool inWindow = false) => MeasureTextInk(button, button.Content?.ToString(), scale, inWindow);
    private static TextInk MeasureTextInk(Control control, string? label, double scale = 1, bool inWindow = false)
    {
        BitmapSource bitmap = inWindow ? CaptureInWindow(control) : CaptureElement(control, scale); var stride = bitmap.PixelWidth * 4;
        var pixels = new byte[stride * bitmap.PixelHeight]; bitmap.CopyPixels(pixels, stride, 0);
        var color = ((SolidColorBrush)control.Foreground).Color;
        var background = ((SolidColorBrush)control.Background).Color;
        var minX = bitmap.PixelWidth; var maxX = -1; var minY = bitmap.PixelHeight; var maxY = -1;
        for (var y = 0; y < bitmap.PixelHeight; y++) for (var x = 0; x < bitmap.PixelWidth; x++)
        {
            var i = y * stride + x * 4;
            if (x < 3 || y < 3 || x >= bitmap.PixelWidth - 3 || y >= bitmap.PixelHeight - 3 || pixels[i + 3] < 180) continue;
            // Include antialiased glyph edges; near-solid pixels alone lose thin descenders.
            var coverage = (pixels[i + 2] - background.R) / (double)(color.R - background.R);
            if (coverage < .4 || coverage > 1.05 ||
                Math.Abs(pixels[i + 1] - (background.G + coverage * (color.G - background.G))) > 6 ||
                Math.Abs(pixels[i] - (background.B + coverage * (color.B - background.B))) > 6) continue;
            minX = Math.Min(minX, x); maxX = Math.Max(maxX, x); minY = Math.Min(minY, y); maxY = Math.Max(maxY, y);
        }
        return new(label, bitmap.PixelWidth, bitmap.PixelHeight, maxX >= minX,
            (minX + maxX + 1 - bitmap.PixelWidth) / 2d, (minY + maxY + 1 - bitmap.PixelHeight) / 2d, minY);
    }
    private static BitmapSource CaptureInWindow(FrameworkElement element)
    {
        var window = Window.GetWindow(element); var dpi = VisualTreeHelper.GetDpi(window);
        var bitmap = new RenderTargetBitmap((int)Math.Ceiling(window.ActualWidth * dpi.DpiScaleX), (int)Math.Ceiling(window.ActualHeight * dpi.DpiScaleY), dpi.PixelsPerInchX, dpi.PixelsPerInchY, PixelFormats.Pbgra32);
        bitmap.Render(window); var point = element.TransformToAncestor(window).Transform(new Point());
        return new CroppedBitmap(bitmap, new Int32Rect((int)Math.Round(point.X * dpi.DpiScaleX), (int)Math.Round(point.Y * dpi.DpiScaleY), (int)Math.Round(element.ActualWidth * dpi.DpiScaleX), (int)Math.Round(element.ActualHeight * dpi.DpiScaleY)));
    }
    private static RenderTargetBitmap CaptureElement(FrameworkElement element, double scale = 1)
    {
        var visual = new DrawingVisual();
        using (var drawing = visual.RenderOpen())
        {
            var bounds = new Rect(0, 0, element.ActualWidth, element.ActualHeight);
            drawing.DrawRectangle(new SolidColorBrush(Color.FromRgb(25, 30, 35)), null, bounds);
            drawing.DrawRectangle(new VisualBrush(element), null, bounds);
        }
        var bitmap = new RenderTargetBitmap((int)Math.Ceiling(element.ActualWidth * scale), (int)Math.Ceiling(element.ActualHeight * scale), 96 * scale, 96 * scale, PixelFormats.Pbgra32);
        bitmap.Render(visual); return bitmap;
    }
    private static void SaveElement(FrameworkElement element, string path)
    {
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(CaptureElement(element)));
        using var stream = File.Create(path); encoder.Save(stream);
    }
    private static IEnumerable<DependencyObject> Descendants(DependencyObject parent)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i); yield return child;
            foreach (var descendant in Descendants(child)) yield return descendant;
        }
    }
}
