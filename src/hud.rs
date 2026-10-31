//! Flight director HUD. The layout intentionally leaves the centre of the
//! screen clear: the black hole is the instrument.

use egui::{
    Align, Align2, Color32, FontFamily, FontId, Frame, Id, Layout, Margin, RichText, Rounding,
    Sense, Stroke, TextStyle, Vec2,
};

use crate::clock::{earth_date, format_span, EARTH_TIME_LAPSE};
use crate::physics::{format_duration, Relativity, Ship};

/// Alpha-blended colour for `const` contexts. egui stores premultiplied
/// colours, and `from_rgba_unmultiplied` is not const in egui 0.28.
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let a16 = a as u16;
    Color32::from_rgba_premultiplied(
        ((r as u16 * a16 + 127) / 255) as u8,
        ((g as u16 * a16 + 127) / 255) as u8,
        ((b as u16 * a16 + 127) / 255) as u8,
        a,
    )
}

const BG: Color32 = rgba(5, 9, 14, 224);
const BORDER: Color32 = rgba(110, 186, 205, 52);
const CYAN: Color32 = Color32::from_rgb(112, 225, 255);
const CYAN_DIM: Color32 = Color32::from_rgb(58, 137, 155);
const AMBER: Color32 = Color32::from_rgb(255, 181, 100);
const WHITE: Color32 = Color32::from_rgb(229, 241, 246);
const MUTED: Color32 = Color32::from_rgb(133, 154, 164);
const FAINT: Color32 = Color32::from_rgb(83, 101, 112);
const RED: Color32 = Color32::from_rgb(255, 87, 73);

pub struct HudInfo<'a> {
    pub ship: &'a Ship,
    pub target_view: [f32; 3],
    pub paused: bool,
    pub fps: f32,
    pub render_scale: f32,
    pub epoch_unix: f64,
    pub earth_elapsed: f64,
    pub journey_time: f64,
    pub detailed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Play,
    Restart,
    Quit,
}

pub fn configure_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "pixel".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/PixelifySans.ttf")),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .get_mut(&family)
            .unwrap()
            .insert(0, "pixel".into());
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(24.0, FontFamily::Proportional),
    );
    style.spacing.item_spacing = Vec2::new(7.0, 5.0);
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(MUTED);
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = BG;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_rounding = Rounding::same(2.0);
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.selection.bg_fill = CYAN_DIM;
    style.visuals = visuals;
    ctx.set_style(style);
}

pub fn draw(ctx: &egui::Context, info: &HudInfo<'_>) {
    let rel = info.ship.relativity();
    let screen = ctx.screen_rect();
    let margin = 22.0;
    let crosshair = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("crosshair"),
    ));
    let center = screen.center();
    for axis in [Vec2::X, Vec2::Y] {
        let endpoints = [center - axis * 6.0, center + axis * 6.0];
        crosshair.line_segment(endpoints, Stroke::new(3.0_f32, Color32::BLACK));
        crosshair.line_segment(endpoints, Stroke::new(1.0_f32, WHITE));
    }
    // A physically distant hole is small. Mark its actual bearing rather
    // than enlarging the shadow or replacing it with a screen-space sprite.
    let [x, y, z] = info.target_view;
    if z > 0.0 && !info.ship.lost {
        let tan_half = (58.0_f32.to_radians() * 0.5).tan();
        let scale = screen.height() * 0.5 / tan_half;
        let pos = screen.center() + Vec2::new(x / z, -y / z) * scale;
        if screen.shrink(30.0).contains(pos) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Background,
                Id::new("hole_bearing"),
            ));
            let radius = (screen.height() / (rel.radius_rs * tan_half)).clamp(12.0, 70.0);
            for side in [-1.0, 1.0] {
                let a = pos + Vec2::new(side * radius, -8.0);
                let b = pos + Vec2::new(side * radius, 8.0);
                painter.line_segment([a, b], Stroke::new(1.0_f32, CYAN_DIM));
            }
            painter.text(
                pos + Vec2::new(0.0, radius + 16.0),
                Align2::CENTER_TOP,
                format!("GARGANTUA  {:.1} Rs", rel.radius_rs),
                FontId::new(12.0, FontFamily::Monospace),
                CYAN,
            );
        }
    }

    title(ctx, info, rel, screen, margin);
    if info.detailed {
        time_panel(ctx, info, rel, screen, margin);
        light_panel(ctx, rel, screen, margin);
    } else {
        compact_clocks(ctx, info, rel);
    }
    journey_panel(ctx, info);
}

/// The title screen / pause menu. Returns the user's choice.
pub fn draw_menu(ctx: &egui::Context, can_resume: bool) -> MenuAction {
    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("menu_shade"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(104));
        });

    let mut action = MenuAction::None;
    egui::Area::new(Id::new("menu_panel"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::none()
                .fill(BG)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .inner_margin(Margin::symmetric(34.0, 28.0))
                .rounding(Rounding::same(3.0))
                .show(ui, |ui| {
                    ui.set_width(438.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("GARGANTUA")
                                .size(42.0)
                                .color(WHITE)
                                .family(FontFamily::Proportional),
                        );
                        ui.add_space(-4.0);
                        micro(ui, "A SCRIPTED DESCENT");
                    });

                    ui.add_space(12.0);
                    hairline(ui);

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("A four-minute journey into curved spacetime.")
                                .size(14.0)
                                .color(WHITE),
                        );
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(
                                "Gravity bends the starlight. Velocity slows your clock. \
                                 Earth keeps moving.",
                            )
                            .size(11.0)
                            .color(MUTED),
                        );
                    });

                    ui.add_space(16.0);
                    if menu_button(
                        ui,
                        if can_resume {
                            "RESUME JOURNEY"
                        } else {
                            "BEGIN JOURNEY"
                        },
                        true,
                    )
                    .clicked()
                    {
                        action = MenuAction::Play;
                    }
                    ui.add_space(7.0);
                    if menu_button(ui, "REPLAY FROM THE BEGINNING", false).clicked() {
                        action = MenuAction::Restart;
                    }
                    ui.add_space(7.0);
                    if menu_button(ui, "QUIT", false).clicked() {
                        action = MenuAction::Quit;
                    }

                    ui.add_space(16.0);
                    hairline(ui);
                    ui.add_space(2.0);
                    ui.vertical_centered(|ui| {
                        micro(ui, "DRAG OR ARROW KEYS TO LOOK AROUND");
                        micro(ui, "SPACE PAUSE  /  X RECENTER  /  R REPLAY");
                        micro(
                            ui,
                            "THE ROUTE IS SCRIPTED. THE MUSIC STARTS WITH THE JOURNEY.",
                        );
                    });

                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("M = 10⁶ M☉   ·   Rs = 2.95×10⁹ m   ·   SPIN 0")
                                .size(9.0)
                                .color(FAINT),
                        );
                    });
                });
        });
    action
}

fn menu_button(ui: &mut egui::Ui, text: &str, primary: bool) -> egui::Response {
    let (fill, stroke, color) = if primary {
        (
            CYAN,
            Stroke::new(1.0_f32, Color32::from_rgb(180, 245, 255)),
            Color32::from_rgb(4, 16, 20),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(14, 22, 30, 220),
            Stroke::new(1.0_f32, BORDER),
            WHITE,
        )
    };
    let button = egui::Button::new(
        RichText::new(text)
            .size(13.0)
            .color(color)
            .family(FontFamily::Monospace),
    )
    .fill(fill)
    .stroke(stroke)
    .rounding(Rounding::same(2.0));
    ui.add_sized(Vec2::new(ui.available_width(), 42.0), button)
}

fn panel_frame() -> Frame {
    Frame::none()
        .fill(BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(Margin::symmetric(15.0, 13.0))
        .rounding(Rounding::same(2.0))
}

fn micro(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(10.0)
            .color(MUTED)
            .family(FontFamily::Proportional),
    );
}

fn mono(ui: &mut egui::Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.label(
        RichText::new(text)
            .size(size)
            .color(color)
            .family(FontFamily::Monospace),
    );
}

fn hairline(ui: &mut egui::Ui) {
    ui.add_space(5.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        0.0,
        Color32::from_rgba_unmultiplied(120, 180, 200, 36),
    );
    ui.add_space(5.0);
}

fn title(
    ctx: &egui::Context,
    info: &HudInfo<'_>,
    rel: Relativity,
    _screen: egui::Rect,
    margin: f32,
) {
    egui::Area::new(Id::new("title"))
        .anchor(Align2::LEFT_TOP, Vec2::splat(margin))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(245.0);
                ui.label(RichText::new("GARGANTUA").size(26.0).color(WHITE));
                micro(ui, "A SCRIPTED DESCENT / FREE LOOK");
                ui.add_space(8.0);
                mono(
                    ui,
                    format!("{:.3} Rs FROM THE CENTRE", rel.radius_rs),
                    13.0,
                    CYAN,
                );
                mono(
                    ui,
                    format!("LOCAL SPEED {:.4} c", rel.local_speed_c),
                    11.0,
                    MUTED,
                );
                mono(
                    ui,
                    format!(
                        "{:.0} FPS / {:.0}% RENDER",
                        info.fps,
                        info.render_scale * 100.0
                    ),
                    10.0,
                    FAINT,
                );
            });
        });
}

fn compact_clocks(ctx: &egui::Context, info: &HudInfo<'_>, rel: Relativity) {
    egui::Area::new(Id::new("compact_clocks"))
        .anchor(Align2::RIGHT_TOP, Vec2::new(-22.0, 22.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(245.0);
                micro(ui, "DISTANT CLOCK / YOUR CLOCK");
                mono(ui, format_ratio(rel.time_ratio), 28.0, CYAN);
                hairline(ui);
                micro(ui, "PHYSICAL ELAPSED TIME");
                mono(
                    ui,
                    format!("FAR   {}", format_duration(info.ship.far_seconds())),
                    12.0,
                    WHITE,
                );
                mono(
                    ui,
                    format!("YOU   {}", format_duration(info.ship.proper_seconds())),
                    12.0,
                    WHITE,
                );
                micro(ui, "I  CLOCK AND LIGHT DETAILS");
            });
        });
}

fn journey_panel(ctx: &egui::Context, info: &HudInfo<'_>) {
    let fact = crate::journey::fact(info.journey_time);
    egui::Area::new(Id::new("journey_fact"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -24.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(600.0);
                ui.label(RichText::new(fact.title).size(17.0).color(CYAN));
                ui.add_space(5.0);
                ui.label(RichText::new(fact.text).size(14.0).color(WHITE));
                ui.add_space(10.0);
                let progress =
                    (info.journey_time / crate::journey::DURATION).clamp(0.0, 1.0) as f32;
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(600.0)
                        .fill(CYAN_DIM)
                        .show_percentage(),
                );
                micro(
                    ui,
                    &format!(
                        "{:02}:{:02} / 04:06   /   {}",
                        info.journey_time as u32 / 60,
                        info.journey_time as u32 % 60,
                        if info.paused {
                            "PAUSED"
                        } else if info.ship.lost {
                            "OUTRO / FINAL VIEW"
                        } else {
                            "SOUNDTRACK SYNCED"
                        }
                    ),
                );
            });
        });
    egui::Area::new(Id::new("transport"))
        .anchor(Align2::LEFT_BOTTOM, Vec2::new(22.0, -24.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                micro(ui, "DRAG / ARROWS   LOOK AROUND");
                micro(ui, "SPACE / P       PAUSE");
                micro(ui, "X               RECENTER VIEW");
                micro(ui, "R               REPLAY");
                micro(ui, "H               HIDE OVERLAYS");
                micro(ui, "I               CLOCK DETAILS");
                micro(ui, "1 / 2 / 3       QUALITY");
            });
        });
}

fn time_panel(
    ctx: &egui::Context,
    info: &HudInfo<'_>,
    rel: Relativity,
    screen: egui::Rect,
    margin: f32,
) {
    egui::Area::new(Id::new("clocks"))
        .anchor(Align2::RIGHT_TOP, Vec2::new(-margin, margin))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(322.0);
                let earth = earth_date(info.epoch_unix + info.earth_elapsed);
                ui.horizontal(|ui| {
                    micro(ui, "EARTH CHRONICLE");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        micro(ui, &format!("TIME-LAPSE ×{EARTH_TIME_LAPSE:.0}"));
                    });
                });
                ui.label(
                    RichText::new(&earth.date)
                        .size(20.0)
                        .color(WHITE)
                        .family(FontFamily::Monospace),
                );
                ui.label(
                    RichText::new(&earth.time)
                        .size(12.0)
                        .color(CYAN)
                        .family(FontFamily::Monospace),
                );

                hairline(ui);

                egui::Grid::new("clock grid")
                    .num_columns(2)
                    .spacing(Vec2::new(22.0, 2.0))
                    .show(ui, |ui| {
                        micro(ui, "FAR CLOCK (PHYSICAL)");
                        micro(ui, "SHIP PROPER  τ");
                        ui.end_row();
                        mono(ui, format_duration(info.ship.far_seconds()), 15.0, CYAN);
                        mono(ui, format_duration(info.ship.proper_seconds()), 15.0, WHITE);
                        ui.end_row();
                    });

                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        micro(
                            ui,
                            if info.ship.lost {
                                "FINAL SCRIPTED OBSERVER"
                            } else {
                                "PROPER TIME DILATION"
                            },
                        );
                        ui.label(
                            RichText::new(if info.ship.lost {
                                "FINAL CLOCK RATIO"
                            } else {
                                "CLOCK DIVERGENCE"
                            })
                            .size(15.0)
                            .color(if info.ship.lost { AMBER } else { WHITE })
                            .family(FontFamily::Proportional),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let color = if rel.time_ratio > 20.0 {
                            RED
                        } else if rel.time_ratio > 3.0 {
                            AMBER
                        } else {
                            CYAN
                        };
                        ui.label(
                            RichText::new(format_ratio(rel.time_ratio))
                                .size(23.0)
                                .color(color)
                                .family(FontFamily::Monospace),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        micro(ui, "FAR CLOCK / SHIP CLOCK");
                    });
                });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    micro(ui, "1 MIN ABOARD EQUALS");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        mono(
                            ui,
                            format!("{} OUTSIDE", format_span(rel.time_ratio as f64 * 60.0)),
                            12.0,
                            WHITE,
                        );
                    });
                });

                ui.label(
                    RichText::new(format!(
                        "CLOCK GAP  +{:.1} s",
                        (info.ship.far_seconds() - info.ship.proper_seconds()).max(0.0)
                    ))
                    .size(12.0)
                    .color(AMBER),
                );
                ui.label(
                    RichText::new(if rel.radius_rs > 30.0 {
                        "WEAK GRAVITY: CLOCKS NEARLY AGREE"
                    } else {
                        "STRONGER GRAVITY: WATCH THE CLOCK GAP"
                    })
                    .size(10.0)
                    .color(CYAN),
                );
                factor_row(ui, "GRAVITY", rel.alpha, CYAN);
                factor_row(ui, "VELOCITY", rel.speed_dilation(), CYAN_DIM);
                factor_row(ui, "COMBINED", rel.proper_per_far(), AMBER);

                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    micro(ui, "OUTBOUND LIGHT");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        mono(
                            ui,
                            format!("λ ×{:.3}", 1.0 / rel.alpha.max(0.001)),
                            11.5,
                            if rel.alpha < 0.8 { AMBER } else { MUTED },
                        );
                    });
                });
                ui.add_space(2.0);
                ui.label(
                    RichText::new(if info.ship.lost {
                        "FILM FREEZE AT 1.005 Rs / LOCAL TIME WOULD CONTINUE"
                    } else {
                        "CHRONICLE IS TIME-LAPSED · RATIO IS PHYSICAL"
                    })
                    .size(8.0)
                    .color(if info.ship.lost { AMBER } else { FAINT }),
                );
            });
        });
    let _ = screen;
}

fn factor_row(ui: &mut egui::Ui, label: &str, value: f32, color: Color32) {
    let value = value.clamp(0.0, 1.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(9.5).color(FAINT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            mono(ui, format!("{:.4}", value), 10.5, color);
            let width = 118.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 3.0), Sense::hover());
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(130, 185, 200, 22),
            );
            let frect = egui::Rect::from_min_size(rect.min, Vec2::new(width * value, 3.0));
            ui.painter().rect_filled(frect, 0.0, color);
        });
    });
}

fn light_panel(ctx: &egui::Context, rel: Relativity, screen: egui::Rect, margin: f32) {
    let outbound = 550.0 / rel.alpha.max(0.001) as f64;
    let inbound = 550.0 * rel.alpha as f64;
    egui::Area::new(Id::new("light"))
        .anchor(Align2::RIGHT_TOP, Vec2::new(-margin, margin + 420.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(322.0);
                micro(ui, "RELATIVISTIC OPTICS");
                ui.label(RichText::new("LIGHT STRETCH").size(16.5).color(WHITE));
                ui.add_space(3.0);
                hairline(ui);

                light_row(ui, "YOUR SIGNAL OUT", "550 nm", outbound, rel.alpha < 0.8);
                light_row(ui, "STARLIGHT ARRIVING", "550 nm", inbound, rel.alpha < 0.8);

                ui.add_space(4.0);
                micro(
                    ui,
                    "GRAVITATIONAL SHIFT ONLY · DISK BEAMING IS IN THE RENDER",
                );
            });
        });
    let _ = screen;
}

fn light_row(ui: &mut egui::Ui, label: &str, rest: &str, observed: f64, alert: bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            micro(ui, label);
            ui.horizontal(|ui| {
                swatch(ui, wavelength_color(550.0));
                ui.label(RichText::new(rest).size(10.0).color(FAINT));
                ui.label(RichText::new("→").size(10.0).color(FAINT));
                swatch(ui, wavelength_color(observed));
                mono(
                    ui,
                    format!("{:.0} nm", observed),
                    11.0,
                    if alert { AMBER } else { WHITE },
                );
            });
        });
    });
}

fn swatch(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 9.0), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, color);
    ui.painter().rect_stroke(
        rect,
        1.0,
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 28)),
    );
}

fn wavelength_color(nm: f64) -> Color32 {
    // Approximate visible-spectrum response, clamped into a useful swatch.
    let (r, g, b) = if nm < 380.0 {
        (0.34, 0.0, 0.56)
    } else if nm < 440.0 {
        ((440.0 - nm) / 60.0, 0.0, 1.0)
    } else if nm < 490.0 {
        (0.0, (nm - 440.0) / 50.0, 1.0)
    } else if nm < 510.0 {
        (0.0, 1.0, (510.0 - nm) / 20.0)
    } else if nm < 580.0 {
        ((nm - 510.0) / 70.0, 1.0, 0.0)
    } else if nm < 645.0 {
        (1.0, (645.0 - nm) / 65.0, 0.0)
    } else if nm < 780.0 {
        (1.0, 0.0, 0.0)
    } else {
        (0.24, 0.0, 0.0)
    };
    let (r, g, b) = (
        (r.clamp(0.0, 1.0).powf(0.8) * 255.0) as u8,
        (g.clamp(0.0, 1.0).powf(0.8) * 255.0) as u8,
        (b.clamp(0.0, 1.0).powf(0.8) * 255.0) as u8,
    );
    Color32::from_rgb(r, g, b)
}

fn format_ratio(ratio: f32) -> String {
    if ratio < 10.0 {
        format!("{:.3}×", ratio)
    } else if ratio < 100.0 {
        format!("{:.2}×", ratio)
    } else if ratio < 1.0e5 {
        format!("{:.1}×", ratio)
    } else {
        format!("{:.2e}×", ratio)
    }
}
