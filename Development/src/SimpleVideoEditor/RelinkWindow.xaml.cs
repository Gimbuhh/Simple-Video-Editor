using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Windows;
using Microsoft.Win32;
using SimpleVideoEditor.Services;

namespace SimpleVideoEditor;

public partial class RelinkWindow : Window
{
    private readonly ProjectDocument document;
    public ObservableCollection<MissingRecording> Missing { get; } = [];
    public ProjectDocument? ResolvedProject { get; private set; }
    public RelinkWindow(ProjectDocument document)
    {
        this.document = document;
        foreach (var path in ProjectStore.SourcePaths(document).Where(p => !LocalRecordingPath.Exists(p))) Missing.Add(new(path));
        InitializeComponent(); DataContext = this; MissingList.SelectedIndex = 0; RefreshState();
    }
    public void LocateRecording(string original, string replacement)
    {
        replacement = LocalRecordingPath.Validate(replacement);
        if (!LocalRecordingPath.Exists(replacement)) throw new FileNotFoundException("The replacement recording is missing.", replacement);
        var row = Missing.First(r => string.Equals(r.Original, original, StringComparison.OrdinalIgnoreCase));
        row.Replacement = replacement;
        var folder = Path.GetDirectoryName(replacement)!;
        foreach (var other in Missing.Where(r => r.Replacement == null && string.Equals(Path.GetDirectoryName(r.Original), Path.GetDirectoryName(row.Original), StringComparison.OrdinalIgnoreCase)))
        {
            var sibling = Path.Combine(folder, Path.GetFileName(other.Original));
            if (LocalRecordingPath.Exists(sibling)) other.Replacement = LocalRecordingPath.Validate(sibling);
        }
        MissingList.SelectedItem = Missing.FirstOrDefault(r => r.Replacement == null) ?? row;
        RefreshState();
    }
    private void RefreshState()
    {
        var remaining = Missing.Count(r => r.Replacement == null);
        RemainingText.Text = remaining == 0 ? "All recordings located." : $"{remaining} recording{(remaining == 1 ? "" : "s")} missing.";
        OpenButton.IsEnabled = remaining == 0;
    }
    private void Locate_Click(object sender, RoutedEventArgs e)
    {
        if (MissingList.SelectedItem is not MissingRecording row) return;
        var picker = new OpenFileDialog { Title = "Locate " + row.Name, FileName = row.Name, CheckFileExists = true, Filter = "Video files|*.mp4;*.mkv;*.mov;*.avi;*.webm;*.m4v|All files|*.*" };
        if (picker.ShowDialog(this) == true)
        {
            try { LocateRecording(row.Original, picker.FileName); }
            catch (Exception ex) { MessageBox.Show(this, ex.Message, "Could not locate recording", MessageBoxButton.OK, MessageBoxImage.Error); }
        }
    }
    private void Open_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            ResolvedProject = ProjectStore.Relink(document, Missing.Where(r => r.Replacement != null).ToDictionary(r => r.Original, r => r.Replacement!, StringComparer.OrdinalIgnoreCase));
            DialogResult = true;
        }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, "Could not locate recordings", MessageBoxButton.OK, MessageBoxImage.Error); }
    }
}

public sealed class MissingRecording(string original) : INotifyPropertyChanged
{
    public string Original { get; } = original;
    public string Name => Path.GetFileName(Original);
    private string? replacement;
    public string? Replacement { get => replacement; set { replacement = value; PropertyChanged?.Invoke(this, new(nameof(Replacement))); PropertyChanged?.Invoke(this, new(nameof(ReplacementLabel))); } }
    public string ReplacementLabel => Replacement == null ? "Missing" : "Located · " + Replacement;
    public event PropertyChangedEventHandler? PropertyChanged;
}
