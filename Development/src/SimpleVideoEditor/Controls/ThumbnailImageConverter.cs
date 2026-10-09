using System.Globalization;
using System.Windows.Data;
using System.Windows.Media.Imaging;

namespace SimpleVideoEditor.Controls;

/// <summary>Loads small preview images into memory so displayed cards do not lock cache files.</summary>
public sealed class ThumbnailImageConverter : IValueConverter
{
    public object? Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is not string path) return null;
        try
        {
            var image = new BitmapImage(); image.BeginInit(); image.CacheOption = BitmapCacheOption.OnLoad;
            image.UriSource = new Uri(path, UriKind.Absolute); image.DecodePixelWidth = 240;
            image.EndInit(); image.Freeze(); return image;
        }
        catch { return null; } // An unavailable thumbnail must not prevent editing its recording.
    }
    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotSupportedException();
}
