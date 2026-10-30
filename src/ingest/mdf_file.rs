//! ASAM MDF4 / MF4 ingest via `falcon_mdf`.

use std::path::Path;

use falcon_mdf::Mf4File;

use super::csv_file::dataset_from_raw_table;
use super::detect::RawTable;
use crate::model::Dataset;

/// Load every channel group with numeric measurement channels as its own dataset.
///
/// Master/time channel becomes X; remaining numeric channels become Y series.
/// Display names: `file.mf4` when one group, else `file.mf4 :: GroupN`.
pub fn load_mdf_path(path: &Path) -> anyhow::Result<Vec<Dataset>> {
    let file = Mf4File::open(path)
        .map_err(|e| anyhow::anyhow!("mdf open {}: {e}", path.display()))?;
    let meta_len = std::fs::metadata(path)?.len();
    let file_stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.mf4");

    // Collect (dg, cg) pairs that have at least one non-master channel.
    let mut groups: Vec<(usize, usize, String)> = Vec::new();
    for (dg_i, dg) in file.data_groups().iter().enumerate() {
        for (cg_i, cg) in dg.channel_groups.iter().enumerate() {
            let has_data = cg.channels.iter().any(|c| !c.is_master());
            if !has_data || cg.sample_count == 0 {
                continue;
            }
            let label = if !cg.acquisition_name.trim().is_empty() {
                cg.acquisition_name.clone()
            } else if !cg.comment.trim().is_empty() {
                cg.comment.clone()
            } else {
                format!("Group{}", groups.len() + 1)
            };
            groups.push((dg_i, cg_i, label));
        }
    }

    if groups.is_empty() {
        anyhow::bail!("mdf has no measurement channel groups: {}", path.display());
    }

    let multi = groups.len() > 1;
    let mut out = Vec::new();

    for (dg_i, cg_i, label) in &groups {
        let cg = &file.data_groups()[*dg_i].channel_groups[*cg_i];
        let master = file.master_channel(*dg_i, *cg_i);

        // Header + columns: time first when present.
        let mut names: Vec<String> = Vec::new();
        let mut columns: Vec<Vec<f64>> = Vec::new();

        let n_samples = cg.sample_count as usize;
        if let Some(m) = master {
            let series = file
                .time_series(m)
                .map_err(|e| anyhow::anyhow!("mdf master '{}': {e}", m.name))?;
            let mut times = series.timestamps().to_vec();
            if times.len() != n_samples {
                // Fall back to signal values if timestamp axis length mismatches.
                times = series.values_f64();
            }
            if times.len() != n_samples {
                times = (0..n_samples).map(|i| i as f64).collect();
            }
            names.push(if m.name.trim().is_empty() {
                "time".into()
            } else {
                m.name.clone()
            });
            columns.push(times);
        } else {
            names.push("index".into());
            columns.push((0..n_samples).map(|i| i as f64).collect());
        }

        for ch in cg.channels.iter().filter(|c| !c.is_master()) {
            let series = match file.time_series(ch) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let vals = series.values_f64();
            if vals.is_empty() {
                continue;
            }
            let mut col = vals;
            if col.len() > n_samples {
                col.truncate(n_samples);
            } else if col.len() < n_samples {
                col.resize(n_samples, f64::NAN);
            }
            names.push(ch.name.clone());
            columns.push(col);
        }

        if columns.len() < 2 {
            // Only time/index — nothing to plot.
            continue;
        }

        let table = columns_to_table(&names, &columns);
        let mut ds = dataset_from_raw_table(path, table, meta_len)?;
        ds.display_name = if multi {
            format!("{file_stem} :: {label}")
        } else {
            file_stem.to_string()
        };
        out.push(ds);
    }

    if out.is_empty() {
        anyhow::bail!("mdf has no numeric channels: {}", path.display());
    }
    Ok(out)
}

fn columns_to_table(names: &[String], columns: &[Vec<f64>]) -> RawTable {
    let n = columns.first().map(|c| c.len()).unwrap_or(0);
    let mut rows = Vec::with_capacity(n + 1);
    rows.push(names.to_vec());
    for i in 0..n {
        rows.push(columns.iter().map(|c| trim_float(c[i])).collect());
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
            .join("samples/mdf")
            .join(name)
    }

    #[test]
    fn loads_vehicle_drive_mf4() {
        let path = sample("vehicle_drive.mf4");
        if !path.exists() {
            return;
        }
        let sheets = load_mdf_path(&path).expect("vehicle_drive.mf4");
        assert_eq!(sheets.len(), 1);
        let ds = &sheets[0];
        assert_eq!(ds.record_count(), 400);
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("VehicleSpeed") || c.display_name.contains("VehicleSpeed")),
            "got {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        assert!(
            !ds.channels.iter().any(|c| c.name.eq_ignore_ascii_case("time")),
            "master time should be X"
        );
    }

    #[test]
    fn loads_power_bus_mf4() {
        let path = sample("power_bus.mf4");
        if !path.exists() {
            return;
        }
        let sheets = load_mdf_path(&path).expect("power_bus.mf4");
        assert_eq!(sheets.len(), 1);
        let ds = &sheets[0];
        assert_eq!(ds.record_count(), 300);
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("BusVoltage") || c.display_name.contains("BusVoltage")),
            "got {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
    }
}
