using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using Microsoft.Win32;
using SimpleVideoEditor.Services;

namespace SimpleVideoEditor;

public partial class ExportWindow : Window
{
    private readonly IReadOnlyList<MediaClip> clips;
    private CancellationTokenSource? cancellation;
    private bool running;
    private bool complete;
    public ExportWindow(IReadOnlyList<MediaClip> clips)
    {
        this.clips = clips;
        InitializeComponent();
        SummaryText.Text = $"{clips.Count} sections · {Timecode.Format(TimelineLayout.Duration(clips))} · {clips[0].FrameRate:0.##} fps";
        OriginalResolutionItem.Content = $"{clips[0].Width} × {clips[0].Height} · same as first clip";
    }
    private void Browse_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new SaveFileDialog { Title = "Save combined video", Filter = "MP4 video|*.mp4", FileName = "Highlights.mp4", DefaultExt = ".mp4", AddExtension = true, OverwritePrompt = true };
        if (dialog.ShowDialog(this) == true) OutputInput.Text = dialog.FileName;
    }
    private void Quality_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (QualityHint == null) return;
        QualityHint.Text = QualityInput.SelectedIndex switch { 0 => "Best detail; larger files.", 1 => "Balanced detail and file size.", _ => "Smaller files; less fine detail." };
    }
    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        if (running || complete) return;
        if (string.IsNullOrWhiteSpace(OutputInput.Text)) Browse_Click(sender, e);
        if (string.IsNullOrWhiteSpace(OutputInput.Text)) return;
        if (File.Exists(OutputInput.Text) && MessageBox.Show(this, "Replace the existing output file?", "Export video", MessageBoxButton.YesNo, MessageBoxImage.Question) != MessageBoxResult.Yes) return;
        var first = clips[0];
        var dimensions = ResolutionInput.SelectedIndex switch { 1 => (1920, 1080), 2 => (1280, 720), _ => (first.Width / 2 * 2, first.Height / 2 * 2) };
        var settings = new ExportSettings(CodecInput.SelectedIndex == 0 ? ExportCodec.Av1 : ExportCodec.H264, (ExportQuality)QualityInput.SelectedIndex, dimensions.Item1, dimensions.Item2, first.FrameRate, GpuInput.IsChecked == true);
        running = true; cancellation = new(); OptionsPanel.IsEnabled = false; StartButton.IsEnabled = false; CancelButton.Content = "Cancel export"; ProgressBar.Visibility = Visibility.Visible;
        try
        {
            await new ExportService().ExportAsync(clips, OutputInput.Text, settings, new Progress<ExportProgress>(p => { ProgressBar.Value = p.Fraction; ProgressText.Text = $"{p.Fraction:P0} · {p.Message}"; }), cancellation.Token);
            complete = true; ProgressText.Text = "Your video is ready."; RevealButton.Visibility = Visibility.Visible; StartButton.Visibility = Visibility.Collapsed;
        }
        catch (OperationCanceledException) { ProgressText.Text = "Export canceled. You can change settings and try again."; }
        catch (Exception ex) { ProgressText.Text = "Export failed. Your source recordings are safe."; MessageBox.Show(this, ex.Message, "Export failed", MessageBoxButton.OK, MessageBoxImage.Error); }
        finally { running = false; cancellation.Dispose(); cancellation = null; OptionsPanel.IsEnabled = !complete; StartButton.IsEnabled = !complete; CancelButton.Content = "Close"; CancelButton.IsEnabled = true; }
    }
    private void Cancel_Click(object sender, RoutedEventArgs e) { if (running) { cancellation?.Cancel(); CancelButton.IsEnabled = false; ProgressText.Text = "Canceling export…"; } else Close(); }
    private void Reveal_Click(object sender, RoutedEventArgs e)
    {
        var start = new ProcessStartInfo("explorer.exe") { UseShellExecute = true };
        start.ArgumentList.Add("/select,"); start.ArgumentList.Add(OutputInput.Text); Process.Start(start);
    }
    private void Window_Closing(object? sender, CancelEventArgs e) { if (running) { e.Cancel = true; cancellation?.Cancel(); ProgressText.Text = "Canceling export… Close the window once cancellation finishes."; } }
}
