use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::channel::{AxisSide, SeriesBinding};
use super::dataset::{ChartDrawOptions, ParsePlan};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionFile {
    pub path: String,
    pub follow: bool,
    /// Channel-list filter: unchecked hides this file's channels (series stay on charts).
    #[serde(default = "default_true")]
    pub channels_visible: bool,
    pub plan: Option<ParsePlan>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionChart {
    pub id: Uuid,
    pub title: String,
    pub series: Vec<SeriesBinding>,
    pub follow_tail: bool,
    #[serde(default)]
    pub draw: ChartDrawOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChartLayout {
    /// Multiple chart panes share one overlay stack (default: one chart fills the canvas).
    #[default]
    Overlay,
    /// Chart panes stacked vertically.
    Stacked,
    /// Chart panes side-by-side.
    SideBySide,
}

/// Nested chart tiling axis (Favnyr-style binary splits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileSplit {
    /// Top / bottom (horizontal divider).
    Rows,
    /// Left / right (vertical divider).
    Cols,
}

fn half_ratio() -> f32 {
    0.5
}

/// Binary tile tree: leaves are charts; splits nest Rows/Cols arbitrarily
/// (e.g. one on top + two side-by-side below).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChartTileTree {
    Leaf {
        chart_id: Uuid,
    },
    Split {
        dir: TileSplit,
        #[serde(default = "half_ratio")]
        ratio: f32,
        first: Box<ChartTileTree>,
        second: Box<ChartTileTree>,
    },
}

impl ChartTileTree {
    pub fn leaf(chart_id: Uuid) -> Self {
        Self::Leaf { chart_id }
    }

    /// Flat tree from chart ids + legacy layout hint.
    pub fn from_chart_ids(ids: &[Uuid], layout: ChartLayout) -> Self {
        if ids.is_empty() {
            return Self::leaf(Uuid::nil());
        }
        if ids.len() == 1 || matches!(layout, ChartLayout::Overlay) {
            return Self::leaf(ids[0]);
        }
        let dir = match layout {
            ChartLayout::SideBySide => TileSplit::Cols,
            _ => TileSplit::Rows,
        };
        let mut node = Self::leaf(ids[0]);
        for &id in &ids[1..] {
            node = Self::Split {
                dir,
                ratio: 0.5,
                first: Box::new(node),
                second: Box::new(Self::leaf(id)),
            };
        }
        // Balance ratios along a chain so leaves share space evenly
        node.rebalance_chain(ids.len());
        node
    }

    fn rebalance_chain(&mut self, total_leaves: usize) {
        if total_leaves <= 1 {
            return;
        }
        // Walk left-spine Splits and set ratio so each leaf gets 1/n of remaining
        let mut remaining = total_leaves;
        let mut cur = self;
        while let Self::Split { ratio, first, second, .. } = cur {
            *ratio = 1.0 / remaining as f32;
            remaining -= 1;
            // Prefer walking the side that still has splits
            if matches!(second.as_ref(), Self::Split { .. }) {
                cur = second.as_mut();
            } else if matches!(first.as_ref(), Self::Split { .. }) {
                cur = first.as_mut();
            } else {
                break;
            }
        }
    }

    pub fn collect_ids(&self, out: &mut Vec<Uuid>) {
        match self {
            Self::Leaf { chart_id } => out.push(*chart_id),
            Self::Split { first, second, .. } => {
                first.collect_ids(out);
                second.collect_ids(out);
            }
        }
    }

    pub fn contains(&self, id: Uuid) -> bool {
        match self {
            Self::Leaf { chart_id } => *chart_id == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Swap two leaf chart ids (no-op if either missing).
    pub fn swap_leaves(&mut self, a: Uuid, b: Uuid) {
        if a == b {
            return;
        }
        self.map_leaf_ids(|id| {
            if id == a {
                b
            } else if id == b {
                a
            } else {
                id
            }
        });
    }

    pub fn map_leaf_ids(&mut self, mut f: impl FnMut(Uuid) -> Uuid) {
        self.map_leaf_ids_dyn(&mut f);
    }

    fn map_leaf_ids_dyn(&mut self, f: &mut dyn FnMut(Uuid) -> Uuid) {
        match self {
            Self::Leaf { chart_id } => *chart_id = f(*chart_id),
            Self::Split { first, second, .. } => {
                first.map_leaf_ids_dyn(f);
                second.map_leaf_ids_dyn(f);
            }
        }
    }

    /// Replace leaf `target` with a split placing `incoming` on `edge`.
    /// Returns false if target leaf not found.
    pub fn split_at(
        &mut self,
        target: Uuid,
        incoming: Uuid,
        edge: TileEdge,
    ) -> bool {
        match self {
            Self::Leaf { chart_id } if *chart_id == target => {
                let keep = Self::leaf(target);
                let neu = Self::leaf(incoming);
                let (dir, first, second) = match edge {
                    TileEdge::North => (TileSplit::Rows, neu, keep),
                    TileEdge::South => (TileSplit::Rows, keep, neu),
                    TileEdge::West => (TileSplit::Cols, neu, keep),
                    TileEdge::East => (TileSplit::Cols, keep, neu),
                };
                *self = Self::Split {
                    dir,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                first.split_at(target, incoming, edge) || second.split_at(target, incoming, edge)
            }
        }
    }

    /// Remove a leaf; collapse single-child splits. Returns true if found.
    pub fn remove_leaf(&mut self, id: Uuid) -> bool {
        if self.is_leaf_id(id) {
            *self = Self::leaf(Uuid::nil());
            return true;
        }
        match self {
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                if first.is_leaf_id(id) {
                    *self = second.as_ref().clone();
                    return true;
                }
                if second.is_leaf_id(id) {
                    *self = first.as_ref().clone();
                    return true;
                }
                first.remove_leaf(id) || second.remove_leaf(id)
            }
        }
    }

    fn is_leaf_id(&self, id: Uuid) -> bool {
        matches!(self, Self::Leaf { chart_id } if *chart_id == id)
    }

    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf { chart_id } if *chart_id != Uuid::nil() => 1,
            Self::Leaf { .. } => 0,
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// Update split ratio at a path of first/second steps (`false` = first, `true` = second).
    pub fn set_ratio_at(&mut self, path: &[bool], ratio: f32) -> bool {
        let ratio = ratio.clamp(0.15, 0.85);
        if path.is_empty() {
            return match self {
                Self::Split { ratio: r, .. } => {
                    *r = ratio;
                    true
                }
                Self::Leaf { .. } => false,
            };
        }
        match self {
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                let child = if path[0] { second.as_mut() } else { first.as_mut() };
                child.set_ratio_at(&path[1..], ratio)
            }
        }
    }

    /// Ensure every chart id appears exactly once; append missing as stacked at root.
    pub fn sync_with_charts(&mut self, charts: &[super::dataset::ChartState], layout: ChartLayout) {
        let valid: std::collections::HashSet<Uuid> = charts.iter().map(|c| c.id).collect();
        if valid.is_empty() {
            *self = Self::leaf(Uuid::nil());
            return;
        }
        // Drop leaves whose charts are gone
        while let Some(dead) = {
            let mut ids = Vec::new();
            self.collect_ids(&mut ids);
            ids.into_iter().find(|id| !valid.contains(id))
        } {
            if !self.remove_leaf(dead) {
                break;
            }
        }
        // If tree empty / nil after prune, rebuild
        let mut present = Vec::new();
        self.collect_ids(&mut present);
        present.retain(|id| valid.contains(id) && *id != Uuid::nil());
        if present.is_empty() {
            let ids: Vec<Uuid> = charts.iter().map(|c| c.id).collect();
            *self = Self::from_chart_ids(&ids, layout);
            return;
        }
        // Append charts missing from the tree (stacked at root)
        for c in charts {
            if !self.contains(c.id) {
                let old = self.clone();
                *self = Self::Split {
                    dir: TileSplit::Rows,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(Self::leaf(c.id)),
                };
            }
        }
    }
}

/// Cardinal edge for nested tile splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileEdge {
    North,
    South,
    West,
    East,
}

impl std::fmt::Display for TileEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::North => write!(f, "top"),
            Self::South => write!(f, "bottom"),
            Self::West => write!(f, "left"),
            Self::East => write!(f, "right"),
        }
    }
}

/// How Y series share axes inside a chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum YAxisMode {
    /// All series on one shared Y scale (single plot).
    #[default]
    Overlay,
    /// Left/right series on one plot with dual Y axes (independent scales).
    Multi,
}

/// Dockable workbench panes (Favnyr-style leaves; charts stay central).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockPane {
    Files,
    Channels,
    Info,
    Saved,
    Table,
}

impl DockPane {
    pub fn title(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Channels => "Channels",
            Self::Info => "Info",
            Self::Saved => "Saved",
            Self::Table => "Table",
        }
    }

    /// Header glyph (matches icon rail). Table uses a painted grid in the UI.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Files => "📁",
            Self::Channels => "📶",
            Self::Info => "ℹ",
            Self::Saved => "★",
            // Fallback only — UI paints a 3×2 grid (box glyphs often missing).
            Self::Table => "#",
        }
    }
}

/// Column stacks for dockable panes. Charts remain in the center canvas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockLayout {
    /// Left column, top → bottom.
    #[serde(default = "default_left_stack")]
    pub left: Vec<DockPane>,
    /// Right column, top → bottom (Info + Saved stacked by default).
    #[serde(default = "default_right_stack")]
    pub right: Vec<DockPane>,
    /// Bottom strip panes (Table).
    #[serde(default = "default_bottom_stack")]
    pub bottom: Vec<DockPane>,
}

fn default_left_stack() -> Vec<DockPane> {
    vec![DockPane::Files, DockPane::Channels]
}
fn default_right_stack() -> Vec<DockPane> {
    vec![DockPane::Info, DockPane::Saved]
}
fn default_bottom_stack() -> Vec<DockPane> {
    vec![DockPane::Table]
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            left: default_left_stack(),
            right: default_right_stack(),
            bottom: default_bottom_stack(),
        }
    }
}

impl DockLayout {
    /// Ensure every pane appears exactly once across stacks.
    pub fn normalize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let filter = |v: &mut Vec<DockPane>, seen: &mut std::collections::HashSet<DockPane>| {
            v.retain(|p| seen.insert(*p));
        };
        filter(&mut self.left, &mut seen);
        filter(&mut self.right, &mut seen);
        filter(&mut self.bottom, &mut seen);
        for p in [
            DockPane::Files,
            DockPane::Channels,
            DockPane::Info,
            DockPane::Saved,
            DockPane::Table,
        ] {
            if !seen.contains(&p) {
                match p {
                    DockPane::Table => self.bottom.push(p),
                    DockPane::Info | DockPane::Saved => self.right.push(p),
                    _ => self.left.push(p),
                }
            }
        }
    }

    pub fn move_pane(&mut self, pane: DockPane, dest: DockColumn, index: usize) {
        self.left.retain(|p| *p != pane);
        self.right.retain(|p| *p != pane);
        self.bottom.retain(|p| *p != pane);
        let stack = match dest {
            DockColumn::Left => &mut self.left,
            DockColumn::Right => &mut self.right,
            DockColumn::Bottom => &mut self.bottom,
        };
        let i = index.min(stack.len());
        stack.insert(i, pane);
        self.normalize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockColumn {
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFlags {
    pub show_files: bool,
    pub show_channels: bool,
    pub show_info: bool,
    pub show_table: bool,
    pub show_saved: bool,
    #[serde(default)]
    pub chart_layout: ChartLayout,
    /// Nested Favnyr tile tree (overrides flat chart_layout when present).
    #[serde(default)]
    pub chart_tree: Option<ChartTileTree>,
    #[serde(default)]
    pub y_axis_mode: YAxisMode,
    /// Favnyr-ish column stacks (persist with session).
    #[serde(default)]
    pub dock: DockLayout,
}

impl Default for UiFlags {
    fn default() -> Self {
        Self {
            show_files: true,
            show_channels: true,
            show_info: true,
            show_table: false,
            show_saved: true,
            chart_layout: ChartLayout::Overlay,
            chart_tree: None,
            y_axis_mode: YAxisMode::Overlay,
            dock: DockLayout::default(),
        }
    }
}

impl UiFlags {
    pub fn pane_visible(&self, pane: DockPane) -> bool {
        match pane {
            DockPane::Files => self.show_files,
            DockPane::Channels => self.show_channels,
            DockPane::Info => self.show_info,
            DockPane::Saved => self.show_saved,
            DockPane::Table => self.show_table,
        }
    }

    pub fn set_pane_visible(&mut self, pane: DockPane, on: bool) {
        match pane {
            DockPane::Files => self.show_files = on,
            DockPane::Channels => self.show_channels = on,
            DockPane::Info => self.show_info = on,
            DockPane::Saved => self.show_saved = on,
            DockPane::Table => self.show_table = on,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Session {
    pub files: Vec<SessionFile>,
    pub charts: Vec<SessionChart>,
    pub ui: UiFlags,
    /// Recently opened paths (most-recent first), capped at ~25; may include closed files.
    #[serde(default)]
    pub recent_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSlot {
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// Open files (paths) that belong to this preset.
    pub slots: Vec<PresetSlot>,
    pub charts: Vec<SessionChart>,
    /// Chart tile layout when the preset was saved.
    #[serde(default)]
    pub chart_layout: ChartLayout,
    /// Nested tile tree (preferred over flat chart_layout).
    #[serde(default)]
    pub chart_tree: Option<ChartTileTree>,
    /// Dual-Y vs shared-Y mode.
    #[serde(default)]
    pub y_axis_mode: YAxisMode,
}

pub fn session_path() -> PathBuf {
    dirs_next_data().join("csv-bench").join("session.toml")
}

pub fn presets_path() -> PathBuf {
    dirs_next_data().join("csv-bench").join("presets.toml")
}

fn dirs_next_data() -> PathBuf {
    if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(p);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share");
    }
    PathBuf::from(".").join("csv-bench-data")
}

pub fn save_session(session: &Session) -> anyhow::Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(session)?;
    std::fs::write(path, s)?;
    Ok(())
}

pub fn load_session() -> Option<Session> {
    let path = session_path();
    let bytes = std::fs::read_to_string(path).ok()?;
    toml::from_str(&bytes).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetLibrary {
    pub presets: Vec<Preset>,
}

pub fn load_presets() -> PresetLibrary {
    let path = presets_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_presets(lib: &PresetLibrary) -> anyhow::Result<()> {
    let path = presets_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(lib)?)?;
    Ok(())
}

pub fn default_series_color(i: usize) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [
        [66, 135, 245],
        [245, 102, 66],
        [66, 200, 120],
        [200, 66, 180],
        [240, 180, 50],
        [50, 200, 200],
        [180, 80, 80],
        [120, 120, 220],
    ];
    PALETTE[i % PALETTE.len()]
}

pub fn binding(channel_id: Uuid, index: usize, axis: AxisSide) -> SeriesBinding {
    SeriesBinding {
        channel_id,
        axis,
        color: default_series_color(index),
        visible: true,
        channel_key: None,
        file_path: None,
        raw_column: None,
    }
}

/// Stamp path / display / column keys onto a binding for preset/session remapping.
pub fn stamp_series_keys(
    binding: &mut SeriesBinding,
    file_path: &str,
    display_name: &str,
    raw_column: usize,
) {
    binding.file_path = Some(file_path.to_string());
    binding.channel_key = Some(display_name.to_string());
    binding.raw_column = Some(raw_column);
}

/// Match a saved series to a live channel after files were reopened (new UUIDs).
pub fn resolve_channel_id(
    datasets: &[super::dataset::Dataset],
    binding: &SeriesBinding,
) -> Option<Uuid> {
    // Fast path: id still valid in the current session
    for ds in datasets {
        if ds.channel_by_id(binding.channel_id).is_some() {
            return Some(binding.channel_id);
        }
    }

    let want_path = binding.file_path.as_deref();
    let want_key = binding.channel_key.as_deref();
    let want_col = binding.raw_column;

    let path_matches = |ds: &super::dataset::Dataset| -> bool {
        let Some(want) = want_path else {
            return true; // no path stored — allow any file
        };
        paths_equivalent(&ds.path, want)
    };

    // Prefer display_name within matching file(s)
    if let Some(key) = want_key {
        for ds in datasets.iter().filter(|d| path_matches(d)) {
            if let Some(ch) = ds.channels.iter().find(|c| c.display_name == key) {
                return Some(ch.id);
            }
        }
        // Fall back: name-only match in matching files
        for ds in datasets.iter().filter(|d| path_matches(d)) {
            if let Some(ch) = ds.channels.iter().find(|c| c.name == key || c.display_name == key)
            {
                return Some(ch.id);
            }
        }
    }

    // Column-index fallback within matching file
    if let Some(col) = want_col {
        for ds in datasets.iter().filter(|d| path_matches(d)) {
            if let Some(ch) = ds.channels.iter().find(|c| c.raw_column_index == col) {
                return Some(ch.id);
            }
        }
    }

    // Last resort: key match across any open file (slot reorder / path change)
    if let Some(key) = want_key {
        for ds in datasets {
            if let Some(ch) = ds.channels.iter().find(|c| c.display_name == key) {
                return Some(ch.id);
            }
        }
    }
    None
}

fn paths_equivalent(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let na = std::path::Path::new(a);
    let nb = std::path::Path::new(b);
    if na.file_name() == nb.file_name() && na.file_name().is_some() {
        return true;
    }
    // Tolerate relative vs absolute
    na.canonicalize()
        .ok()
        .zip(nb.canonicalize().ok())
        .map(|(ca, cb)| ca == cb)
        .unwrap_or(false)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChannelMeta, Dataset, HeaderRole, ParsePlan, XAxis, XMode};

    fn fake_ds(path: &str, chans: &[(&str, usize)]) -> Dataset {
        let id = uuid::Uuid::new_v4();
        let channels: Vec<ChannelMeta> = chans
            .iter()
            .enumerate()
            .map(|(i, (name, raw))| ChannelMeta {
                id: uuid::Uuid::new_v4(),
                dataset_id: id,
                column_index: i,
                raw_column_index: *raw,
                name: (*name).into(),
                node: None,
                units: None,
                display_name: (*name).into(),
            })
            .collect();
        Dataset {
            id,
            path: path.into(),
            display_name: "t".into(),
            plan: ParsePlan {
                delimiter: b',',
                header_rows: 1,
                roles: vec![HeaderRole::Names],
                data_start_row: 1,
                width: chans.len() + 1,
                x_axis: XAxis {
                    mode: XMode::Index,
                    column_index: None,
                    confidence: 0.0,
                },
            },
            columns: channels.iter().map(|_| vec![1.0]).collect(),
            x: vec![0.0],
            x_is_time: false,
            follow: false,
            channels_visible: true,
            last_offset: 0,
            channels,
        }
    }

    #[test]
    fn remap_by_path_and_display_keeps_axis() {
        let ds = fake_ds("/tmp/a.csv", &[("Temp", 1), ("Flow", 2)]);
        let flow_id = ds.channels[1].id;
        let mut b = binding(uuid::Uuid::new_v4(), 0, AxisSide::Right);
        stamp_series_keys(&mut b, "/tmp/a.csv", "Flow", 2);
        let got = resolve_channel_id(&[ds], &b).expect("remap");
        assert_eq!(got, flow_id);
        assert_eq!(b.axis, AxisSide::Right);
    }
}
