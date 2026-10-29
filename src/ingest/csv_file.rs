use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use uuid::Uuid;

use crate::model::{ChannelMeta, Dataset, HeaderRole, ParsePlan, XMode};

use super::detect::{detect_parse_plan, sniff_delimiter, RawTable};
use super::time_detect::parse_time_value;

pub fn load_csv_path(path: &Path) -> anyhow::Result<Dataset> {
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        anyhow::bail!("empty file: {}", path.display());
    }
    let delim = sniff_delimiter(&text);
    let table = parse_raw(&text, delim)?;
    if table.rows.is_empty() {
        anyhow::bail!("no rows parsed: {}", path.display());
    }
    let mut plan = detect_parse_plan(&table);
    plan.delimiter = delim;
    build_dataset(path, table, plan, text.len() as u64)
}

pub fn append_csv_growth(ds: &mut Dataset) -> anyhow::Result<usize> {
    let mut file = File::open(&ds.path)?;
    let meta = file.metadata()?;
    let len = meta.len();
    if len < ds.last_offset {
        // truncated / replaced
        let reloaded = load_csv_path(Path::new(&ds.path))?;
        let follow = ds.follow;
        let channels_visible = ds.channels_visible;
        *ds = reloaded;
        ds.follow = follow;
        ds.channels_visible = channels_visible;
        return Ok(ds.record_count());
    }
    if len == ds.last_offset {
        return Ok(0);
    }
    file.seek(SeekFrom::Start(ds.last_offset))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    // If file didn't end with newline previously, we may have a partial line — keep simple: require lines
    let delim = ds.plan.delimiter;
    let mut added = 0usize;
    for line in buf.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cells = split_line(line, delim);
        if cells.len() < ds.plan.width {
            continue;
        }
        push_data_row(ds, &cells)?;
        added += 1;
    }
    ds.last_offset = len;
    Ok(added)
}

fn parse_raw(text: &str, delim: u8) -> anyhow::Result<RawTable> {
    let mut rows = Vec::new();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delim)
        .flexible(true)
        .from_reader(text.as_bytes());
    for rec in rdr.records() {
        let rec = rec?;
        rows.push(rec.iter().map(|s| s.to_string()).collect());
    }
    Ok(RawTable { rows })
}

fn split_line(line: &str, delim: u8) -> Vec<String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delim)
        .flexible(true)
        .from_reader(line.as_bytes());
    if let Some(Ok(rec)) = rdr.records().next() {
        rec.iter().map(|s| s.to_string()).collect()
    } else {
        line.split(delim as char).map(|s| s.to_string()).collect()
    }
}

fn build_dataset(
    path: &Path,
    table: RawTable,
    plan: ParsePlan,
    last_offset: u64,
) -> anyhow::Result<Dataset> {
    let id = Uuid::new_v4();
    let display_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();

    let names_row = plan.roles.iter().position(|r| *r == HeaderRole::Names);
    let nodes_row = plan.roles.iter().position(|r| *r == HeaderRole::Nodes);
    let units_row = plan.roles.iter().position(|r| *r == HeaderRole::Units);

    let width = plan.width.max(1);
    let mut channels = Vec::new();
    let mut col_map: Vec<Option<usize>> = vec![None; width]; // raw col -> channel col index
    let mut channel_cols: Vec<usize> = Vec::new();

    let x_col = match plan.x_axis.mode {
        XMode::Time | XMode::Column => plan.x_axis.column_index,
        XMode::Index => None,
    };

    for c in 0..width {
        if Some(c) == x_col {
            continue;
        }
        let name = resolve_column_name(&table, &plan, names_row, c);
        // Skip label-like first column if it matches role labels
        if c == 0 && matches_label_name(&name) && plan.header_rows > 0 {
            continue;
        }
        // Skip sparse comment/meta columns with no usable header and little numeric data later
        let node = nodes_row.and_then(|r| cell_at(&table, r, c));
        let units = units_row.and_then(|r| cell_at(&table, r, c));
        let display = ChannelMeta::build_display(
            &name,
            node.as_deref().filter(|s| !s.is_empty()),
            units.as_deref().filter(|s| !s.is_empty()),
        );
        let ch_index = channel_cols.len();
        channel_cols.push(c);
        col_map[c] = Some(ch_index);
        channels.push(ChannelMeta {
            id: Uuid::new_v4(),
            dataset_id: id,
            column_index: ch_index,
            raw_column_index: c,
            name,
            node,
            units,
            display_name: display,
        });
    }

    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); channels.len()];
    let mut x: Vec<f64> = Vec::new();
    let x_is_time = plan.x_axis.mode == XMode::Time;

    for (i, row) in table.rows.iter().enumerate().skip(plan.data_start_row) {
        if row.len() < width.saturating_sub(1) {
            continue;
        }
        let xv = if let Some(xc) = x_col {
            row.get(xc)
                .and_then(|s| parse_time_value(s).or_else(|| parse_number(s)))
                .unwrap_or((i - plan.data_start_row) as f64)
        } else {
            (i - plan.data_start_row) as f64
        };
        x.push(xv);
        for (raw_c, ch_i) in col_map.iter().enumerate() {
            let Some(ch_i) = ch_i else { continue };
            let v = row
                .get(raw_c)
                .and_then(|s| parse_number(s))
                .unwrap_or(f64::NAN);
            columns[*ch_i].push(v);
        }
    }

    Ok(Dataset {
        id,
        path: path.display().to_string(),
        display_name,
        plan,
        channels,
        columns,
        x,
        x_is_time,
        follow: false,
        channels_visible: true,
        last_offset,
    })
}

fn matches_label_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "sensor" | "name" | "names" | "node" | "nodes" | "units" | "unit" | "timestamp" | "time"
    )
}

fn cell_at(table: &RawTable, row: usize, col: usize) -> Option<String> {
    table
        .rows
        .get(row)?
        .get(col)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Prefer Names-role cell; otherwise first useful header cell for this column.
fn resolve_column_name(
    table: &RawTable,
    plan: &ParsePlan,
    names_row: Option<usize>,
    col: usize,
) -> String {
    if let Some(r) = names_row {
        if let Some(s) = cell_at(table, r, col) {
            return s;
        }
    }
    for (ri, role) in plan.roles.iter().enumerate() {
        if matches!(
            role,
            HeaderRole::Units | HeaderRole::Title | HeaderRole::Skip | HeaderRole::DType
        ) {
            continue;
        }
        if let Some(s) = cell_at(table, ri, col) {
            if !matches_label_name(&s) && !looks_like_unit_token(&s) {
                return s;
            }
        }
    }
    // Last resort: any non-empty, non-unit header cell
    for ri in 0..plan.header_rows {
        if matches!(plan.roles.get(ri), Some(HeaderRole::Units | HeaderRole::Title)) {
            continue;
        }
        if let Some(s) = cell_at(table, ri, col) {
            if !looks_like_unit_token(&s) {
                return s;
            }
        }
    }
    format!("col_{}", col + 1)
}

fn looks_like_unit_token(t: &str) -> bool {
    let lower = t.trim().to_lowercase();
    lower.len() <= 16
        && (lower.contains('°')
            || lower.contains('%')
            || lower.contains("kpa")
            || lower.contains("h2o")
            || lower.contains('/')
            || lower.starts_with("in."))
}

fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = t.parse::<f64>() {
        return Some(v);
    }
    // European decimal comma: "2,6" / "-53,15"
    if t.contains(',') && !t.contains('.') {
        return t.replace(',', ".").parse::<f64>().ok();
    }
    None
}

fn push_data_row(ds: &mut Dataset, cells: &[String]) -> anyhow::Result<()> {
    let width = ds.plan.width;
    let x_col = match ds.plan.x_axis.mode {
        XMode::Time | XMode::Column => ds.plan.x_axis.column_index,
        XMode::Index => None,
    };
    let xv = if let Some(xc) = x_col {
        cells
            .get(xc)
            .and_then(|s| parse_time_value(s).or_else(|| parse_number(s)))
            .unwrap_or(ds.x.len() as f64)
    } else {
        ds.x.len() as f64
    };
    ds.x.push(xv);

    let _ = (width, x_col);
    for (i, ch) in ds.channels.iter().enumerate() {
        let v = cells
            .get(ch.raw_column_index)
            .and_then(|s| parse_number(s))
            .unwrap_or(f64::NAN);
        if let Some(col) = ds.columns.get_mut(i) {
            col.push(v);
        }
    }
    Ok(())
}

/// True when a sheet has at least one non-empty cell.
fn sheet_nonempty(rows: &[Vec<String>]) -> bool {
    rows.iter()
        .any(|r| r.iter().any(|c| !c.trim().is_empty()))
}

/// Load every non-empty sheet in an XLSX workbook as its own dataset.
/// Display names are `file.xlsx :: SheetName`.
pub fn load_xlsx_workbook(path: &Path) -> anyhow::Result<Vec<Dataset>> {
    use calamine::{Reader, Xlsx};
    let mut workbook: Xlsx<_> = calamine::open_workbook(path)?;
    let file_stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("workbook.xlsx");
    let meta = std::fs::metadata(path)?;
    let mut out = Vec::new();
    for (sheet_name, range) in workbook.worksheets() {
        let mut rows: Vec<Vec<String>> = Vec::new();
        for row in range.rows() {
            rows.push(row.iter().map(|c| c.to_string()).collect());
        }
        if !sheet_nonempty(&rows) {
            continue;
        }
        let table = RawTable { rows };
        let plan = detect_parse_plan(&table);
        let mut ds = build_dataset(path, table, plan, meta.len())?;
        ds.display_name = format!("{file_stem} :: {sheet_name}");
        out.push(ds);
    }
    if out.is_empty() {
        anyhow::bail!("empty workbook (no non-empty sheets): {}", path.display());
    }
    Ok(out)
}

/// Load first non-empty sheet via calamine (compat wrapper around workbook load).
pub fn load_xlsx_path(path: &Path) -> anyhow::Result<Dataset> {
    let mut sheets = load_xlsx_workbook(path)?;
    Ok(sheets.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_xlsx_single_sheet_sample() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples/excel/seattle_weather.xlsx");
        if !path.exists() {
            return;
        }
        let sheets = load_xlsx_workbook(&path).expect("single-sheet xlsx");
        assert_eq!(sheets.len(), 1);
        assert!(
            sheets[0].display_name.contains("seattle_weather.xlsx ::"),
            "got {}",
            sheets[0].display_name
        );
        assert!(!sheets[0].channels.is_empty());
        assert!(sheets[0].record_count() > 0);
        let one = load_xlsx_path(&path).expect("compat first sheet");
        assert_eq!(one.display_name, sheets[0].display_name);
    }

    #[test]
    fn loads_xlsx_multi_sheet_weather_combo() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples/excel/weather_combo.xlsx");
        if !path.exists() {
            return;
        }
        let sheets = load_xlsx_workbook(&path).expect("multi-sheet xlsx");
        assert!(
            sheets.len() >= 2,
            "expected ≥2 non-empty sheets, got {}",
            sheets.len()
        );
        for ds in &sheets {
            assert!(
                ds.display_name.starts_with("weather_combo.xlsx :: "),
                "got {}",
                ds.display_name
            );
            assert!(!ds.channels.is_empty(), "{}", ds.display_name);
            assert!(ds.record_count() > 0, "{}", ds.display_name);
        }
        let names: Vec<_> = sheets.iter().map(|d| d.display_name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("Seattle")),
            "missing Seattle sheet: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("Vega") || n.contains("Temps")),
            "missing second weather sheet: {names:?}"
        );
    }

    #[test]
    fn loads_demo_multiline_csv() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/demo_multiline.csv");
        let ds = load_csv_path(&path).expect("demo csv");
        assert!(ds.channels.len() >= 3, "channels={}", ds.channels.len());
        assert!(ds.record_count() >= 10, "rows={}", ds.record_count());
        assert!(ds.x_is_time);
        assert!(ds
            .channels
            .iter()
            .any(|c| c.display_name.contains("TankA") || c.name.contains("Temp")));
    }

    #[test]
    fn loads_demo_index_csv() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/demo_index.csv");
        if !path.exists() {
            return;
        }
        let ds = load_csv_path(&path).expect("index csv");
        assert!(!ds.channels.is_empty());
        assert!(ds.record_count() > 0);
    }

    #[test]
    fn loads_otsg2_header_sample_with_real_names() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/otsg2_header_sample.tsv");
        if !path.exists() {
            return;
        }
        let ds = load_csv_path(&path).expect("otsg2 sample");
        assert!(
            ds.channels.iter().any(|c| c.name.contains("PT-200")),
            "expected PT-200 channel, got: {:?}",
            ds.channels
                .iter()
                .take(8)
                .map(|c| &c.display_name)
                .collect::<Vec<_>>()
        );
        assert!(
            !ds.channels
                .iter()
                .take(10)
                .all(|c| c.name.starts_with("col_")),
            "names should not be col_N fallbacks"
        );
        assert!(ds.channels.iter().any(|c| {
            c.units
                .as_deref()
                .map(|u| u.contains("kPa") || u.contains("H2O"))
                .unwrap_or(false)
        }));
    }
}

#[cfg(test)]
mod wind_laurel_probe {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_wind_laurel() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/wind_laurel.csv");
        let ds = load_csv_path(&path).expect("wind_laurel");
        assert_eq!(ds.channels.len(), 2, "Time col excluded from channels");
        assert!(ds.x_is_time, "Time column must be XMode::Time");
        assert!(
            ds.channels
                .iter()
                .any(|c| c.display_name.contains("Std Dev") || c.name.contains("Std Dev")),
            "got names: {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        assert!(
            ds.channels
                .iter()
                .any(|c| c.display_name.contains("Sampled Avg") || c.name.contains("Sampled")),
            "got names: {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        assert!(
            ds.x.iter().any(|x| *x > 9.0e8),
            "X should be unix seconds, got {:?}",
            ds.x.iter().take(3).cloned().collect::<Vec<_>>()
        );
        for ch in &ds.channels {
            let finite = ds.columns[ch.column_index]
                .iter()
                .filter(|y| y.is_finite())
                .count();
            assert!(finite > 0, "channel {} has no finite samples", ch.display_name);
        }
    }
}
