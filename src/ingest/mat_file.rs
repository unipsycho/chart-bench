//! MATLAB Level-5 `.mat` ingest (numeric arrays via `matfile`).

use std::fs::File;
use std::path::Path;

use matfile::{MatFile, NumericData};

use super::csv_file::dataset_from_raw_table;
use super::detect::RawTable;
use crate::model::Dataset;

/// Load numeric column vectors from a Level-5 MAT file.
///
/// Prefers a time-like variable (`time`, `time_s`, `t`, …) as X; otherwise uses
/// sample index. Non-numeric / complex-only arrays are skipped.
pub fn load_mat_path(path: &Path) -> anyhow::Result<Dataset> {
    let file = File::open(path).map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let meta_len = file.metadata()?.len();
    let mat = MatFile::parse(file).map_err(|e| anyhow::anyhow!("mat parse {}: {e}", path.display()))?;

    let mut names: Vec<String> = Vec::new();
    let mut columns: Vec<Vec<f64>> = Vec::new();

    for array in mat.arrays() {
        let Some(col) = numeric_column(array.data(), array.size()) else {
            continue;
        };
        if col.is_empty() {
            continue;
        }
        names.push(array.name().to_string());
        columns.push(col);
    }

    if columns.is_empty() {
        anyhow::bail!("mat has no numeric channels: {}", path.display());
    }

    // Align to shortest channel length (ragged MAT vars are uncommon but safe).
    let n = columns.iter().map(|c| c.len()).min().unwrap_or(0);
    if n == 0 {
        anyhow::bail!("mat numeric channels are empty: {}", path.display());
    }
    for c in &mut columns {
        c.truncate(n);
    }

    let table = columns_to_table(&names, &columns);
    dataset_from_raw_table(path, table, meta_len)
}

/// Extract a real-valued column vector (Nx1 / 1xN / flat 1-D).
fn numeric_column(data: &NumericData, size: &[usize]) -> Option<Vec<f64>> {
    let real: Vec<f64> = match data {
        NumericData::Double { real, imag } if imag.is_none() => real.clone(),
        NumericData::Single { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::Int8 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::UInt8 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::Int16 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::UInt16 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::Int32 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::UInt32 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::Int64 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        NumericData::UInt64 { real, imag } if imag.is_none() => {
            real.iter().map(|v| *v as f64).collect()
        }
        _ => return None,
    };

    // Accept column/row vectors and flat length-N; skip 2-D+ matrices for now.
    let elems: usize = size.iter().product();
    if elems != real.len() {
        return None;
    }
    let non_one: Vec<usize> = size.iter().copied().filter(|&d| d > 1).collect();
    if non_one.len() > 1 {
        return None;
    }
    Some(real)
}

fn columns_to_table(names: &[String], columns: &[Vec<f64>]) -> RawTable {
    let n = columns.first().map(|c| c.len()).unwrap_or(0);
    let mut rows = Vec::with_capacity(n + 1);
    rows.push(names.to_vec());
    for i in 0..n {
        rows.push(
            columns
                .iter()
                .map(|c| trim_float(c[i]))
                .collect(),
        );
    }
    RawTable { rows }
}

fn trim_float(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let s = format!("{v}");
    if s.contains('e') || s.contains('E') {
        format!("{v:.15}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples/matlab")
            .join(name)
    }

    #[test]
    fn loads_plant_process_mat() {
        let path = sample("plant_process.mat");
        if !path.exists() {
            return;
        }
        let ds = load_mat_path(&path).expect("plant_process.mat");
        assert_eq!(ds.record_count(), 240);
        assert!(ds.channels.len() >= 3, "channels={}", ds.channels.len());
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("pressure") || c.display_name.contains("pressure")),
            "got {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        // time_s should be X (column mode or time), not a plotted channel
        assert!(
            !ds.channels.iter().any(|c| c.name == "time_s"),
            "time_s should be X, not a channel"
        );
        assert!(
            matches!(
                ds.plan.x_axis.mode,
                crate::model::XMode::Column | crate::model::XMode::Time
            ),
            "mode={:?}",
            ds.plan.x_axis.mode
        );
    }

    #[test]
    fn loads_motor_vibration_mat() {
        let path = sample("motor_vibration.mat");
        if !path.exists() {
            return;
        }
        let ds = load_mat_path(&path).expect("motor_vibration.mat");
        assert_eq!(ds.record_count(), 500);
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("accel") || c.display_name.contains("accel")),
            "got {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("rpm") || c.display_name.contains("rpm"))
        );
    }
}
