//! Schwarzschild clock normalization for the scripted observer.
//! Units: G=c=1, Rs=1. Motion is authored by journey.rs; finite spatial
//! 4-velocity is normalized to derive local speed and both physical clocks.

use glam::Vec3;

pub const RS: f32 = 1.0;
pub const HORIZON_STOP: f32 = 1.0001;
pub const RS_METERS: f64 = 2.953_250_077e9;
pub const RS_OVER_C: f64 = RS_METERS / 299_792_458.0;

#[derive(Clone, Copy, Debug)]
pub struct Relativity {
    pub alpha: f32,      // sqrt(1 - Rs/r), gravitational clock factor
    pub gamma: f32,      // Lorentz factor relative to static observers
    pub time_ratio: f32, // dt_far / dτ_ship
}

#[derive(Clone, Debug)]
pub struct Ship {
    pub position: Vec3,
    pub velocity: Vec3, // dx/dτ in the areal Schwarzschild chart
    pub proper_time: f64,
    pub coordinate_time: f64,
    pub lost: bool,
}

impl Default for Ship {
    fn default() -> Self {
        // A distant stationary observer; the timeline supplies the trajectory.
        let position = Vec3::new(0.0, 140.0, 480.0);
        // Begin at rest in the distant static frame. A circular start gave
        // the ship enough angular momentum to miss the hole on every dive.
        let velocity = Vec3::ZERO;

        Self {
            position,
            velocity,
            proper_time: 0.0,
            coordinate_time: 0.0,
            lost: false,
        }
    }
}

impl Ship {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn local_velocity(&self) -> Vec3 {
        let rel = self.relativity();
        let radial = self.position.normalize();
        let wr = self.velocity.dot(radial);
        (self.velocity + radial * (wr / rel.alpha - wr)) / rel.gamma
    }

    pub fn relativity(&self) -> Relativity {
        let r = self.position.length().max(1.0 + 1e-5);
        let alpha = (1.0 - RS / r).max(1e-6).sqrt();
        let n = self.position / r;
        let wr = self.velocity.dot(n);
        let wt = (self.velocity - n * wr).length();
        let gamma = (1.0 + wr * wr / (alpha * alpha) + wt * wt).max(1.0).sqrt();
        let u_t = gamma / alpha;
        Relativity {
            alpha,
            gamma,
            time_ratio: u_t,
        }
    }

    pub fn near_horizon(&self) -> f32 {
        let r = self.position.length();
        (1.0 - ((r - 1.0) / 0.55)).clamp(0.0, 1.0)
    }

    pub fn proper_seconds(&self) -> f64 {
        self.proper_time * RS_OVER_C
    }

    pub fn far_seconds(&self) -> f64 {
        self.coordinate_time * RS_OVER_C
    }
}

pub fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let days = (s / 86_400.0).floor();
    let hours = ((s % 86_400.0) / 3_600.0).floor();
    let minutes = ((s % 3_600.0) / 60.0).floor();
    let secs = (s % 60.0).floor();
    if days >= 1.0 {
        format!("{:.0}d {:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_clock_matches_schwarzschild() {
        let mut observer = Ship::default();
        for r in [500.0_f32, 10.0, 3.0, 1.1, HORIZON_STOP] {
            observer.position = Vec3::Z * r;
            let expected = 1.0 / (1.0 - RS / r).sqrt();
            assert!((observer.relativity().time_ratio - expected).abs() < 0.0001);
        }
    }
    #[test]
    fn local_velocity_matches_lorentz_factor() {
        let mut observer = Ship::default();
        observer.position = Vec3::Z * 2.0;
        observer.velocity = Vec3::new(0.2, 0.0, -0.4);
        let gamma = observer.relativity().gamma;
        assert!(
            (observer.local_velocity().length_squared() - (1.0 - 1.0 / (gamma * gamma))).abs()
                < 0.00001
        );
    }
}
