//! A soundtrack-timed, deterministic, timelike camera path.
//! Geometry is scripted; clock factors use its actual local 4-velocity.
use crate::physics::{Ship, HORIZON_STOP};
use glam::{DVec3, Vec3};

pub const CLIMAX: f64 = 230.0;
pub const DURATION: f64 = 246.28;
const HZ: f64 = 120.0;
const PROPER_RATE: f64 = 24.0;
// Preserve the lapse in log-gap form below float32 rendering precision.
const FINAL_GAP: f64 = 1.0e-15;
const KEYS: [(f64, f64, f64); 8] = [
    (0.0, 500.0, 0.0),
    (65.0, 120.0, std::f64::consts::TAU),
    (130.0, 25.0, std::f64::consts::TAU + 1.5),
    (180.0, 8.0, std::f64::consts::TAU + 3.8),
    (210.0, 3.0, std::f64::consts::TAU + 6.5),
    (222.0, 1.6, std::f64::consts::TAU + 7.4),
    (230.0, 1.0 + FINAL_GAP, std::f64::consts::TAU + 8.2),
    (DURATION, 1.0 + FINAL_GAP, std::f64::consts::TAU + 8.2),
];

pub struct Fact {
    pub start: f64,
    pub text: &'static str,
}
pub const FACTS: &[Fact] = &[
    Fact { start: 0.0,
        text: "There it is. From this far away, your clock and a distant clock almost agree. Take a look around. Our path is already set." },
    Fact { start: 35.0,
        text: "We are circling now. That darkness is a shadow, larger than the horizon itself. Those are directions from which no light reaches your eyes." },
    Fact { start: 70.0,
        text: "See the light folding over the darkness? Some of it comes from the far side of the disk. Spacetime curves its path all the way around to you." },
    Fact { start: 112.0,
        text: "One side burns brighter. That gas is moving toward you; its light is shifted and beamed into your view. The other side is moving away." },
    Fact { start: 153.0,
        text: "Watch the two clocks drift apart. Gravity and motion both change their rates. A distant observer counts more time than you do." },
    Fact { start: 196.0,
        text: "We are nearing the last stable circular orbit: three horizon radii from the centre. Farther in, an unpowered circular orbit cannot stay stable." },
    Fact { start: 218.0,
        text: "The photon sphere is close. Here, light can circle the hole on an unstable path. Rays that nearly escape draw those thin, repeated images." },
    Fact { start: 226.0,
        text: "The horizon is ahead. Past that boundary, no light you send can return to the outside universe." },
    Fact { start: 230.0,
        text: "This is our last frame, just outside the horizon. The film has stopped; your own clock would not. A falling observer would continue across." },
];
pub fn fact(time: f64) -> &'static Fact {
    FACTS
        .iter()
        .rev()
        .find(|f| time >= f.start)
        .unwrap_or(&FACTS[0])
}

fn ordinate(index: usize, angle: bool) -> f64 {
    if angle {
        KEYS[index].2
    } else {
        (KEYS[index].1 - 1.0).ln()
    }
}
fn slope(index: usize, angle: bool) -> f64 {
    if index == 0 || index >= 6 {
        return 0.0;
    }
    let before =
        (ordinate(index, angle) - ordinate(index - 1, angle)) / (KEYS[index].0 - KEYS[index - 1].0);
    let after =
        (ordinate(index + 1, angle) - ordinate(index, angle)) / (KEYS[index + 1].0 - KEYS[index].0);
    if before * after <= 0.0 {
        0.0
    } else {
        2.0 * before * after / (before + after)
    }
}
fn curve(time: f64, angle: bool) -> (f64, f64) {
    let t = time.clamp(0.0, CLIMAX);
    let i = (0..6).find(|&i| t <= KEYS[i + 1].0).unwrap_or(5);
    let span = KEYS[i + 1].0 - KEYS[i].0;
    let u = ((t - KEYS[i].0) / span).clamp(0.0, 1.0);
    let a = ordinate(i, angle);
    let b = ordinate(i + 1, angle);
    let ma = slope(i, angle) * span;
    let mb = slope(i + 1, angle) * span;
    let value = (2.0 * u * u * u - 3.0 * u * u + 1.0) * a
        + (u * u * u - 2.0 * u * u + u) * ma
        + (-2.0 * u * u * u + 3.0 * u * u) * b
        + (u * u * u - u * u) * mb;
    let derivative = ((6.0 * u * u - 6.0 * u) * a
        + (3.0 * u * u - 4.0 * u + 1.0) * ma
        + (-6.0 * u * u + 6.0 * u) * b
        + (3.0 * u * u - 2.0 * u) * mb)
        / span;
    (value, derivative)
}

/// Position and derivative with respect to soundtrack seconds, in areal units.
pub fn path(time: f64) -> (DVec3, DVec3) {
    let (gap, gap_rate) = curve(time, false);
    let r = 1.0 + gap.exp();
    let dr = gap_rate * gap.exp();
    let (phi, omega) = curve(time, true);
    let tilt: f64 = 0.28; // keep the equatorial disk in view throughout the spiral
    let n = DVec3::new(phi.sin() * tilt.cos(), tilt.sin(), phi.cos() * tilt.cos());
    let tangent = DVec3::new(phi.cos() * tilt.cos(), 0.0, -phi.sin() * tilt.cos());
    (n * r, n * dr + tangent * (r * omega))
}
fn rates(time: f64) -> (f64, f64, Vec3) {
    let (position, derivative) = path(time);
    let gap = curve(time, false).0.exp();
    let radius = 1.0 + gap;
    let n = position.normalize();
    let alpha = (gap / radius).sqrt();
    let wr = derivative.dot(n);
    let wt = derivative - n * wr;
    // Normalize the Schwarzschild metric with a fixed proper-time playback
    // rate. The distant clock follows the actual trajectory, not an effect curve.
    let proper_rate = PROPER_RATE;
    let coordinate_rate =
        (proper_rate * proper_rate + wr * wr / (alpha * alpha) + wt.length_squared()).sqrt()
            / alpha;
    let spatial_u = derivative / proper_rate;
    (proper_rate, coordinate_rate, spatial_u.as_vec3())
}

pub struct Journey {
    pub elapsed: f64,
    clocks: Vec<(f64, f64)>,
}
impl Journey {
    pub fn new() -> Self {
        let count = (CLIMAX * HZ) as usize;
        let mut clocks = Vec::with_capacity(count + 1);
        clocks.push((0.0, 0.0));
        for i in 0..count {
            let (proper, coordinate, _) = rates((i as f64 + 0.5) / HZ);
            let previous = clocks[i];
            clocks.push((previous.0 + proper / HZ, previous.1 + coordinate / HZ));
        }
        Self {
            elapsed: 0.0,
            clocks,
        }
    }
    pub fn seek(&mut self, time: f64, ship: &mut Ship) {
        self.elapsed = time.max(0.0);
        let t = self.elapsed.min(CLIMAX);
        let (position, _) = path(t);
        let (_, _, velocity) = rates(t);
        let sample = t * HZ;
        let i = (sample.floor() as usize).min(self.clocks.len() - 1);
        let j = (i + 1).min(self.clocks.len() - 1);
        let mix = sample - i as f64;
        // Optical rays use a finite float32 cutoff; clocks retain the true
        // double-precision trajectory. Do not round the lapse to zero.
        ship.position =
            (position.normalize() * position.length().max(HORIZON_STOP as f64)).as_vec3();
        ship.velocity = velocity;
        ship.proper_time = self.clocks[i].0 * (1.0 - mix) + self.clocks[j].0 * mix;
        ship.coordinate_time = self.clocks[i].1 * (1.0 - mix) + self.clocks[j].1 * mix;
        let outro = (self.elapsed - CLIMAX).max(0.0);
        ship.proper_time += PROPER_RATE * outro;
        ship.coordinate_time += PROPER_RATE * (rates(CLIMAX).1 / PROPER_RATE) * outro;
        ship.lost = self.elapsed >= CLIMAX;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_revolution_before_plunge_and_monotone_radius() {
        assert!((curve(65.0, true).0 - std::f64::consts::TAU).abs() < 1e-10);
        let mut previous = 501.0;
        for i in 0..=23000 {
            let t = i as f64 / 100.0;
            let radius = path(t).0.length();
            assert!(radius <= previous + 1e-9);
            assert!(rates(t).0.is_finite() && rates(t).0 > 0.0);
            previous = radius;
        }
        assert!((previous - (1.0 + FINAL_GAP)).abs() < 1e-12);
    }
    #[test]
    fn clocks_and_route_ignore_frame_rate_and_input() {
        let mut a = Journey::new();
        let mut b = Journey::new();
        let mut sa = Ship::default();
        let mut sb = Ship::default();
        for i in 0..=13800 {
            a.seek(i as f64 / 60.0, &mut sa);
        }
        for i in 0..=6900 {
            b.seek(i as f64 / 30.0, &mut sb);
        }
        assert_eq!(sa.position, sb.position);
        assert_eq!(sa.proper_time, sb.proper_time);
        assert_eq!(sa.coordinate_time, sb.coordinate_time);
        assert!(sa.coordinate_time > sa.proper_time && sa.lost);
        let clocks = (sa.proper_time, sa.coordinate_time);
        a.seek(DURATION, &mut sa);
        assert!(sa.proper_time > clocks.0 && sa.coordinate_time > clocks.1);
        let ratio = (sa.coordinate_time - clocks.1) / (sa.proper_time - clocks.0);
        assert!((ratio - rates(CLIMAX).1 / PROPER_RATE).abs() < 1e-6);
        let end_position = sa.position;
        a.seek(DURATION + 60.0, &mut sa);
        assert_eq!(sa.position, end_position);
        assert!(sa.proper_time > clocks.0 + PROPER_RATE * 60.0);
    }
    #[test]
    fn horizon_clock_limit_is_large_and_timelike() {
        let mut previous = 0.0;
        for t in [222.0, 224.0, 226.0, 228.0, 230.0] {
            let (proper, far, _) = rates(t);
            let ratio = far / proper;
            assert!(ratio > previous);
            previous = ratio;
        }
        assert!(previous > 30_000_000.0);
        let gap = curve(CLIMAX, false).0.exp();
        assert!((previous * (gap / (1.0 + gap)).sqrt() - 1.0).abs() < 1e-6);
    }
    #[test]
    fn world_velocity_agrees_with_clock_normalisation() {
        let mut journey = Journey::new();
        let mut ship = Ship::default();
        for t in [10.0, 65.0, 160.0, 210.0] {
            journey.seek(t, &mut ship);
            let (proper, coordinate, _) = rates(t);
            assert!(
                (ship.relativity().time_ratio as f64 / (coordinate / proper) - 1.0).abs() < 0.001
            );
            assert!(ship.local_velocity().length() < 1.0);
        }
    }
}
