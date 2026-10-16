use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

pub const BG: Color32 = Color32::from_rgb(246, 246, 243);
pub const SIDEBAR: Color32 = Color32::from_rgb(237, 238, 234);
pub const SURFACE: Color32 = Color32::from_rgb(253, 253, 251);
pub const RAISED: Color32 = Color32::from_rgb(241, 242, 238);
pub const BORDER: Color32 = Color32::from_rgb(218, 221, 216);
pub const TEXT: Color32 = Color32::from_rgb(36, 42, 42);
pub const MUTED: Color32 = Color32::from_rgb(91, 100, 100);
pub const DIM: Color32 = Color32::from_rgb(106, 115, 115);
pub const ACCENT: Color32 = Color32::from_rgb(52, 88, 166);
pub const SELECTED: Color32 = Color32::from_rgb(226, 232, 243);
pub const INK: Color32 = Color32::WHITE;
pub const SUCCESS: Color32 = Color32::from_rgb(49, 120, 88);
pub const WARNING: Color32 = Color32::from_rgb(156, 105, 26);
pub const ERROR: Color32 = Color32::from_rgb(175, 62, 55);

pub fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Adwaita".into(),
        egui::FontData::from_static(include_bytes!("../assets/AdwaitaSans-Regular.ttf")).into(),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Adwaita".into());
    ctx.set_fonts(fonts);
    let art = image::load_from_memory(include_bytes!("../assets/mouse.png"))
        .expect("bundled mouse diagram")
        .into_rgba8();
    let texture = ctx.load_texture(
        "mouse-diagram",
        egui::ColorImage::from_rgba_unmultiplied(
            [art.width() as usize, art.height() as usize],
            &art,
        ),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mouse-diagram"), texture));
    let mut v = egui::Visuals::light();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SURFACE;
    v.faint_bg_color = SURFACE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = 8.into();
    v.selection.bg_fill = SELECTED;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.handle_shape = egui::style::HandleShape::Circle;
    v.slider_trailing_fill = true;
    v.hyperlink_color = ACCENT;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = 5.into();
        w.fg_stroke = Stroke::new(1.4, TEXT);
        w.bg_stroke = Stroke::new(1.0, BORDER);
    }
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.hovered.bg_fill = Color32::from_rgb(230, 233, 230);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(230, 233, 230);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, DIM);
    v.widgets.active.bg_fill = SELECTED;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.open.bg_fill = RAISED;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 10.0);
        s.spacing.button_padding = vec2(12.0, 7.0);
        s.spacing.interact_size = vec2(34.0, 32.0);
        s.spacing.slider_width = 190.0;
        s.spacing.combo_width = 140.0;
        s.animation_time = 0.15;
        for (style, size) in [
            (egui::TextStyle::Body, 13.0),
            (egui::TextStyle::Button, 13.0),
            (egui::TextStyle::Small, 12.0),
            (egui::TextStyle::Heading, 24.0),
        ] {
            s.text_styles.insert(style, FontId::proportional(size));
        }
    });
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(18)
}
pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(egui::RichText::new(text.into()).size(12.0).color(MUTED));
}
pub fn title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(egui::RichText::new(title).size(25.0).strong());
    if !subtitle.is_empty() {
        muted(ui, subtitle);
    }
    ui.add_space(10.0);
}
pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).strong().color(INK))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(5)
}
pub fn secondary(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .fill(SURFACE)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(5)
}
pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(egui::RichText::new(text).size(11.0).color(color));
}
pub fn separator(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, BORDER));
}

#[derive(Clone, Copy)]
pub enum Icon {
    Settings,
    Mouse,
    Grid,
    Sliders,
    Sun,
    Shield,
    Check,
    Battery,
    Radio,
    Link,
}
pub fn icon(p: &egui::Painter, r: Rect, which: Icon, color: Color32) {
    let xy = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let st = Stroke::new(1.6, color);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| xy(x, y)).collect(),
            st,
        ));
    };
    match which {
        Icon::Settings => {
            p.circle_stroke(r.center(), r.width() * 0.27, st);
            p.circle_stroke(r.center(), r.width() * 0.08, st);
            for n in 0..8 {
                let a = n as f32 * std::f32::consts::TAU / 8.0;
                let v = egui::vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        r.center() + v * r.width() * 0.29,
                        r.center() + v * r.width() * 0.42,
                    ],
                    st,
                );
            }
        }
        Icon::Mouse => {
            p.rect_stroke(
                Rect::from_min_max(xy(0.25, 0.07), xy(0.75, 0.93)),
                7,
                st,
                StrokeKind::Inside,
            );
            line(&[(0.5, 0.2), (0.5, 0.4)]);
        }
        Icon::Grid => {
            for x in [0.1, 0.57] {
                for y in [0.1, 0.57] {
                    p.rect_stroke(
                        Rect::from_min_max(xy(x, y), xy(x + 0.33, y + 0.33)),
                        2,
                        st,
                        StrokeKind::Inside,
                    );
                }
            }
        }
        Icon::Sliders => {
            for (y, x) in [(0.25, 0.35), (0.5, 0.65), (0.75, 0.45)] {
                line(&[(0.08, y), (0.92, y)]);
                p.circle_filled(xy(x, y), 2.5, color);
            }
        }
        Icon::Sun => {
            p.circle_stroke(r.center(), r.width() * 0.22, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let v = vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        r.center() + v * r.width() * 0.34,
                        r.center() + v * r.width() * 0.45,
                    ],
                    st,
                );
            }
        }
        Icon::Shield => line(&[
            (0.5, 0.05),
            (0.86, 0.2),
            (0.81, 0.65),
            (0.5, 0.93),
            (0.19, 0.65),
            (0.14, 0.2),
            (0.5, 0.05),
        ]),
        Icon::Check => line(&[(0.15, 0.5), (0.4, 0.75), (0.85, 0.25)]),
        Icon::Battery => {
            p.rect_stroke(
                Rect::from_min_max(xy(0.08, 0.25), xy(0.83, 0.75)),
                3,
                st,
                StrokeKind::Inside,
            );
            line(&[(0.93, 0.43), (0.93, 0.57)]);
            p.rect_filled(Rect::from_min_max(xy(0.19, 0.36), xy(0.55, 0.64)), 1, color);
        }
        Icon::Radio => {
            for radius in [0.24, 0.43] {
                let pts = (0..16)
                    .map(|i| {
                        let a = 3.7 + i as f32 * 2.0 / 15.0;
                        xy(0.5, 0.8) + vec2(a.cos(), a.sin()) * r.width() * radius
                    })
                    .collect();
                p.add(egui::Shape::line(pts, st));
            }
            p.circle_filled(xy(0.5, 0.8), 2.0, color);
        }
        Icon::Link => line(&[
            (0.2, 0.15),
            (0.2, 0.5),
            (0.5, 0.5),
            (0.5, 0.85),
            (0.5, 0.5),
            (0.8, 0.5),
            (0.8, 0.15),
        ]),
    }
}

fn activate(ui: &Ui, r: &Response) -> bool {
    r.clicked()
        || r.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space))
}
pub fn nav(ui: &mut Ui, name: &str, which: Icon, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    let fill = if selected {
        SELECTED
    } else if response.hovered() {
        SURFACE
    } else {
        SIDEBAR
    };
    ui.painter().rect_filled(rect, 5, fill);
    icon(
        ui.painter(),
        Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(17.0, 17.0)),
        which,
        if selected { ACCENT } else { MUTED },
    );
    ui.painter().text(
        pos2(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0),
        if selected { ACCENT } else { TEXT },
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 5, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    }
    let clicked = activate(ui, &response);
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(32.0, 18.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    if activate(ui, &response) && ui.is_enabled() {
        *value = !*value;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *value);
    ui.painter().rect_filled(
        rect,
        9,
        if *value {
            ACCENT
        } else {
            Color32::from_rgb(174, 181, 179)
        },
    );
    let center = pos2(
        egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t),
        rect.center().y,
    );
    ui.painter().circle_filled(center, 6.0, INK);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            13,
            Stroke::new(1.0, ACCENT),
            StrokeKind::Inside,
        );
    }
    response.on_hover_text(label)
}
pub fn tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(if selected { ACCENT } else { MUTED }))
            .fill(if selected {
                SELECTED
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, 32.0)),
    )
}
pub fn section_tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).color(if selected { TEXT } else { MUTED }))
            .frame(false)
            .min_size(vec2(0.0, 36.0)),
    );
    if selected {
        let r = response.rect;
        ui.painter()
            .hline(r.x_range(), r.bottom(), Stroke::new(2.0, ACCENT));
    }
    response
}
pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str, sub: &str) {
    ui.vertical(|ui| {
        muted(ui, label);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.label(egui::RichText::new(value).size(32.0));
            ui.label(egui::RichText::new(unit).size(12.0).color(MUTED));
        });
        ui.label(egui::RichText::new(sub).size(11.0).color(MUTED));
    });
}

pub fn status(ui: &mut Ui, text: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.0, color);
        ui.label(egui::RichText::new(text).size(11.0).color(MUTED));
    });
}

/// Original SVG artwork with shared coordinates for LEDs, callouts and hit testing.
/// Rendering a preview never sends a command to the device.
pub fn mouse(
    p: &egui::Painter,
    rect: Rect,
    colors: &[Color32],
    hotspots: bool,
    selected: Option<usize>,
) {
    let scale = (rect.width() / 300.0).min(rect.height() / 400.0);
    let art_rect = Rect::from_center_size(rect.center(), vec2(300.0, 400.0) * scale);
    let point = |x: f32, y: f32| art_rect.min + vec2(x, y) * scale;
    if let Some(texture) = p
        .ctx()
        .data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new("mouse-diagram")))
    {
        p.image(
            texture.id(),
            art_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    for i in 0..8 {
        let c = colors.get(i).copied().unwrap_or(ACCENT);
        let points = (0..=12)
            .map(|j| {
                let t = (i as f32 + 0.04 + 0.92 * j as f32 / 12.0) / 8.0;
                let at = light_point(t);
                point(at.x, at.y)
            })
            .collect::<Vec<_>>();
        p.add(egui::Shape::line(
            points.clone(),
            Stroke::new(5.5 * scale, c.gamma_multiply(0.16)),
        ));
        p.add(egui::Shape::line(points, Stroke::new(2.5 * scale, c)));
    }
    if hotspots {
        for (i, (x, y)) in button_points().iter().enumerate() {
            let at = point(*x, *y);
            let (ax, ay) = button_anchors()[i];
            let anchor = point(ax, ay);
            let active = selected == Some(i);
            let color = if active { ACCENT } else { DIM };
            if anchor.distance(at) > 12.0 {
                let edge = at + (anchor - at).normalized() * 10.0;
                p.line_segment(
                    [anchor, edge],
                    Stroke::new(2.5, Color32::from_white_alpha(160)),
                );
                p.line_segment([anchor, edge], Stroke::new(1.0, color));
                p.circle_filled(anchor, 2.3, color);
            }
            p.circle_filled(at, 10.0, if active { ACCENT } else { SURFACE });
            p.circle_stroke(
                at,
                10.0,
                Stroke::new(1.0, if active { ACCENT } else { BORDER }),
            );
            p.text(
                at,
                Align2::CENTER_CENTER,
                format!("{}", i + 1),
                FontId::proportional(10.0),
                if active { INK } else { TEXT },
            );
        }
    }
}

/// Matches the two cubic curves of the light channel in assets/mouse.svg.
fn light_point(t: f32) -> Pos2 {
    let (t, points) = if t <= 0.6 {
        (
            t / 0.6,
            [
                pos2(68.0, 232.0),
                pos2(73.0, 290.0),
                pos2(89.0, 294.0),
                pos2(145.0, 296.0),
            ],
        )
    } else {
        (
            (t - 0.6) / 0.4,
            [
                pos2(145.0, 296.0),
                pos2(181.0, 297.0),
                pos2(184.0, 288.0),
                pos2(196.0, 238.0),
            ],
        )
    };
    let u = 1.0 - t;
    let p = points[0].to_vec2() * u.powi(3)
        + points[1].to_vec2() * (3.0 * u * u * t)
        + points[2].to_vec2() * (3.0 * u * t * t)
        + points[3].to_vec2() * t.powi(3);
    (p + vec2(30.0, 10.0)).to_pos2()
}

pub fn button_points() -> [(f32, f32); 11] {
    [
        (125.0, 80.0),
        (203.0, 80.0),
        (280.0, 90.0),
        (20.0, 265.0),
        (20.0, 315.0),
        (20.0, 215.0),
        (20.0, 105.0),
        (280.0, 160.0),
        (280.0, 220.0),
        (20.0, 50.0),
        (20.0, 160.0),
    ]
}
fn button_anchors() -> [(f32, f32); 11] {
    [
        (125.0, 80.0),
        (203.0, 80.0),
        (160.0, 113.0),
        (85.0, 232.0),
        (83.0, 265.0),
        (88.0, 198.0),
        (153.0, 118.0),
        (168.0, 118.0),
        (162.0, 194.0),
        (105.0, 82.0),
        (96.0, 149.0),
    ]
}
pub fn mouse_hit(rect: Rect, pos: Pos2) -> Option<usize> {
    let scale = (rect.width() / 300.0).min(rect.height() / 400.0);
    let origin = rect.center() - vec2(150.0, 200.0) * scale;
    button_points()
        .iter()
        .enumerate()
        .find(|(_, (x, y))| (origin + vec2(*x, *y) * scale).distance(pos) <= 13.0)
        .map(|(i, _)| i)
}
pub fn colors(color: [u8; 3]) -> [Color32; 8] {
    [Color32::from_rgb(color[0], color[1], color[2]); 8]
}

/// Full-width card with a predictable inset, including content that would otherwise shrink.
pub fn panel<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let width = (ui.available_width() - 38.0).max(0.0);
    card().show(ui, |ui| {
        ui.set_min_width(width);
        content(ui)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callouts_hit_the_same_button_at_each_preview_size() {
        for size in [vec2(160.0, 300.0), vec2(260.0, 300.0), vec2(400.0, 600.0)] {
            let rect = Rect::from_min_size(pos2(37.0, 51.0), size);
            let scale = (size.x / 300.0).min(size.y / 400.0);
            let origin = rect.center() - vec2(150.0, 200.0) * scale;
            for (i, (x, y)) in button_points().iter().enumerate() {
                let position = origin + vec2(*x, *y) * scale;
                assert_eq!(mouse_hit(rect, position), Some(i));
                assert!(rect.contains(position));
            }
            assert_eq!(
                mouse_hit(rect, rect.right_bottom() + vec2(30.0, 30.0)),
                None
            );
        }
    }
}
