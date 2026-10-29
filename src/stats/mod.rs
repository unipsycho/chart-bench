use crate::model::Dataset;

#[derive(Debug, Clone, Default)]
pub struct FileStats {
    pub records: usize,
    pub channels: usize,
    pub x_min: f64,
    pub x_max: f64,
    pub x_is_time: bool,
    pub median_dx: f64,
    pub spacing_class: &'static str,
    pub gap_count: usize,
    pub largest_gap: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelStats {
    pub n: usize,
    pub missing: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

pub fn file_stats(ds: &Dataset) -> FileStats {
    let records = ds.record_count();
    let (x_min, x_max) = if ds.x.is_empty() {
        (0.0, 0.0)
    } else {
        let mut mn = f64::INFINITY;
        let mut mx = f64::NEG_INFINITY;
        for &v in &ds.x {
            mn = mn.min(v);
            mx = mx.max(v);
        }
        (mn, mx)
    };

    let mut diffs: Vec<f64> = Vec::new();
    for w in ds.x.windows(2) {
        let d = w[1] - w[0];
        if d.is_finite() && d >= 0.0 {
            diffs.push(d);
        }
    }
    diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_dx = if diffs.is_empty() {
        0.0
    } else {
        diffs[diffs.len() / 2]
    };
    let mut gap_count = 0usize;
    let mut largest_gap: f64 = 0.0;
    let thr = if median_dx > 0.0 {
        median_dx * 5.0
    } else {
        f64::INFINITY
    };
    for &d in &diffs {
        if d > thr {
            gap_count += 1;
            largest_gap = largest_gap.max(d);
        }
    }
    let spacing_class = if diffs.len() < 3 {
        "unknown"
    } else {
        let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let var = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / diffs.len() as f64;
        let cv = if mean > 1e-12 {
            var.sqrt() / mean
        } else {
            0.0
        };
        if cv < 0.05 && gap_count == 0 {
            "regular"
        } else if cv < 0.25 {
            "mostly regular"
        } else {
            "sporadic"
        }
    };

    FileStats {
        records,
        channels: ds.channels.len(),
        x_min,
        x_max,
        x_is_time: ds.x_is_time,
        median_dx,
        spacing_class,
        gap_count,
        largest_gap,
    }
}

pub fn channel_stats(values: &[f64]) -> ChannelStats {
    let mut n = 0usize;
    let mut missing = 0usize;
    let mut sum = 0.0;
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in values {
        if v.is_finite() {
            n += 1;
            sum += v;
            min = min.min(v);
            max = max.max(v);
        } else {
            missing += 1;
        }
    }
    ChannelStats {
        n,
        missing,
        min: if n > 0 { min } else { f64::NAN },
        max: if n > 0 { max } else { f64::NAN },
        mean: if n > 0 { sum / n as f64 } else { f64::NAN },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChannelMeta, Dataset, ParsePlan, XAxis, XMode};
    use uuid::Uuid;

    fn tiny_ds(x: Vec<f64>, col: Vec<f64>) -> Dataset {
        let id = Uuid::nil();
        Dataset {
            id,
            path: "t.csv".into(),
            display_name: "t".into(),
            plan: ParsePlan {
                delimiter: b',',
                header_rows: 1,
                roles: vec![],
                data_start_row: 1,
                width: 2,
                x_axis: XAxis {
                    mode: XMode::Index,
                    column_index: None,
                    confidence: 0.0,
                },
            },
            channels: vec![ChannelMeta {
                id: Uuid::from_u128(1),
                dataset_id: id,
                column_index: 0,
                raw_column_index: 1,
                name: "a".into(),
                node: None,
                units: None,
                display_name: "a".into(),
            }],
            columns: vec![col],
            x,
            x_is_time: false,
            follow: false,
            channels_visible: true,
            last_offset: 0,
        }
    }

    #[test]
    fn regular_spacing() {
        let ds = tiny_ds(vec![0.0, 1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0, 4.0]);
        let st = file_stats(&ds);
        assert_eq!(st.spacing_class, "regular");
        assert_eq!(st.gap_count, 0);
    }

    #[test]
    fn channel_mean() {
        let cs = channel_stats(&[1.0, 2.0, 3.0, f64::NAN]);
        assert_eq!(cs.n, 3);
        assert_eq!(cs.missing, 1);
        assert!((cs.mean - 2.0).abs() < 1e-9);
    }
}

pub fn sparkline(values: &[f64], out_len: usize) -> Vec<f64> {
    if values.is_empty() || out_len == 0 {
        return vec![0.0; out_len];
    }
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let start = i * values.len() / out_len;
        let end = ((i + 1) * values.len() / out_len).max(start + 1);
        let slice = &values[start..end.min(values.len())];
        let mut sum = 0.0;
        let mut n = 0;
        for &v in slice {
            if v.is_finite() {
                sum += v;
                n += 1;
            }
        }
        out.push(if n > 0 { sum / n as f64 } else { f64::NAN });
    }
    out
}
