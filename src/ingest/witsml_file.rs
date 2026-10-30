//! WITSML 1.x wellLog XML ingest (mnemonic curves → channels).

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::csv_file::dataset_from_raw_table;
use super::detect::RawTable;
use crate::model::Dataset;

/// Load every `wellLog` in a WITSML XML document as its own dataset.
///
/// Uses `mnemonicList` + comma-separated `data` rows. Display names are
/// `file.xml :: <log name>` when multiple logs are present.
pub fn load_witsml_path(path: &Path) -> anyhow::Result<Vec<Dataset>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    let meta_len = text.len() as u64;
    let logs = parse_well_logs(&text)?;
    if logs.is_empty() {
        anyhow::bail!("witsml has no wellLog with data: {}", path.display());
    }

    let file_stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("witsml.xml");
    let multi = logs.len() > 1;
    let mut out = Vec::new();

    for log in logs {
        if log.mnemonics.is_empty() || log.rows.is_empty() {
            continue;
        }
        let mut rows = Vec::with_capacity(log.rows.len() + 1);
        rows.push(log.mnemonics.clone());
        // Optional units row helps display; detector treats short unit tokens as Units role.
        if log.units.iter().any(|u| !u.trim().is_empty())
            && log.units.len() == log.mnemonics.len()
        {
            rows.push(log.units);
        }
        rows.extend(log.rows);

        let table = RawTable { rows };
        let mut ds = dataset_from_raw_table(path, table, meta_len)?;
        let label = if log.name.trim().is_empty() {
            "wellLog".into()
        } else {
            log.name
        };
        ds.display_name = if multi {
            format!("{file_stem} :: {label}")
        } else {
            format!("{file_stem} :: {label}")
        };
        out.push(ds);
    }

    if out.is_empty() {
        anyhow::bail!("witsml logs had no usable curves: {}", path.display());
    }
    Ok(out)
}

struct WellLogData {
    name: String,
    mnemonics: Vec<String>,
    units: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_well_logs(text: &str) -> anyhow::Result<Vec<WellLogData>> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut logs: Vec<WellLogData> = Vec::new();
    let mut in_well_log = false;
    let mut in_log_data = false;
    let mut cur_name = String::new();
    let mut mnemonics: Vec<String> = Vec::new();
    let mut units: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut buf = Vec::new();
    let mut capture: Option<&'static str> = None;
    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "wellLog" => {
                        in_well_log = true;
                        cur_name.clear();
                        mnemonics.clear();
                        units.clear();
                        rows.clear();
                    }
                    "logData" if in_well_log => in_log_data = true,
                    "name" if in_well_log && !in_log_data => {
                        // Top-level wellLog <name>, not nameWell / nameWellbore.
                        // Track via path depth carefully: only capture when parent is wellLog.
                        // Simpler: accept first non-empty <name> that isn't nameWell*.
                        capture = Some("name");
                        text_buf.clear();
                    }
                    "mnemonicList" if in_log_data => {
                        capture = Some("mnemonicList");
                        text_buf.clear();
                    }
                    "unitList" if in_log_data => {
                        capture = Some("unitList");
                        text_buf.clear();
                    }
                    "data" if in_log_data => {
                        capture = Some("data");
                        text_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let _ = local_name(e.name().as_ref());
            }
            Ok(Event::Text(t)) => {
                if capture.is_some() {
                    text_buf.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::CData(t)) => {
                if capture.is_some() {
                    text_buf.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "name" if capture == Some("name") => {
                        if cur_name.is_empty() {
                            cur_name = text_buf.trim().to_string();
                        }
                        capture = None;
                    }
                    "mnemonicList" if capture == Some("mnemonicList") => {
                        mnemonics = split_csv_list(&text_buf);
                        capture = None;
                    }
                    "unitList" if capture == Some("unitList") => {
                        units = split_csv_list(&text_buf);
                        capture = None;
                    }
                    "data" if capture == Some("data") => {
                        let cells = split_csv_list(&text_buf);
                        if !cells.is_empty() {
                            rows.push(cells);
                        }
                        capture = None;
                    }
                    "logData" => in_log_data = false,
                    "wellLog" => {
                        if !mnemonics.is_empty() && !rows.is_empty() {
                            logs.push(WellLogData {
                                name: cur_name.clone(),
                                mnemonics: mnemonics.clone(),
                                units: units.clone(),
                                rows: rows.clone(),
                            });
                        }
                        in_well_log = false;
                        in_log_data = false;
                    }
                    _ => {
                        if matches!(
                            capture,
                            Some("name") | Some("mnemonicList") | Some("unitList") | Some("data")
                        ) && local
                            == capture.unwrap()
                        {
                            // already handled above for matching tags
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!("witsml xml: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(logs)
}

fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    s.rsplit('}')
        .next()
        .unwrap_or(&s)
        .rsplit(':')
        .next()
        .unwrap_or(&s)
        .to_string()
}

fn split_csv_list(s: &str) -> Vec<String> {
    s.split(',').map(|p| p.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_well_log_sample() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples/witsml/well_log_sample.xml");
        if !path.exists() {
            return;
        }
        let sheets = load_witsml_path(&path).expect("well_log_sample.xml");
        assert_eq!(sheets.len(), 1);
        let ds = &sheets[0];
        assert_eq!(ds.record_count(), 21);
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("ROP") || c.display_name.contains("ROP")),
            "got {:?}",
            ds.channels.iter().map(|c| &c.display_name).collect::<Vec<_>>()
        );
        assert!(
            ds.channels
                .iter()
                .any(|c| c.name.contains("WOB") || c.display_name.contains("WOB"))
        );
        // TIME mnemonic should become X
        assert!(
            !ds.channels
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case("TIME")),
            "TIME should be X axis"
        );
        assert!(ds.display_name.contains("Demo Time Log") || ds.display_name.contains("well_log"));
    }
}
