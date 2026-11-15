//! Finite, event-triggered illustrations shared by Focus and browser connection.
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::time::{Duration, Instant};

pub struct Motion {
    key: String,
    started: Instant,
    enabled: bool,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            key: String::new(),
            started: Instant::now(),
            enabled: system_motion_enabled(),
        }
    }
}
impl Motion {
    pub fn replay(&mut self, ctx: &egui::Context) {
        self.started = Instant::now();
        ctx.request_repaint();
    }
    pub fn progress(&mut self, ui: &egui::Ui, key: String, capture: bool) -> f32 {
        if key != self.key {
            self.key = key;
            self.started = Instant::now();
        }
        if capture || !self.enabled {
            return 1.0;
        }
        let t = (self.started.elapsed().as_secs_f32() / 1.25).min(1.0);
        if t < 1.0 {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
        t
    }
}

fn system_motion_enabled() -> bool {
    #[cfg(windows)]
    {
        let mut enabled = 1i32;
        // SAFETY: SPI writes one BOOL to a valid stack allocation; it changes no settings.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut enabled as *mut i32).cast(),
                0,
            );
        }
        enabled != 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

pub fn scene(
    ui: &mut egui::Ui,
    progress: f32,
    connection: bool,
    connected: bool,
    labels: &[String],
    caption: &str,
    replay_label: &str,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 120.0), Sense::hover());
    let p = ui.painter_at(rect);
    let dark = ui.visuals().dark_mode;
    let ink = if dark {
        Color32::from_rgb(167, 210, 179)
    } else {
        Color32::from_rgb(36, 86, 62)
    };
    let surface = if dark {
        Color32::from_rgb(31, 47, 38)
    } else {
        Color32::from_rgb(232, 238, 219)
    };
    let gold = Color32::from_rgb(180, 148, 90);
    p.rect_filled(rect, 14.0, surface);
    let center = rect.center() + Vec2::new(0.0, -5.0);
    let settle = 1.0 - (-7.0 * progress).exp() * (13.0 * progress).cos();
    if connection {
        let left = center + Vec2::new(-130.0, -6.0);
        let right = center + Vec2::new(130.0, -6.0);
        for i in 0..18 {
            let x = left.x + 40.0 + i as f32 * 10.0;
            p.circle_filled(Pos2::new(x, center.y - 6.0), 1.0, ink.gamma_multiply(0.3));
        }
        // Packets travel both ways only during a state transition, never when settled.
        if progress < 1.0 {
            for i in 0..4 {
                let t = (progress * 1.5 + i as f32 / 4.0).fract();
                let x = left.x + 38.0 + t * 184.0;
                p.circle_filled(Pos2::new(x, center.y - 13.0), 3.0, ink);
                p.circle_filled(Pos2::new(left.x + right.x - x, center.y + 1.0), 2.5, gold);
            }
        }
        let monitor = Rect::from_center_size(
            left + Vec2::new(-20.0 * (1.0 - settle), 0.0),
            Vec2::new(64.0, 42.0),
        );
        p.rect_filled(monitor, 6.0, ui.visuals().extreme_bg_color);
        p.rect_stroke(
            monitor,
            6.0,
            Stroke::new(1.5, ink),
            egui::StrokeKind::Inside,
        );
        p.line_segment(
            [
                monitor.center_bottom(),
                monitor.center_bottom() + Vec2::new(0.0, 9.0),
            ],
            Stroke::new(2.0, ink),
        );
        p.line_segment(
            [left + Vec2::new(-14.0, 30.0), left + Vec2::new(14.0, 30.0)],
            Stroke::new(2.0, ink),
        );
        let browser = Rect::from_center_size(
            right + Vec2::new(20.0 * (1.0 - settle), 0.0),
            Vec2::new(64.0, 48.0),
        );
        p.rect_filled(browser, 7.0, ui.visuals().extreme_bg_color);
        p.rect_stroke(
            browser,
            7.0,
            Stroke::new(1.5, ink),
            egui::StrokeKind::Inside,
        );
        p.line_segment(
            [
                browser.left_top() + Vec2::new(0.0, 12.0),
                browser.right_top() + Vec2::new(0.0, 12.0),
            ],
            Stroke::new(1.0, ink.gamma_multiply(0.4)),
        );
        for i in 0..3 {
            p.circle_filled(
                browser.left_top() + Vec2::new(8.0 + i as f32 * 6.0, 6.0),
                1.3,
                gold,
            );
        }
        p.circle_stroke(
            browser.center() + Vec2::new(0.0, 5.0),
            9.0,
            Stroke::new(1.5, ink),
        );
        let mark = if connected { "✓" } else { "↔" };
        p.circle_filled(center + Vec2::new(0.0, -6.0), 16.0, surface);
        p.text(
            center + Vec2::new(0.0, -6.0),
            Align2::CENTER_CENTER,
            mark,
            FontId::proportional(20.0),
            if connected { ink } else { gold },
        );
    } else {
        for ring in 0..2 {
            let radius = 34.0 + ring as f32 * 12.0 + (1.0 - progress) * 12.0;
            p.circle_stroke(center, radius, Stroke::new(1.0, ink.gamma_multiply(0.12)));
        }
        let shield_center = center + Vec2::new(0.0, (1.0 - settle) * 18.0);
        let points = [
            (-23.0, -27.0),
            (0.0, -36.0),
            (23.0, -27.0),
            (21.0, 7.0),
            (0.0, 30.0),
            (-21.0, 7.0),
        ]
        .into_iter()
        .map(|(x, y)| shield_center + Vec2::new(x, y))
        .collect();
        p.add(egui::Shape::convex_polygon(
            points,
            ink.gamma_multiply(0.13),
            Stroke::new(1.6, ink),
        ));
        p.add(egui::Shape::line(
            vec![
                shield_center + Vec2::new(-10.0, -3.0),
                shield_center + Vec2::new(-2.0, 5.0),
                shield_center + Vec2::new(12.0, -13.0),
            ],
            Stroke::new(2.7, ink),
        ));
        for (i, label) in labels.iter().take(2).enumerate() {
            let label = if label.len() > 20 {
                format!("{}…", &label[..17])
            } else {
                label.clone()
            };
            let direction = if i == 0 { -1.0 } else { 1.0 };
            let x = direction * (145.0 - (progress * std::f32::consts::PI).sin().max(0.0) * 45.0);
            let site = Rect::from_center_size(
                center + Vec2::new(x, if i == 0 { -14.0 } else { 10.0 }),
                Vec2::new(150.0, 31.0),
            );
            p.rect_filled(site, 8.0, ui.visuals().extreme_bg_color);
            p.text(
                site.center() + Vec2::new(-6.0, 0.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                ink,
            );
            p.text(
                site.right_center() + Vec2::new(-12.0, 0.0),
                Align2::CENTER_CENTER,
                "×",
                FontId::proportional(13.0),
                gold,
            );
        }
        if progress < 1.0 {
            let burst = (progress * 2.0 - 0.6).clamp(0.0, 1.0);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                p.circle_filled(
                    center + Vec2::angled(a) * (30.0 + burst * 48.0),
                    2.0,
                    gold.gamma_multiply(1.0 - burst),
                );
            }
        }
    }
    p.text(
        rect.center_bottom() + Vec2::new(0.0, -12.0),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(10.0),
        ink.gamma_multiply(0.8),
    );
    let replay = ui
        .put(
            Rect::from_min_size(
                rect.right_bottom() - Vec2::new(30.0, 28.0),
                Vec2::splat(24.0),
            ),
            egui::Button::new("↻").frame(false),
        )
        .on_hover_text(replay_label)
        .clicked();
    ui.add_space(10.0);
    replay
}
