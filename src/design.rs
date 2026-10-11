use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

pub const BG: Color32 = Color32::from_rgb(23, 25, 29);
pub const SIDEBAR: Color32 = Color32::from_rgb(16, 18, 21);
pub const SURFACE: Color32 = Color32::from_rgb(31, 34, 39);
pub const RAISED: Color32 = Color32::from_rgb(40, 44, 50);
pub const BORDER: Color32 = Color32::from_rgb(53, 58, 65);
pub const TEXT: Color32 = Color32::from_rgb(240, 243, 246);
pub const MUTED: Color32 = Color32::from_rgb(164, 172, 184);
pub const DIM: Color32 = Color32::from_rgb(119, 129, 143);
pub const ACCENT: Color32 = Color32::from_rgb(205, 244, 130);
pub const INK: Color32 = Color32::from_rgb(25, 36, 17);
pub const WARNING: Color32 = Color32::from_rgb(242, 195, 110);
pub const ERROR: Color32 = Color32::from_rgb(252, 149, 141);

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
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SIDEBAR;
    v.faint_bg_color = SURFACE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = 16.into();
    v.selection.bg_fill = Color32::from_rgb(62, 79, 45);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = 8.into();
        w.fg_stroke = Stroke::new(1.4, TEXT);
        w.bg_stroke = Stroke::new(1.0, BORDER);
    }
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.hovered.bg_fill = Color32::from_rgb(52, 57, 64);
    v.widgets.hovered.weak_bg_fill = RAISED;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, DIM);
    v.widgets.active.bg_fill = Color32::from_rgb(58, 70, 44);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.open.bg_fill = RAISED;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(12.0, 12.0);
        s.spacing.button_padding = vec2(14.0, 9.0);
        s.spacing.interact_size = vec2(38.0, 36.0);
        s.spacing.slider_width = 190.0;
        s.spacing.combo_width = 140.0;
        s.animation_time = 0.15;
        for (style, size) in [
            (egui::TextStyle::Body, 14.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Small, 12.0),
            (egui::TextStyle::Heading, 26.0),
        ] {
            s.text_styles.insert(style, FontId::proportional(size));
        }
    });
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(16)
        .inner_margin(20)
}
pub fn eyebrow(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(10.5)
            .strong()
            .color(MUTED)
            .extra_letter_spacing(1.4),
    );
}
pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(egui::RichText::new(text.into()).size(13.0).color(MUTED));
}
pub fn title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(egui::RichText::new(title).size(28.0).strong());
    muted(ui, subtitle);
    ui.add_space(12.0);
}
pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).strong().color(INK))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .min_size(vec2(0.0, 38.0))
        .corner_radius(8)
}
pub fn secondary(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .fill(RAISED)
        .min_size(vec2(0.0, 36.0))
        .corner_radius(8)
}
pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).strong().color(color));
        });
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
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    let fill = if selected {
        Color32::from_rgb(38, 44, 33)
    } else if response.hovered() {
        SURFACE
    } else {
        SIDEBAR
    };
    ui.painter().rect_filled(rect, 9, fill);
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.left_top() + vec2(0.0, 13.0), vec2(3.0, 20.0)),
            2,
            ACCENT,
        );
    }
    icon(
        ui.painter(),
        Rect::from_center_size(pos2(rect.left() + 25.0, rect.center().y), vec2(19.0, 19.0)),
        which,
        if selected { ACCENT } else { MUTED },
    );
    ui.painter().text(
        pos2(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(14.0),
        if selected { TEXT } else { MUTED },
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 9, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    }
    let clicked = activate(ui, &response);
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
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
        11,
        if *value {
            ACCENT
        } else {
            Color32::from_rgb(71, 77, 86)
        },
    );
    let center = pos2(
        egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t),
        rect.center().y,
    );
    ui.painter()
        .circle_filled(center, 8.0, if *value { INK } else { TEXT });
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
                Color32::from_rgb(45, 53, 37)
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, 34.0)),
    )
}
pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str, sub: &str, which: Icon) {
    card().inner_margin(18).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            eyebrow(ui, label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icon(ui.painter(), r, which, DIM);
            });
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(value).size(28.0).strong());
            ui.label(egui::RichText::new(unit).size(12.0).color(MUTED));
        });
        ui.label(egui::RichText::new(sub).size(11.0).color(MUTED));
    });
}

/// A code-native top view; visual previews never write to the device.
pub fn mouse(
    p: &egui::Painter,
    rect: Rect,
    colors: &[Color32],
    hotspots: bool,
    selected: Option<usize>,
) {
    let scale = (rect.width() / 205.0).min(rect.height() / 320.0);
    let center = rect.center();
    let rot = if hotspots { 0.0f32 } else { -0.22 };
    let point = |x: f32, y: f32| {
        let v = vec2(x, y) * scale;
        center
            + vec2(
                v.x * rot.cos() - v.y * rot.sin(),
                v.x * rot.sin() + v.y * rot.cos(),
            )
    };
    let path = |coords: &[(f32, f32)]| coords.iter().map(|&(x, y)| point(x, y)).collect::<Vec<_>>();
    // Soft ground shadow and concentric studio backdrop.
    for (radius, alpha) in [(118.0, 5), (96.0, 6), (76.0, 7)] {
        p.circle_filled(
            center + vec2(0.0, 35.0) * scale,
            radius * scale,
            Color32::from_black_alpha(alpha),
        );
    }
    let shell = [
        (-48.0, -139.0),
        (-18.0, -149.0),
        (18.0, -150.0),
        (48.0, -136.0),
        (65.0, -112.0),
        (71.0, -70.0),
        (72.0, -15.0),
        (76.0, 34.0),
        (72.0, 87.0),
        (55.0, 124.0),
        (26.0, 145.0),
        (-14.0, 151.0),
        (-49.0, 139.0),
        (-72.0, 114.0),
        (-86.0, 75.0),
        (-90.0, 25.0),
        (-78.0, -16.0),
        (-65.0, -67.0),
        (-62.0, -111.0),
    ];
    let shadow = path(&shell)
        .into_iter()
        .map(|pt| pt + vec2(9.0, 14.0) * scale)
        .collect();
    p.add(egui::Shape::convex_polygon(
        shadow,
        Color32::from_black_alpha(60),
        Stroke::NONE,
    ));
    p.add(egui::Shape::convex_polygon(
        path(&shell),
        Color32::from_rgb(14, 16, 19),
        Stroke::new(1.5 * scale, Color32::from_rgb(64, 69, 76)),
    ));
    let upper = [
        (-45.0, -134.0),
        (-16.0, -144.0),
        (17.0, -145.0),
        (44.0, -133.0),
        (59.0, -108.0),
        (63.0, -63.0),
        (58.0, -13.0),
        (42.0, 38.0),
        (4.0, 62.0),
        (-39.0, 43.0),
        (-55.0, 2.0),
        (-56.0, -61.0),
        (-56.0, -105.0),
    ];
    p.add(egui::Shape::convex_polygon(
        path(&upper),
        Color32::from_rgb(39, 43, 49),
        Stroke::new(0.8 * scale, Color32::from_rgb(75, 79, 86)),
    ));
    let palm = [
        (-53.0, 4.0),
        (-37.0, 35.0),
        (3.0, 51.0),
        (43.0, 26.0),
        (61.0, -8.0),
        (65.0, 34.0),
        (61.0, 81.0),
        (45.0, 113.0),
        (18.0, 131.0),
        (-16.0, 135.0),
        (-47.0, 122.0),
        (-67.0, 93.0),
        (-76.0, 48.0),
    ];
    p.add(egui::Shape::convex_polygon(
        path(&palm),
        Color32::from_rgb(30, 33, 38),
        Stroke::new(0.8 * scale, Color32::from_rgb(56, 60, 66)),
    ));
    // Split click plates, thumb rest, wheel housing and tactile details.
    p.add(egui::Shape::line(
        path(&[(-3.0, -143.0), (-5.0, -61.0), (0.0, -19.0)]),
        Stroke::new(3.0 * scale, Color32::from_rgb(12, 14, 17)),
    ));
    p.add(egui::Shape::line(
        path(&[
            (-54.0, -10.0),
            (-32.0, -2.0),
            (-5.0, -13.0),
            (14.0, -6.0),
            (58.0, -17.0),
        ]),
        Stroke::new(2.0 * scale, Color32::from_rgb(11, 13, 16)),
    ));
    for i in 0..9 {
        let y = 32.0 + i as f32 * 7.0;
        p.add(egui::Shape::line(
            path(&[(-84.0, y), (-73.0, y - 8.0)]),
            Stroke::new(1.0 * scale, Color32::from_rgb(53, 56, 62)),
        ));
    }
    let wheel = path(&[
        (-12.0, -115.0),
        (9.0, -114.0),
        (10.0, -61.0),
        (-10.0, -60.0),
    ]);
    p.add(egui::Shape::convex_polygon(
        wheel,
        Color32::from_rgb(12, 14, 17),
        Stroke::new(1.0 * scale, Color32::from_rgb(88, 92, 99)),
    ));
    for i in 0..10 {
        let y = -108.0 + i as f32 * 4.4;
        p.add(egui::Shape::line(
            path(&[(-7.0, y), (5.0, y)]),
            Stroke::new(2.2 * scale, Color32::from_rgb(97, 102, 109)),
        ));
    }
    for coords in [
        vec![(-12.0, -47.0), (8.0, -46.0), (8.0, -30.0), (-11.0, -30.0)],
        vec![
            (-64.0, -41.0),
            (-54.0, -42.0),
            (-53.0, -20.0),
            (-68.0, -15.0),
        ],
        vec![(-73.0, -5.0), (-59.0, -9.0), (-61.0, 10.0), (-79.0, 19.0)],
        vec![
            (-49.0, -109.0),
            (-37.0, -112.0),
            (-37.0, -92.0),
            (-50.0, -88.0),
        ],
        vec![
            (-51.0, -82.0),
            (-39.0, -85.0),
            (-38.0, -66.0),
            (-52.0, -62.0),
        ],
    ] {
        p.add(egui::Shape::convex_polygon(
            path(&coords),
            Color32::from_rgb(24, 27, 31),
            Stroke::new(0.8 * scale, Color32::from_rgb(91, 95, 102)),
        ));
    }
    // Eight separate segments follow the curved LIGHTSYNC band.
    for i in 0..8 {
        let x1 = -61.0 + i as f32 * 15.0;
        let x2 = x1 + 13.0;
        let y = |x: f32| 109.0 - (x / 65.0).powi(2) * 28.0;
        let c = colors.get(i).copied().unwrap_or(ACCENT);
        let seg = path(&[
            (x1, y(x1)),
            ((x1 + x2) * 0.5, y((x1 + x2) * 0.5)),
            (x2, y(x2)),
        ]);
        p.add(egui::Shape::line(
            seg.clone(),
            Stroke::new(11.0 * scale, c.gamma_multiply(0.09)),
        ));
        p.add(egui::Shape::line(
            seg.clone(),
            Stroke::new(6.0 * scale, c.gamma_multiply(0.2)),
        ));
        p.add(egui::Shape::line(seg, Stroke::new(2.5 * scale, c)));
    }
    p.text(
        point(0.0, 69.0),
        Align2::CENTER_CENTER,
        "G",
        FontId::proportional(29.0 * scale),
        Color32::from_rgb(121, 127, 134),
    );
    if hotspots {
        for (i, (x, y)) in button_points().iter().enumerate() {
            let at = point(*x, *y);
            let active = selected == Some(i);
            p.circle_filled(
                at,
                10.0 * scale,
                if active {
                    ACCENT
                } else {
                    Color32::from_rgb(62, 68, 77)
                },
            );
            p.circle_stroke(
                at,
                10.0 * scale,
                Stroke::new(
                    1.0,
                    if active {
                        ACCENT
                    } else {
                        Color32::from_rgb(115, 125, 137)
                    },
                ),
            );
            p.text(
                at,
                Align2::CENTER_CENTER,
                format!("{}", i + 1),
                FontId::proportional(10.0 * scale),
                if active { INK } else { TEXT },
            );
        }
    }
}
pub fn button_points() -> [(f32, f32); 11] {
    [
        (-28.0, -75.0),
        (34.0, -82.0),
        (-1.0, -83.0),
        (-71.0, 5.0),
        (-83.0, 45.0),
        (-61.0, -32.0),
        (-22.0, -100.0),
        (19.0, -102.0),
        (-1.0, -37.0),
        (-45.0, -103.0),
        (-47.0, -73.0),
    ]
}
pub fn mouse_hit(rect: Rect, pos: Pos2) -> Option<usize> {
    let scale = (rect.width() / 205.0).min(rect.height() / 320.0);
    button_points()
        .iter()
        .enumerate()
        .find(|(_, p)| pos.distance(rect.center() + vec2(p.0, p.1) * scale) < 15.0 * scale)
        .map(|(i, _)| i)
}
pub fn colors(color: [u8; 3]) -> [Color32; 8] {
    [Color32::from_rgb(color[0], color[1], color[2]); 8]
}

/// Full-width card with a predictable inset, including content that would otherwise shrink.
pub fn panel<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let width = (ui.available_width() - 42.0).max(0.0);
    card().show(ui, |ui| {
        ui.set_min_width(width);
        content(ui)
    })
}
