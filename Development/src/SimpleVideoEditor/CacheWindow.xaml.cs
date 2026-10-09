using System.ComponentModel;
using System.Windows;
using SimpleVideoEditor.Services;

namespace SimpleVideoEditor;

public partial class CacheWindow : Window
{
    private readonly Func<Task<CacheClearResult>> clear;
    private bool clearing, closed;
    public CacheWindow(Func<Task<CacheClearResult>> clear) { this.clear = clear; InitializeComponent(); }
    private async Task RefreshUsageAsync()
    {
        var usage = await Task.Run(PreviewCache.Inspect);
        if (closed) return;
        ThumbnailSizeText.Text = PreviewCache.SizeLabel(usage.ThumbnailBytes);
        WaveformSizeText.Text = PreviewCache.SizeLabel(usage.WaveformBytes);
        TotalSizeText.Text = PreviewCache.SizeLabel(usage.TotalBytes);
        ClearButton.IsEnabled = !clearing && usage.Files > 0;
    }
    private async void Window_Loaded(object sender, RoutedEventArgs e)
    {
        try { await RefreshUsageAsync(); }
        catch (Exception ex) { StatusText.Text = "Could not read the cache · " + ex.Message; }
    }
    private async void Clear_Click(object sender, RoutedEventArgs e)
    {
        if (clearing) return;
        clearing = true; ClearButton.IsEnabled = CloseButton.IsEnabled = false;
        StatusText.Text = "Clearing cache…";
        try
        {
            var result = await clear();
            StatusText.Text = $"Freed {PreviewCache.SizeLabel(result.FreedBytes)}." + (result.InUse > 0 ? $" {result.InUse} files are in use; try again after closing other editor windows." : "");
        }
        catch (Exception ex) { StatusText.Text = "Could not clear the cache · " + ex.Message; }
        finally
        {
            clearing = false; CloseButton.IsEnabled = true;
            try { await RefreshUsageAsync(); } catch (Exception ex) { StatusText.Text = "Could not read the cache · " + ex.Message; }
        }
    }
    private void Close_Click(object sender, RoutedEventArgs e) => Close();
    private void Window_Closing(object? sender, CancelEventArgs e) { e.Cancel = clearing; if (!clearing) closed = true; }
}
