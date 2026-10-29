use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeta {
    pub id: Uuid,
    pub dataset_id: Uuid,
    /// Dense index into Dataset::columns
    pub column_index: usize,
    /// Original CSV/XLS column index (for append / re-parse)
    pub raw_column_index: usize,
    pub name: String,
    pub node: Option<String>,
    pub units: Option<String>,
    pub display_name: String,
}

impl ChannelMeta {
    pub fn build_display(name: &str, node: Option<&str>, units: Option<&str>) -> String {
        let mut s = String::new();
        if let Some(n) = node.filter(|n| !n.is_empty()) {
            s.push_str(n);
            s.push_str(" / ");
        }
        s.push_str(name);
        if let Some(u) = units.filter(|u| !u.is_empty()) {
            s.push_str(" (");
            s.push_str(u);
            s.push(')');
        }
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesBinding {
    pub channel_id: Uuid,
    pub axis: AxisSide,
    pub color: [u8; 3],
    pub visible: bool,
    /// Stable remapping key after reload (channel display_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_key: Option<String>,
    /// Source file path so presets/sessions rebind across new UUIDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Fallback: raw CSV/XLS column index within the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_column: Option<usize>,
}
