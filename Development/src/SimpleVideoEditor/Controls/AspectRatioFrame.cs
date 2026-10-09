using System.Windows;
using System.Windows.Controls;

namespace SimpleVideoEditor.Controls;

/// <summary>Fits the entire preview inside its available space without padding the video surface.</summary>
public sealed class AspectRatioFrame : Decorator
{
    public static readonly DependencyProperty AspectRatioProperty = DependencyProperty.Register(
        nameof(AspectRatio), typeof(double), typeof(AspectRatioFrame),
        new FrameworkPropertyMetadata(16d / 9, FrameworkPropertyMetadataOptions.AffectsMeasure),
        value => value is double ratio && double.IsFinite(ratio) && ratio > 0);

    public double AspectRatio { get => (double)GetValue(AspectRatioProperty); set => SetValue(AspectRatioProperty, value); }

    private Size Fit(Size available)
    {
        var width = Math.Min(available.Width, available.Height * AspectRatio);
        if (double.IsPositiveInfinity(width)) width = Child?.DesiredSize.Width ?? 0;
        return new Size(width, width / AspectRatio);
    }

    protected override Size MeasureOverride(Size constraint)
    {
        var size = Fit(constraint);
        Child?.Measure(size);
        return size;
    }

    protected override Size ArrangeOverride(Size available)
    {
        var size = Fit(available);
        Child?.Arrange(new Rect((available.Width - size.Width) / 2, (available.Height - size.Height) / 2, size.Width, size.Height));
        return available;
    }
}
