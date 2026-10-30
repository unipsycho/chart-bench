use chrono::NaiveDateTime;

use super::detect::RawTable;

pub struct TimeDetect {
    pub column_index: Option<usize>,
    pub confidence: f32,
    pub is_time: bool,
}

const NAME_CUES: &[&str] = &[
    "timestamp",
    "datetime",
    "date_time",
    "date time",
    "time",
    "date",
    "epoch",
    "unix",
    "systime",
    "gps_time",
    "sample_time",
    "ts",
    "t",
];

pub fn detect_time_column(table: &RawTable, data_start: usize, width: usize) -> TimeDetect {
    if width == 0 || data_start >= table.rows.len() {
        return TimeDetect {
            column_index: None,
            confidence: 0.0,
            is_time: false,
        };
    }

    let header_names: Vec<String> = (0..width)
        .map(|c| {
            // Prefer a Names-like header: scan first few rows for non-label cells
            table
                .rows
                .iter()
                .take(data_start.max(1))
                .find_map(|r| {
                    let cell = r.get(c)?.trim();
                    if cell.is_empty() {
                        return None;
                    }
                    let lower = cell.to_lowercase();
                    if matches!(
                        lower.as_str(),
                        "sensor" | "node" | "nodes" | "units" | "unit" | "name" | "names"
                    ) && c == 0
                    {
                        return None;
                    }
                    Some(cell.to_string())
                })
                .unwrap_or_default()
        })
        .collect();

    let sample_end = (data_start + 40).min(table.rows.len());
    let mut best: Option<(usize, f32, bool)> = None;

    for col in 0..width {
        let name = header_names[col].to_lowercase();
        let mut name_bonus = 0.0;
        for cue in NAME_CUES {
            if name == *cue || name.contains(cue) {
                name_bonus = if name == *cue { 0.45 } else { 0.3 };
                break;
            }
        }

        let mut parse_hits = 0usize;
        let mut total = 0usize;
        let mut last_v: Option<f64> = None;
        let mut mono = 0usize;
        let mut mono_total = 0usize;

        for r in data_start..sample_end {
            let Some(cell) = table.rows[r].get(col) else {
                continue;
            };
            let t = cell.trim();
            if t.is_empty() {
                continue;
            }
            total += 1;
            if let Some(v) = parse_time_value(t) {
                parse_hits += 1;
                if let Some(prev) = last_v {
                    mono_total += 1;
                    if v >= prev {
                        mono += 1;
                    }
                }
                last_v = Some(v);
            }
        }

        if total == 0 {
            continue;
        }
        let parse_ratio = parse_hits as f32 / total as f32;
        let mono_ratio = if mono_total > 0 {
            mono as f32 / mono_total as f32
        } else {
            0.0
        };
        let conf = name_bonus + parse_ratio * 0.5 + mono_ratio * 0.15;
        let is_time = parse_ratio > 0.6 || (name_bonus > 0.0 && parse_ratio > 0.35);
        if best.map(|(_, c, _)| conf > c).unwrap_or(true) {
            best = Some((col, conf, is_time));
        }
    }

    match best {
        Some((col, conf, is_time)) if conf > 0.25 => TimeDetect {
            column_index: Some(col),
            confidence: conf.min(1.0),
            is_time,
        },
        _ => TimeDetect {
            column_index: None,
            confidence: 0.0,
            is_time: false,
        },
    }
}

pub fn parse_time_value(s: &str) -> Option<f64> {
    let t = s.trim();
    // Epoch seconds / ms / µs only — never treat arbitrary floats as time.
    if let Ok(v) = t.parse::<f64>() {
        if (1.0e9..2.0e10).contains(&v) {
            return Some(v);
        }
        if (1.0e12..2.0e13).contains(&v) {
            return Some(v / 1000.0);
        }
        if (1.0e15..2.0e16).contains(&v) {
            return Some(v / 1.0e6);
        }
    }

    // LabVIEW-style: "9:24:34.354 AM 2019-10-30"
    if let Some(v) = parse_ampm_datetime(t) {
        return Some(v);
    }

    // Strip trailing Z / UTC designator so NaiveDateTime formats match ISO-8601.
    let t = t
        .strip_suffix('Z')
        .or_else(|| t.strip_suffix('z'))
        .unwrap_or(t)
        .trim_end();
    // Also accept "+00:00" / "-00:00" style offsets by chopping a trailing ±HH:MM.
    let t = strip_numeric_tz_offset(t);

    const FMTS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M", // wind_laurel / minute-resolution logs
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%d/%m/%Y %H:%M:%S",
        "%m/%d/%Y %H:%M:%S",
        "%Y-%m-%d",
    ];
    for f in FMTS {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(t, f) {
            // chrono 0.4.34: timestamp() on NaiveDateTime treats value as UTC
            return Some(
                ndt.timestamp() as f64 + f64::from(ndt.timestamp_subsec_millis()) / 1000.0,
            );
        }
        if *f == "%Y-%m-%d" {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(t, f) {
                let ndt = d.and_hms_opt(0, 0, 0)?;
                return Some(ndt.timestamp() as f64);
            }
        }
    }
    None
}

/// Chop a trailing `+HH:MM` / `-HH:MM` / `+HHMM` timezone suffix when present.
fn strip_numeric_tz_offset(t: &str) -> &str {
    // Look for the last + or - that looks like a TZ offset (after a time portion).
    let bytes = t.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'+' || (bytes[i] == b'-' && i > 10) {
            let suf = &t[i + 1..];
            // HH:MM or HHMM
            if matches!(suf.len(), 4 | 5)
                && suf.chars().all(|c| c.is_ascii_digit() || c == ':')
            {
                return t[..i].trim_end();
            }
            break;
        }
    }
    t
}

/// Parse strings like `9:24:34.354 AM 2019-10-30` or `09:24:34 PM 2019-10-30`.
fn parse_ampm_datetime(t: &str) -> Option<f64> {
    let upper = t.to_uppercase();
    let (am, rest) = if let Some(i) = upper.find(" AM") {
        (true, format!("{}{}", &t[..i], &t[i + 3..]))
    } else if let Some(i) = upper.find(" PM") {
        (false, format!("{}{}", &t[..i], &t[i + 3..]))
    } else {
        return None;
    };
    let rest = rest.trim();
    // Expect: H:MM:SS[.fff] YYYY-MM-DD
    let mut parts = rest.split_whitespace();
    let time_s = parts.next()?;
    let date_s = parts.next()?;
    let (hms, frac) = match time_s.split_once('.') {
        Some((h, f)) => (h, f.chars().take(3).collect::<String>()),
        None => (time_s, "0".into()),
    };
    let mut hms_parts = hms.split(':');
    let mut h: u32 = hms_parts.next()?.parse().ok()?;
    let m: u32 = hms_parts.next()?.parse().ok()?;
    let s: u32 = hms_parts.next().unwrap_or("0").parse().ok()?;
    if !am && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    let date = chrono::NaiveDate::parse_from_str(date_s, "%Y-%m-%d").ok()?;
    let ndt = date.and_hms_milli_opt(h, m, s, frac.parse().unwrap_or(0))?;
    Some(ndt.timestamp() as f64 + f64::from(ndt.timestamp_subsec_millis()) / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso() {
        let v = parse_time_value("2024-10-01T08:00:00").unwrap();
        assert!(v > 1.0e9);
        let z = parse_time_value("2024-06-01T12:00:00Z").unwrap();
        assert!(z > 1.0e9);
        let offset = parse_time_value("2024-06-01T12:00:00.000+00:00").unwrap();
        assert!((offset - z).abs() < 1.0);
    }

    #[test]
    fn parses_epoch_ms() {
        let v = parse_time_value("1727770000000").unwrap();
        assert!((v - 1727770000.0).abs() < 1.0);
    }

    #[test]
    fn rejects_plain_floats() {
        assert!(parse_time_value("21.1").is_none());
        assert!(parse_time_value("-53.15").is_none());
    }

    #[test]
    fn parses_labview_ampm() {
        let v = parse_time_value("9:24:34.354 AM 2019-10-30").unwrap();
        assert!(v > 1.0e9);
    }

    #[test]
    fn parses_minute_resolution_datetime() {
        let v = parse_time_value("2001-06-11 11:00").unwrap();
        // 2001-06 is ~9.92e8 s (before the 1e9 epoch landmark)
        assert!((9.9e8..1.0e9).contains(&v), "got {v}");
    }
}
