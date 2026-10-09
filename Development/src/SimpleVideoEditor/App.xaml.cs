using System.IO;
using System.Windows;

namespace SimpleVideoEditor;

public partial class App : Application
{
    public App()
    {
        // Avoid an application-owned Direct3D presentation surface that capture overlays can hook.
        System.Windows.Media.RenderOptions.ProcessRenderMode = System.Windows.Interop.RenderMode.SoftwareOnly;
    }
    protected override void OnStartup(StartupEventArgs e)
    {
        DispatcherUnhandledException += (_, args) =>
        {
            if (args.Handled) return;
            var folder = Services.ProjectStore.DataDirectory;
            Directory.CreateDirectory(folder);
            File.AppendAllText(Path.Combine(folder, "errors.log"), $"{DateTimeOffset.Now}\n{args.Exception}\n");
            MessageBox.Show(args.Exception.Message, "Something went wrong", MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true;
        };
        base.OnStartup(e);
        if (MainWindow == null) { MainWindow = new MainWindow(); MainWindow.Show(); }
    }
}
