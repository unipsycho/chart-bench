use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::channel::ChannelMeta;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XMode {
    Time,
    Index,
    Column,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XAxis {
    pub mode: XMode,
    pub column_index: Option<usize>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderRole {
    Names,
    Nodes,
    Units,
    DType,
    Title,
    Skip,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePlan {
    pub delimiter: u8,
    pub header_rows: usize,
    pub roles: Vec<HeaderRole>,
    pub data_start_row: usize,
    pub width: usize,
    pub x_axis: XAxis,
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub id: Uuid,
    pub path: String,
    pub display_name: String,
    pub plan: ParsePlan,
    pub channels: Vec<ChannelMeta>,
    /// Parallel to channels: numeric samples (f64::NAN = missing)
    pub columns: Vec<Vec<f64>>,
    /// X values (time as seconds since epoch, or index)
    pub x: Vec<f64>,
    pub x_is_time: bool,
    /// Live file-growth watch (append ingest). Independent of channel-list filter.
    pub follow: bool,
    /// When false, this file's channels are hidden from the Channels list (charts keep series).
    pub channels_visible: bool,
    pub last_offset: u64,
}

impl Dataset {
    pub fn record_count(&self) -> usize {
        self.x.len()
    }

    pub fn channel_by_id(&self, id: Uuid) -> Option<&ChannelMeta> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn column_for(&self, id: Uuid) -> Option<&[f64]> {
        let ch = self.channel_by_id(id)?;
        self.columns.get(ch.column_index).map(|c| c.as_slice())
    }
}

/// How series are drawn on a chart pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    #[default]
    Line,
    /// Stair-step line (sample-and-hold).
    Step,
    Points,
    Bar,
    /// Filled field under the curve down to y = 0.
    Area,
}

/// Stroke pattern for line / step / area outlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrokeStyleKind {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// Per-chart draw / style options (persisted with the session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDrawOptions {
    pub kind: ChartKind,
    /// Line / outline thickness.
    pub line_width: f32,
    pub stroke: StrokeStyleKind,
    /// Draw markers on top of line / step / area.
    pub show_markers: bool,
    pub marker_size: f32,
    /// Bar width in plot-X units (auto if <= 0).
    pub bar_width: f32,
    pub show_legend: bool,
    pub show_grid: bool,
    pub show_crosshair: bool,
    /// Horizontal reference at y = 0.
    pub show_zero_line: bool,
    /// Plot panel fill (RGB). Missing in old sessions → default dark.
    #[serde(default = "default_plot_bg")]
    pub background: [u8; 3],
}

fn default_plot_bg() -> [u8; 3] {
    [18, 15, 26]
}

impl Default for ChartDrawOptions {
    fn default() -> Self {
        Self {
            kind: ChartKind::Line,
            line_width: 1.5,
            stroke: StrokeStyleKind::Solid,
            show_markers: false,
            marker_size: 3.0,
            bar_width: 0.0,
            show_legend: true,
            show_grid: true,
            show_crosshair: true,
            show_zero_line: false,
            background: default_plot_bg(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChartState {
    pub id: Uuid,
    pub title: String,
    pub series: Vec<super::channel::SeriesBinding>,
    pub follow_tail: bool,
    pub draw: ChartDrawOptions,
    /// Collapsible draw-options strip under the chart toolbar.
    pub show_draw_options: bool,
}

impl ChartState {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            series: Vec::new(),
            follow_tail: true,
            draw: ChartDrawOptions::default(),
            show_draw_options: false,
        }
    }
}
