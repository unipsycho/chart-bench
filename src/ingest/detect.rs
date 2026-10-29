use crate::model::{HeaderRole, ParsePlan, XAxis, XMode};

use super::time_detect::{detect_time_column, TimeDetect};

#[derive(Debug, Clone)]
pub struct RawTable {
    pub rows: Vec<Vec<String>>,
}

impl RawTable {
    pub fn width_of(&self, row: usize) -> usize {
        self.rows.get(row).map(|r| r.len()).unwrap_or(0)
    }
}

pub fn sniff_delimiter(sample: &str) -> u8 {
    let mut scores = [(b',', 0usize), (b';', 0), (b'\t', 0), (b'|', 0)];
    for line in sample.lines().take(30) {
        if line.trim().is_empty() {
            continue;
        }
        for (d, s) in scores.iter_mut() {
            *s += line.bytes().filter(|b| *b == *d).count();
        }
    }
    scores
        .into_iter()
        .max_by_key(|(_, s)| *s)
        .map(|(d, _)| d)
        .unwrap_or(b',')
}

fn data_likeness(cells: &[String]) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    let mut score = 0.0;
    for c in cells {
        let t = c.trim();
        if t.is_empty() {
            continue;
        }
        if t.parse::<f64>().is_ok() {
            score += 1.0;
        } else if looks_like_datetime(t) {
            score += 1.0;
        } else if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") {
            score += 0.5;
        } else {
            score -= 0.35;
        }
    }
    score / cells.len() as f32
}

fn looks_like_datetime(s: &str) -> bool {
    let t = s.trim();
    // Require a real date/time shape — not instrument tags like "PT-200 INLET".
    if t.contains(':') && (t.contains('-') || t.contains('/') || t.contains('T')) {
        return true;
    }
    if (t.contains('-') || t.contains('/')) && t.chars().filter(|c| c.is_ascii_digit()).count() >= 6
    {
        return true;
    }
    false
}

fn nonempty_count(cells: &[String]) -> usize {
    cells.iter().filter(|c| !c.trim().is_empty()).count()
}

fn role_for_header_row(cells: &[String], row_idx: usize, data_width: usize) -> HeaderRole {
    let joined = cells.join(" ").to_lowercase();
    let label = cells
        .first()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if label == "units" || label == "unit" || joined.contains('°') || unitish_row(cells) {
        return HeaderRole::Units;
    }
    if label == "node" || label == "nodes" || label == "source" || label == "device" {
        return HeaderRole::Nodes;
    }
    if label == "sensor" || label == "name" || label == "names" || label == "channel" {
        return HeaderRole::Names;
    }
    if label == "dtype" || label == "type" {
        return HeaderRole::DType;
    }
    // Short meta / title rows (OTSG2: "Test 01" / "Operator:") — not channel names.
    // Must be sparse vs data width: a full-width 2–3 col header (wind_laurel) is Names.
    let filled = nonempty_count(cells);
    if filled <= 3 && filled + 2 < data_width {
        return HeaderRole::Title;
    }
    if row_idx == 0 && filled <= 3 && filled < data_width {
        return HeaderRole::Title;
    }
    // Wide identifier rows (instrument tags) are Names even without a "Sensor" label.
    if filled >= (data_width / 2).max(4) && nameish_row(cells) {
        return HeaderRole::Names;
    }
    if row_idx == 0 {
        HeaderRole::Names
    } else {
        HeaderRole::Unknown
    }
}

fn nameish_row(cells: &[String]) -> bool {
    let mut hits = 0usize;
    let mut n = 0usize;
    for c in cells {
        let t = c.trim();
        if t.is_empty() {
            continue;
        }
        n += 1;
        let looks_num = t.parse::<f64>().is_ok();
        let looks_time = looks_like_datetime(t);
        if !looks_num && !looks_time {
            hits += 1;
        }
    }
    n > 0 && hits as f32 / n as f32 > 0.55
}

/// Pick / repair Names role so title/units rows are never used as the name source.
fn finalize_header_roles(roles: &mut [HeaderRole], table: &RawTable, width: usize) {
    for (r, role) in roles.iter_mut().enumerate() {
        let Some(cells) = table.rows.get(r) else {
            continue;
        };
        if unitish_row(cells) {
            *role = HeaderRole::Units;
            continue;
        }
        let filled = nonempty_count(cells);
        if filled <= 3 && cells.len() < width {
            *role = HeaderRole::Title;
        }
    }

    // Never keep Names on a units/title row.
    for (r, role) in roles.iter_mut().enumerate() {
        if *role != HeaderRole::Names {
            continue;
        }
        let Some(cells) = table.rows.get(r) else {
            continue;
        };
        if unitish_row(cells) || nonempty_count(cells) < (width / 3).max(2) {
            *role = if unitish_row(cells) {
                HeaderRole::Units
            } else {
                HeaderRole::Title
            };
        }
    }

    if roles.iter().any(|r| *r == HeaderRole::Names) {
        return;
    }

    // Best wide non-units / non-title header row (instrument tag row).
    let mut best: Option<(usize, f32)> = None;
    for (r, role) in roles.iter().enumerate() {
        if matches!(
            role,
            HeaderRole::Units | HeaderRole::Title | HeaderRole::Skip | HeaderRole::DType
        ) {
            continue;
        }
        let Some(cells) = table.rows.get(r) else {
            continue;
        };
        if unitish_row(cells) {
            continue;
        }
        let filled = nonempty_count(cells) as f32;
        let mut score = filled + if nameish_row(cells) { 40.0 } else { 0.0 };
        let head = cells.first().map(|s| s.to_lowercase()).unwrap_or_default();
        if head.contains("datetime") || head.contains("time") || head.contains("date") {
            score += 25.0;
        }
        if best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((r, score));
        }
    }
    if let Some((r, _)) = best {
        roles[r] = HeaderRole::Names;
    } else if let Some((r, _)) = roles
        .iter()
        .enumerate()
        .rev()
        .find(|(_, role)| !matches!(role, HeaderRole::Units | HeaderRole::Title | HeaderRole::Skip))
    {
        roles[r] = HeaderRole::Names;
    }
}

fn looks_like_unit(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() || t.len() > 16 {
        return false;
    }
    let lower = t.to_lowercase();
    lower.contains('°')
        || lower.contains('/')
        || lower.contains('%')
        || lower.contains("kpa")
        || lower.contains("psi")
        || lower.contains("h2o")
        || lower.contains("mbar")
        || lower.contains("deg")
        || lower == "c"
        || lower == "f"
        || lower == "v"
        || lower == "a"
        || lower == "ma"
        || lower == "rpm"
        || lower == "hz"
        || lower == "kw"
        || lower == "w"
        || lower == "kg"
        || lower == "lbs"
        || lower == "gpm"
        || lower == "lpm"
        || lower == "l/hr"
        || lower == "l/min"
        || lower.starts_with("in.")
}

fn unitish_row(cells: &[String]) -> bool {
    let mut hits = 0usize;
    let mut n = 0usize;
    for c in cells.iter().skip(1) {
        let t = c.trim();
        if t.is_empty() {
            continue;
        }
        n += 1;
        if looks_like_unit(t) {
            hits += 1;
        }
    }
    // Majority of non-empty cells look like units (OTSG2 units row is sparse on the left).
    n >= 2 && hits as f32 / n as f32 >= 0.5
}

/// Detect header depth + data start from a raw string table (first ~N rows).
pub fn detect_parse_plan(table: &RawTable) -> ParsePlan {
    let n = table.rows.len();
    if n == 0 {
        return ParsePlan {
            delimiter: b',',
            header_rows: 0,
            roles: vec![],
            data_start_row: 0,
            width: 0,
            x_axis: XAxis {
                mode: XMode::Index,
                column_index: None,
                confidence: 0.0,
            },
        };
    }

    let widths: Vec<usize> = table.rows.iter().map(|r| r.len()).collect();
    let mut best_w = widths.iter().copied().max().unwrap_or(1);
    let mut best_run = (0usize, 0usize);
    let mut i = 0;
    while i < widths.len() {
        let w = widths[i];
        let mut j = i;
        while j < widths.len() && widths[j] == w {
            j += 1;
        }
        let len = j - i;
        if w >= 2 && len > best_run.1 {
            best_run = (i, len);
            best_w = w;
        }
        i = j;
    }

    let run_start = best_run.0;
    let run_end = best_run.0 + best_run.1;
    let mut data_start = run_start;
    for r in run_start..run_end {
        if data_likeness(&table.rows[r]) > 0.35 {
            data_start = r;
            break;
        }
        data_start = r + 1;
    }
    if data_start >= n {
        data_start = n.saturating_sub(1);
    }

    let header_rows = data_start;
    let mut roles = Vec::with_capacity(header_rows);
    for r in 0..header_rows {
        roles.push(role_for_header_row(&table.rows[r], r, best_w));
    }
    finalize_header_roles(&mut roles, table, best_w);

    let TimeDetect {
        column_index,
        confidence,
        is_time,
    } = detect_time_column(table, data_start, best_w);

    let x_axis = if is_time {
        XAxis {
            mode: XMode::Time,
            column_index,
            confidence,
        }
    } else if let Some(ci) = column_index.filter(|_| confidence > 0.2) {
        XAxis {
            mode: XMode::Column,
            column_index: Some(ci),
            confidence,
        }
    } else {
        XAxis {
            mode: XMode::Index,
            column_index: None,
            confidence: 0.0,
        }
    };

    ParsePlan {
        delimiter: b',',
        header_rows,
        roles,
        data_start_row: data_start,
        width: best_w,
        x_axis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_table() -> RawTable {
        RawTable {
            rows: vec![
                vec![
                    "Sensor".into(),
                    "Temp".into(),
                    "Temp".into(),
                    "Pressure".into(),
                    "Flow".into(),
                ],
                vec![
                    "Node".into(),
                    "TankA".into(),
                    "TankB".into(),
                    "TankA".into(),
                    "Line1".into(),
                ],
                vec![
                    "Units".into(),
                    "°C".into(),
                    "°C".into(),
                    "kPa".into(),
                    "L/min".into(),
                ],
                vec![
                    "2024-10-01T08:00:00".into(),
                    "21.1".into(),
                    "22.0".into(),
                    "101.3".into(),
                    "3.20".into(),
                ],
                vec![
                    "2024-10-01T08:00:01".into(),
                    "21.2".into(),
                    "22.1".into(),
                    "101.2".into(),
                    "3.18".into(),
                ],
            ],
        }
    }

    #[test]
    fn detects_three_header_rows_and_time() {
        let plan = detect_parse_plan(&demo_table());
        assert_eq!(plan.header_rows, 3, "roles={:?}", plan.roles);
        assert_eq!(plan.data_start_row, 3);
        assert_eq!(plan.width, 5);
        assert!(matches!(plan.x_axis.mode, XMode::Time));
        assert_eq!(plan.x_axis.column_index, Some(0));
        assert!(
            plan.roles.iter().any(|r| *r == HeaderRole::Names),
            "roles={:?}",
            plan.roles
        );
        assert!(plan.roles.iter().any(|r| *r == HeaderRole::Nodes));
        assert!(plan.roles.iter().any(|r| *r == HeaderRole::Units));
    }

    #[test]
    fn detects_otsg_style_title_then_names_units() {
        let table = RawTable {
            rows: vec![
                vec!["Test 01:".into(), "OTSG2 LOGDATA".into()],
                vec!["Operator:".into(), "Arnel".into(), "Angeles".into()],
                vec![
                    "DateTime".into(),
                    "Elapsed".into(),
                    "PT-200 INLET".into(),
                    "LIT-200".into(),
                ],
                vec!["".into(), "".into(), "kPa".into(), "in.H2O".into()],
                vec![
                    "9:24:34.354 AM 2019-10-30".into(),
                    "15.7".into(),
                    "-53.1".into(),
                    "14.8".into(),
                ],
                vec![
                    "9:24:44.354 AM 2019-10-30".into(),
                    "15.9".into(),
                    "-53.3".into(),
                    "14.8".into(),
                ],
            ],
        };
        let plan = detect_parse_plan(&table);
        assert_eq!(plan.data_start_row, 4);
        assert!(plan.roles.iter().any(|r| *r == HeaderRole::Title));
        assert!(plan.roles.iter().any(|r| *r == HeaderRole::Names));
        assert!(plan.roles.iter().any(|r| *r == HeaderRole::Units));
        let names_i = plan
            .roles
            .iter()
            .position(|r| *r == HeaderRole::Names)
            .unwrap();
        assert_eq!(names_i, 2, "names must be instrument row, not title");
        assert_eq!(plan.x_axis.column_index, Some(0));
        assert!(matches!(plan.x_axis.mode, XMode::Time));
    }

    #[test]
    fn sniff_comma() {
        let s = "a,b,c\n1,2,3\n";
        assert_eq!(sniff_delimiter(s), b',');
    }

    #[test]
    fn wind_laurel_header_is_names_not_title() {
        let table = RawTable {
            rows: vec![
                vec![
                    "10 Min Std Dev".into(),
                    "Time".into(),
                    "10 Min Sampled Avg".into(),
                ],
                vec!["2.73".into(), "2001-06-11 11:00".into(), "22.3".into()],
                vec!["1.98".into(), "2001-06-11 11:10".into(), "23".into()],
            ],
        };
        let plan = detect_parse_plan(&table);
        assert_eq!(plan.data_start_row, 1);
        assert!(
            plan.roles.first() == Some(&HeaderRole::Names),
            "roles={:?}",
            plan.roles
        );
        assert_eq!(plan.x_axis.column_index, Some(1));
        assert!(
            matches!(plan.x_axis.mode, XMode::Time),
            "expected Time, got {:?}",
            plan.x_axis
        );
    }
}
