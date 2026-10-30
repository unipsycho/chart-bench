//! SQLite ingest — each time-series table becomes a dataset (`file :: table`).

use std::path::Path;

use rusqlite::{types::ValueRef, Connection, OpenFlags};

use super::csv_file::dataset_from_raw_table;
use super::detect::{detect_parse_plan, RawTable};
use crate::model::{Dataset, XMode};

/// Load tables that look like time series as separate datasets.
///
/// Prefer tables with a detectable timestamp / time-like column and numeric
/// channels (same multi-sheet UX as XLSX). Tables without a time-like column
/// are included only when no timed tables exist, using index X.
pub fn load_sqlite_path(path: &Path) -> anyhow::Result<Vec<Dataset>> {
    let meta_len = std::fs::metadata(path)?.len();
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| anyhow::anyhow!("sqlite open {}: {e}", path.display()))?;

    let tables = list_user_tables(&conn)?;
    if tables.is_empty() {
        anyhow::bail!("sqlite has no tables: {}", path.display());
    }

    let file_stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("db.sqlite");

    let mut timed: Vec<(String, RawTable, i32)> = Vec::new();
    let mut untimed: Vec<(String, RawTable, i32)> = Vec::new();

    for table in &tables {
        let Some(raw) = read_table(&conn, table)? else {
            continue;
        };
        if raw.rows.len() <= 1 {
            continue;
        }
        let plan = detect_parse_plan(&raw);
        let numeric_channels = plan.width.saturating_sub(match plan.x_axis.mode {
            XMode::Time | XMode::Column => 1,
            XMode::Index => 0,
        });
        if numeric_channels == 0 && plan.width < 2 {
            continue;
        }
        // Score: prefer more rows + time X.
        let score = plan.width as i32 + (raw.rows.len() as i32).min(10_000);
        let is_timed = matches!(plan.x_axis.mode, XMode::Time)
            || (matches!(plan.x_axis.mode, XMode::Column)
                && plan
                    .x_axis
                    .column_index
                    .map(|c| column_name_looks_temporal(&raw, c))
                    .unwrap_or(false));
        if is_timed {
            timed.push((table.clone(), raw, score));
        } else if has_mostly_numeric_data(&raw, &plan) {
            untimed.push((table.clone(), raw, score));
        }
    }

    timed.sort_by(|a, b| b.2.cmp(&a.2));
    untimed.sort_by(|a, b| b.2.cmp(&a.2));

    let chosen: Vec<(String, RawTable)> = if !timed.is_empty() {
        timed.into_iter().map(|(n, t, _)| (n, t)).collect()
    } else {
        untimed.into_iter().map(|(n, t, _)| (n, t)).collect()
    };

    if chosen.is_empty() {
        anyhow::bail!(
            "sqlite has no time-series-like tables: {}",
            path.display()
        );
    }

    let mut out = Vec::new();
    for (table, raw) in chosen {
        let mut ds = dataset_from_raw_table(path, raw, meta_len)?;
        ds.display_name = format!("{file_stem} :: {table}");
        out.push(ds);
    }
    Ok(out)
}

fn list_user_tables(conn: &Connection) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut names = Vec::new();
    for r in rows {
        names.push(r?);
    }
    Ok(names)
}

fn read_table(conn: &Connection, table: &str) -> anyhow::Result<Option<RawTable>> {
    // Quote identifier safely (reject weird names).
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Ok(None);
    }
    let sql = format!("SELECT * FROM \"{table}\"");
    let mut stmt = conn.prepare(&sql)?;
    let col_count = stmt.column_count();
    if col_count == 0 {
        return Ok(None);
    }
    let header: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("col").to_string())
        .collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(header);

    let mut query = stmt.query([])?;
    while let Some(row) = query.next()? {
        let mut cells = Vec::with_capacity(col_count);
        for i in 0..col_count {
            cells.push(value_to_cell(row.get_ref(i)?));
        }
        rows.push(cells);
    }
    Ok(Some(RawTable { rows }))
}

fn value_to_cell(v: ValueRef<'_>) -> String {
    match v {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => {
            if !f.is_finite() {
                String::new()
            } else {
                let s = format!("{f}");
                if s.contains('e') || s.contains('E') {
                    format!("{f:.15}")
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                } else {
                    s
                }
            }
        }
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(_) => String::new(),
    }
}

fn column_name_looks_temporal(table: &RawTable, col: usize) -> bool {
    let name = table
        .rows
        .first()
        .and_then(|r| r.get(col))
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    const CUES: &[&str] = &[
        "timestamp",
        "datetime",
        "date_time",
        "time",
        "date",
        "epoch",
        "unix",
        "ts",
        "t",
    ];
    CUES.iter().any(|c| name == *c || name.contains(c))
}

fn has_mostly_numeric_data(table: &RawTable, plan: &crate::model::ParsePlan) -> bool {
    if table.rows.len() <= plan.data_start_row {
        return false;
    }
    let mut nums = 0usize;
    let mut total = 0usize;
    for row in table.rows.iter().skip(plan.data_start_row).take(40) {
        for (i, cell) in row.iter().enumerate().take(plan.width) {
            if Some(i) == plan.x_axis.column_index {
                continue;
            }
            total += 1;
            let t = cell.trim();
            if t.parse::<f64>().is_ok()
                || (t.contains(',') && !t.contains('.') && t.replace(',', ".").parse::<f64>().is_ok())
            {
                nums += 1;
            }
        }
    }
    total > 0 && (nums as f32 / total as f32) > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_plant_logs_sqlite_as_two_tables() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples/sqlite/plant_logs.sqlite");
        if !path.exists() {
            return;
        }
        let sheets = load_sqlite_path(&path).expect("plant_logs.sqlite");
        assert_eq!(sheets.len(), 2, "expected process_log + vibration_log");
        for ds in &sheets {
            assert!(
                ds.display_name.starts_with("plant_logs.sqlite :: "),
                "got {}",
                ds.display_name
            );
            assert!(ds.x_is_time, "{} should use time X", ds.display_name);
            assert!(!ds.channels.is_empty(), "{}", ds.display_name);
            assert!(ds.record_count() > 0, "{}", ds.display_name);
        }
        let names: Vec<_> = sheets.iter().map(|d| d.display_name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("process_log")), "{names:?}");
        assert!(
            names.iter().any(|n| n.contains("vibration_log")),
            "{names:?}"
        );
    }
}
