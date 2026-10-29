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

// Center the painted glyphs, not the font's advance/line box. Font bearings
// and unused ascent/descent otherwise make a mathematically centered label drift.
pub(super) struct ActionButton {
    text: egui::WidgetText,
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    frame: Option<bool>,
    min_size: Vec2,
    row_baseline: bool,
}
pub(super) fn button(text: impl Into<egui::WidgetText>) -> ActionButton {
    ActionButton {
        text: text.into(),
        fill: None,
        stroke: None,
        frame: None,
        min_size: Vec2::ZERO,
        row_baseline: false,
    }
}
/// Inline text actions share the same line metrics as adjacent labels.
pub(super) fn row_button(text: impl Into<egui::WidgetText>) -> ActionButton {
    ActionButton {
        row_baseline: true,
        ..button(text)
    }
}
pub(super) fn action_row<R>(
    ui: &mut egui::Ui,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.scope(|ui| {
        // Reserve the final height before placing labels/spinners. Growing a
        // horizontal row after its first label otherwise lowers later buttons.
        // Use laid-out line height: font metrics alone omit galley rounding
        // at fractional DPI and can leave the first label one pixel higher.
        let line_height = egui::WidgetText::from("Mg")
            .into_galley(ui, None, f32::INFINITY, egui::TextStyle::Button)
            .size()
            .y;
        ui.spacing_mut().interact_size.y = ui
            .spacing()
            .interact_size
            .y
            .max(line_height + 2.0 * ui.spacing().button_padding.y);
        ui.horizontal(contents)
    })
    .inner
}
impl ActionButton {
    pub(super) fn fill(mut self, fill: Color32) -> Self {
        self.fill = Some(fill);
        self
    }
    pub(super) fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self.frame = Some(true);
        self
    }
    pub(super) fn frame(mut self, frame: bool) -> Self {
        self.frame = Some(frame);
        self
    }
    pub(super) fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }
}
impl egui::Widget for ActionButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let padding = if self.frame.unwrap_or(ui.visuals().button_frame) {
            ui.spacing().button_padding
        } else {
            Vec2::ZERO
        };
        let galley = self.text.into_galley(
            ui,
            None,
            (ui.available_width() - 2.0 * padding.x).max(0.0),
            egui::TextStyle::Button,
        );
        let slot = ui.next_auto_id().with("action-label");
        let mut button =
            egui::Button::new(egui::Atom::custom(slot, galley.size())).min_size(self.min_size);
        if let Some(fill) = self.fill {
            button = button.fill(fill);
        }
        if let Some(stroke) = self.stroke {
            button = button.stroke(stroke);
        }
        if let Some(frame) = self.frame {
            button = button.frame(frame);
        }
        // Keep egui's input, focus, disabled rendering, frames and hit testing.
        let response = button.atom_ui(ui).response;
        paint_button_label(ui, &response, galley, self.row_baseline);
        response
    }
}

pub(super) fn menu_button<R>(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let galley = text.into().into_galley(
        ui,
        None,
        (ui.available_width() - 2.0 * ui.spacing().button_padding.x).max(0.0),
        egui::TextStyle::Button,
    );
    let slot = ui.next_auto_id().with("menu-label");
    let menu = ui.menu_button(egui::Atom::custom(slot, galley.size()), contents);
    paint_button_label(ui, &menu.response, galley, false);
    menu
}

fn paint_button_label(
    ui: &egui::Ui,
    response: &egui::Response,
    galley: Arc<egui::Galley>,
    row_baseline: bool,
) {
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), galley.text())
    });
    if ui.is_rect_visible(response.rect) {
        let ink = if galley.mesh_bounds.is_positive() {
            galley.mesh_bounds
        } else {
            galley.rect
        };
        let origin = Pos2::new(
            response.rect.center().x - ink.center().x,
            response.rect.center().y
                - if row_baseline {
                    galley.rect.center().y
                } else {
                    ink.center().y
                },
        );
        let ppp = ui.pixels_per_point();
        let origin = Pos2::new(
            (origin.x * ppp).round() / ppp,
            (origin.y * ppp).round() / ppp,
        );
        ui.painter()
            .galley(origin, galley, ui.style().interact(response).text_color());
    }
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
    #[test]
    fn action_rows_paint_labels_on_a_shared_baseline() {
        for fallback in [false, true] {
            let ctx = egui::Context::default();
            configure(&ctx);
            if fallback {
                ctx.set_fonts(egui::FontDefinitions::default());
                ctx.style_mut(|s| {
                    s.text_styles
                        .insert(egui::TextStyle::Heading, FontId::proportional(15.5));
                });
            }
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                for timeline in [false, true] {
                    for enabled in [false, true] {
                        let mut input = egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(900.0, 200.0),
                            )),
                            ..Default::default()
                        };
                        input
                            .viewports
                            .get_mut(&egui::ViewportId::ROOT)
                            .unwrap()
                            .native_pixels_per_point = Some(scale);
                        let mut buttons = Vec::new();
                        let mut spinner = Rect::NOTHING;
                        let output = ctx.run(input, |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                action_row(ui, |ui| {
                                    if timeline {
                                        ui.heading("Timeline");
                                        ui.separator();
                                        for label in ["Undo", "Redo", "Split", "Delete"] {
                                            buttons.push((
                                                label,
                                                ui.add_enabled(enabled, row_button(label)).rect,
                                            ));
                                        }
                                    } else {
                                        spinner = ui.spinner().rect;
                                        ui.label("Exporting…");
                                        buttons.push((
                                            "Cancel",
                                            ui.add_enabled(enabled, row_button("Cancel")).rect,
                                        ));
                                    }
                                });
                            });
                        });
                        let text = |label: &str| {
                            output
                                .shapes
                                .iter()
                                .find_map(|s| match &s.shape {
                                    egui::Shape::Text(text) if text.galley.text() == label => {
                                        Some(text)
                                    }
                                    _ => None,
                                })
                                .expect("Missing painted row label")
                        };
                        // Inspect the actual painted glyph baseline, including the
                        // galley row offset, rather than comparing widget boxes.
                        let baseline = |label: &str| {
                            let text = text(label);
                            let row = &text.galley.rows[0];
                            // epaint snaps each TextShape origin during tessellation.
                            (text.pos.y * scale).round() / scale + row.pos.y + row.glyphs[0].pos.y
                        };
                        let reference = baseline(if timeline { "Undo" } else { "Exporting…" });
                        for (label, rect) in &buttons {
                            let delta = (baseline(label) - reference) * scale;
                            assert!(
                                delta.abs() <= 0.51,
                                "{label} baseline drifts {delta} pixels: scale={scale}, fallback={fallback}, enabled={enabled}"
                            );
                            let text = text(label);
                            assert!(rect.contains_rect(
                                text.galley.mesh_bounds.translate(text.pos.to_vec2())
                            ));
                            let center = text.pos.x + text.galley.mesh_bounds.center().x;
                            assert!(((center - rect.center().x) * scale).abs() <= 0.51);
                        }
                        if !timeline {
                            assert!(
                                ((spinner.center().y - buttons[0].1.center().y) * scale).abs()
                                    <= 0.51,
                                "Spinner and Cancel must share the final row center"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn action_labels_center_visible_glyphs_at_each_scale_and_state() {
        for fallback in [false, true] {
            let ctx = egui::Context::default();
            configure(&ctx);
            ctx.enable_accesskit();
            if fallback {
                ctx.set_fonts(egui::FontDefinitions::default());
                ctx.style_mut(|s| {
                    s.text_styles
                        .insert(egui::TextStyle::Heading, FontId::proportional(15.5));
                });
            }
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                for layout in [
                    egui::Layout::left_to_right(egui::Align::Center),
                    egui::Layout::right_to_left(egui::Align::Center),
                    egui::Layout::top_down(egui::Align::Min),
                ] {
                    for label in [
                        "+ Import",
                        "Export",
                        "Save",
                        "Reset",
                        "Trim before",
                        "×",
                        "Ångström",
                        "File",
                        "…",
                    ] {
                        for size in [Vec2::ZERO, Vec2::new(140.0, 40.0)] {
                            if matches!(label, "File" | "…") && size != Vec2::ZERO {
                                continue;
                            }
                            for state in ["rest", "hover", "press", "focus", "disabled"] {
                                let input = |events| {
                                    let mut input = egui::RawInput {
                                        screen_rect: Some(Rect::from_min_size(
                                            Pos2::ZERO,
                                            Vec2::new(640.0, 480.0),
                                        )),
                                        events,
                                        ..Default::default()
                                    };
                                    input
                                        .viewports
                                        .get_mut(&egui::ViewportId::ROOT)
                                        .unwrap()
                                        .native_pixels_per_point = Some(scale);
                                    input
                                };
                                let draw = |ctx: &egui::Context| {
                                    egui::CentralPanel::default()
                                        .show(ctx, |ui| {
                                            ui.with_layout(layout, |ui| {
                                                if matches!(label, "File" | "…") {
                                                    return ui
                                                        .add_enabled_ui(state != "disabled", |ui| {
                                                            menu_button(ui, label, |ui| {
                                                                ui.label("Menu item");
                                                            })
                                                            .response
                                                        })
                                                        .inner;
                                                }
                                                let mut action = button(label).min_size(size);
                                                if label == "+ Import" {
                                                    action =
                                                        action.stroke(Stroke::new(1.0, BORDER));
                                                }
                                                if label == "Export" {
                                                    action = action.fill(PRIMARY);
                                                }
                                                ui.add_enabled(state != "disabled", action)
                                            })
                                            .inner
                                        })
                                        .inner
                                };
                                egui::Popup::close_all(&ctx);
                                let mut response = None;
                                let _ = ctx.run(
                                    input(vec![
                                        egui::Event::PointerButton {
                                            pos: Pos2::ZERO,
                                            button: egui::PointerButton::Primary,
                                            pressed: false,
                                            modifiers: Default::default(),
                                        },
                                        egui::Event::PointerGone,
                                    ]),
                                    |ctx| {
                                        response = Some(draw(ctx));
                                    },
                                );
                                let response = response.unwrap();
                                ctx.memory_mut(|m| {
                                    m.surrender_focus(response.id);
                                    if state == "focus" {
                                        m.request_focus(response.id);
                                    }
                                });
                                let mut events = Vec::new();
                                if state == "hover" || state == "press" {
                                    events.push(egui::Event::PointerMoved(response.rect.center()));
                                }
                                if state == "press" {
                                    events.push(egui::Event::PointerButton {
                                        pos: response.rect.center(),
                                        button: egui::PointerButton::Primary,
                                        pressed: true,
                                        modifiers: Default::default(),
                                    });
                                }
                                let mut rect = Rect::NOTHING;
                                let output = ctx.run(input(events), |ctx| {
                                    let response = draw(ctx);
                                    match state {
                                        "hover" => assert!(response.hovered()),
                                        "press" => assert!(response.is_pointer_button_down_on()),
                                        "focus" => assert!(response.has_focus()),
                                        "disabled" => assert!(!response.enabled()),
                                        _ => {}
                                    }
                                    rect = response.rect;
                                });
                                assert!((ctx.pixels_per_point() - scale).abs() < 0.001);
                                let text = output
                                    .shapes
                                    .iter()
                                    .find_map(|s| match &s.shape {
                                        egui::Shape::Text(text) if text.galley.text() == label => {
                                            Some(text)
                                        }
                                        _ => None,
                                    })
                                    .expect("Missing painted action label");
                                let center = text.pos + text.galley.mesh_bounds.center().to_vec2();
                                let delta = (center - rect.center()) * scale;
                                assert!(
                                    delta.x.abs() <= 0.51 && delta.y.abs() <= 0.51,
                                    "Visible label {label} drifts {delta:?} pixels at scale {scale}, {state}, fallback={fallback}, {layout:?}"
                                );
                                assert!(
                                    rect.contains_rect(
                                        text.galley.mesh_bounds.translate(text.pos.to_vec2())
                                    ),
                                    "Action glyphs escape the button"
                                );
                                let tree = output.platform_output.accesskit_update.unwrap();
                                assert!(
                                    tree.nodes.iter().any(|(_, node)| node.role()
                                        == egui::accesskit::Role::Button
                                        && node.label() == Some(label))
                                );
                            }
                        }
                    }
                }
            }
        }
    }

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
