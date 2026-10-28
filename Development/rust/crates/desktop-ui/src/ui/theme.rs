use super::*;

pub(super) const PANEL: Color32 = Color32::from_rgb(25, 29, 36);
pub(super) const CANVAS: Color32 = Color32::from_rgb(13, 16, 21);
pub(super) const SURFACE: Color32 = Color32::from_rgb(36, 42, 52);
pub(super) const HOVER: Color32 = Color32::from_rgb(45, 53, 65);
pub(super) const BORDER: Color32 = Color32::from_rgb(56, 66, 81);
pub(super) const TEXT: Color32 = Color32::from_rgb(237, 241, 247);
pub(super) const SECONDARY: Color32 = Color32::from_rgb(185, 196, 211);
pub(super) const ACCENT: Color32 = Color32::from_rgb(76, 154, 255);
pub(super) const PRIMARY: Color32 = Color32::from_rgb(31, 103, 213);
pub(super) const SELECTED: Color32 = Color32::from_rgb(30, 48, 73);
pub(super) const AUDIO: Color32 = Color32::from_rgb(29, 38, 52);
pub(super) const WAVE: Color32 = Color32::from_rgb(128, 164, 215);

pub(super) fn panel(color: Color32) -> egui::Frame {
    egui::Frame::new().fill(color).inner_margin(12)
}
pub(super) fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut heading = egui::FontFamily::Proportional;
    // Use installed Windows fonts without redistributing their files. The
    // bundled egui fonts remain the fallback on machines without these faces.
    if let Some(windows) = std::env::var_os("WINDIR") {
        let directory = PathBuf::from(windows).join("Fonts");
        for (name, file, family) in [
            ("windows-ui", "segoeui.ttf", egui::FontFamily::Proportional),
            (
                "windows-semibold",
                "seguisb.ttf",
                egui::FontFamily::Name("ui-semibold".into()),
            ),
            ("windows-mono", "consola.ttf", egui::FontFamily::Monospace),
        ] {
            if let Ok(bytes) = std::fs::read(directory.join(file)) {
                fonts
                    .font_data
                    .insert(name.into(), egui::FontData::from_owned(bytes).into());
                let fallback = fonts.families[&egui::FontFamily::Proportional].clone();
                fonts
                    .families
                    .entry(family.clone())
                    .or_insert(fallback)
                    .insert(0, name.into());
                if name == "windows-semibold" {
                    heading = family;
                }
            }
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::new(15.5, heading));
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(36.0, 20.0);
    style.spacing.slider_rail_height = 4.0;
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = egui::CornerRadius::same(7);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.extreme_bg_color = CANVAS;
    visuals.text_edit_bg_color = Some(SURFACE);
    visuals.weak_text_color = Some(SECONDARY);
    visuals.selection.bg_fill = SELECTED;
    visuals.selection.stroke = Stroke::new(1.5, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    visuals.disabled_alpha = 0.55;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = SURFACE;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, TEXT);
        widget.corner_radius = egui::CornerRadius::same(5);
        widget.expansion = 0.0;
    }
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.active.weak_bg_fill = SELECTED;
    visuals.widgets.active.bg_fill = SELECTED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    visuals.widgets.open.weak_bg_fill = SELECTED;
    style.visuals = visuals;
    ctx.set_style(style);
}

pub(super) enum TransportIcon {
    Play,
    Pause,
    Previous,
    Next,
}
pub(super) fn transport_button(
    ui: &mut egui::Ui,
    icon: TransportIcon,
    label: &str,
) -> egui::Response {
    let response = ui
        .add(egui::Button::new("").min_size(Vec2::new(36.0, 32.0)))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let center = response.rect.center();
    let color = ui.style().interact(&response).fg_stroke.color;
    let painter = ui.painter();
    match icon {
        TransportIcon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-4.0, -7.0),
                    center + Vec2::new(7.0, 0.0),
                    center + Vec2::new(-4.0, 7.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        TransportIcon::Pause => {
            for x in [-5.0, 2.0] {
                painter.rect_filled(
                    Rect::from_min_size(center + Vec2::new(x, -7.0), Vec2::new(3.0, 14.0)),
                    1.0,
                    color,
                );
            }
        }
        TransportIcon::Previous | TransportIcon::Next => {
            let direction = if matches!(icon, TransportIcon::Previous) {
                -1.0
            } else {
                1.0
            };
            painter.line_segment(
                [
                    center + Vec2::new(direction * 7.0, -6.0),
                    center + Vec2::new(direction * 7.0, 6.0),
                ],
                Stroke::new(1.5, color),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-direction * 5.0, -6.0),
                    center + Vec2::new(direction * 4.0, 0.0),
                    center + Vec2::new(-direction * 5.0, 6.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
    response
}
#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(color: Color32) -> f64 {
        let linear = |value: u8| {
            let v = f64::from(value) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    #[test]
    fn normal_text_and_export_label_have_readable_contrast() {
        for (foreground, background) in [
            (TEXT, PANEL),
            (SECONDARY, PANEL),
            (SECONDARY, SURFACE),
            (TEXT, SELECTED),
            (Color32::WHITE, PRIMARY),
        ] {
            let (a, b) = (luminance(foreground), luminance(background));
            assert!(
                (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5,
                "Insufficient text contrast: {foreground:?} on {background:?}"
            );
        }
    }
}
