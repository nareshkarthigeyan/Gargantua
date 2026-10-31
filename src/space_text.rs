//! Pixel holograms anchored in world space along the initial approach corridor.
//! These are actual mesh glyphs: perspective, depth, and passing the plane all
//! come from the same camera transform as the spacecraft.
use crate::ship_mesh::ShipVertex;
use glam::Vec3;

pub fn append(out: &mut Vec<ShipVertex>, camera: Vec3, forward: Vec3, time: f64) {
    for fact in crate::journey::FACTS {
        if time < fact.start || time > fact.start + 16.0 || fact.start >= crate::journey::CLIMAX {
            continue;
        }
        let anchor = crate::journey::path((fact.start + 8.0).min(229.0))
            .0
            .as_vec3();
        let from = crate::journey::path(fact.start).0.as_vec3();
        let normal = (from - anchor).normalize_or_zero();
        let right = Vec3::Y.cross(normal).normalize_or_zero();
        let up = normal.cross(right).normalize_or_zero();
        let distance = (from - anchor).length().max(0.01);
        if (anchor - camera).dot(forward) <= 0.0 {
            continue;
        }
        let cell = distance * 0.0020;
        line(
            out,
            anchor + up * (cell * 14.0),
            right,
            up,
            cell,
            fact.title,
            [0.60, 0.76, 0.80],
        );
    }
}

fn line(
    out: &mut Vec<ShipVertex>,
    anchor: Vec3,
    right: Vec3,
    up: Vec3,
    cell: f32,
    text: &str,
    color: [f32; 3],
) {
    let width = text.chars().count() as f32 * 6.0 * cell;
    let normal = right.cross(up);
    for (index, ch) in text.chars().enumerate() {
        let glyph = glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                let x = index as f32 * 6.0 * cell + col as f32 * cell - width * 0.5;
                let y = -(row as f32) * cell;
                let a = anchor + right * x + up * y;
                let b = a + right * (cell * 0.86);
                let c = b - up * (cell * 0.86);
                let d = a - up * (cell * 0.86);
                for p in [a, b, c, a, c, d] {
                    out.push(ShipVertex {
                        position: p.to_array(),
                        normal: normal.to_array(),
                        color,
                        emissive: 0.85,
                    });
                }
            }
        }
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        _ => [0; 7],
    }
}
