use eframe::egui::{self, Align2, Color32, FontId, Response, Sense, Stroke, StrokeKind};

pub const BUTTON_HEIGHT: f32 = 42.0;

#[derive(Clone, Copy)]
pub enum Icon {
    NewGame,
    Undo,
    Help,
    Teach,
    Expand,
    Collapse,
    Settings,
    Close,
}

pub fn button(
    ui: &mut egui::Ui,
    label: &str,
    icon: Icon,
    show_label: bool,
    selected: Option<bool>,
    enabled: bool,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let ink = Color32::from_white_alpha(if ui.is_enabled() { 240 } else { 85 });
        let galley = show_label.then(|| {
            ui.painter()
                .layout_no_wrap(label.into(), FontId::proportional(14.0), ink)
        });
        let size = galley
            .as_ref()
            .map(|g| egui::vec2(g.size().x + 48.0, BUTTON_HEIGHT))
            .unwrap_or(egui::Vec2::splat(BUTTON_HEIGHT));
        let (rect, response) = ui.allocate_at_least(size, Sense::click());
        response.widget_info(|| match selected {
            Some(on) => {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label)
            }
            None => egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label),
        });
        if ui.is_rect_visible(rect) {
            let on = selected == Some(true);
            let hover = response.hovered() && ui.is_enabled();
            let fill = if response.is_pointer_button_down_on() {
                Color32::from_black_alpha(80)
            } else if hover {
                Color32::from_rgba_unmultiplied(90, 118, 109, 115)
            } else if on {
                Color32::from_rgba_unmultiplied(84, 111, 105, 100)
            } else {
                Color32::from_rgba_unmultiplied(25, 40, 39, 112)
            };
            let edge = if response.has_focus() {
                200
            } else if on || hover {
                115
            } else if ui.is_enabled() {
                65
            } else {
                28
            };
            ui.painter().rect(
                rect,
                rect.height() / 2.0,
                fill,
                Stroke::new(1.0, Color32::from_white_alpha(edge)),
                StrokeKind::Inside,
            );
            let center = if let Some(galley) = galley {
                let icon_center = egui::pos2(rect.left() + 21.0, rect.center().y);
                ui.painter().galley(
                    egui::pos2(rect.left() + 35.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    ink,
                );
                icon_center
            } else {
                rect.center()
            };
            paint_icon(ui.painter(), center, icon, ink);
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(38, 63, 54, 240))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(55)))
        .corner_radius(26)
        .inner_margin(24)
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(75),
        })
}

pub fn settings_style(ui: &mut egui::Ui) {
    ui.spacing_mut().button_padding = egui::vec2(16.0, 10.0);
    ui.spacing_mut().interact_size.y = BUTTON_HEIGHT;
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 12.0);
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(Color32::from_white_alpha(240));
    visuals.window_fill = Color32::from_rgba_unmultiplied(38, 63, 54, 248);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_white_alpha(65));
    visuals.window_corner_radius = 18.into();
    visuals.menu_corner_radius = 18.into();
    visuals.selection.bg_fill = Color32::from_rgb(65, 104, 85);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = 18.into();
        widget.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(60));
        widget.fg_stroke.color = Color32::from_white_alpha(240);
    }
    visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(35);
    visuals.widgets.inactive.bg_fill = Color32::from_black_alpha(35);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(20);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 93, 78);
    visuals.widgets.active.bg_fill = Color32::from_rgb(48, 80, 65);
    visuals.widgets.open.bg_fill = Color32::from_rgb(48, 80, 65);
}

pub(crate) fn paint_icon(p: &egui::Painter, c: egui::Pos2, icon: Icon, ink: Color32) {
    let stroke = Stroke::new(1.5, ink);
    let at = |x, y| c + egui::vec2(x, y);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            stroke,
        ))
    };
    match icon {
        Icon::NewGame => {
            line(&[(-5.5, 0.0), (5.5, 0.0)]);
            line(&[(0.0, -5.5), (0.0, 5.5)]);
        }
        Icon::Close => {
            line(&[(-4.5, -4.5), (4.5, 4.5)]);
            line(&[(-4.5, 4.5), (4.5, -4.5)]);
        }
        Icon::Expand | Icon::Collapse => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    if matches!(icon, Icon::Expand) {
                        line(&[(x * 3.0, y * 7.0), (x * 7.0, y * 7.0), (x * 7.0, y * 3.0)]);
                    } else {
                        line(&[(x * 7.0, y * 3.0), (x * 3.0, y * 3.0), (x * 3.0, y * 7.0)]);
                    }
                }
            }
        }
        Icon::Help => {
            p.circle_stroke(c, 7.0, stroke);
            p.text(
                c + egui::vec2(0.0, 0.3),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(12.0),
                ink,
            );
        }
        Icon::Teach => {
            line(&[
                (-9.0, -3.0),
                (0.0, -7.0),
                (9.0, -3.0),
                (0.0, 1.0),
                (-9.0, -3.0),
            ]);
            line(&[
                (-5.0, -0.5),
                (-5.0, 4.0),
                (0.0, 6.0),
                (5.0, 4.0),
                (5.0, -0.5),
            ]);
            line(&[(9.0, -3.0), (9.0, 4.0)]);
        }
        Icon::Undo => {
            p.add(egui::Shape::CubicBezier(
                egui::epaint::CubicBezierShape::from_points_stroke(
                    [at(-6.0, -3.0), at(8.0, -10.0), at(12.0, 9.0), at(-2.0, 6.0)],
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ),
            ));
            line(&[(-2.0, -7.5), (-6.5, -3.0), (-2.0, 1.0)]);
        }
        Icon::Settings => {
            let points = (0..64)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 64.0;
                    let r = if matches!(i % 8, 2..=5) { 8.6 } else { 6.4 };
                    at(a.cos() * r, a.sin() * r)
                })
                .collect();
            p.add(egui::Shape::closed_line(points, stroke));
            p.circle_stroke(c, 2.7, stroke);
        }
    }
}
