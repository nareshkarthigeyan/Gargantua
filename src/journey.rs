//! A soundtrack-timed, deterministic, timelike camera path.
//! Geometry is scripted; clock factors use its actual local 4-velocity.
use crate::physics::{Ship, HORIZON_STOP};
use glam::{DVec3, Vec3};

pub const CLIMAX: f64 = 230.0;
pub const DURATION: f64 = 246.28;
const HZ: f64 = 120.0;
const COORDINATE_RATE: f64 = 120.0;
const KEYS: [(f64, f64, f64); 8] = [
    (0.0, 500.0, 0.0),
    (65.0, 120.0, std::f64::consts::TAU),
    (130.0, 25.0, std::f64::consts::TAU + 1.5),
    (180.0, 8.0, std::f64::consts::TAU + 3.8),
    (210.0, 3.0, std::f64::consts::TAU + 6.5),
    (222.0, 1.6, std::f64::consts::TAU + 7.4),
    (230.0, HORIZON_STOP as f64, std::f64::consts::TAU + 8.2),
    (DURATION, HORIZON_STOP as f64, std::f64::consts::TAU + 8.2),
];

pub struct Fact {
    pub start: f64,
    pub title: &'static str,
    pub text: &'static str,
}
pub const FACTS: &[Fact] = &[
    Fact { start: 0.0, title: "01 / A DISTANT OBSERVER",
        text: "At 500 Schwarzschild radii, gravity barely separates your clock from a distant clock." },
    Fact { start: 35.0, title: "02 / A FULL REVOLUTION",
        text: "The shadow is larger than the event horizon. It marks directions from which light cannot reach you." },
    Fact { start: 70.0, title: "03 / FOLLOW THE CURVE",
        text: "The far side of the disk appears above and below the hole because its light curves around it." },
    Fact { start: 112.0, title: "04 / THE BRIGHT SIDE",
        text: "Orbiting gas approaching you is Doppler shifted and beamed brighter than gas moving away." },
    Fact { start: 153.0, title: "05 / TWO CLOCKS",
        text: "Gravity and motion both change the clock ratio. The calendar is time-lapsed; the ratio is physical." },
    Fact { start: 196.0, title: "06 / THE LAST STABLE CIRCULAR ORBIT",
        text: "For this non-rotating hole, the ISCO is at 3 Rs. Stable circular geodesics do not exist farther inward." },
    Fact { start: 218.0, title: "07 / THE PHOTON SPHERE",
        text: "At 1.5 Rs, light can follow unstable circular orbits. Nearby rays form higher-order images." },
    Fact { start: 226.0, title: "08 / THE EVENT HORIZON",
        text: "The horizon is at 1 Rs. No future-directed signal from inside it can reach the outside universe." },
    Fact { start: 230.0, title: "09 / THE LAST SIGNAL",
        text: "This film freezes at 1.005 Rs. A falling observer's own clock would keep ticking across the horizon." },
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
    let radius = position.length();
    let n = position / radius;
    let alpha = (1.0 - 1.0 / radius).sqrt();
    let wr = derivative.dot(n);
    let wt = derivative - n * wr;
    let local_per_real = n * (wr / (alpha * alpha)) + wt / alpha;
    // Reduce the film's time compression in the strong field so orbiting
    // disk texture remains temporally resolved instead of strobing.
    let x = ((radius - 8.0) / 112.0).clamp(0.0, 1.0);
    let nominal_rate = 6.0 + (COORDINATE_RATE - 6.0) * x * x * (3.0 - 2.0 * x);
    let coordinate_rate = nominal_rate.max(local_per_real.length() / 0.90);
    let beta = local_per_real / coordinate_rate;
    let gamma = 1.0 / (1.0 - beta.length_squared()).sqrt();
    let proper_rate = coordinate_rate * alpha / gamma;
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
        self.elapsed = time.clamp(0.0, DURATION);
        let t = self.elapsed.min(CLIMAX);
        let (position, _) = path(t);
        let (_, _, velocity) = rates(t);
        let sample = t * HZ;
        let i = (sample.floor() as usize).min(self.clocks.len() - 1);
        let j = (i + 1).min(self.clocks.len() - 1);
        let mix = sample - i as f64;
        ship.position = position.as_vec3();
        ship.velocity = velocity;
        ship.proper_time = self.clocks[i].0 * (1.0 - mix) + self.clocks[j].0 * mix;
        ship.coordinate_time = self.clocks[i].1 * (1.0 - mix) + self.clocks[j].1 * mix;
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
        assert!((previous - HORIZON_STOP as f64).abs() < 1e-8);
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
        assert_eq!((sa.proper_time, sa.coordinate_time), clocks);
    }
    #[test]
    fn world_velocity_agrees_with_clock_normalisation() {
        let mut journey = Journey::new();
        let mut ship = Ship::default();
        for t in [10.0, 65.0, 160.0, 210.0, 225.0, 229.0] {
            journey.seek(t, &mut ship);
            let (proper, coordinate, _) = rates(t);
            assert!((ship.relativity().time_ratio as f64 - coordinate / proper).abs() < 0.001);
            assert!(ship.relativity().local_speed_c < 0.91);
        }
    }
}
