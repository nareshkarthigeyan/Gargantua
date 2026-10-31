//! One visual-novel dialogue box; the film has no corner instrumentation.
use egui::{
    Align2, Color32, FontFamily, FontId, Frame, Id, Margin, RichText, Rounding, Stroke, TextStyle,
    Vec2,
};

const INK: Color32 = Color32::from_rgb(9, 12, 18);
const WHITE: Color32 = Color32::from_rgb(240, 237, 222);
const MUTED: Color32 = Color32::from_rgb(145, 159, 169);
const ACCENT: Color32 = Color32::from_rgb(156, 222, 233);

pub struct HudInfo {
    pub journey_time: f64,
    pub proper_seconds: f64,
    pub far_seconds: f64,
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
        .insert(TextStyle::Body, FontId::new(20.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(16.0, FontFamily::Proportional),
    );
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(WHITE);
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.window_rounding = Rounding::ZERO;
    ctx.set_style(style);
}

fn dialogue_visible(age: f64) -> bool {
    (0.0..2.0).contains(&age)
}

pub fn draw(ctx: &egui::Context, info: &HudInfo) {
    let fact = crate::journey::fact(info.journey_time);
    let age = (info.journey_time - fact.start).max(0.0);
    egui::Area::new(Id::new("two_clocks"))
        .anchor(Align2::RIGHT_TOP, Vec2::new(-28.0, 24.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::none()
                .fill(Color32::from_black_alpha(180))
                .inner_margin(Margin::symmetric(18.0, 12.0))
                .show(ui, |ui| {
                    ui.set_min_width(200.0);
                    for (label, seconds) in [
                        ("Earth time", info.far_seconds),
                        ("Your time", info.proper_seconds),
                    ] {
                        ui.label(RichText::new(label).size(16.0).color(MUTED));
                        ui.label(
                            RichText::new(crate::physics::format_duration(seconds))
                                .size(25.0)
                                .color(WHITE),
                        );
                    }
                });
        });
    // Full prose remains readable for exactly two seconds, then no box or hints.
    if !dialogue_visible(age) {
        return;
    }
    let text = fact.text;
    let width = (ctx.screen_rect().width() - 112.0).clamp(320.0, 980.0);
    egui::Area::new(Id::new("novel_dialogue"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -28.0))
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(7, 10, 16, 236))
                .stroke(Stroke::new(2.0_f32, Color32::from_rgb(85, 114, 126)))
                .rounding(Rounding::ZERO)
                .inner_margin(Margin::symmetric(28.0, 20.0))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, 76.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.label(RichText::new(text).size(22.0).color(WHITE));
                        },
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("I · Immersive").size(13.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("Esc · Menu").size(11.0).color(MUTED));
                        });
                    });
                });
        });
}

pub fn draw_menu(ctx: &egui::Context, can_resume: bool) -> MenuAction {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        Id::new("menu_shade"),
    ))
    .rect_filled(screen, 0.0, Color32::from_black_alpha(100));
    let mut action = MenuAction::None;
    egui::Area::new(Id::new("menu_panel"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::none()
                .fill(INK)
                .stroke(Stroke::new(1.0_f32, ACCENT))
                .inner_margin(Margin::symmetric(34.0, 30.0))
                .show(ui, |ui| {
                    ui.set_width(460.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("GARGANTUA").size(44.0).color(WHITE));
                        ui.label(RichText::new("A SCRIPTED DESCENT").size(13.0).color(ACCENT));
                        ui.add_space(20.0);
                        ui.label(RichText::new("Four minutes. One final signal.").size(19.0));
                        ui.add_space(20.0);
                    });
                    for (label, choice) in [
                        (
                            if can_resume {
                                "RESUME JOURNEY"
                            } else {
                                "BEGIN JOURNEY"
                            },
                            MenuAction::Play,
                        ),
                        ("REPLAY FROM THE BEGINNING", MenuAction::Restart),
                        ("QUIT", MenuAction::Quit),
                    ] {
                        if ui
                            .add_sized(
                                [460.0, 43.0],
                                egui::Button::new(RichText::new(label).size(16.0)),
                            )
                            .clicked()
                        {
                            action = choice;
                        }
                        ui.add_space(8.0);
                    }
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("MOVE YOUR MOUSE OR TRACKPAD TO LOOK")
                                .size(12.0)
                                .color(ACCENT),
                        );
                        ui.label(
                            RichText::new(
                                "No clicking or dragging needed. Esc releases the pointer.",
                            )
                            .size(13.0)
                            .color(MUTED),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("I  IMMERSIVE   /   SPACE  PAUSE   /   X  RECENTER")
                                .size(12.0)
                                .color(MUTED),
                        );
                    });
                });
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dialogue_disappears_after_two_seconds() {
        assert!(dialogue_visible(0.0));
        assert!(dialogue_visible(1.999));
        assert!(!dialogue_visible(2.0));
        assert!(!dialogue_visible(100.0));
    }
}
