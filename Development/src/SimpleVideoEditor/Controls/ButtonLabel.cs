using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace SimpleVideoEditor.Controls;

/// <summary>Centers labels with a common baseline, independent of their letters.</summary>
public sealed class ButtonLabel : Control
{
    public ButtonLabel()
    {
        TextOptions.SetTextFormattingMode(this, TextFormattingMode.Display);
        TextOptions.SetTextRenderingMode(this, TextRenderingMode.ClearType);
        TextOptions.SetTextHintingMode(this, TextHintingMode.Fixed);
    }
    public static readonly DependencyProperty TextProperty = DependencyProperty.Register(nameof(Text), typeof(string), typeof(ButtonLabel),
        new FrameworkPropertyMetadata("", FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));
    public string Text { get => (string)GetValue(TextProperty); set => SetValue(TextProperty, value); }

    private FormattedText Format(string value) => new(value, CultureInfo.CurrentUICulture, FlowDirection,
        new Typeface(FontFamily, FontStyle, FontWeight, FontStretch), FontSize, Foreground,
        null, TextFormattingMode.Display, VisualTreeHelper.GetDpi(this).PixelsPerDip);

    protected override Size MeasureOverride(Size constraint)
    {
        var text = Format(Text);
        return new Size(Math.Ceiling(text.WidthIncludingTrailingWhitespace), Math.Ceiling(text.Height));
    }
    protected override void OnRender(DrawingContext context)
    {
        var text = Format(Text);
        var ink = text.BuildGeometry(new Point()).Bounds;
        if (ink.IsEmpty) return;
        // Use outlines only to measure visible ink; native text drawing preserves font hinting.
        var dpi = VisualTreeHelper.GetDpi(this);
        var x = (ActualWidth - ink.Width) / 2 - ink.X;
        // A capital and a descender establish the same vertical reference for every word.
        // Centering each word's ink separately raises labels such as "Open" above "New".
        // Symbol-only controls still center their individual symbols like icons.
        var verticalInk = ink;
        if (Text.Any(char.IsLetter))
        {
            var capitals = Format("H").BuildGeometry(new Point()).Bounds;
            var descenders = Format("Hg").BuildGeometry(new Point()).Bounds;
            // Balance capital-only words and words with descenders around the button center.
            verticalInk = new Rect(capitals.X, capitals.Y, capitals.Width, (capitals.Height + descenders.Height) / 2);
        }
        var y = Math.Round(((ActualHeight - verticalInk.Height) / 2 - verticalInk.Y) * dpi.DpiScaleY) / dpi.DpiScaleY;
        context.DrawText(text, new(x, y));
    }
}
