//! Earth-side wall clock.
//!
//! The flight simulation's coordinate time is physical (seconds per Rs/c),
//! which is far too slow to watch a calendar move. The chronicle therefore
//! applies a fixed presentation time-lapse, clearly labelled in the HUD, so
//! days and years visibly pass while you fly. The relativistic clock ratio is
//! still computed separately from the Schwarzschild 4-velocity.

/// Earth seconds per physical coordinate second. At the default sim rate this
/// is six Earth hours per real second when far from the hole.
pub const EARTH_TIME_LAPSE: f64 = 274.0;

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Wall-clock seconds since the Unix epoch in the machine's local timezone.
/// Formatting treats the result as if it were UTC, which yields local time.
pub fn unix_now() -> f64 {
    let now = chrono::Local::now();
    let utc = now.timestamp() as f64 + now.timestamp_subsec_nanos() as f64 * 1e-9;
    utc + now.offset().local_minus_utc() as f64
}

#[derive(Clone, Debug)]
pub struct EarthDate {
    pub date: String,
    pub time: String,
}

/// Convert fractional Unix seconds into a UTC calendar label.
pub fn earth_date(unix: f64) -> EarthDate {
    let unix = unix.max(0.0);
    let days = (unix / 86_400.0).floor() as i64;
    let secs_of_day = unix - days as f64 * 86_400.0;
    let (year, month, day) = civil_from_days(days);
    let hour = (secs_of_day / 3_600.0).floor() as u32;
    let minute = ((secs_of_day % 3_600.0) / 60.0).floor() as u32;
    let second = (secs_of_day % 60.0).floor() as u32;
    EarthDate {
        date: format!("{year} {} {day:02}", MONTHS[(month - 1).min(11) as usize]),
        time: format!("{hour:02}:{minute:02}:{second:02} LOCAL"),
    }
}

/// Howard Hinnant's proleptic Gregorian conversion, days since 1970-01-01.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Compact span for the "one minute aboard equals" readout.
pub fn format_span(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let years = s / (365.25 * 86_400.0);
    if years >= 1.0 {
        return format!("{years:.2} years");
    }
    let days = s / 86_400.0;
    if days >= 1.0 {
        return format!("{days:.1} days");
    }
    let hours = s / 3_600.0;
    if hours >= 1.0 {
        return format!("{hours:.1} hours");
    }
    format!("{:.1} min", s / 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epoch_dates() {
        assert_eq!(earth_date(0.0).date, "1970 JAN 01");
        // 2026-11-01 00:00:00 UTC
        assert_eq!(earth_date(1_793_491_200.0).date, "2026 NOV 01");
        assert_eq!(earth_date(1_793_491_200.0).time, "00:00:00 LOCAL");
    }

    #[test]
    fn span_labels() {
        assert_eq!(format_span(60.0), "1.0 min");
        assert_eq!(format_span(86_400.0), "1.0 days");
        assert_eq!(format_span(365.25 * 86_400.0), "1.00 years");
    }
}
