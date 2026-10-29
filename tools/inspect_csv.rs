//! Standalone CSV inspector — compile with rustc (no Cargo deps):
//!   rustc tools/inspect_csv.rs -o /tmp/inspect_csv && /tmp/inspect_csv samples/demo_multiline.csv

use std::env;
use std::fs;
use std::process;

fn sniff_delimiter(sample: &str) -> char {
    let mut scores = [(',', 0usize), (';', 0), ('\t', 0), ('|', 0)];
    for line in sample.lines().take(30) {
        for (d, s) in scores.iter_mut() {
            *s += line.chars().filter(|c| *c == *d).count();
        }
    }
    scores
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(d, _)| *d)
        .unwrap_or(',')
}

fn split_line(line: &str, delim: char) -> Vec<String> {
    line.split(delim).map(|s| s.to_string()).collect()
}

fn data_likeness(cells: &[String]) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    let mut score = 0.0f32;
    for c in cells {
        let t = c.trim();
        if t.is_empty() {
            continue;
        }
        if t.parse::<f64>().is_ok() {
            score += 1.0;
        } else if (t.contains('-') || t.contains('/')) && (t.contains(':') || t.len() >= 8) {
            score += 1.0;
        } else {
            score -= 0.35;
        }
    }
    score / cells.len() as f32
}

fn looks_time(s: &str) -> bool {
    let t = s.trim();
    if t.contains('T') && t.contains('-') {
        return true;
    }
    if t.contains(':') && (t.contains('-') || t.contains('/')) {
        return true;
    }
    if let Ok(v) = t.parse::<f64>() {
        return (1.0e9..2.0e10).contains(&v) || (1.0e12..2.0e13).contains(&v);
    }
    false
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: inspect_csv <file.csv>");
        process::exit(2);
    });
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("read {path}: {e}");
        process::exit(1);
    });
    let delim = sniff_delimiter(&text);
    let rows: Vec<Vec<String>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| split_line(l, delim))
        .collect();
    if rows.is_empty() {
        eprintln!("empty file");
        process::exit(1);
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut data_start = 0usize;
    for (i, r) in rows.iter().enumerate() {
        if data_likeness(r) > 0.35 {
            data_start = i;
            break;
        }
        data_start = i + 1;
    }
    let header_rows = data_start;
    let mut time_col = None;
    if data_start < rows.len() {
        for c in 0..width {
            let mut hits = 0;
            let mut n = 0;
            for r in rows.iter().skip(data_start).take(40) {
                if let Some(cell) = r.get(c) {
                    if cell.trim().is_empty() {
                        continue;
                    }
                    n += 1;
                    if looks_time(cell) {
                        hits += 1;
                    }
                }
            }
            if n > 0 && hits as f32 / n as f32 > 0.6 {
                time_col = Some(c);
                break;
            }
        }
    }

    println!("file: {path}");
    println!("delimiter: {delim:?}");
    println!("width: {width}");
    println!("header_rows: {header_rows}");
    println!("data_start_row: {data_start}");
    println!("x_mode: {}", if time_col.is_some() { "Time" } else { "Index" });
    if let Some(c) = time_col {
        println!("time_column: {c}");
    }
    println!("records: {}", rows.len().saturating_sub(data_start));
    println!("--- header ---");
    for (i, r) in rows.iter().take(header_rows).enumerate() {
        println!("  [{i}] {}", r.join(" | "));
    }
    println!("--- channels (non-X) ---");
    let names = rows.first().cloned().unwrap_or_default();
    for c in 0..width {
        if Some(c) == time_col {
            continue;
        }
        let name = names.get(c).map(|s| s.as_str()).unwrap_or("?");
        if c == 0 && matches!(name.to_lowercase().as_str(), "sensor" | "name" | "names") {
            continue;
        }
        let mut vals = Vec::new();
        for r in rows.iter().skip(data_start) {
            if let Some(v) = r.get(c).and_then(|s| s.trim().parse::<f64>().ok()) {
                vals.push(v);
            }
        }
        let (mn, mx) = vals
            .iter()
            .copied()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
        println!(
            "  col {c}: {name}  n={}  min={mn:.4}  max={mx:.4}",
            vals.len()
        );
    }
}
