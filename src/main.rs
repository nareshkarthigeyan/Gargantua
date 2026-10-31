#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! GARGANTUA — a soundtrack-timed journey with Schwarzschild ray tracing.
//!
//! Native app:
//!     cargo run --release
//!
//! Headless still (useful for visual regression checks):
//!     cargo run --release -- --screenshot shots/orbit.png
//!     cargo run --release -- --screenshot shots/horizon.png --horizon --frames 4

mod app;
mod hud;
mod journey;
mod music;
mod optics;
mod physics;
mod renderer;
mod ship_mesh;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--validate-optics") {
        return optics::validate();
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if let Some(options) = app::parse_screenshot_args(&args) {
        app::render_screenshot(&options)
    } else {
        app::run_native()
    }
}

fn print_help() {
    println!(
        "{}",
        r#"GARGANTUA — A Scripted Descent

Run: gargantua

A 4:06 soundtrack-timed spiral from 500 Rs. One complete revolution by
1:05, then a gradual descent. The final approach peaks at 3:50; the
outro holds at 1.0001 Rs. Movement is scripted; looking is unrestricted.

Move mouse / trackpad / arrow keys     Look around (no held button)
X                                      Recenter the view
Space / P                              Pause the film and music
R                                      Replay both from the beginning
Esc                                    Pause menu
H                                      Hide overlays
I                                      Immersive mode (hide every overlay)
1 / 2 / 3                              Quality ceiling

Validation:
  gargantua --validate-optics
  gargantua --benchmark --at 180 --width 1400 --height 900

Stills (do not play audio):
  gargantua --screenshot image.png --at 210
  --width N --height N --menu --no-hud

The trajectory is authored, not a freefall geodesic. Rays are numerically
integrated in Schwarzschild geometry. Clock ratios use the actual observer
4-velocity. The final freeze is cinematic; local time continues at a horizon.
"#
    );
}
