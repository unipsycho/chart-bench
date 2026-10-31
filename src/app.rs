use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;
use egui_plot::{
    AxisHints, Bar, BarChart, HLine, HPlacement, Legend, Line, LineStyle, Plot, PlotBounds,
    PlotPoints, PlotUi, Points,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use uuid::Uuid;

use crate::ingest::{
    append_csv_growth, load_csv_path, load_csv_path_window, load_mat_path, load_mdf_path,
    load_parquet_path, load_parquet_path_with_progress, load_sqlite_path, load_witsml_path,
    load_xlsx_workbook, probe_csv_path, probe_parquet_path, RowWindow, MAX_LOAD_ROWS,
    MAX_PARQUET_FILE_BYTES,
};
use crate::model::{
    binding, load_presets, load_session, resolve_channel_id, save_presets, save_session,
    session_path, stamp_series_keys, AxisSide, ChartDrawOptions, ChartKind, ChartLayout,
    ChartState, ChartTileTree, Dataset, DockColumn, DockLayout, DockPane, Preset, PresetLibrary,
    PresetSlot, Session, SessionChart, SessionFile, StrokeStyleKind, TileEdge, TileSplit, UiFlags,
    YAxisMode,
};
use crate::stats::{channel_stats, file_stats, sparkline};

pub struct ChartBenchApp {
    pub datasets: Vec<Dataset>,
    pub charts: Vec<ChartState>,
    pub ui: UiFlags,
    pub selected_dataset: Option<Uuid>,
    pub selected_channel: Option<Uuid>,
    /// Chart pane that receives channel add (click / DnD / keys).
    pub active_chart: usize,
    /// Index into the filtered Channels list for keyboard highlight.
    pub channel_cursor: Option<usize>,
    /// Hovered / ↑↓ channel shown as faded preview on the active chart.
    pub channel_preview: Option<Uuid>,
    /// Channels list accepts ↑↓ / Space/Enter / ←→ when focused.
    pub channels_focused: bool,
    /// Keyboard last drove the channel cursor — ignore sticky hover until the pointer moves.
    channel_nav_from_keys: bool,
    /// Multi-selected channel ids (Ctrl/Shift/marquee).
    channel_multi: HashSet<Uuid>,
    /// Anchor row index for Shift+click range select.
    channel_anchor: Option<usize>,
    /// Marquee rubber-band in screen coords (Shift+drag on the channel list).
    channel_marquee: Option<(egui::Pos2, egui::Pos2)>,
    /// Persisted bottom-strip height (user can drag the splitter).
    pub table_strip_height: f32,
    /// Hit targets collected each frame for Favnyr-style dock drop preview.
    dock_hits: Vec<DockHit>,
    /// Last valid Favnyr landing while dragging — committed on release even if
    /// another drop zone steals/clears the egui DnD payload first.
    pending_favnyr: Option<FavnyrCommit>,
    /// Chart ids shown in independent OS / egui pop-out windows.
    popped_charts: HashSet<Uuid>,
    /// Last painted plot rect per chart (egui points) — used to crop Save PNG.
    plot_rects: HashMap<Uuid, egui::Rect>,
    /// After Save PNG: wait for ViewportCommand::Screenshot reply.
    pending_png: Option<PendingPngExport>,
    /// Chart ids that should snap plot X/Y on next first data view (empty → series/preview).
    /// Cleared after one auto-fit so a later manual zoom is kept when adding more series.
    reset_plot_auto: HashSet<Uuid>,
    /// Background Parquet open (windowed load) — progress + result on the UI thread.
    pending_parquet: Option<PendingParquetLoad>,
    /// >10 M-row open: wait for first/last choice before starting the load.
    pending_row_pick: Option<PendingRowPick>,
    pub status: String,
    pub last_save: Instant,
    pub presets: PresetLibrary,
    pub preset_name: String,
    pub channel_filter: String,
    /// Most-recent-first absolute/display paths (cap 25); includes closed files.
    pub recent_files: Vec<String>,
    watch_tx: Sender<PathBuf>,
    watch_rx: Receiver<PathBuf>,
    _watcher: Option<RecommendedWatcher>,
    watched: HashMap<String, ()>,
}

impl Default for ChartBenchApp {
    fn default() -> Self {
        let (watch_tx, watch_rx) = mpsc::channel();
        let mut app = Self {
            datasets: Vec::new(),
            charts: vec![ChartState::new("Chart 1")],
            ui: UiFlags::default(),
            selected_dataset: None,
            selected_channel: None,
            active_chart: 0,
            channel_cursor: None,
            channel_preview: None,
            channels_focused: false,
            channel_nav_from_keys: false,
            channel_multi: HashSet::new(),
            channel_anchor: None,
            channel_marquee: None,
            table_strip_height: 140.0,
            dock_hits: Vec::new(),
            pending_favnyr: None,
            popped_charts: HashSet::new(),
            plot_rects: HashMap::new(),
            pending_png: None,
            reset_plot_auto: HashSet::new(),
            pending_parquet: None,
            pending_row_pick: None,
            status: "Open a CSV / XLSX or load the demo sample.".into(),
            last_save: Instant::now(),
            presets: load_presets(),
            preset_name: "My preset".into(),
            channel_filter: String::new(),
            recent_files: Vec::new(),
            watch_tx,
            watch_rx,
            _watcher: None,
            watched: HashMap::new(),
        };
        app.init_watcher();
        app.try_restore_session();
        app.ui.dock.normalize();
        app.ensure_chart_tree();
        app.active_chart = app.active_chart.min(app.charts.len().saturating_sub(1));
        // Empty charts need one auto-fit when the first series / hover preview appears.
        for c in &app.charts {
            if c.series.is_empty() {
                app.reset_plot_auto.insert(c.id);
            }
        }
        app
    }
}

/// Messages from a background Parquet windowed load.
enum ParquetLoadMsg {
    Progress {
        rg_done: usize,
        rg_total: usize,
        rows: usize,
        rows_total: usize,
    },
    Done(Result<Dataset, String>),
}

struct PendingParquetLoad {
    path_display: String,
    window: RowWindow,
    file_total_rows: usize,
    started: Instant,
    rx: Receiver<ParquetLoadMsg>,
}

/// Format kind waiting on the >10 M-row first/last dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowPickKind {
    Parquet,
    Csv,
}

struct PendingRowPick {
    path: PathBuf,
    kind: RowPickKind,
    total_rows: usize,
    file_bytes: u64,
}

impl ChartBenchApp {
    fn init_watcher(&mut self) {
        let tx = self.watch_tx.clone();
        match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(ev) = res {
                    for p in ev.paths {
                        let _ = tx.send(p);
                    }
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(500)),
        ) {
            Ok(w) => self._watcher = Some(w),
            Err(e) => self.status = format!("File watcher unavailable: {e}"),
        }
    }

    fn try_restore_session(&mut self) {
        let Some(sess) = load_session() else {
            return;
        };
        self.ui = sess.ui;
        self.ui.dock.normalize();
        self.recent_files = sess.recent_files;
        let mut restore_errs: Vec<String> = Vec::new();
        for f in sess.files {
            // Blocking Parquet so channel remap below sees datasets immediately.
            match self.open_path_ex(PathBuf::from(&f.path), false) {
                Ok(()) => {
                    if let Some(ds) = self.datasets.last_mut() {
                        ds.follow = f.follow;
                        ds.channels_visible = f.channels_visible;
                        if f.follow {
                            let p = ds.path.clone();
                            self.watch_path(&p);
                        }
                    }
                }
                Err(e) => restore_errs.push(format!("{}: {e}", f.path)),
            }
        }
        if !restore_errs.is_empty() {
            self.status = format!(
                "Session restore: {} file(s) failed — {}",
                restore_errs.len(),
                restore_errs.join("; ")
            );
        }
        if !sess.charts.is_empty() {
            self.charts = sess
                .charts
                .into_iter()
                .map(|c| ChartState {
                    id: c.id,
                    title: c.title,
                    series: c.series,
                    follow_tail: c.follow_tail,
                    draw: c.draw,
                    show_draw_options: false,
                })
                .collect();
            ensure_unique_chart_titles(&mut self.charts);
            // Channel UUIDs are fresh after open_path — rebind by path/name/column
            self.remap_all_chart_series();
        }
    }

    /// Fill stable file/channel keys on every series (for preset + session remapping).
    fn stamp_all_series_keys(&mut self) {
        // Build (channel_id → path/name/col) without overlapping borrows
        let mut meta: HashMap<Uuid, (String, String, usize)> = HashMap::new();
        for ds in &self.datasets {
            for ch in &ds.channels {
                meta.insert(
                    ch.id,
                    (ds.path.clone(), ch.display_name.clone(), ch.raw_column_index),
                );
            }
        }
        for chart in &mut self.charts {
            for s in &mut chart.series {
                if let Some((path, name, col)) = meta.get(&s.channel_id) {
                    stamp_series_keys(s, path, name, *col);
                }
            }
        }
    }

    /// After reopening files, map saved series onto current channel UUIDs (keep L/R axes).
    fn remap_all_chart_series(&mut self) {
        let mut charts = std::mem::take(&mut self.charts);
        for chart in &mut charts {
            let mut kept = Vec::with_capacity(chart.series.len());
            for mut s in chart.series.drain(..) {
                if let Some(id) = resolve_channel_id(&self.datasets, &s) {
                    s.channel_id = id;
                    // Refresh keys from the live channel
                    for ds in &self.datasets {
                        if let Some(ch) = ds.channel_by_id(id) {
                            stamp_series_keys(
                                &mut s,
                                &ds.path,
                                &ch.display_name,
                                ch.raw_column_index,
                            );
                            break;
                        }
                    }
                    kept.push(s);
                }
            }
            chart.series = kept;
        }
        self.charts = charts;
    }

    fn autosave(&mut self) {
        if self.last_save.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.stamp_all_series_keys();
        let session = self.snapshot_session();
        let _ = save_session(&session);
        self.last_save = Instant::now();
    }

    fn snapshot_session(&self) -> Session {
        Session {
            files: self
                .datasets
                .iter()
                .map(|d| SessionFile {
                    path: d.path.clone(),
                    follow: d.follow,
                    channels_visible: d.channels_visible,
                    plan: Some(d.plan.clone()),
                })
                .collect(),
            charts: self
                .charts
                .iter()
                .map(|c| SessionChart {
                    id: c.id,
                    title: c.title.clone(),
                    series: c.series.clone(),
                    follow_tail: c.follow_tail,
                    draw: c.draw.clone(),
                })
                .collect(),
            ui: self.ui.clone(),
            recent_files: self.recent_files.clone(),
        }
    }

    /// Remember path in Recent (MRU, cap 25). Call after a successful open.
    fn push_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(25);
    }

    fn open_path(&mut self, path: PathBuf) -> anyhow::Result<()> {
        // Interactive opens: Parquet loads in the background with status progress.
        self.open_path_ex(path, true)
    }

    /// `parquet_async`: background full-row load + progress (UI). `false` for session/preset restore.
    fn open_path_ex(&mut self, path: PathBuf, parquet_async: bool) -> anyhow::Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        // Multi-dataset formats: each sheet/table/log/group becomes its own dataset.
        let multi: Option<Vec<Dataset>> = match ext.as_str() {
            "xlsx" | "xls" => Some(load_xlsx_workbook(&path)?),
            "mf4" | "mdf" => Some(load_mdf_path(&path)?),
            "xml" | "witsml" => Some(load_witsml_path(&path)?),
            "sqlite" | "db" | "sqlite3" => Some(load_sqlite_path(&path)?),
            _ => None,
        };
        if let Some(sheets) = multi {
            return self.push_multi_datasets(path, sheets, "sheets");
        }
        let ds = match ext.as_str() {
            "parquet" | "pq" => {
                if parquet_async {
                    self.begin_parquet_open(path)?;
                    return Ok(());
                }
                // Session/preset restore: under 10 M → all; over → first 10 M (no dialog).
                load_parquet_path(&path)?
            }
            "mat" => load_mat_path(&path)?,
            _ => {
                if parquet_async && matches!(ext.as_str(), "csv" | "tsv" | "txt") {
                    self.begin_csv_open(path)?;
                    return Ok(());
                }
                load_csv_path(&path)?
            }
        };
        self.finish_dataset_load(ds);
        Ok(())
    }

    /// Push a finished dataset into the session (sync formats + Parquet background Done).
    fn finish_dataset_load(&mut self, ds: Dataset) {
        self.status = format!(
            "Loaded {} — {} channels, {} records, X={:?}",
            ds.display_name,
            ds.channels.len(),
            ds.record_count(),
            ds.plan.x_axis.mode
        );
        self.push_recent(&ds.path);
        self.selected_dataset = Some(ds.id);
        self.datasets.push(ds);
    }

    /// Interactive Parquet open: probe → dialog if >10 M rows → background windowed load.
    fn begin_parquet_open(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if self.pending_parquet.is_some() {
            anyhow::bail!("Already loading a Parquet file — wait for it to finish.");
        }
        if self.pending_row_pick.is_some() {
            anyhow::bail!("Choose first or last 10 M rows for the open file first.");
        }
        let probe = probe_parquet_path(&path)?;
        if probe.total_rows > MAX_LOAD_ROWS {
            self.status = format!(
                "{} has {} rows — choose first or last {} to load.",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                fmt_status_count(probe.total_rows),
                fmt_status_count(MAX_LOAD_ROWS)
            );
            self.pending_row_pick = Some(PendingRowPick {
                path,
                kind: RowPickKind::Parquet,
                total_rows: probe.total_rows,
                file_bytes: probe.file_bytes,
            });
            return Ok(());
        }
        self.start_parquet_load(path, RowWindow::All, probe.total_rows, probe.file_bytes)
    }

    /// Interactive CSV open: probe → dialog if >10 M lines → sync windowed load.
    fn begin_csv_open(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if self.pending_row_pick.is_some() {
            anyhow::bail!("Choose first or last 10 M rows for the open file first.");
        }
        let probe = probe_csv_path(&path)?;
        if probe.approx_rows > MAX_LOAD_ROWS {
            self.status = format!(
                "{} has ~{} rows — choose first or last {} to load.",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                fmt_status_count(probe.approx_rows),
                fmt_status_count(MAX_LOAD_ROWS)
            );
            self.pending_row_pick = Some(PendingRowPick {
                path,
                kind: RowPickKind::Csv,
                total_rows: probe.approx_rows,
                file_bytes: probe.file_bytes,
            });
            return Ok(());
        }
        let ds = load_csv_path_window(&path, RowWindow::All)?;
        self.finish_dataset_load(ds);
        Ok(())
    }

    /// Refuse >10 GiB up front; stream a row window on a worker with status progress.
    fn start_parquet_load(
        &mut self,
        path: PathBuf,
        window: RowWindow,
        file_total_rows: usize,
        meta_len: u64,
    ) -> anyhow::Result<()> {
        if self.pending_parquet.is_some() {
            anyhow::bail!("Already loading a Parquet file — wait for it to finish.");
        }
        if meta_len > MAX_PARQUET_FILE_BYTES {
            anyhow::bail!(
                "Parquet file is {:.1} GiB (limit {:.0} GiB). Open a smaller file or split the data.",
                meta_len as f64 / (1024.0 * 1024.0 * 1024.0),
                MAX_PARQUET_FILE_BYTES as f64 / (1024.0 * 1024.0 * 1024.0)
            );
        }
        let path_display = path.display().to_string();
        let gib = meta_len as f64 / (1024.0 * 1024.0 * 1024.0);
        let win_note = match window {
            RowWindow::All => String::new(),
            RowWindow::Head => format!(
                " — first {} of {}",
                fmt_status_count(MAX_LOAD_ROWS.min(file_total_rows)),
                fmt_status_count(file_total_rows)
            ),
            RowWindow::Tail => format!(
                " — last {} of {}",
                fmt_status_count(MAX_LOAD_ROWS.min(file_total_rows)),
                fmt_status_count(file_total_rows)
            ),
        };
        self.status = if gib >= 0.05 {
            format!("Loading {path_display} ({gib:.2} GiB{win_note})…")
        } else {
            format!("Loading {path_display}{win_note}…")
        };
        let (tx, rx) = mpsc::channel();
        let path_bg = path.clone();
        std::thread::spawn(move || {
            let mut last = Instant::now();
            let result = load_parquet_path_with_progress(
                &path_bg,
                window,
                &mut |rg_done, rg_total, rows, rows_total| {
                    let due = last.elapsed() >= Duration::from_millis(150) || rg_done == rg_total;
                    if due {
                        let _ = tx.send(ParquetLoadMsg::Progress {
                            rg_done,
                            rg_total,
                            rows,
                            rows_total,
                        });
                        last = Instant::now();
                    }
                },
            );
            let _ = tx.send(ParquetLoadMsg::Done(result.map_err(|e| e.to_string())));
        });
        self.pending_parquet = Some(PendingParquetLoad {
            path_display,
            window,
            file_total_rows,
            started: Instant::now(),
            rx,
        });
        self.push_recent(&path.display().to_string());
        Ok(())
    }

    /// Apply the user's first/last choice from the large-file dialog.
    fn confirm_row_pick(&mut self, window: RowWindow) {
        let Some(pick) = self.pending_row_pick.take() else {
            return;
        };
        match pick.kind {
            RowPickKind::Parquet => {
                if let Err(e) =
                    self.start_parquet_load(pick.path, window, pick.total_rows, pick.file_bytes)
                {
                    self.status = format!("Open failed: {e}");
                }
            }
            RowPickKind::Csv => match load_csv_path_window(&pick.path, window) {
                Ok(ds) => self.finish_dataset_load(ds),
                Err(e) => self.status = format!("Open failed: {e}"),
            },
        }
    }

    /// Drain background Parquet progress / completion (call each frame).
    fn poll_parquet_load(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_parquet.as_ref() else {
            return;
        };
        let mut progress: Option<(usize, usize, usize, usize)> = None;
        let mut done: Option<Result<Dataset, String>> = None;
        while let Ok(msg) = pending.rx.try_recv() {
            match msg {
                ParquetLoadMsg::Progress {
                    rg_done,
                    rg_total,
                    rows,
                    rows_total,
                } => progress = Some((rg_done, rg_total, rows, rows_total)),
                ParquetLoadMsg::Done(r) => done = Some(r),
            }
        }
        if let Some((rg_done, rg_total, rows, rows_total)) = progress {
            let (name, win_note) = self
                .pending_parquet
                .as_ref()
                .map(|p| {
                    let note = match p.window {
                        RowWindow::All => String::new(),
                        RowWindow::Head => format!(
                            ", first {} of {}",
                            fmt_status_count(MAX_LOAD_ROWS.min(p.file_total_rows)),
                            fmt_status_count(p.file_total_rows)
                        ),
                        RowWindow::Tail => format!(
                            ", last {} of {}",
                            fmt_status_count(MAX_LOAD_ROWS.min(p.file_total_rows)),
                            fmt_status_count(p.file_total_rows)
                        ),
                    };
                    (p.path_display.clone(), note)
                })
                .unwrap_or_default();
            let pct = if rg_total > 0 {
                (rg_done * 100) / rg_total
            } else {
                0
            };
            self.status = format!(
                "Loading {name}… {pct}% ({} / {} rows{win_note}, rg {rg_done}/{rg_total})",
                fmt_status_count(rows),
                fmt_status_count(rows_total)
            );
            ctx.request_repaint();
        }
        if let Some(result) = done {
            let (elapsed, window, file_total) = self
                .pending_parquet
                .as_ref()
                .map(|p| (p.started.elapsed(), p.window, p.file_total_rows))
                .unwrap_or((Duration::ZERO, RowWindow::All, 0));
            self.pending_parquet = None;
            match result {
                Ok(ds) => {
                    let rows = ds.record_count();
                    let ch = ds.channels.len();
                    let name = ds.display_name.clone();
                    self.finish_dataset_load(ds);
                    let window_note = match window {
                        RowWindow::All => format!("{} records", fmt_status_count(rows)),
                        RowWindow::Head => format!(
                            "first {} of {} rows",
                            fmt_status_count(rows),
                            fmt_status_count(file_total)
                        ),
                        RowWindow::Tail => format!(
                            "last {} of {} rows",
                            fmt_status_count(rows),
                            fmt_status_count(file_total)
                        ),
                    };
                    self.status = format!(
                        "Loaded {name} — {ch} channels, {window_note} in {:.1}s",
                        elapsed.as_secs_f32()
                    );
                }
                Err(e) => {
                    self.status = format!("Open failed: {e}");
                }
            }
            ctx.request_repaint();
        } else if self.pending_parquet.is_some() {
            // Keep the UI alive while the worker streams row groups.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Centered modal: load first vs last 10 M rows for oversized files.
    fn show_row_window_dialog(&mut self, ctx: &egui::Context) {
        let Some(pick) = self.pending_row_pick.as_ref() else {
            return;
        };
        let file_name = pick
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string();
        let total = pick.total_rows;
        let gib = pick.file_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let kind = match pick.kind {
            RowPickKind::Parquet => "Parquet",
            RowPickKind::Csv => "CSV",
        };

        // Dim the rest of the UI while choosing.
        egui::Area::new(egui::Id::new("row_window_modal_dim"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen = ui.ctx().screen_rect();
                ui.allocate_response(screen.size(), egui::Sense::click());
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    egui::Color32::from_black_alpha(160),
                );
            });

        let mut choice: Option<RowWindow> = None;
        let mut cancel = false;
        egui::Window::new("Large file")
            .id(egui::Id::new("row_window_modal"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    egui::RichText::new(format!("{kind}: {file_name}"))
                        .strong()
                        .size(15.0),
                );
                ui.add_space(6.0);
                ui.label(format!(
                    "This file has {} rows (~{:.2} GiB). Chart-Bench loads at most {} rows.",
                    fmt_status_count(total),
                    gib,
                    fmt_status_count(MAX_LOAD_ROWS)
                ));
                ui.add_space(4.0);
                ui.label("Which window should be loaded?");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [190.0, 32.0],
                            egui::Button::new(format!(
                                "First {} rows",
                                fmt_status_count(MAX_LOAD_ROWS)
                            )),
                        )
                        .on_hover_text("Load the beginning / head of the file")
                        .clicked()
                    {
                        choice = Some(RowWindow::Head);
                    }
                    if ui
                        .add_sized(
                            [190.0, 32.0],
                            egui::Button::new(format!(
                                "Last {} rows",
                                fmt_status_count(MAX_LOAD_ROWS)
                            )),
                        )
                        .on_hover_text("Load the end / tail of the file")
                        .clicked()
                    {
                        choice = Some(RowWindow::Tail);
                    }
                });
                ui.add_space(8.0);
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });

        if let Some(window) = choice {
            self.confirm_row_pick(window);
        } else if cancel {
            self.pending_row_pick = None;
            self.status = "Open cancelled.".into();
        }
    }

    /// Push several datasets from one file (XLSX sheets, SQLite tables, …).
    fn push_multi_datasets(
        &mut self,
        path: PathBuf,
        sheets: Vec<Dataset>,
        kind: &str,
    ) -> anyhow::Result<()> {
        let n = sheets.len();
        let path_s = path.display().to_string();
        self.push_recent(&path_s);
        let mut last_id = None;
        let mut summary = Vec::new();
        for ds in sheets {
            summary.push(format!(
                "{} ({} ch, {} rows)",
                ds.display_name,
                ds.channels.len(),
                ds.record_count()
            ));
            last_id = Some(ds.id);
            self.datasets.push(ds);
        }
        self.selected_dataset = last_id.or(self.selected_dataset);
        self.status = if n == 1 {
            format!("Loaded {}", summary.join(", "))
        } else {
            format!("Loaded {n} {kind} — {}", summary.join("; "))
        };
        Ok(())
    }

    fn pick_and_open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Data",
                &[
                    "csv", "tsv", "txt", "xlsx", "xls", "parquet", "pq", "mat", "mf4", "mdf",
                    "xml", "witsml", "sqlite", "db", "sqlite3",
                ],
            )
            .add_filter("CSV / TSV", &["csv", "tsv", "txt"])
            .add_filter("Excel", &["xlsx", "xls"])
            .add_filter("Parquet", &["parquet", "pq"])
            .add_filter("MATLAB", &["mat"])
            .add_filter("MDF / MF4", &["mf4", "mdf"])
            .add_filter("WITSML XML", &["xml", "witsml"])
            .add_filter("SQLite", &["sqlite", "db", "sqlite3"])
            .pick_file()
        {
            if let Err(e) = self.open_path(path) {
                self.status = format!("Open failed: {e}");
            }
        }
    }

    /// Resolve OS / egui dropped files to paths and open them.
    fn open_dropped_files(&mut self, dropped: Vec<egui::DroppedFile>) {
        let mut opened = 0usize;
        let mut last_err: Option<String> = None;
        for f in dropped {
            match resolve_dropped_file(&f) {
                Ok(path) => match self.open_path(path) {
                    Ok(()) => opened += 1,
                    Err(e) => last_err = Some(e.to_string()),
                },
                Err(e) => last_err = Some(e),
            }
        }
        if opened == 0 {
            if let Some(e) = last_err {
                self.status = format!("Drop open failed: {e}");
            } else {
                self.status = "Drop open failed: no usable file path.".into();
            }
        } else if opened > 1 {
            self.status = format!("Loaded {opened} dropped files.");
        }
    }

    fn open_demo(&mut self) {
        let candidates = [
            PathBuf::from("samples/demo_multiline.csv"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/demo_multiline.csv"),
        ];
        for c in candidates {
            if c.exists() {
                if let Err(e) = self.open_path(c) {
                    self.status = format!("Demo load failed: {e}");
                }
                return;
            }
        }
        self.status = "Demo sample not found (samples/demo_multiline.csv)".into();
    }

    fn watch_path(&mut self, path: &str) {
        if self.watched.contains_key(path) {
            return;
        }
        if let Some(w) = self._watcher.as_mut() {
            if w.watch(std::path::Path::new(path), RecursiveMode::NonRecursive)
                .is_ok()
            {
                self.watched.insert(path.to_string(), ());
            }
        }
    }

    fn poll_follows(&mut self) {
        while let Ok(p) = self.watch_rx.try_recv() {
            let ps = p.display().to_string();
            for ds in &mut self.datasets {
                if ds.follow && (ds.path == ps || ps.ends_with(&ds.path)) {
                    match append_csv_growth(ds) {
                        Ok(n) if n > 0 => self.status = format!("{}: +{n} rows", ds.display_name),
                        Ok(_) => {}
                        Err(e) => self.status = format!("Follow error: {e}"),
                    }
                }
            }
        }
    }

    fn dataset(&self, id: Uuid) -> Option<&Dataset> {
        self.datasets.iter().find(|d| d.id == id)
    }

    fn add_channel_to_chart(&mut self, chart_idx: usize, channel_id: Uuid, axis: AxisSide) {
        // Honor requested side; Overlay collapses everything to left.
        let axis = match self.ui.y_axis_mode {
            YAxisMode::Overlay => AxisSide::Left,
            YAxisMode::Multi => axis,
        };
        // Capture remapping keys before borrowing charts mutably
        let meta = self.datasets.iter().find_map(|ds| {
            ds.channel_by_id(channel_id)
                .map(|ch| (ds.path.clone(), ch.display_name.clone(), ch.raw_column_index))
        });
        let Some(chart) = self.charts.get_mut(chart_idx) else {
            return;
        };
        if chart.series.iter().any(|s| s.channel_id == channel_id) {
            return;
        }
        // Do not re-arm auto-fit here — empty charts / clear / last-remove already mark
        // reset_plot_auto. First L/R or hover preview consumes it once; later adds keep zoom.
        let idx = chart.series.len();
        let mut b = binding(channel_id, idx, axis);
        if let Some((path, name, col)) = meta {
            stamp_series_keys(&mut b, &path, &name, col);
        }
        chart.series.push(b);
        self.status = format!("Added series to {}", chart.title);
    }

    fn remove_channel_from_chart(&mut self, chart_idx: usize, channel_id: Uuid) {
        let Some(chart) = self.charts.get_mut(chart_idx) else {
            return;
        };
        let before = chart.series.len();
        chart.series.retain(|s| s.channel_id != channel_id);
        if chart.series.len() < before {
            self.status = format!("Removed series from {}", chart.title);
            // Back to empty → next L/R or hover preview should auto-fit again.
            if chart.series.is_empty() {
                let id = chart.id;
                self.reset_plot_auto.insert(id);
            }
        }
    }

    /// Axis side for a channel on a chart, if plotted.
    fn channel_axis_on_chart(&self, chart_idx: usize, channel_id: Uuid) -> Option<AxisSide> {
        self.charts
            .get(chart_idx)
            .and_then(|c| c.series.iter().find(|s| s.channel_id == channel_id).map(|s| s.axis))
    }

    /// ←/→ / L/R: toggle that Y axis (same side → remove; other → flip; missing → add).
    fn toggle_channel_on_chart(&mut self, chart_idx: usize, channel_id: Uuid, axis: AxisSide) {
        let existing = self
            .charts
            .get(chart_idx)
            .and_then(|c| c.series.iter().find(|s| s.channel_id == channel_id).map(|s| s.axis));
        match existing {
            Some(cur) if cur == axis => self.remove_channel_from_chart(chart_idx, channel_id),
            Some(_) => {
                if let Some(s) = self
                    .charts
                    .get_mut(chart_idx)
                    .and_then(|c| c.series.iter_mut().find(|s| s.channel_id == channel_id))
                {
                    s.axis = axis;
                    self.status = format!(
                        "Moved series to {} axis",
                        match axis {
                            AxisSide::Left => "left",
                            AxisSide::Right => "right",
                        }
                    );
                }
            }
            None => {
                if axis == AxisSide::Right && self.ui.y_axis_mode == YAxisMode::Overlay {
                    self.ui.y_axis_mode = YAxisMode::Multi;
                }
                self.add_channel_to_chart(chart_idx, channel_id, axis);
            }
        }
    }

    /// Pick Left/Right so new series joins a similar-ranged existing group when possible.
    fn auto_axis_for_channel(&self, chart_idx: usize, channel_id: Uuid) -> Option<AxisSide> {
        let (mn, mx) = self.channel_range(channel_id)?;
        let mid = (mn + mx) * 0.5;
        let span = (mx - mn).abs().max(1e-9);
        let chart = self.charts.get(chart_idx)?;
        let mut left_hits = 0i32;
        let mut right_hits = 0i32;
        for s in &chart.series {
            let Some((amn, amx)) = self.channel_range(s.channel_id) else {
                continue;
            };
            let amid = (amn + amx) * 0.5;
            let aspan = (amx - amn).abs().max(1e-9);
            // Similar if mids within ~3× span of each other and spans within 10×.
            let similar = (mid - amid).abs() < (span + aspan) * 3.0
                && span / aspan < 10.0
                && aspan / span < 10.0;
            if similar {
                match s.axis {
                    AxisSide::Left => left_hits += 1,
                    AxisSide::Right => right_hits += 1,
                }
            }
        }
        if left_hits == 0 && right_hits == 0 {
            // Prefer left unless values are far from existing left cluster.
            let left_mids: Vec<f64> = chart
                .series
                .iter()
                .filter(|s| s.axis == AxisSide::Left)
                .filter_map(|s| self.channel_range(s.channel_id))
                .map(|(a, b)| (a + b) * 0.5)
                .collect();
            if left_mids.is_empty() {
                return Some(AxisSide::Left);
            }
            let left_mid = left_mids.iter().sum::<f64>() / left_mids.len() as f64;
            if (mid - left_mid).abs() > span.max(left_mid.abs()) * 5.0 {
                return Some(AxisSide::Right);
            }
            return Some(AxisSide::Left);
        }
        if right_hits > left_hits {
            Some(AxisSide::Right)
        } else {
            Some(AxisSide::Left)
        }
    }

    fn channel_range(&self, channel_id: Uuid) -> Option<(f64, f64)> {
        for ds in &self.datasets {
            if let Some(col) = ds.column_for(channel_id) {
                let mut mn = f64::INFINITY;
                let mut mx = f64::NEG_INFINITY;
                for &v in col {
                    if v.is_finite() {
                        mn = mn.min(v);
                        mx = mx.max(v);
                    }
                }
                if mn.is_finite() && mx.is_finite() {
                    return Some((mn, mx));
                }
            }
        }
        None
    }

    fn duplicate_chart(&mut self, idx: usize) {
        self.duplicate_chart_at(idx, usize::MAX);
    }

    /// Next free "Chart N" title — never collides with an existing chart name.
    fn next_unique_chart_title(&self) -> String {
        unique_chart_title(&self.charts)
    }

    /// Clone chart `from` and append; nest under source leaf (south) in the tile tree.
    fn duplicate_chart_at(&mut self, from: usize, to: usize) {
        let Some(src) = self.charts.get(from).cloned() else {
            return;
        };
        let src_id = src.id;
        let title = self.next_unique_chart_title();
        let chart = ChartState {
            id: Uuid::new_v4(),
            title,
            series: src.series,
            follow_tail: src.follow_tail,
            draw: src.draw,
            show_draw_options: false,
        };
        let new_id = chart.id;
        let insert = if to == usize::MAX {
            self.charts.len()
        } else {
            to.min(self.charts.len())
        };
        self.charts.insert(insert, chart);
        self.active_chart = insert;
        if self.charts.len() > 1 && self.ui.chart_layout == ChartLayout::Overlay {
            self.ui.chart_layout = ChartLayout::Stacked;
        }
        // Place the new leaf exactly once via split_at. Do NOT sync_with_charts first:
        // sync appends missing ids, then split_at inserts again → two tiles for one chart
        // (three visible panes); moving one leaf then collapses back to two.
        if self.ui.chart_tree.is_none() {
            let prior: Vec<Uuid> = self
                .charts
                .iter()
                .filter(|c| c.id != new_id)
                .map(|c| c.id)
                .collect();
            self.ui.chart_tree =
                Some(ChartTileTree::from_chart_ids(&prior, self.ui.chart_layout));
        } else if let Some(tree) = self.ui.chart_tree.as_mut() {
            while tree.contains(new_id) {
                if !tree.remove_leaf(new_id) {
                    break;
                }
            }
        }
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            if !tree.split_at(src_id, new_id, TileEdge::South) {
                tree.sync_with_charts(&self.charts, self.ui.chart_layout);
            }
        }
        self.status = "Duplicated chart (split).".into();
    }

    /// Keep `chart_tree` in sync with `charts` (create from flat layout if missing).
    fn ensure_chart_tree(&mut self) {
        let ids: Vec<Uuid> = self.charts.iter().map(|c| c.id).collect();
        if self.ui.chart_tree.is_none() {
            self.ui.chart_tree = Some(ChartTileTree::from_chart_ids(&ids, self.ui.chart_layout));
        }
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            tree.sync_with_charts(&self.charts, self.ui.chart_layout);
        }
    }

    /// Swap two charts' tile positions (Favnyr center drop).
    fn swap_chart_tiles(&mut self, a: Uuid, b: Uuid) {
        if a == b {
            return;
        }
        self.ensure_chart_tree();
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            tree.swap_leaves(a, b);
        }
        if let Some(i) = self.charts.iter().position(|c| c.id == a) {
            self.active_chart = i;
        }
        self.status = "Swapped charts.".into();
    }

    /// Nest `moving` into a split of `target` on `edge` (always move — never clone).
    fn nest_chart_split(&mut self, moving: Uuid, target: Uuid, edge: TileEdge) {
        if moving == target {
            return;
        }
        self.ensure_chart_tree();
        // Detach moving leaf first so split_at can place it under target
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            tree.remove_leaf(moving);
        }
        if let Some(i) = self.charts.iter().position(|c| c.id == moving) {
            self.active_chart = i;
        }
        if self.charts.len() > 1 && self.ui.chart_layout == ChartLayout::Overlay {
            self.ui.chart_layout = ChartLayout::Stacked;
        }
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            if !tree.split_at(target, moving, edge) {
                // Fallback: attach at root on that edge
                let old = tree.clone();
                let neu = ChartTileTree::leaf(moving);
                let (dir, first, second) = match edge {
                    TileEdge::North => (TileSplit::Rows, neu, old),
                    TileEdge::South => (TileSplit::Rows, old, neu),
                    TileEdge::West => (TileSplit::Cols, neu, old),
                    TileEdge::East => (TileSplit::Cols, old, neu),
                };
                *tree = ChartTileTree::Split {
                    dir,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
            }
            tree.sync_with_charts(&self.charts, self.ui.chart_layout);
        }
        self.status = format!("Nested split ({edge}).");
    }

    /// Create an empty chart leaf (drag into place); duplicate stays on the toolbar only.
    fn add_empty_chart(&mut self) {
        let title = self.next_unique_chart_title();
        let chart = ChartState::new(title);
        let id = chart.id;
        self.charts.push(chart);
        self.reset_plot_auto.insert(id);
        self.active_chart = self.charts.len() - 1;
        if self.charts.len() > 1 && self.ui.chart_layout == ChartLayout::Overlay {
            self.ui.chart_layout = ChartLayout::Stacked;
        }
        self.ensure_chart_tree();
        self.status = "Added empty chart — drag title to place.".into();
    }

    fn remove_chart_at(&mut self, idx: usize) {
        if idx >= self.charts.len() || self.charts.len() <= 1 {
            return;
        }
        let id = self.charts[idx].id;
        self.popped_charts.remove(&id);
        self.charts.remove(idx);
        if let Some(tree) = self.ui.chart_tree.as_mut() {
            tree.remove_leaf(id);
            tree.sync_with_charts(&self.charts, self.ui.chart_layout);
        }
        if self.active_chart >= self.charts.len() {
            self.active_chart = self.charts.len().saturating_sub(1);
        }
    }

    fn apply_favnyr_theme(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        let panel = egui::Color32::from_rgb(32, 28, 42);
        let window = egui::Color32::from_rgb(26, 22, 34);
        let stroke = egui::Color32::from_rgb(140, 110, 190);
        let accent = egui::Color32::from_rgb(168, 130, 220);
        visuals.panel_fill = panel;
        visuals.window_fill = window;
        visuals.extreme_bg_color = egui::Color32::from_rgb(18, 15, 26);
        visuals.widgets.noninteractive.bg_fill = panel;
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 34, 54);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(54, 44, 72);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(64, 52, 88);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, stroke.gamma_multiply(0.45));
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, stroke.gamma_multiply(0.55));
        // Strong selection fill — Files table + other selected rows must read clearly
        visuals.selection.bg_fill = egui::Color32::from_rgb(88, 58, 140);
        visuals.selection.stroke = egui::Stroke::new(1.5_f32, accent);
        visuals.hyperlink_color = accent;
        // Keep UI text bright; plot scopes set contrast ink from chart background luminance.
        visuals.override_text_color = Some(egui::Color32::from_rgb(230, 224, 240));
        ctx.set_visuals(visuals);
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.window_margin = egui::Margin::same(8.0);
            style.visuals.window_rounding = egui::Rounding::same(6.0);
            style.visuals.menu_rounding = egui::Rounding::same(6.0);
        });
    }

    fn save_current_as_preset(&mut self) {
        let name = self.preset_name.trim().to_string();
        if name.is_empty() {
            self.status = "Preset name required.".into();
            return;
        }
        // Ensure L/R bindings carry path + channel keys (survive new UUIDs on restore)
        self.stamp_all_series_keys();
        let preset = Preset {
            name: name.clone(),
            slots: self
                .datasets
                .iter()
                .map(|d| PresetSlot {
                    name: d.display_name.clone(),
                    path: Some(d.path.clone()),
                })
                .collect(),
            charts: self
                .charts
                .iter()
                .map(|c| SessionChart {
                    id: c.id,
                    title: c.title.clone(),
                    series: c.series.clone(),
                    follow_tail: c.follow_tail,
                    draw: c.draw.clone(),
                })
                .collect(),
            chart_layout: self.ui.chart_layout,
            chart_tree: self.ui.chart_tree.clone(),
            y_axis_mode: self.ui.y_axis_mode,
        };
        if let Some(existing) = self.presets.presets.iter_mut().find(|p| p.name == name) {
            *existing = preset;
        } else {
            self.presets.presets.push(preset);
        }
        match save_presets(&self.presets) {
            Ok(()) => {
                self.status = format!(
                    "Saved preset '{name}' ({} files, {} charts).",
                    self.datasets.len(),
                    self.charts.len()
                )
            }
            Err(e) => self.status = format!("Preset save failed: {e}"),
        }
    }

    fn apply_preset(&mut self, idx: usize) {
        let Some(preset) = self.presets.presets.get(idx).cloned() else {
            return;
        };
        self.datasets.clear();
        let mut opened = 0usize;
        let mut open_errs: Vec<String> = Vec::new();
        for slot in &preset.slots {
            if let Some(path) = &slot.path {
                // Blocking Parquet so series remap sees datasets in this call.
                match self.open_path_ex(PathBuf::from(path), false) {
                    Ok(()) => opened += 1,
                    Err(e) => open_errs.push(format!("{path}: {e}")),
                }
            }
        }
        self.ui.chart_layout = preset.chart_layout;
        self.ui.y_axis_mode = preset.y_axis_mode;
        // Remap preset tree leaf ids → fresh chart ids (parallel order)
        let old_ids: Vec<Uuid> = preset.charts.iter().map(|c| c.id).collect();
        if !preset.charts.is_empty() {
            self.charts = preset
                .charts
                .into_iter()
                .map(|c| ChartState {
                    id: Uuid::new_v4(), // fresh chart ids; series remapped below
                    title: c.title,
                    series: c.series,
                    follow_tail: c.follow_tail,
                    draw: c.draw,
                    show_draw_options: false,
                })
                .collect();
            ensure_unique_chart_titles(&mut self.charts);
            self.remap_all_chart_series();
        }
        if let Some(mut tree) = preset.chart_tree {
            let map: HashMap<Uuid, Uuid> = old_ids
                .iter()
                .zip(self.charts.iter().map(|c| c.id))
                .map(|(o, n)| (*o, n))
                .collect();
            tree.map_leaf_ids(|id| map.get(&id).copied().unwrap_or(id));
            tree.sync_with_charts(&self.charts, self.ui.chart_layout);
            self.ui.chart_tree = Some(tree);
        } else {
            self.ui.chart_tree = None;
            self.ensure_chart_tree();
        }
        self.active_chart = 0;
        let series_n: usize = self.charts.iter().map(|c| c.series.len()).sum();
        self.status = if open_errs.is_empty() {
            format!(
                "Applied preset '{}' — {opened} files, {} charts, {series_n} series.",
                preset.name,
                self.charts.len()
            )
        } else {
            format!(
                "Applied preset '{}' — {opened} files ok, {} failed: {}",
                preset.name,
                open_errs.len(),
                open_errs.join("; ")
            )
        };
    }

    fn export_chart_csv(&self, chart_idx: usize) -> anyhow::Result<()> {
        let chart = self
            .charts
            .get(chart_idx)
            .ok_or_else(|| anyhow::anyhow!("no chart"))?;
        let path = rfd::FileDialog::new()
            .set_file_name(format!("{}.csv", chart.title.replace(' ', "_")))
            .save_file()
            .ok_or_else(|| anyhow::anyhow!("cancelled"))?;
        let mut wtr = csv::Writer::from_path(path)?;
        let mut headers = vec!["x".to_string()];
        let mut cols: Vec<Vec<f64>> = Vec::new();
        let mut x_vals: Vec<f64> = Vec::new();
        for s in &chart.series {
            for ds in &self.datasets {
                if let Some(ch) = ds.channel_by_id(s.channel_id) {
                    headers.push(ch.display_name.clone());
                    if let Some(c) = ds.columns.get(ch.column_index) {
                        if x_vals.is_empty() {
                            x_vals = ds.x.clone();
                        }
                        cols.push(c.clone());
                    }
                }
            }
        }
        wtr.write_record(&headers)?;
        for i in 0..x_vals.len() {
            let mut row = vec![format!("{}", x_vals[i])];
            for c in &cols {
                let v = c.get(i).copied().unwrap_or(f64::NAN);
                row.push(if v.is_finite() {
                    format!("{v}")
                } else {
                    String::new()
                });
            }
            wtr.write_record(&row)?;
        }
        wtr.flush()?;
        Ok(())
    }
}

/// In-flight Save PNG — filled on toolbar click, completed when Screenshot arrives.
struct PendingPngExport {
    chart_id: Uuid,
    title: String,
    crop: egui::Rect,
    viewport_id: egui::ViewportId,
}

impl eframe::App for ChartBenchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_follows();
        self.poll_parquet_load(ctx);
        self.show_row_window_dialog(ctx);
        self.autosave();
        Self::apply_favnyr_theme(ctx);
        self.poll_png_export(ctx);

        // fcitx/ibus sends Ime::Enabled for latin TextEdit; egui then strips Backspace/arrows
        // from the widget's events. Drop IME events so Filter (and other fields) edit normally.
        ctx.input_mut(|i| {
            i.events.retain(|e| !matches!(e, egui::Event::Ime(_)));
        });

        // OS file drops (file manager → window). Keep UI live while hovering a drag.
        let hovering_os_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering_os_files {
            ctx.request_repaint();
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            self.open_dropped_files(dropped);
        }

        // Keyboard shortcuts (toolbar removed — Open lives in Files panel)
        let mut open_dlg = false;
        let mut save_now = false;
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::O) {
                open_dlg = true;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::S) {
                save_now = true;
            }
        });
        if open_dlg {
            self.pick_and_open_file();
        }
        if save_now {
            self.last_save = Instant::now() - Duration::from_secs(10);
            self.autosave();
            self.status = "Session saved (Ctrl+S).".into();
        }

        // Bottom summary strip (counts + status only — brand lives in left header)
        egui::TopBottomPanel::bottom("summary")
            .exact_height(28.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(22, 18, 30))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(90, 70, 130),
                    ))
                    .inner_margin(egui::Margin::symmetric(10.0, 4.0)),
            )
            .show(ctx, |ui| {
                let n_ch: usize = self.datasets.iter().map(|d| d.channels.len()).sum();
                let n_series: usize = self.charts.iter().map(|c| c.series.len()).sum();
                let n_rows: usize = self.datasets.iter().map(|d| d.record_count()).sum();
                ui.horizontal(|ui| {
                    ui.small(format!(
                        "{} files · {} channels · {} series · {} rows · {} charts",
                        self.datasets.len(),
                        n_ch,
                        n_series,
                        n_rows,
                        self.charts.len()
                    ));
                    ui.separator();
                    ui.label(
                        egui::RichText::new(&self.status)
                            .small()
                            .weak()
                            .color(egui::Color32::from_rgb(180, 165, 210)),
                    );
                });
            });

        // Bottom dock stack before side columns so sides get remaining height.
        // Cap height so Table never eats the chart; splitter still resizes within the cap.
        self.ui.dock.normalize();
        self.dock_hits.clear();
        let dragging_pane = egui::DragAndDrop::has_payload_of_type::<DockPane>(ctx);
        let dragging_chart = egui::DragAndDrop::has_payload_of_type::<ChartDrag>(ctx);
        let dragging_layout = dragging_pane || dragging_chart;
        let bottom_visible: Vec<DockPane> = self
            .ui
            .dock
            .bottom
            .iter()
            .copied()
            .filter(|p| self.ui.pane_visible(*p))
            .collect();
        if !bottom_visible.is_empty() || dragging_layout {
            let h = self.table_strip_height.clamp(72.0, 260.0);
            let panel = egui::TopBottomPanel::bottom("dock_bottom")
                .resizable(true)
                .default_height(h)
                .min_height(72.0)
                .max_height(260.0)
                .frame(dock_frame());
            let resp = panel.show(ctx, |ui| {
                for pane in &bottom_visible {
                    self.dock_section(ui, DockColumn::Bottom, *pane, false);
                }
                if bottom_visible.is_empty() {
                    ui.weak("Drop here for bottom strip");
                }
            });
            // Entire bottom strip is the drop target (X bands for stack/split)
            self.dock_hits.push(DockHit {
                kind: DockHitKind::ColumnBody {
                    col: DockColumn::Bottom,
                    len: bottom_visible.len(),
                },
                rect: resp.response.rect,
            });
            // Remember splitter height for next frame / session feel
            let new_h = resp.response.rect.height().clamp(72.0, 260.0);
            if (new_h - self.table_strip_height).abs() > 1.0 {
                self.table_strip_height = new_h;
            }
        }

        // Left sidebar: brand header above Files/menu rail + dock stack
        let left_visible: Vec<DockPane> = self
            .ui
            .dock
            .left
            .iter()
            .copied()
            .filter(|p| self.ui.pane_visible(*p))
            .collect();
        {
            // Rail 48 + dock (~200 default); keep prior dock width range
            let resp = egui::SidePanel::left("left_sidebar")
                .default_width(248.0)
                .width_range(144.0..=468.0)
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(22, 18, 30))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(100, 80, 140),
                        ))
                        .inner_margin(egui::Margin::ZERO),
                )
                .show(ctx, |ui| {
                    // Brand: logo centered above; Chart-Bench + version centered below (no frame)
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        ui.add(
                            egui::Image::new(egui::include_image!(
                                "../assets/agile-automation-logo.png"
                            ))
                            .max_height(48.0)
                            .maintain_aspect_ratio(true),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Chart-Bench  {}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(13.0)
                            .strong()
                            .color(egui::Color32::from_rgb(200, 175, 240)),
                        );
                    });
                    ui.add_space(6.0);

                    // Files/menu icon rail + left dock under the brand
                    let body_h = ui.available_height();
                    ui.horizontal(|ui| {
                        ui.set_min_height(body_h);
                        egui::Frame::none()
                            .inner_margin(egui::Margin::symmetric(6.0, 8.0))
                            .show(ui, |ui| {
                                ui.set_width(36.0);
                                ui.vertical_centered(|ui| {
                                    icon_toggle(ui, &mut self.ui.show_files, "📁", "Files");
                                    icon_toggle(ui, &mut self.ui.show_channels, "📶", "Channels");
                                    icon_toggle(ui, &mut self.ui.show_info, "ℹ", "Info");
                                    // Painted grid — ▤ / box glyphs often missing from egui fonts
                                    icon_toggle_table(ui, &mut self.ui.show_table);
                                    icon_toggle(ui, &mut self.ui.show_saved, "★", "Saved");
                                });
                            });
                        // Dock column fills remaining sidebar width
                        let dock_frame = dock_frame().inner_margin(egui::Margin::symmetric(4.0, 4.0));
                        let dock_resp = dock_frame.show(ui, |ui| {
                            ui.set_min_width(96.0);
                            ui.set_max_width(ui.available_width());
                            ui.set_min_height(body_h);
                            if !left_visible.is_empty() || dragging_layout {
                                self.render_dock_column(ui, DockColumn::Left, &left_visible);
                                if left_visible.is_empty() {
                                    ui.weak("Drop here");
                                }
                            }
                        });
                        // Full dock body accepts drag; Y decides stack above / below
                        self.dock_hits.push(DockHit {
                            kind: DockHitKind::ColumnBody {
                                col: DockColumn::Left,
                                len: left_visible.len(),
                            },
                            rect: dock_resp.response.rect,
                        });
                    });
                });
            let _ = resp; // panel response unused; dock hit uses inner dock rect
        }

        // Right dock stack — entire column is Favnyr drop target (not a tiny footer box)
        let right_visible: Vec<DockPane> = self
            .ui
            .dock
            .right
            .iter()
            .copied()
            .filter(|p| self.ui.pane_visible(*p))
            .collect();
        if !right_visible.is_empty() || dragging_layout {
            let resp = egui::SidePanel::right("dock_right")
                .default_width(260.0)
                .min_width(160.0)
                .frame(dock_frame())
                .show(ctx, |ui| {
                    self.render_dock_column(ui, DockColumn::Right, &right_visible);
                });
            self.dock_hits.push(DockHit {
                kind: DockHitKind::ColumnBody {
                    col: DockColumn::Right,
                    len: right_visible.len(),
                },
                rect: resp.response.rect,
            });
        }

        // Nested tile tree — never flatten L-shapes into a single strip
        if self.charts.len() > 1 && self.ui.chart_layout == ChartLayout::Overlay {
            self.ui.chart_layout = ChartLayout::Stacked;
        }
        self.ensure_chart_tree();

        let central = egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(24, 20, 32))
                    .stroke(egui::Stroke::new(
                        1.5,
                        egui::Color32::from_rgb(150, 120, 200),
                    ))
                    .inner_margin(egui::Margin::same(8.0))
                    .rounding(egui::Rounding::same(6.0)),
            )
            .show(ctx, |ui| {
                let tree = self
                    .ui
                    .chart_tree
                    .clone()
                    .unwrap_or_else(|| ChartTileTree::leaf(Uuid::nil()));
                let avail = ui.available_size();
                self.render_chart_tree(ui, &tree, avail, &[]);
            });
        self.dock_hits.push(DockHit {
            kind: DockHitKind::Canvas,
            rect: central.response.rect,
        });

        // Favnyr-style purple landing preview (dock panes + chart tiles)
        self.favnyr_dock_drop_preview(ctx);

        // Independent OS windows for popped-out charts
        self.show_chart_popouts(ctx);

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

impl ChartBenchApp {
    /// Finish Save PNG once the viewport Screenshot event arrives.
    fn poll_png_export(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_png else {
            return;
        };
        let viewport_id = pending.viewport_id;
        let image = ctx.input(|i| {
            i.events.iter().find_map(|ev| match ev {
                egui::Event::Screenshot {
                    viewport_id: vid,
                    image,
                } if *vid == viewport_id => Some(std::sync::Arc::clone(image)),
                _ => None,
            })
        });
        let Some(full) = image else {
            // Screenshot is async (next frame) — keep pending and repaint.
            ctx.request_repaint();
            return;
        };
        let pending = self.pending_png.take().expect("pending_png checked");
        let ppp = ctx.pixels_per_point();
        match crate::export_png::save_plot_png(&pending.title, &full, pending.crop, ppp) {
            Ok(path) => {
                self.status = format!("Saved PNG ({})", path.display());
            }
            Err(e) => {
                self.status = format!("Save PNG: {e}");
            }
        }
        let _ = pending.chart_id;
    }

    /// Render each popped chart in its own egui / OS viewport.
    fn show_chart_popouts(&mut self, ctx: &egui::Context) {
        let ids: Vec<Uuid> = self.popped_charts.iter().copied().collect();
        let mut close = Vec::new();
        for id in ids {
            let Some(idx) = self.charts.iter().position(|c| c.id == id) else {
                close.push(id);
                continue;
            };
            let title = self.charts[idx].title.clone();
            let vp_id = egui::ViewportId::from_hash_of(("chart_popout", id));
            let mut want_close = false;
            let mut vp = egui::ViewportBuilder::default()
                .with_title(format!("Chart-Bench — {title}"))
                .with_inner_size([960.0, 640.0])
                .with_min_inner_size([420.0, 280.0]);
            if let Some(icon) = load_window_icon() {
                vp = vp.with_icon(icon);
            }
            ctx.show_viewport_immediate(
                vp_id,
                vp,
                |ctx, class| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        want_close = true;
                    }
                    if class == egui::ViewportClass::Embedded {
                        egui::Window::new(title.clone())
                            .id(egui::Id::new(("embed_pop", id)))
                            .default_size([720.0, 480.0])
                            .show(ctx, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.small_button("Close pop-out").clicked() {
                                        want_close = true;
                                    }
                                });
                                self.draw_plot(ui, idx);
                            });
                    } else {
                        egui::CentralPanel::default()
                            .frame(
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(24, 20, 32))
                                    .inner_margin(8.0),
                            )
                            .show(ctx, |ui| {
                                ui.horizontal(|ui| {
                                    paint_chart_glyph(ui);
                                    ui.label(
                                        egui::RichText::new(&title).strong().size(15.0),
                                    );
                                    if ui.small_button("Close pop-out").clicked() {
                                        want_close = true;
                                    }
                                });
                                self.draw_plot(ui, idx);
                            });
                    }
                },
            );
            if want_close {
                close.push(id);
            }
        }
        for id in close {
            self.popped_charts.remove(&id);
        }
    }

    /// Stack dock panes in a side column.
    /// When Channels is present: no outer column scroll — Channels fills leftover height
    /// and scrolls internally; other panes keep natural height (avoids dual scrollbars).
    fn render_dock_column(&mut self, ui: &mut egui::Ui, col: DockColumn, panes: &[DockPane]) {
        ui.set_width(ui.available_width());
        let channels_at = panes.iter().position(|p| *p == DockPane::Channels);
        if let Some(ch_i) = channels_at {
            let avail = ui.available_size();
            ui.allocate_ui_with_layout(
                avail,
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(ui.available_width());
                    // Bottom stack first (hint + panes below Channels), then fill above
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        ui.set_width(ui.available_width());
                        ui.small("drag titles · drop top/bottom to stack");
                        ui.add_space(4.0);
                        for pane in panes[ch_i + 1..].iter().rev() {
                            self.dock_section(ui, col, *pane, false);
                        }
                        let rem = ui.available_size();
                        ui.allocate_ui_with_layout(
                            rem,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(ui.available_width());
                                for pane in &panes[..ch_i] {
                                    self.dock_section(ui, col, *pane, false);
                                }
                                let ch_size = egui::vec2(
                                    ui.available_width(),
                                    ui.available_height().max(72.0),
                                );
                                ui.allocate_ui_with_layout(
                                    ch_size,
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        self.dock_section(ui, col, DockPane::Channels, true);
                                    },
                                );
                            },
                        );
                    });
                },
            );
        } else {
            // No Channels fill pane — outer scroll for tall Info/Saved/Files stacks
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for pane in panes {
                        self.dock_section(ui, col, *pane, false);
                    }
                    ui.add_space(4.0);
                    ui.small("drag titles · drop top/bottom to stack");
                });
        }
    }

    fn dock_section(
        &mut self,
        ui: &mut egui::Ui,
        _col: DockColumn,
        pane: DockPane,
        fill: bool,
    ) {
        let frame = egui::Frame::none()
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(90, 70, 130),
            ))
            .inner_margin(egui::Margin::same(6.0))
            .rounding(4.0);

        let body = |ui: &mut egui::Ui, app: &mut Self| {
            // Draggable title — rearrange via DnD (no chevron move buttons)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let id = egui::Id::new(("dock_hdr", pane));
                let drag = ui.dnd_drag_source(id, pane, |ui| {
                    let fill = if ui.ctx().is_being_dragged(id) {
                        egui::Color32::from_rgb(90, 70, 140)
                    } else {
                        egui::Color32::from_rgb(48, 40, 68)
                    };
                    egui::Frame::none()
                        .fill(fill)
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(150, 120, 200),
                        ))
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                        .rounding(4.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if pane == DockPane::Table {
                                    paint_table_glyph(ui, egui::vec2(14.0, 12.0));
                                } else {
                                    ui.label(
                                        egui::RichText::new(pane.glyph())
                                            .strong()
                                            .size(14.0)
                                            .color(egui::Color32::from_rgb(235, 225, 255)),
                                    );
                                }
                                ui.label(
                                    egui::RichText::new(pane.title())
                                        .strong()
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(235, 225, 255)),
                                );
                            });
                        });
                });
                drag.response.on_hover_text(
                    "Drag title — drop on a column: stack above or below (or take empty column)",
                );
                // Files header: New chart (empty leaf — drag title to place)
                if pane == DockPane::Files
                    && files_new_chart_button(ui)
                        .on_hover_text("Add empty chart — then drag its title into place")
                        .clicked()
                {
                    app.add_empty_chart();
                }
            });

            ui.add_space(4.0);
            match pane {
                DockPane::Files => app.files_pane(ui),
                DockPane::Channels => app.channels_pane(ui),
                DockPane::Info => app.info_pane(ui),
                DockPane::Saved => app.saved_pane(ui),
                DockPane::Table => app.table_pane(ui),
            };
        };

        if fill {
            let size = ui.available_size();
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_height(size.y);
                frame.show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    body(ui, self);
                });
            });
        } else {
            frame.show(ui, |ui| body(ui, self));
            ui.add_space(6.0);
        }
    }

    /// Recursively render nested Rows/Cols tile tree (L-shapes stay nested).
    /// `path` indexes into the live `chart_tree` for splitter ratio writes (`false`=first).
    fn render_chart_tree(
        &mut self,
        ui: &mut egui::Ui,
        node: &ChartTileTree,
        size: egui::Vec2,
        path: &[bool],
    ) {
        match node {
            ChartTileTree::Leaf { chart_id } => {
                if *chart_id == Uuid::nil() || self.charts.is_empty() {
                    return;
                }
                let Some(idx) = self.charts.iter().position(|c| c.id == *chart_id) else {
                    return;
                };
                // Always top-down — Cols parent is left_to_right; allocate_ui would
                // inherit that and put the title chrome as a stray bar beside the plot.
                let tile = ui.allocate_ui_with_layout(
                    size,
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_size(size);
                        ui.set_max_size(size);
                        ui.set_clip_rect(ui.max_rect());
                        egui::Frame::none()
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                egui::Color32::from_rgb(100, 80, 140),
                            ))
                            .inner_margin(4.0)
                            .rounding(4.0)
                            .show(ui, |ui| {
                                self.chart_ui(ui, idx);
                            });
                    },
                );
                self.dock_hits.push(DockHit {
                    kind: DockHitKind::ChartTile {
                        index: idx,
                        chart_id: *chart_id,
                    },
                    rect: tile.response.rect,
                });
            }
            ChartTileTree::Split {
                dir,
                ratio,
                first,
                second,
            } => {
                let r = ratio.clamp(0.15, 0.85);
                let gap = 6.0_f32;
                let mut path_first = path.to_vec();
                path_first.push(false);
                let mut path_second = path.to_vec();
                path_second.push(true);
                match dir {
                    TileSplit::Rows => {
                        let usable = (size.y - gap).max(80.0);
                        let h1 = (usable * r).max(40.0);
                        let h2 = (usable - h1).max(40.0);
                        ui.allocate_ui_with_layout(
                            size,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.render_chart_tree(
                                    ui,
                                    first,
                                    egui::vec2(size.x, h1),
                                    &path_first,
                                );
                                if let Some(nr) =
                                    tile_splitter(ui, false, size.x, gap, usable, r, path)
                                {
                                    if let Some(tree) = self.ui.chart_tree.as_mut() {
                                        tree.set_ratio_at(path, nr);
                                    }
                                }
                                self.render_chart_tree(
                                    ui,
                                    second,
                                    egui::vec2(size.x, h2),
                                    &path_second,
                                );
                            },
                        );
                    }
                    TileSplit::Cols => {
                        let usable = (size.x - gap).max(240.0);
                        let w1 = (usable * r).max(120.0);
                        let w2 = (usable - w1).max(120.0);
                        ui.allocate_ui_with_layout(
                            size,
                            egui::Layout::left_to_right(egui::Align::Min),
                            |ui| {
                                self.render_chart_tree(
                                    ui,
                                    first,
                                    egui::vec2(w1, size.y),
                                    &path_first,
                                );
                                if let Some(nr) =
                                    tile_splitter(ui, true, size.y, gap, usable, r, path)
                                {
                                    if let Some(tree) = self.ui.chart_tree.as_mut() {
                                        tree.set_ratio_at(path, nr);
                                    }
                                }
                                self.render_chart_tree(
                                    ui,
                                    second,
                                    egui::vec2(w2, size.y),
                                    &path_second,
                                );
                            },
                        );
                    }
                }
            }
        }
    }

    /// Apply a resolved Favnyr dock/chart drop (Swap or nested split).
    fn apply_favnyr_commit(&mut self, ctx: &egui::Context, commit: FavnyrCommit, label: &str) {
        match commit {
            FavnyrCommit::Dock { pane, col, idx } => {
                let _ = egui::DragAndDrop::take_payload::<DockPane>(ctx);
                self.ui.dock.move_pane(pane, col, idx);
                self.status = format!("{} → {:?} ({})", pane.title(), col, label);
            }
            FavnyrCommit::ChartSwap { a, b } => {
                let _ = egui::DragAndDrop::take_payload::<ChartDrag>(ctx);
                self.swap_chart_tiles(a, b);
            }
            FavnyrCommit::ChartSplit {
                from,
                target,
                edge,
            } => {
                let _ = egui::DragAndDrop::take_payload::<ChartDrag>(ctx);
                self.nest_chart_split(from, target, edge);
                self.status = format!("Chart split ({label})");
            }
        }
    }

    /// Purple translucent Favnyr landing preview while dragging a dock pane or chart tile.
    fn favnyr_dock_drop_preview(&mut self, ctx: &egui::Context) {
        let dragging_pane = egui::DragAndDrop::payload::<DockPane>(ctx).map(|p| *p);
        let dragging_chart = egui::DragAndDrop::payload::<ChartDrag>(ctx).map(|p| p.0);
        let released = ctx.input(|i| i.pointer.any_released());

        // Plot Uuid drop zones call take_payload::<Uuid> on release, which clears ANY
        // payload (including ChartDrag) when the type mismatches — so by the time we
        // run here the chart payload may already be gone. Commit the stashed landing.
        if dragging_pane.is_none() && dragging_chart.is_none() {
            if released {
                if let Some(commit) = self.pending_favnyr.take() {
                    let label = match commit {
                        FavnyrCommit::ChartSplit { .. } => "SPLIT",
                        FavnyrCommit::ChartSwap { .. } => "Swap",
                        FavnyrCommit::Dock { .. } => "stack",
                    };
                    self.apply_favnyr_commit(ctx, commit, label);
                }
            } else if !egui::DragAndDrop::has_any_payload(ctx) {
                self.pending_favnyr = None;
            }
            return;
        }

        let Some(pos) = ctx
            .pointer_interact_pos()
            .or_else(|| ctx.pointer_latest_pos())
        else {
            return;
        };

        let Some(hit) = pick_dock_hit(&self.dock_hits, pos) else {
            return;
        };

        let action = resolve_favnyr_action(&hit, pos, dragging_pane, dragging_chart);
        let Some(mut action) = action else {
            return;
        };

        // Resolve ChartDrag index (stashed as u128) → real chart Uuid
        if let Some(from_idx) = dragging_chart {
            let Some(from_id) = self.charts.get(from_idx).map(|c| c.id) else {
                return;
            };
            match &mut action.commit {
                FavnyrCommit::ChartSwap { a, b } => {
                    *a = from_id;
                    if *b == Uuid::nil() {
                        // Canvas center — no-op
                        self.pending_favnyr = None;
                        return;
                    }
                    if *a == *b {
                        self.pending_favnyr = None;
                        return;
                    }
                }
                FavnyrCommit::ChartSplit {
                    from,
                    target,
                    edge,
                } => {
                    *from = from_id;
                    if *target == Uuid::nil() {
                        // Root-level nest: pick a leaf to split against by edge
                        // Prefer first/last leaf as anchor for North/West vs South/East
                        self.ensure_chart_tree();
                        let mut ids = Vec::new();
                        if let Some(t) = &self.ui.chart_tree {
                            t.collect_ids(&mut ids);
                        }
                        ids.retain(|id| *id != Uuid::nil());
                        let anchor = match edge {
                            TileEdge::North | TileEdge::West => ids.first().copied(),
                            TileEdge::South | TileEdge::East => ids.last().copied(),
                        };
                        if let Some(anchor) = anchor {
                            if anchor == from_id {
                                // Only one leaf / self-anchor — cannot move onto self
                                self.pending_favnyr = None;
                                return;
                            }
                            *target = anchor;
                        } else {
                            self.pending_favnyr = None;
                            return;
                        }
                    } else if *target == from_id {
                        // Edge on own tile — no-op (never clone on drag)
                        self.pending_favnyr = None;
                        return;
                    }
                }
                FavnyrCommit::Dock { .. } => {}
            }
        }

        // Favnyr chart overlay: center Swap pill + T/R/B/L targets
        if matches!(hit.kind, DockHitKind::ChartTile { .. }) && dragging_chart.is_some() {
            paint_chart_favnyr_overlay(ctx, hit.rect, action.landing);
        } else {
            paint_favnyr_overlay(ctx, hit.rect, action.preview, action.label, action.landing);
        }
        self.pending_favnyr = Some(action.commit);

        if released {
            self.pending_favnyr = None;
            self.apply_favnyr_commit(ctx, action.commit, action.label);
        }
    }

    fn files_pane(&mut self, ui: &mut egui::Ui) {
        // Top-right: Open + Recent (paths not currently open, ~25 MRU)
        let mut open_clicked = false;
        let mut recent_pick: Option<String> = None;
        let mut load_demo = false;
        let open_paths: HashSet<String> = self.datasets.iter().map(|d| d.path.clone()).collect();
        let recent_closed: Vec<String> = self
            .recent_files
            .iter()
            .filter(|p| !open_paths.contains(*p))
            .cloned()
            .collect();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let recent_label = if recent_closed.is_empty() {
                    "Recent".to_string()
                } else {
                    format!("Recent ({})", recent_closed.len().min(25))
                };
                // Same painted height/chrome as Open… (aligned toolbar row)
                let recent_resp = files_recent_button(ui, &recent_label)
                    .on_hover_text("Recently closed files / Demo sample");
                let popup_id = ui.make_persistent_id("files_recent_popup");
                if recent_resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(
                    ui,
                    popup_id,
                    &recent_resp,
                    egui::popup::PopupCloseBehavior::CloseOnClick,
                    |ui| {
                        ui.set_min_width(recent_resp.rect.width().max(160.0));
                        if recent_closed.is_empty() {
                            ui.weak("No recent closed files");
                        } else {
                            for p in &recent_closed {
                                let name = std::path::Path::new(p)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(p.as_str());
                                if ui
                                    .selectable_label(false, name)
                                    .on_hover_text(p)
                                    .clicked()
                                {
                                    recent_pick = Some(p.clone());
                                }
                            }
                        }
                        ui.separator();
                        if ui
                            .selectable_label(false, "Demo sample…")
                            .on_hover_text("Load samples/demo_multiline.csv")
                            .clicked()
                        {
                            load_demo = true;
                        }
                    },
                );
                if files_open_button(ui)
                    .on_hover_text("Open CSV / TSV / XLSX / Parquet (Ctrl+O)")
                    .clicked()
                {
                    open_clicked = true;
                }
            });
        });
        if open_clicked {
            self.pick_and_open_file();
        }
        if let Some(p) = recent_pick {
            if let Err(e) = self.open_path(PathBuf::from(p)) {
                self.status = format!("Open failed: {e}");
            }
        }
        if load_demo {
            self.open_demo();
        }

        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if hovering {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(90, 70, 160, 60))
                .stroke(egui::Stroke::new(
                    1.5_f32,
                    egui::Color32::from_rgb(180, 150, 255),
                ))
                .inner_margin(egui::Margin::same(6.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.strong("Drop to open CSV / TSV / XLSX / Parquet");
                });
            ui.add_space(4.0);
        }

        if self.datasets.is_empty() && !hovering {
            ui.weak("No files open. Drop a CSV / XLSX, or use Open / Recent.");
            return;
        }

        let mut toggle_visible: Option<(Uuid, bool)> = None;
        let mut select: Option<Uuid> = None;
        let mut remove: Option<Uuid> = None;

        // Snapshot rows so the table body can borrow without holding &self.datasets
        let rows: Vec<(Uuid, String, usize, bool, bool)> = self
            .datasets
            .iter()
            .map(|ds| {
                (
                    ds.id,
                    ds.display_name.clone(),
                    ds.record_count(),
                    ds.channels_visible,
                    Some(ds.id) == self.selected_dataset,
                )
            })
            .collect();

        // Compact rows; tight X↔checkbox / X↔name gaps via item_spacing
        let row_h = 18.0_f32;
        let header_h = 16.0_f32;
        let list_max_h = row_h * 10.0;
        let prev_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 1.0);

        use egui_extras::{Column, TableBuilder};
        TableBuilder::new(ui)
            .id_salt("files_table")
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(row_h)
            .max_scroll_height(list_max_h)
            .auto_shrink([false, true])
            .column(Column::exact(16.0)) // Channels-visible / Follow
            .column(Column::exact(14.0)) // Close
            .column(Column::remainder().clip(true).at_least(48.0)) // Name
            .column(Column::auto().at_least(32.0)) // Rows
            .header(header_h, |mut header| {
                // Blank over row Follow/visibility checkboxes (header glyph did nothing)
                header.col(|_ui| {});
                header.col(|_ui| {});
                header.col(|ui| {
                    ui.small(egui::RichText::new("File").strong());
                });
                header.col(|ui| {
                    ui.small(egui::RichText::new("Rows").strong());
                });
            })
            .body(|mut body| {
                for (id, name, records, visible_val, selected) in &rows {
                    body.row(row_h, |mut row| {
                        row.set_selected(*selected);
                        // Visible-in-Channels filter (not file-growth follow)
                        row.col(|ui| {
                            // Left accent bar on the selected row
                            if *selected {
                                let r = ui.max_rect();
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        r.min,
                                        egui::vec2(3.0, r.height()),
                                    ),
                                    0.0,
                                    egui::Color32::from_rgb(200, 160, 255),
                                );
                            }
                            let mut visible = *visible_val;
                            if ui
                                .add(egui::Checkbox::without_text(&mut visible))
                                .on_hover_text(
                                    "Show this file's channels in the Channels list (charts keep plotted series)",
                                )
                                .changed()
                            {
                                toggle_visible = Some((*id, visible));
                            }
                        });
                        // Close — painted red X (no font tofu / button padding)
                        row.col(|ui| {
                            let close = paint_close_x_glyph(ui);
                            if close.on_hover_text("Close file").clicked() {
                                remove = Some(*id);
                            }
                        });
                        // Name
                        row.col(|ui| {
                            let text = if *selected {
                                egui::RichText::new(name)
                                    .strong()
                                    .color(egui::Color32::WHITE)
                            } else {
                                egui::RichText::new(name)
                                    .color(egui::Color32::from_rgb(200, 190, 220))
                            };
                            ui.add(egui::Label::new(text).truncate())
                                .on_hover_text(name);
                        });
                        // Row count
                        row.col(|ui| {
                            let t = if *selected {
                                egui::RichText::new(format!("{records}"))
                                    .strong()
                                    .color(egui::Color32::WHITE)
                                    .size(11.0)
                            } else {
                                egui::RichText::new(format!("{records}")).size(11.0)
                            };
                            ui.label(t);
                        });
                        if row.response().clicked() {
                            select = Some(*id);
                        }
                    });
                }
            });
        ui.spacing_mut().item_spacing = prev_spacing;

        if let Some(id) = select {
            self.selected_dataset = Some(id);
        }
        if let Some(id) = remove {
            self.datasets.retain(|d| d.id != id);
            if self.selected_dataset == Some(id) {
                self.selected_dataset = self.datasets.first().map(|d| d.id);
            }
        }
        if let Some((id, visible)) = toggle_visible {
            if let Some(ds) = self.datasets.iter_mut().find(|d| d.id == id) {
                ds.channels_visible = visible;
            }
        }
    }

    fn filtered_channels(&self) -> Vec<(Uuid, Uuid, String, Vec<f64>)> {
        // (dataset_id, channel_id, display_name, spark samples)
        let filter = self.channel_filter.to_lowercase();
        let mut out = Vec::new();
        for ds in &self.datasets {
            // Unchecked Files visibility = hide from Channels list only
            if !ds.channels_visible {
                continue;
            }
            for ch in &ds.channels {
                if !filter.is_empty()
                    && !ch.display_name.to_lowercase().contains(&filter)
                    && !ch.name.to_lowercase().contains(&filter)
                {
                    continue;
                }
                let spark = ds
                    .columns
                    .get(ch.column_index)
                    .map(|c| sparkline_by_x(&ds.x, c, 48))
                    .unwrap_or_default();
                out.push((ds.id, ch.id, ch.display_name.clone(), spark));
            }
        }
        out
    }

    fn channels_pane(&mut self, ui: &mut egui::Ui) {
        let active = self.active_chart.min(self.charts.len().saturating_sub(1));
        // Stay within the dock column — never grow the left panel from content
        ui.set_width(ui.available_width());
        // Active chart picker — tile click and this dropdown stay in sync
        let titles: Vec<String> = self.charts.iter().map(|c| c.title.clone()).collect();
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.small("Chart");
            let label = titles
                .get(active)
                .cloned()
                .unwrap_or_else(|| "—".into());
            egui::ComboBox::from_id_salt("channels_active_chart")
                .selected_text(label)
                .width((ui.available_width() - 4.0).max(60.0))
                .show_ui(ui, |ui| {
                    for (i, title) in titles.iter().enumerate() {
                        if ui.selectable_value(&mut self.active_chart, i, title).changed() {
                            self.status = format!("Editing {title}");
                        }
                    }
                });
        });
        ui.small("Up/Down navigate · Space/Enter = select · Left/Right = L/R Y");
        // Filter owns Backspace/Delete/arrows/selection — never run list shortcuts while it has focus
        let mut filter_focused = false;
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.label("Filter");
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.channel_filter)
                    .id_salt("channels_filter")
                    .desired_width(f32::INFINITY),
            );
            filter_focused = edit.has_focus();
            if filter_focused {
                self.channels_focused = false;
            }
            // Only show multi chrome for real multi-select (avoids width jump on single click)
            if self.channel_multi.len() > 1 {
                ui.weak(format!("{}", self.channel_multi.len()));
                if ui.small_button("×").on_hover_text("Clear selection").clicked() {
                    self.channel_multi.clear();
                }
            }
        });

        let rows = self.filtered_channels();
        if self.channel_cursor.map(|i| i >= rows.len()).unwrap_or(false) {
            self.channel_cursor = rows.len().checked_sub(1);
        }

        // List shortcuts only when the channel list is focused (not Filter / other text input)
        let typing = filter_focused || ui.ctx().wants_keyboard_input();
        let pointer_moved = ui.input(|i| i.pointer.delta().length_sq() > 0.0);
        let modifiers = ui.input(|i| i.modifiers);
        if self.channels_focused && !typing && !rows.is_empty() {
            let mut key_axis_left = false;
            let mut key_axis_right = false;
            let mut key_space = false;
            let mut key_enter = false;
            let mut cursor = self.channel_cursor;
            let mut moved_by_key = false;
            // Consume so keys don't leak to plots / focus traversal while list owns them
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                let next = cursor.map(|c| c + 1).unwrap_or(0);
                cursor = Some(next.min(rows.len() - 1));
                moved_by_key = true;
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                cursor = Some(cursor.unwrap_or(0).saturating_sub(1));
                moved_by_key = true;
            }
            // Space/Enter = list selection; ←/→ = left/right Y axis on active chart
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
                key_space = true;
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                key_enter = true;
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)) {
                key_axis_left = true;
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)) {
                key_axis_right = true;
            }
            if moved_by_key || key_space || key_enter || key_axis_left || key_axis_right {
                self.channel_nav_from_keys = true;
            }
            self.channel_cursor = cursor;
            // Space: focus/select cursor row (toggle in multi). Enter: add cursor to selection.
            if let Some(ci) = cursor {
                if let Some((did, cid, _, _)) = rows.get(ci) {
                    if key_space {
                        if self.channel_multi.contains(cid) {
                            self.channel_multi.remove(cid);
                        } else {
                            self.channel_multi.insert(*cid);
                        }
                        self.channel_anchor = Some(ci);
                        self.selected_channel = Some(*cid);
                        self.selected_dataset = Some(*did);
                    }
                    if key_enter {
                        self.channel_multi.insert(*cid);
                        self.channel_anchor = Some(ci);
                        self.selected_channel = Some(*cid);
                        self.selected_dataset = Some(*did);
                    }
                }
            }
            // ←/→ apply to multi-selection when present, else cursor row
            let targets: Vec<Uuid> = if !self.channel_multi.is_empty()
                && (key_axis_left || key_axis_right)
            {
                self.channel_multi.iter().copied().collect()
            } else if let Some(ci) = cursor {
                rows.get(ci).map(|(_, cid, _, _)| vec![*cid]).unwrap_or_default()
            } else {
                Vec::new()
            };
            if key_axis_left {
                for cid in &targets {
                    self.selected_channel = Some(*cid);
                    if let Some((did, _, _, _)) = rows.iter().find(|(_, c, _, _)| *c == *cid) {
                        self.selected_dataset = Some(*did);
                    }
                    self.toggle_channel_on_chart(active, *cid, AxisSide::Left);
                }
            }
            if key_axis_right {
                for cid in &targets {
                    self.selected_channel = Some(*cid);
                    if let Some((did, _, _, _)) = rows.iter().find(|(_, c, _, _)| *c == *cid) {
                        self.selected_dataset = Some(*did);
                    }
                    self.toggle_channel_on_chart(active, *cid, AxisSide::Right);
                }
            }
        }

        let mut select_ch: Option<(Uuid, Uuid)> = None;
        let mut axis_left: Vec<Uuid> = Vec::new();
        let mut axis_right: Vec<Uuid> = Vec::new();
        let mut hover_preview: Option<Uuid> = None;
        let mut hover_index: Option<usize> = None;
        let mut row_rects: Vec<(usize, egui::Rect, Uuid)> = Vec::new();
        let mut range_select: Option<usize> = None;
        let mut ctrl_toggle: Option<(usize, Uuid)> = None;
        let mut plain_click: Option<(usize, Uuid, Uuid)> = None; // i, did, cid

        let shift_held = modifiers.shift;
        let ctrl_held = modifiers.command || modifiers.ctrl;

        // Fill leftover dock height; scroll only inside this list (no outer column scroll)
        let list_resp = egui::ScrollArea::vertical()
            .id_salt("channels_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Pack rows tightly — less vertical whitespace between channels
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().button_padding = egui::vec2(3.0, 0.0);
                // Group by dataset for collapsing headers
                let mut last_ds = None;
                let cursor = self.channel_cursor;
                let multi = self.channel_multi.clone();
                for (i, (did, cid, name, spark)) in rows.iter().enumerate() {
                    if last_ds != Some(*did) {
                        last_ds = Some(*did);
                        let ds_name = self
                            .datasets
                            .iter()
                            .find(|d| d.id == *did)
                            .map(|d| d.display_name.clone())
                            .unwrap_or_else(|| "file".into());
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("— {ds_name} —")).weak().size(11.0),
                            )
                            .truncate(),
                        );
                    }
                    let on_axis = self.channel_axis_on_chart(active, *cid);
                    let on_left = on_axis == Some(AxisSide::Left);
                    let on_right = on_axis == Some(AxisSide::Right);
                    let on_chart = on_axis.is_some();
                    let kb_cursor = cursor == Some(i);
                    let in_multi = multi.contains(cid);
                    // Fixed 1px stroke (transparent when idle) so selection never widens rows
                    let stroke_color = if in_multi {
                        egui::Color32::from_rgb(160, 200, 255)
                    } else if kb_cursor {
                        egui::Color32::from_rgb(180, 150, 230)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let row = egui::Frame::none()
                        .fill(if in_multi {
                            egui::Color32::from_rgb(56, 48, 96)
                        } else if on_chart {
                            egui::Color32::from_rgb(42, 68, 58)
                        } else if kb_cursor {
                            egui::Color32::from_rgb(48, 40, 68)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::new(1.0_f32, stroke_color))
                        .inner_margin(egui::Margin::symmetric(3.0, 0.0))
                        .rounding(2.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_min_height(18.0);
                            ui.horizontal(|ui| {
                                ui.set_width(ui.available_width());
                                ui.spacing_mut().item_spacing.x = 4.0;
                                // L / R axis toggles — colored when this channel is on that axis
                                let l_color = if on_left {
                                    egui::Color32::from_rgb(120, 210, 160)
                                } else {
                                    egui::Color32::from_rgb(140, 140, 150)
                                };
                                let r_color = if on_right {
                                    egui::Color32::from_rgb(120, 180, 230)
                                } else {
                                    egui::Color32::from_rgb(140, 140, 150)
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("L").size(11.0).color(l_color),
                                        )
                                        .small()
                                        .fill(if on_left {
                                            egui::Color32::from_rgb(36, 72, 52)
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        }),
                                    )
                                    .on_hover_text("Toggle left Y (selection / this)")
                                    .clicked()
                                {
                                    if multi.is_empty() {
                                        axis_left.push(*cid);
                                    } else {
                                        axis_left.extend(multi.iter().copied());
                                    }
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("R").size(11.0).color(r_color),
                                        )
                                        .small()
                                        .fill(if on_right {
                                            egui::Color32::from_rgb(36, 56, 84)
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        }),
                                    )
                                    .on_hover_text("Toggle right Y (selection / this)")
                                    .clicked()
                                {
                                    if multi.is_empty() {
                                        axis_right.push(*cid);
                                    } else {
                                        axis_right.extend(multi.iter().copied());
                                    }
                                }
                                let mut text = egui::RichText::new(name).size(12.0);
                                if on_chart {
                                    text = text.color(egui::Color32::from_rgb(180, 240, 200));
                                } else if in_multi {
                                    text = text.color(egui::Color32::from_rgb(200, 220, 255));
                                }
                                let lab = ui.add(
                                    egui::Label::new(text)
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                );
                                draw_sparkline(ui, *cid, spark);
                                lab
                            })
                            .inner
                        });
                    let response = row.inner;
                    row_rects.push((i, row.response.rect, *cid));
                    // Skip channel-DnD while Shift/Ctrl select is active
                    if !shift_held && !ctrl_held {
                        response.dnd_set_drag_payload(*cid);
                    }
                    let row_clicked = response.clicked() || row.response.clicked();
                    if row_clicked && !filter_focused {
                        self.channels_focused = true;
                        self.channel_nav_from_keys = false;
                        if shift_held {
                            range_select = Some(i);
                        } else if ctrl_held {
                            ctrl_toggle = Some((i, *cid));
                        } else {
                            plain_click = Some((i, *did, *cid));
                        }
                    }
                    if response.hovered() || row.response.contains_pointer() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        // Hover must not steal focus from the Filter TextEdit
                        if !filter_focused {
                            self.channels_focused = true;
                        }
                        hover_preview = Some(*cid);
                        hover_index = Some(i);
                        if pointer_moved {
                            self.channel_nav_from_keys = false;
                        }
                    }
                }
                if rows.is_empty() {
                    ui.weak("No channels — open a file.");
                }
            });

        // Shift+drag marquee selection box over the channel list
        let list_rect = list_resp.inner_rect;
        let pointer = ui.input(|i| i.pointer.clone());
        let mut marquee_committed = false;
        if shift_held && list_rect.contains(pointer.hover_pos().unwrap_or(egui::pos2(-1.0, -1.0)))
        {
            if pointer.primary_pressed() {
                if let Some(origin) = pointer.press_origin().or(pointer.hover_pos()) {
                    self.channel_marquee = Some((origin, origin));
                }
            }
        }
        if let Some((origin, _)) = self.channel_marquee {
            if let Some(pos) = pointer.hover_pos() {
                let band = egui::Rect::from_two_pos(origin, pos);
                if pointer.primary_down() {
                    self.channel_marquee = Some((origin, pos));
                    let mut next = HashSet::new();
                    for (_, rect, cid) in &row_rects {
                        if rect.intersects(band) {
                            next.insert(*cid);
                        }
                    }
                    if !ctrl_held {
                        self.channel_multi = next;
                    } else {
                        self.channel_multi.extend(next);
                    }
                }
                if pointer.primary_released() {
                    if band.width() > 4.0 || band.height() > 4.0 {
                        marquee_committed = true;
                    }
                    self.channel_marquee = None;
                }
            }
            if !shift_held && !pointer.primary_down() {
                self.channel_marquee = None;
            }
        }
        if let Some((a, b)) = self.channel_marquee {
            let band = egui::Rect::from_two_pos(a, b);
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("channel_marquee"),
            ));
            painter.rect_filled(
                band,
                egui::Rounding::same(2.0),
                egui::Color32::from_rgba_unmultiplied(100, 150, 255, 40),
            );
            painter.rect_stroke(
                band,
                egui::Rounding::same(2.0),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(140, 180, 255)),
            );
        }

        let list_hovered = list_rect.contains(
            ui.input(|i| i.pointer.hover_pos())
                .unwrap_or(egui::pos2(-1.0, -1.0)),
        );
        // List hover/click focus only when Filter is not editing
        if list_hovered && !filter_focused {
            self.channels_focused = true;
        }
        if list_hovered && !filter_focused && ui.input(|i| i.pointer.any_click()) {
            self.channels_focused = true;
            self.channel_nav_from_keys = false;
        }

        // Apply click selection modes (skip if a marquee box just finished)
        if marquee_committed {
            // keep channel_multi from the rubber-band; set cursor to last hovered
            if let Some(i) = hover_index {
                self.channel_cursor = Some(i);
                self.channel_anchor = Some(i);
            }
        } else if let Some(i) = range_select {
            let anchor = self.channel_anchor.unwrap_or(i);
            let lo = anchor.min(i);
            let hi = anchor.max(i);
            self.channel_multi.clear();
            for (_, cid, _, _) in rows.iter().skip(lo).take(hi - lo + 1) {
                self.channel_multi.insert(*cid);
            }
            self.channel_cursor = Some(i);
            if let Some((_, cid, _, _)) = rows.get(i) {
                self.selected_channel = Some(*cid);
            }
        } else if let Some((i, cid)) = ctrl_toggle {
            if self.channel_multi.contains(&cid) {
                self.channel_multi.remove(&cid);
            } else {
                self.channel_multi.insert(cid);
            }
            self.channel_cursor = Some(i);
            self.channel_anchor = Some(i);
            self.selected_channel = Some(cid);
        } else if let Some((i, did, cid)) = plain_click {
            // Plain click selects/focuses the row — chart axis via L/R or ←/→
            self.channel_multi.clear();
            self.channel_cursor = Some(i);
            self.channel_anchor = Some(i);
            select_ch = Some((did, cid));
        }

        // Mouse over a row (and not key-driven): cursor follows hover
        if !self.channel_nav_from_keys {
            if let Some(i) = hover_index {
                self.channel_cursor = Some(i);
            }
        }

        // Preview: keys while Channels focused; hover while a row is under the pointer.
        // Leaving the list (no row hover) clears sticky preview — do not keep last dotted series.
        // Prefer hover_preview over list_rect.contains (fill-scroll layouts can desync inner_rect).
        self.channel_preview = if self.channel_nav_from_keys && self.channels_focused {
            self.channel_cursor
                .and_then(|i| rows.get(i).map(|(_, cid, _, _)| *cid))
        } else if hover_preview.is_some() {
            hover_preview
        } else {
            None
        };

        if let Some((did, cid)) = select_ch {
            self.selected_dataset = Some(did);
            self.selected_channel = Some(cid);
        }
        for cid in axis_left {
            self.toggle_channel_on_chart(active, cid, AxisSide::Left);
            self.channel_preview = None;
        }
        for cid in axis_right {
            self.toggle_channel_on_chart(active, cid, AxisSide::Right);
            self.channel_preview = None;
        }
    }

    fn info_pane(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.selected_dataset {
            if let Some(ds) = self.dataset(id) {
                let st = file_stats(ds);
                ui.label(format!("Records: {}", st.records));
                ui.label(format!("Channels: {}", st.channels));
                if st.x_is_time {
                    // Human-readable UTC range (same chrono format as hover readout)
                    ui.label(format!(
                        "Time: {} → {}",
                        format_unix_full(st.x_min),
                        format_unix_full(st.x_max)
                    ));
                } else {
                    ui.label(format!("X index: {:.0} .. {:.0}", st.x_min, st.x_max));
                }
                ui.label(format!(
                    "Spacing: {} · median Δ={:.4} · gaps={}",
                    st.spacing_class, st.median_dx, st.gap_count
                ));
                ui.label(format!(
                    "Headers: {} · data@{} · X={:?}",
                    ds.plan.header_rows, ds.plan.data_start_row, ds.plan.x_axis.mode
                ));
                ui.label(format!(
                    "Delimiter: {:?} · width {}",
                    ds.plan.delimiter as char, ds.plan.width
                ));
                if let Some(cid) = self.selected_channel {
                    if let Some(ch) = ds.channel_by_id(cid) {
                        ui.separator();
                        ui.strong(&ch.display_name);
                        if let Some(col) = ds.columns.get(ch.column_index) {
                            let cs = channel_stats(col);
                            ui.label(format!("n={} missing={}", cs.n, cs.missing));
                            ui.label(format!(
                                "min={:.4} max={:.4} mean={:.4}",
                                cs.min, cs.max, cs.mean
                            ));
                        }
                    }
                }
            }
        } else {
            ui.weak("Select a file.");
        }
    }

    fn saved_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("Session autosaves to:");
        ui.monospace(session_path().display().to_string());
        // One row, same button size as top toolbar (Open / Demo / …)
        ui.horizontal_wrapped(|ui| {
            if ui.button("Save session").clicked() {
                self.last_save = Instant::now() - Duration::from_secs(10);
                self.autosave();
                self.status = "Session saved.".into();
            }
            if ui.button("Revert session").clicked() {
                self.datasets.clear();
                self.charts = vec![ChartState::new("Chart 1")];
                self.try_restore_session();
                self.status = "Reverted to last saved session.".into();
            }
            if ui
                .button("Reset dock layout")
                .on_hover_text("Restore default Files|Channels / Info|Saved stacks")
                .clicked()
            {
                self.ui.dock = DockLayout::default();
                self.status = "Dock layout reset.".into();
            }
        });
        ui.separator();
        ui.strong("Presets");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.preset_name);
            if ui.button("Save").clicked() {
                self.save_current_as_preset();
            }
        });
        let mut apply = None;
        let mut delete = None;
        for (i, p) in self.presets.presets.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.button(&p.name).clicked() {
                    apply = Some(i);
                }
                if ui.small_button("×").clicked() {
                    delete = Some(i);
                }
            });
        }
        if let Some(i) = apply {
            self.apply_preset(i);
        }
        if let Some(i) = delete {
            self.presets.presets.remove(i);
            let _ = save_presets(&self.presets);
        }
    }

    fn table_pane(&mut self, ui: &mut egui::Ui) {
        // Cap body so Table cannot runaway-expand the dock (chart stays primary).
        let cap = ui.available_height().clamp(96.0, 200.0);
        if let Some(id) = self.selected_dataset {
            if let Some(ds) = self.dataset(id) {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .max_height(cap)
                    .show(ui, |ui| {
                        egui::Grid::new("tbl").striped(true).show(ui, |ui| {
                            ui.label("x");
                            for ch in ds.channels.iter().take(8) {
                                ui.label(&ch.name);
                            }
                            ui.end_row();
                            for r in 0..ds.record_count().min(50) {
                                ui.label(format!("{:.4}", ds.x[r]));
                                for ch in ds.channels.iter().take(8) {
                                    let v = ds.columns[ch.column_index]
                                        .get(r)
                                        .copied()
                                        .unwrap_or(f64::NAN);
                                    ui.label(if v.is_finite() {
                                        format!("{v:.3}")
                                    } else {
                                        String::new()
                                    });
                                }
                                ui.end_row();
                            }
                        });
                    });
            }
        } else {
            ui.weak("Select a file.");
        }
    }

    fn chart_ui(&mut self, ui: &mut egui::Ui, idx: usize) {
        let title = self
            .charts
            .get(idx)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        let chart_id = self.charts.get(idx).map(|c| c.id);
        let is_active = self.active_chart == idx;
        let popped = chart_id
            .map(|id| self.popped_charts.contains(&id))
            .unwrap_or(false);
        // Title chrome + equal-size icon toolbar (matches chrome height)
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Draggable chart title — Favnyr landing preview on docks / other tiles
            let id = egui::Id::new(("chart_hdr", idx));
            let drag = ui.dnd_drag_source(id, ChartDrag(idx), |ui| {
                let fill = if ui.ctx().is_being_dragged(id) {
                    egui::Color32::from_rgb(90, 70, 140)
                } else if is_active {
                    egui::Color32::from_rgb(56, 48, 88)
                } else {
                    egui::Color32::from_rgb(40, 34, 56)
                };
                egui::Frame::none()
                    .fill(fill)
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(150, 120, 200),
                    ))
                    .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            // Painted line-chart mark — font-independent
                            paint_chart_glyph(ui);
                            let mut text = egui::RichText::new(&title).strong().size(15.0);
                            if is_active {
                                text = text.color(egui::Color32::from_rgb(235, 225, 255));
                            }
                            ui.label(text);
                        });
                    });
            });
            if drag.response.clicked() {
                self.active_chart = idx;
            }
            drag.response.on_hover_text(
                "Drag to move: center = Swap; Top/Right/Bottom/Left = nest-split (never duplicates)",
            );

            // Compact icon toolbar: Options · Save PNG · Export CSV · Clear · Duplicate · Pop-out · Remove
            if let Some(c) = self.charts.get_mut(idx) {
                let open = c.show_draw_options;
                if chart_tool_button(ui, open, ChartToolIcon::Gear)
                    .on_hover_text("Options — type, thickness, style, colors, background")
                    .clicked()
                {
                    c.show_draw_options = !c.show_draw_options;
                }
            }
            if chart_tool_button(ui, false, ChartToolIcon::SavePng)
                .on_hover_text("Save PNG — capture this plot (axes + scales)")
                .clicked()
            {
                if let Some(cid) = chart_id {
                    let crop = self
                        .plot_rects
                        .get(&cid)
                        .copied()
                        .unwrap_or(egui::Rect::NOTHING);
                    self.pending_png = Some(PendingPngExport {
                        chart_id: cid,
                        title: title.clone(),
                        crop,
                        viewport_id: egui::ViewportId::ROOT,
                    });
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot);
                    self.status = "Capturing plot…".into();
                }
            }
            if chart_tool_button(ui, false, ChartToolIcon::Export)
                .on_hover_text("Export CSV")
                .clicked()
            {
                match self.export_chart_csv(idx) {
                    Ok(()) => self.status = "Exported chart CSV.".into(),
                    Err(e) => self.status = format!("Export: {e}"),
                }
            }
            if chart_tool_button(ui, false, ChartToolIcon::Clear)
                .on_hover_text("Clear series")
                .clicked()
            {
                if let Some(c) = self.charts.get_mut(idx) {
                    c.series.clear();
                    let id = c.id;
                    self.reset_plot_auto.insert(id);
                }
            }
            if chart_tool_button(ui, false, ChartToolIcon::Duplicate)
                .on_hover_text("Duplicate chart")
                .clicked()
            {
                self.duplicate_chart(idx);
            }
            if let Some(cid) = chart_id {
                let tip = if popped {
                    "Close pop-out window"
                } else {
                    "Open chart in its own OS window"
                };
                let icon = if popped {
                    ChartToolIcon::PopIn
                } else {
                    ChartToolIcon::PopOut
                };
                if chart_tool_button(ui, popped, icon)
                    .on_hover_text(tip)
                    .clicked()
                {
                    if popped {
                        self.popped_charts.remove(&cid);
                    } else {
                        self.popped_charts.insert(cid);
                        self.status = format!("Popped out {title}.");
                    }
                }
            }
            if self.charts.len() > 1
                && chart_tool_button(ui, false, ChartToolIcon::Close)
                    .on_hover_text("Remove chart")
                    .clicked()
            {
                self.remove_chart_at(idx);
                return;
            }
            if let Some(c) = self.charts.get_mut(idx) {
                ui.checkbox(&mut c.follow_tail, "Follow");
            }
        });
        if self
            .charts
            .get(idx)
            .map(|c| c.show_draw_options)
            .unwrap_or(false)
        {
            self.chart_draw_options_ui(ui, idx);
        }
        // Interacting with a chart makes it active for channel adds
        if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|i| i.pointer.any_pressed()) {
            self.active_chart = idx;
        }

        // Plot fills remaining chart area (no series checkbox strip under it).
        // Only take Uuid payloads — egui's take_payload clears ANY payload on type
        // mismatch, which was aborting chart-tile SPLIT drops over the plot.
        let plot_resp = egui::Frame::none().show(ui, |ui| {
            self.draw_plot(ui, idx);
        });
        if plot_resp.response.contains_pointer()
            && ui.input(|i| i.pointer.any_released())
            && egui::DragAndDrop::has_payload_of_type::<Uuid>(ui.ctx())
        {
            if let Some(ch) = egui::DragAndDrop::take_payload::<Uuid>(ui.ctx()) {
                let axis = if ui.input(|i| i.modifiers.shift) {
                    AxisSide::Right
                } else {
                    AxisSide::Left
                };
                self.add_channel_to_chart(idx, *ch, axis);
            }
        }
    }

    /// Per-chart draw style: type, thickness, stroke, colors, overlays.
    fn chart_draw_options_ui(&mut self, ui: &mut egui::Ui, idx: usize) {
        let labels: Vec<String> = self
            .charts
            .get(idx)
            .map(|chart| {
                chart
                    .series
                    .iter()
                    .map(|s| {
                        self.datasets
                            .iter()
                            .find_map(|d| {
                                d.channel_by_id(s.channel_id)
                                    .map(|c| c.display_name.clone())
                            })
                            .unwrap_or_else(|| "series".into())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(chart) = self.charts.get_mut(idx) else {
            return;
        };
        let chart_id = chart.id;
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(36, 30, 48))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(100, 80, 140),
            ))
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .rounding(4.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Type");
                    egui::ComboBox::from_id_salt(("chart_kind", chart_id))
                        .selected_text(match chart.draw.kind {
                            ChartKind::Line => "Line",
                            ChartKind::Step => "Step",
                            ChartKind::Points => "Points",
                            ChartKind::Bar => "Bar",
                            ChartKind::Area => "Area / field",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut chart.draw.kind, ChartKind::Line, "Line");
                            ui.selectable_value(&mut chart.draw.kind, ChartKind::Step, "Step");
                            ui.selectable_value(
                                &mut chart.draw.kind,
                                ChartKind::Points,
                                "Points",
                            );
                            ui.selectable_value(&mut chart.draw.kind, ChartKind::Bar, "Bar");
                            ui.selectable_value(
                                &mut chart.draw.kind,
                                ChartKind::Area,
                                "Area / field",
                            );
                        });
                    ui.label("Stroke");
                    egui::ComboBox::from_id_salt(("chart_stroke", chart_id))
                        .selected_text(match chart.draw.stroke {
                            StrokeStyleKind::Solid => "Solid",
                            StrokeStyleKind::Dashed => "Dashed",
                            StrokeStyleKind::Dotted => "Dotted",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut chart.draw.stroke,
                                StrokeStyleKind::Solid,
                                "Solid",
                            );
                            ui.selectable_value(
                                &mut chart.draw.stroke,
                                StrokeStyleKind::Dashed,
                                "Dashed",
                            );
                            ui.selectable_value(
                                &mut chart.draw.stroke,
                                StrokeStyleKind::Dotted,
                                "Dotted",
                            );
                        });
                    ui.label("Width");
                    ui.add(
                        egui::DragValue::new(&mut chart.draw.line_width)
                            .speed(0.1)
                            .clamp_range(0.5..=12.0)
                            .suffix(" px"),
                    );
                    ui.checkbox(&mut chart.draw.show_markers, "Markers");
                    if chart.draw.show_markers || chart.draw.kind == ChartKind::Points {
                        ui.add(
                            egui::DragValue::new(&mut chart.draw.marker_size)
                                .speed(0.2)
                                .clamp_range(1.0..=16.0)
                                .prefix("size "),
                        );
                    }
                    if chart.draw.kind == ChartKind::Bar {
                        ui.label("Bar Δx");
                        ui.add(
                            egui::DragValue::new(&mut chart.draw.bar_width)
                                .speed(0.05)
                                .clamp_range(0.0..=1.0e6)
                                .suffix(" (0=auto)"),
                        );
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut chart.draw.show_legend, "Legend");
                    ui.checkbox(&mut chart.draw.show_grid, "Grid");
                    ui.checkbox(&mut chart.draw.show_crosshair, "Crosshair");
                    ui.checkbox(&mut chart.draw.show_zero_line, "Zero line");
                    ui.label("Background");
                    ui.color_edit_button_srgb(&mut chart.draw.background)
                        .on_hover_text("Chart plot background color");
                    if ui.small_button("Reset style").clicked() {
                        chart.draw = ChartDrawOptions::default();
                    }
                });
                if !chart.series.is_empty() {
                    ui.separator();
                    ui.small("Series colors");
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in labels.iter().enumerate() {
                            if let Some(s) = chart.series.get_mut(i) {
                                let mut rgb = s.color;
                                if ui.color_edit_button_srgb(&mut rgb).changed() {
                                    s.color = rgb;
                                }
                                ui.checkbox(&mut s.visible, name.as_str());
                            }
                        }
                    });
                }
            });
        ui.add_space(4.0);
    }

    fn draw_plot(&mut self, ui: &mut egui::Ui, idx: usize) {
        let chart = match self.charts.get(idx) {
            Some(c) => c.clone(),
            None => return,
        };
        let draw = chart.draw.clone();

        let mut left: Vec<(String, [u8; 3], Vec<[f64; 2]>)> = Vec::new();
        let mut right: Vec<(String, [u8; 3], Vec<[f64; 2]>)> = Vec::new();

        for s in &chart.series {
            if !s.visible {
                continue;
            }
            for ds in &self.datasets {
                if let Some(ch) = ds.channel_by_id(s.channel_id) {
                    if let Some(col) = ds.columns.get(ch.column_index) {
                        let step = (col.len() / 40_000).max(1);
                        let mut pts = Vec::new();
                        for (i, &y) in col.iter().enumerate() {
                            if i % step != 0 || !y.is_finite() {
                                continue;
                            }
                            let x = ds.x.get(i).copied().unwrap_or(i as f64);
                            pts.push([x, y]);
                        }
                        // Unsorted time (e.g. NYC taxi Parquet) zigzags unless X is ordered.
                        sort_plot_points_by_x(&mut pts);
                        match s.axis {
                            AxisSide::Left => left.push((ch.display_name.clone(), s.color, pts)),
                            AxisSide::Right => {
                                right.push((format!("{} [R]", ch.display_name), s.color, pts))
                            }
                        }
                    }
                }
            }
        }

        // Faded/dotted preview for hovered or ↑↓ channel on the active chart only
        let mut preview: Option<(String, Vec<[f64; 2]>)> = None;
        if self.active_chart == idx {
            if let Some(pid) = self.channel_preview {
                let already = chart.series.iter().any(|s| s.channel_id == pid);
                if !already {
                    preview = self.preview_points(pid);
                }
            }
        }

        // Dual-Y = one plot with L + R axes (egui_plot shares one Y scale, so R is remapped).
        let dual = self.ui.y_axis_mode == YAxisMode::Multi && !right.is_empty();
        let avail = ui.available_height().max(120.0);
        let left_range = series_y_range(&left);
        let right_range = series_y_range(&right);
        // Map right-axis series into left Y space so both draw in a single plot.
        let remapped_right: Vec<(String, [u8; 3], Vec<[f64; 2]>)> =
            if dual {
                match (left_range, right_range) {
                    (Some(lr), Some(rr)) if !left.is_empty() => remap_series_y(&right, rr, lr),
                    _ => right.clone(),
                }
            } else {
                Vec::new()
            };
        let overlay_series: Vec<&(String, [u8; 3], Vec<[f64; 2]>)> = if dual {
            if left.is_empty() {
                right.iter().collect()
            } else {
                left.iter().chain(remapped_right.iter()).collect()
            }
        } else {
            // Y: shared — every series on one scale.
            left.iter().chain(right.iter()).collect()
        };

        let x_is_time = self.datasets.iter().any(|d| d.x_is_time);
        // Legend-style cursor readout: (name, rgb, y) — native values, not remapped.
        let mut cursor_readout: Vec<(String, [u8; 3], f64)> = Vec::new();
        let mut cursor_x: Option<f64> = None;
        let mut cursor_screen: Option<egui::Pos2> = None;

        // Luminance contrast: light plot bg → faded dark grid/scale ink; dark → quiet light.
        // Custom crosshair + per-series Y (disable default single x/y tip to avoid clutter).
        // with_plot_contrast fills axes + plot chrome from Options background.
        let bg = draw.background;
        // Full plot widget (axes + plot) for Save PNG — response.rect is data-only.
        let full_plot_widget = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), avail),
        );
        crate::chart_theme::with_plot_contrast(ui, bg, |ui| {
            // Link X only when this tile has drawable data — an empty sibling in the
            // shared link group poisons bounds (blank plots after Add chart).
            let has_plot_data = !overlay_series.is_empty() || preview.is_some();
            let mut plot_l = Plot::new(("plot_l", chart.id)).height(avail);
            if has_plot_data {
                plot_l = plot_l.link_axis(egui::Id::new("chart_bench_shared_x"), true, false);
            }
            plot_l = plot_l
                // No hover/wheel/edge axis pan — wheel zooms X via apply_wheel_zoom_x_clamped; drag pans X.
                // Y stays locked auto (fit visible series); do not drag/zoom Y.
                .allow_scroll(false)
                .allow_drag(egui::Vec2b::new(true, false))
                .allow_zoom(false) // custom wheel zoom: X only, fit-all floor
                .allow_boxed_zoom(false)
                .allow_double_click_reset(true)
                .auto_bounds(egui::Vec2b::new(true, true))
                .show_background(true)
                .show_grid(draw.show_grid)
                .show_x(false)
                .show_y(false);
            if draw.show_legend {
                plot_l = plot_l.legend(Legend::default());
            }
            if x_is_time {
                plot_l = plot_l.x_axis_formatter(|mark, _range| format_unix_hms(mark.value));
            }
            // Dual-Y: left ticks = left scale; right ticks remap left-space → right values.
            if dual {
                if left.is_empty() {
                    plot_l = plot_l.custom_y_axes(vec![AxisHints::new_y()
                        .label("R")
                        .placement(HPlacement::Right)]);
                } else if let (Some(lr), Some(rr)) = (left_range, right_range) {
                    plot_l = plot_l.custom_y_axes(vec![
                        AxisHints::new_y()
                            .label("L")
                            .placement(HPlacement::Left),
                        AxisHints::new_y()
                            .label("R")
                            .placement(HPlacement::Right)
                            .formatter(move |mark, _range| {
                                format_axis_tick(map_y(mark.value, lr, rr))
                            }),
                    ]);
                } else {
                    plot_l = plot_l.custom_y_axes(vec![
                        AxisHints::new_y()
                            .label("L")
                            .placement(HPlacement::Left),
                        AxisHints::new_y()
                            .label("R")
                            .placement(HPlacement::Right),
                    ]);
                }
            }
            // First view of data on an empty chart (L/R assign or hover preview) → fit X+Y once.
            // After that, manual zoom is kept when adding more series / hovering others.
            let showing_data = has_plot_data;
            let force_auto = showing_data && self.reset_plot_auto.remove(&chart.id);
            let plot_resp = plot_l.show(ui, |plot_ui| {
                if force_auto {
                    plot_ui.set_auto_bounds(egui::Vec2b::TRUE);
                }
                for (name, color, pts) in &overlay_series {
                    add_styled_series(plot_ui, name, *color, pts, &draw);
                }
                if draw.show_zero_line {
                    // Static reference — screen-space would miss zoom; HLine is ok when not hover-driven.
                    plot_ui.hline(
                        HLine::new(0.0)
                            .color(egui::Color32::from_rgba_unmultiplied(200, 180, 220, 90))
                            .width(1.0)
                            .style(LineStyle::dashed_dense())
                            .name("y = 0"),
                    );
                }
                if let Some((pname, ppts)) = &preview {
                    let points: PlotPoints = ppts.iter().copied().collect();
                    plot_ui.line(
                        Line::new(points)
                            .name(format!("{pname} (preview)"))
                            .color(egui::Color32::from_rgba_unmultiplied(200, 180, 255, 110))
                            .width(1.5)
                            .style(LineStyle::dotted_dense()),
                    );
                }
                let mut fit_src: Vec<&(String, [u8; 3], Vec<[f64; 2]>)> = overlay_series.clone();
                let preview_owned;
                if let Some((ref pname, ref ppts)) = preview {
                    preview_owned = (pname.clone(), [180u8, 160, 220], ppts.clone());
                    fit_src.push(&preview_owned);
                }
                let fit = fit_bounds_for_series(&fit_src);
                // Wheel zooms X only; Y always fits series in the visible X window.
                apply_wheel_zoom_x_and_locked_auto_y(plot_ui, fit.as_ref(), &fit_src);

                // Sample series at pointer X — do NOT add VLine/HLine plot items (they
                // merge into auto_bounds and pan/zoom the axes while hovering, esp. empty plots).
                if plot_ui.response().hovered() {
                    if let Some(p) = plot_ui.pointer_coordinate() {
                        cursor_x = Some(p.x);
                        for (name, color, pts) in left.iter().chain(right.iter()) {
                            if let Some(y) = nearest_y(pts, p.x) {
                                cursor_readout.push(((*name).clone(), *color, y));
                            }
                        }
                        if let Some((pname, ppts)) = &preview {
                            if let Some(y) = nearest_y(ppts, p.x) {
                                cursor_readout.push((
                                    format!("{pname} (preview)"),
                                    [180, 160, 220],
                                    y,
                                ));
                            }
                        }
                    }
                }
            });
            // Remember full widget rect (axes + plot) for Save PNG — not data-only crop.
            self.plot_rects.insert(chart.id, full_plot_widget);

            // Screen-space crosshair — never participates in plot bounds.
            if draw.show_crosshair && plot_resp.response.hovered() {
                if let Some(pos) = plot_resp.response.hover_pos() {
                    cursor_screen = Some(pos);
                    let rect = plot_resp.response.rect;
                    let painter = ui.painter().with_clip_rect(rect);
                    let hair = egui::Color32::from_rgba_unmultiplied(210, 190, 255, 160);
                    let stroke = egui::Stroke::new(1.0_f32, hair);
                    painter.line_segment(
                        [egui::pos2(pos.x, rect.top()), egui::pos2(pos.x, rect.bottom())],
                        stroke,
                    );
                    painter.line_segment(
                        [egui::pos2(rect.left(), pos.y), egui::pos2(rect.right(), pos.y)],
                        stroke,
                    );
                }
            } else if plot_resp.response.hovered() {
                if let Some(pos) = plot_resp.response.hover_pos() {
                    cursor_screen = Some(pos);
                }
            }
        });

        // Corner / near-cursor legend readout of Y at the crosshair X
        if let (Some(x), Some(screen)) = (cursor_x, cursor_screen) {
            let x_label = if x_is_time {
                format!("t = {}", format_unix_full(x))
            } else {
                format!("x = {x:.4}")
            };
            let tip_pos = screen + egui::vec2(14.0, 14.0);
            egui::Area::new(egui::Id::new(("cursor_tip", chart.id)))
                .order(egui::Order::Tooltip)
                .fixed_pos(tip_pos)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(28, 24, 40, 230))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(140, 110, 190),
                        ))
                        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                        .rounding(4.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(x_label)
                                    .strong()
                                    .color(egui::Color32::from_rgb(230, 220, 255)),
                            );
                            if cursor_readout.is_empty() {
                                ui.weak("no series");
                            } else {
                                for (name, color, y) in &cursor_readout {
                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(10.0, 10.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            egui::Rounding::same(2.0),
                                            egui::Color32::from_rgb(
                                                color[0], color[1], color[2],
                                            ),
                                        );
                                        ui.label(format!("{name}  =  {y:.4}"));
                                    });
                                }
                            }
                        });
                });
        }
    }

    /// Downsample a channel to plot points for the faded preview overlay.
    fn preview_points(&self, channel_id: Uuid) -> Option<(String, Vec<[f64; 2]>)> {
        for ds in &self.datasets {
            if let Some(ch) = ds.channel_by_id(channel_id) {
                if let Some(col) = ds.columns.get(ch.column_index) {
                    let step = (col.len() / 8_000).max(1);
                    let mut pts = Vec::new();
                    for (i, &y) in col.iter().enumerate() {
                        if i % step != 0 || !y.is_finite() {
                            continue;
                        }
                        let x = ds.x.get(i).copied().unwrap_or(i as f64);
                        pts.push([x, y]);
                    }
                    sort_plot_points_by_x(&mut pts);
                    if !pts.is_empty() {
                        return Some((ch.display_name.clone(), pts));
                    }
                }
            }
        }
        None
    }
}

fn dock_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 26, 40))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(110, 85, 160),
        ))
        .inner_margin(egui::Margin::same(8.0))
}

/// Drag payload for chart tiles (index into `charts`).
#[derive(Clone, Copy)]
struct ChartDrag(usize);

/// Hit target for Favnyr-style dock / chart drop preview.
#[derive(Clone)]
struct DockHit {
    kind: DockHitKind,
    rect: egui::Rect,
}

#[derive(Clone, Copy)]
enum DockHitKind {
    /// Full dock column / strip — Y (or X for bottom) decides stack/split band.
    ColumnBody {
        col: DockColumn,
        len: usize,
    },
    ChartTile {
        index: usize, // kept for debugging / future hit resolve
        chart_id: Uuid,
    },
    Canvas,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeZone {
    Center,
    West,
    East,
    North,
    South,
}

#[derive(Clone, Copy)]
enum FavnyrCommit {
    Dock {
        pane: DockPane,
        col: DockColumn,
        idx: usize,
    },
    /// Center drop — exchange two leaf positions in the nested tile tree.
    ChartSwap {
        a: Uuid,
        b: Uuid,
    },
    /// Directional drop — nest `from` into a split of `target`'s leaf (move only).
    ChartSplit {
        from: Uuid,
        target: Uuid,
        edge: TileEdge,
    },
}

/// Visual landing kind for the drop overlay (drives label + divider paint).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LandingKind {
    /// Dock pane stack (no chart duplicate).
    Stack,
    /// Favnyr center — Swap charts.
    Swap,
    /// Nest split on that side of the target leaf.
    SplitLeft,
    SplitRight,
    SplitTop,
    SplitBottom,
}

struct FavnyrAction {
    preview: egui::Rect,
    label: &'static str,
    landing: LandingKind,
    commit: FavnyrCommit,
}

fn pick_dock_hit(hits: &[DockHit], pos: egui::Pos2) -> Option<DockHit> {
    let containing: Vec<&DockHit> = hits.iter().filter(|h| h.rect.contains(pos)).collect();
    if containing.is_empty() {
        return None;
    }
    // Prefer full column body over tiny leftovers / canvas
    if let Some(h) = containing
        .iter()
        .find(|h| matches!(h.kind, DockHitKind::ColumnBody { .. }))
    {
        return Some((*h).clone());
    }
    // Prefer chart tiles over the whole canvas
    if let Some(h) = containing
        .iter()
        .find(|h| matches!(h.kind, DockHitKind::ChartTile { .. }))
    {
        return Some((*h).clone());
    }
    containing
        .into_iter()
        .min_by(|a, b| {
            let aa = a.rect.width() * a.rect.height();
            let bb = b.rect.width() * b.rect.height();
            aa.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// Top / bottom (or L / R for bottom strip) — no mid-split; empty column takes whole rect.
fn column_band(rect: egui::Rect, pos: egui::Pos2, col: DockColumn, len: usize) -> (egui::Rect, &'static str, usize) {
    if len == 0 {
        return (rect, "drop here", 0);
    }
    match col {
        DockColumn::Bottom => {
            let w = rect.width().max(1.0);
            let rx = ((pos.x - rect.min.x) / w).clamp(0.0, 1.0);
            if rx < 0.5 {
                (left_half(rect), "stack here", 0)
            } else {
                (right_half(rect), "stack here", usize::MAX)
            }
        }
        DockColumn::Left | DockColumn::Right => {
            let h = rect.height().max(1.0);
            let ry = ((pos.y - rect.min.y) / h).clamp(0.0, 1.0);
            if ry < 0.5 {
                (top_half(rect), "stack above", 0)
            } else {
                (bottom_half(rect), "stack below", usize::MAX)
            }
        }
    }
}

fn resolve_favnyr_action(
    hit: &DockHit,
    pos: egui::Pos2,
    dragging_pane: Option<DockPane>,
    dragging_chart: Option<usize>,
) -> Option<FavnyrAction> {
    if let Some(pane) = dragging_pane {
        return Some(match hit.kind {
            DockHitKind::ColumnBody { col, len } => {
                let (preview, label, idx) = column_band(hit.rect, pos, col, len);
                FavnyrAction {
                    preview,
                    label,
                    landing: LandingKind::Stack,
                    commit: FavnyrCommit::Dock { pane, col, idx },
                }
            }
            DockHitKind::Canvas | DockHitKind::ChartTile { .. } => {
                // Dock panes only move — edge = target column, center = right stack
                let zone = edge_zone(hit.rect, pos);
                let (preview, label, col, idx) = match zone {
                    EdgeZone::West => (left_half(hit.rect), "stack left", DockColumn::Left, usize::MAX),
                    EdgeZone::East => (right_half(hit.rect), "stack right", DockColumn::Right, usize::MAX),
                    EdgeZone::South => {
                        (bottom_half(hit.rect), "stack bottom", DockColumn::Bottom, usize::MAX)
                    }
                    EdgeZone::North => (top_half(hit.rect), "stack top", DockColumn::Left, 0),
                    EdgeZone::Center => (
                        hit.rect
                            .shrink(hit.rect.width().min(hit.rect.height()) * 0.22),
                        "stack here",
                        DockColumn::Right,
                        usize::MAX,
                    ),
                };
                FavnyrAction {
                    preview,
                    label,
                    landing: LandingKind::Stack,
                    commit: FavnyrCommit::Dock { pane, col, idx },
                }
            }
        });
    }

    if let Some(from_idx) = dragging_chart {
        // Resolve dragged chart id later via index in apply — here we need Uuid from hit
        // Caller passes index; ChartTile carries chart_id for the target leaf.
        return Some(match hit.kind {
            DockHitKind::ChartTile { chart_id: target, .. } => {
                let zone = edge_zone(hit.rect, pos);
                // from_idx is still an index — encode as Uuid via temporary; resolve in apply
                // We stash from as Uuid::from_u128(index) is wrong. Use placeholder: commit
                // stores from as target for now? Better: change dragging_chart to Uuid.
                // Here `from_idx` is usize — convert via a side channel in FavnyrCommit.
                // ChartSplit.from / ChartSwap.a are Uuid — resolve below using index→need charts.
                // Keep usize payload on ChartDrag; resolve Uuid in favnyr_dock_drop_preview.
                let (preview, label, landing, edge) = match zone {
                    EdgeZone::North => (
                        top_half(hit.rect),
                        "SPLIT · top",
                        LandingKind::SplitTop,
                        TileEdge::North,
                    ),
                    EdgeZone::South => (
                        bottom_half(hit.rect),
                        "SPLIT · bottom",
                        LandingKind::SplitBottom,
                        TileEdge::South,
                    ),
                    EdgeZone::West => (
                        left_half(hit.rect),
                        "SPLIT · left",
                        LandingKind::SplitLeft,
                        TileEdge::West,
                    ),
                    EdgeZone::East => (
                        right_half(hit.rect),
                        "SPLIT · right",
                        LandingKind::SplitRight,
                        TileEdge::East,
                    ),
                    EdgeZone::Center => (
                        hit.rect.shrink(4.0),
                        "Swap",
                        LandingKind::Swap,
                        TileEdge::South, // unused
                    ),
                };
                if matches!(zone, EdgeZone::Center) {
                    FavnyrAction {
                        preview,
                        label,
                        landing,
                        // Temporary: a=from_idx as bytes — replaced by caller
                        commit: FavnyrCommit::ChartSwap {
                            a: Uuid::from_u128(from_idx as u128),
                            b: target,
                        },
                    }
                } else {
                    FavnyrAction {
                        preview,
                        label,
                        landing,
                        commit: FavnyrCommit::ChartSplit {
                            from: Uuid::from_u128(from_idx as u128),
                            target,
                            edge,
                        },
                    }
                }
            }
            DockHitKind::ColumnBody { col, .. } => {
                let (preview, _, _) = column_band(hit.rect, pos, col, 1);
                let (landing, label, edge) = match col {
                    DockColumn::Left => (LandingKind::SplitLeft, "SPLIT · left", TileEdge::West),
                    DockColumn::Right => (LandingKind::SplitRight, "SPLIT · right", TileEdge::East),
                    DockColumn::Bottom => {
                        (LandingKind::SplitBottom, "SPLIT · bottom", TileEdge::South)
                    }
                };
                FavnyrAction {
                    preview,
                    label,
                    landing,
                    commit: FavnyrCommit::ChartSplit {
                        from: Uuid::from_u128(from_idx as u128),
                        target: Uuid::nil(), // means: split at root
                        edge,
                    },
                }
            }
            DockHitKind::Canvas => {
                let zone = edge_zone(hit.rect, pos);
                let (preview, label, landing, edge) = match zone {
                    EdgeZone::West => (
                        left_half(hit.rect),
                        "SPLIT · left",
                        LandingKind::SplitLeft,
                        TileEdge::West,
                    ),
                    EdgeZone::East => (
                        right_half(hit.rect),
                        "SPLIT · right",
                        LandingKind::SplitRight,
                        TileEdge::East,
                    ),
                    EdgeZone::North => (
                        top_half(hit.rect),
                        "SPLIT · top",
                        LandingKind::SplitTop,
                        TileEdge::North,
                    ),
                    EdgeZone::South => (
                        bottom_half(hit.rect),
                        "SPLIT · bottom",
                        LandingKind::SplitBottom,
                        TileEdge::South,
                    ),
                    EdgeZone::Center => (
                        hit.rect.shrink(8.0),
                        "Swap",
                        LandingKind::Swap,
                        TileEdge::South,
                    ),
                };
                if matches!(zone, EdgeZone::Center) {
                    FavnyrAction {
                        preview,
                        label,
                        landing,
                        commit: FavnyrCommit::ChartSwap {
                            a: Uuid::from_u128(from_idx as u128),
                            b: Uuid::nil(),
                        },
                    }
                } else {
                    FavnyrAction {
                        preview,
                        label,
                        landing,
                        commit: FavnyrCommit::ChartSplit {
                            from: Uuid::from_u128(from_idx as u128),
                            target: Uuid::nil(),
                            edge,
                        },
                    }
                }
            }
        });
    }
    None
}

fn paint_favnyr_overlay(
    ctx: &egui::Context,
    hit_rect: egui::Rect,
    preview: egui::Rect,
    label: &str,
    landing: LandingKind,
) {
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("favnyr_dock_preview"));
    let painter = ctx.layer_painter(layer);
    let is_split = matches!(
        landing,
        LandingKind::SplitLeft
            | LandingKind::SplitRight
            | LandingKind::SplitTop
            | LandingKind::SplitBottom
    );

    // Dim the whole hit target so the landing half reads clearly
    painter.rect_filled(
        hit_rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_rgba_unmultiplied(20, 14, 36, if is_split { 70 } else { 40 }),
    );
    painter.rect_stroke(
        hit_rect,
        egui::Rounding::same(4.0),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(140, 100, 200)),
    );

    // Landing fill — warmer for SPLIT, cooler for Swap/stack
    let fill = if is_split {
        egui::Color32::from_rgba_unmultiplied(160, 90, 220, 120)
    } else if landing == LandingKind::Swap {
        egui::Color32::from_rgba_unmultiplied(110, 70, 180, 110)
    } else {
        egui::Color32::from_rgba_unmultiplied(120, 70, 200, 95)
    };
    let stroke = if is_split {
        egui::Color32::from_rgb(230, 190, 255)
    } else {
        egui::Color32::from_rgb(180, 140, 240)
    };
    painter.rect_filled(preview, egui::Rounding::same(4.0), fill);
    painter.rect_stroke(
        preview,
        egui::Rounding::same(4.0),
        egui::Stroke::new(2.5_f32, stroke),
    );

    // Explicit split seam + chevron so T/B/L/R landings are unmistakable
    if is_split {
        let seam = egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(245, 220, 255));
        match landing {
            LandingKind::SplitLeft => {
                let x = preview.max.x;
                painter.line_segment(
                    [egui::pos2(x, hit_rect.min.y + 4.0), egui::pos2(x, hit_rect.max.y - 4.0)],
                    seam,
                );
                paint_split_chevron(&painter, preview.center(), egui::vec2(-1.0, 0.0));
            }
            LandingKind::SplitRight => {
                let x = preview.min.x;
                painter.line_segment(
                    [egui::pos2(x, hit_rect.min.y + 4.0), egui::pos2(x, hit_rect.max.y - 4.0)],
                    seam,
                );
                paint_split_chevron(&painter, preview.center(), egui::vec2(1.0, 0.0));
            }
            LandingKind::SplitTop => {
                let y = preview.max.y;
                painter.line_segment(
                    [egui::pos2(hit_rect.min.x + 4.0, y), egui::pos2(hit_rect.max.x - 4.0, y)],
                    seam,
                );
                paint_split_chevron(&painter, preview.center(), egui::vec2(0.0, -1.0));
            }
            LandingKind::SplitBottom => {
                let y = preview.min.y;
                painter.line_segment(
                    [egui::pos2(hit_rect.min.x + 4.0, y), egui::pos2(hit_rect.max.x - 4.0, y)],
                    seam,
                );
                paint_split_chevron(&painter, preview.center(), egui::vec2(0.0, 1.0));
            }
            _ => {}
        }
    }

    let galley = ctx.fonts(|f| {
        f.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(240, 230, 255),
        )
    });
    let pad = egui::vec2(12.0, 6.0);
    let label_size = galley.size() + pad * 2.0;
    let label_rect = egui::Rect::from_center_size(preview.center(), label_size);
    painter.rect_filled(
        label_rect,
        egui::Rounding::same(6.0),
        egui::Color32::from_rgba_unmultiplied(40, 28, 64, 235),
    );
    painter.rect_stroke(
        label_rect,
        egui::Rounding::same(6.0),
        egui::Stroke::new(
            1.5_f32,
            if is_split {
                egui::Color32::from_rgb(220, 170, 255)
            } else {
                egui::Color32::from_rgb(170, 130, 230)
            },
        ),
    );
    painter.galley(label_rect.min + pad, galley, egui::Color32::WHITE);
}

/// Favnyr chart drop overlay: dim tile + center **Swap** pill + T/R/B/L arrow pads.
fn paint_chart_favnyr_overlay(ctx: &egui::Context, hit_rect: egui::Rect, landing: LandingKind) {
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("favnyr_chart_preview"));
    let painter = ctx.layer_painter(layer);

    painter.rect_filled(
        hit_rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_rgba_unmultiplied(18, 12, 30, 110),
    );
    painter.rect_stroke(
        hit_rect,
        egui::Rounding::same(4.0),
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(150, 110, 210)),
    );

    // Direction pads (small rounded squares at cardinal midpoints)
    let pad = 28.0_f32;
    let inset = 18.0_f32;
    let dirs = [
        (
            LandingKind::SplitTop,
            egui::pos2(hit_rect.center().x, hit_rect.top() + inset + pad * 0.5),
            egui::vec2(0.0, -1.0),
        ),
        (
            LandingKind::SplitBottom,
            egui::pos2(hit_rect.center().x, hit_rect.bottom() - inset - pad * 0.5),
            egui::vec2(0.0, 1.0),
        ),
        (
            LandingKind::SplitLeft,
            egui::pos2(hit_rect.left() + inset + pad * 0.5, hit_rect.center().y),
            egui::vec2(-1.0, 0.0),
        ),
        (
            LandingKind::SplitRight,
            egui::pos2(hit_rect.right() - inset - pad * 0.5, hit_rect.center().y),
            egui::vec2(1.0, 0.0),
        ),
    ];
    for (kind, center, dir) in dirs {
        let active = landing == kind;
        let r = egui::Rect::from_center_size(center, egui::vec2(pad, pad));
        let fill = if active {
            egui::Color32::from_rgba_unmultiplied(140, 80, 210, 200)
        } else {
            egui::Color32::from_rgba_unmultiplied(50, 36, 72, 180)
        };
        painter.rect_filled(r, egui::Rounding::same(6.0), fill);
        painter.rect_stroke(
            r,
            egui::Rounding::same(6.0),
            egui::Stroke::new(
                1.5_f32,
                if active {
                    egui::Color32::from_rgb(230, 200, 255)
                } else {
                    egui::Color32::from_rgb(140, 110, 190)
                },
            ),
        );
        paint_split_chevron(&painter, center, dir);
    }

    // Half-preview wash for the active split side
    if matches!(
        landing,
        LandingKind::SplitLeft
            | LandingKind::SplitRight
            | LandingKind::SplitTop
            | LandingKind::SplitBottom
    ) {
        let half = match landing {
            LandingKind::SplitLeft => left_half(hit_rect),
            LandingKind::SplitRight => right_half(hit_rect),
            LandingKind::SplitTop => top_half(hit_rect),
            LandingKind::SplitBottom => bottom_half(hit_rect),
            _ => hit_rect,
        };
        painter.rect_filled(
            half,
            egui::Rounding::same(4.0),
            egui::Color32::from_rgba_unmultiplied(150, 90, 220, 55),
        );
    }

    // Center Swap pill (always visible; highlighted when active)
    let swap_active = landing == LandingKind::Swap;
    let galley = ctx.fonts(|f| {
        f.layout_no_wrap(
            "Swap".to_owned(),
            egui::FontId::proportional(15.0),
            egui::Color32::from_rgb(245, 235, 255),
        )
    });
    let pad_xy = egui::vec2(18.0, 8.0);
    let swap_size = egui::vec2(
        (galley.size().x + pad_xy.x * 2.0 + 28.0).max(100.0),
        (galley.size().y + pad_xy.y * 2.0).max(36.0),
    );
    let swap_rect = egui::Rect::from_center_size(hit_rect.center(), swap_size);
    let fill = if swap_active {
        egui::Color32::from_rgb(110, 70, 175)
    } else {
        egui::Color32::from_rgba_unmultiplied(45, 32, 70, 230)
    };
    painter.rect_filled(swap_rect, egui::Rounding::same(8.0), fill);
    painter.rect_stroke(
        swap_rect,
        egui::Rounding::same(8.0),
        egui::Stroke::new(
            2.0_f32,
            if swap_active {
                egui::Color32::from_rgb(230, 200, 255)
            } else {
                egui::Color32::from_rgb(160, 120, 220)
            },
        ),
    );
    // ↔ glyph left of label
    let ink = egui::Color32::from_rgb(235, 220, 255);
    let arrow_x = swap_rect.left() + 14.0;
    let cy = swap_rect.center().y;
    let stroke = egui::Stroke::new(1.8_f32, ink);
    painter.line_segment(
        [egui::pos2(arrow_x, cy), egui::pos2(arrow_x + 16.0, cy)],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(arrow_x + 4.0, cy - 4.0),
            egui::pos2(arrow_x, cy),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(arrow_x + 4.0, cy + 4.0),
            egui::pos2(arrow_x, cy),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(arrow_x + 12.0, cy - 4.0),
            egui::pos2(arrow_x + 16.0, cy),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(arrow_x + 12.0, cy + 4.0),
            egui::pos2(arrow_x + 16.0, cy),
        ],
        stroke,
    );
    painter.galley(
        egui::pos2(arrow_x + 22.0, swap_rect.center().y - galley.size().y * 0.5),
        galley,
        egui::Color32::WHITE,
    );
}

/// Small direction chevron inside the split landing half.
fn paint_split_chevron(painter: &egui::Painter, center: egui::Pos2, dir: egui::Vec2) {
    let ink = egui::Color32::from_rgba_unmultiplied(245, 230, 255, 200);
    let stroke = egui::Stroke::new(2.0_f32, ink);
    let len = 14.0_f32;
    let arm = 8.0_f32;
    // Tip points along dir; two arms back
    let tip = center + dir * len;
    let perp = egui::vec2(-dir.y, dir.x);
    let back = center - dir * (len * 0.15);
    painter.line_segment([back + perp * arm, tip], stroke);
    painter.line_segment([back - perp * arm, tip], stroke);
}

/// Draggable gap between nested chart tiles. `vertical` = Cols (drag X).
/// Returns a new ratio when the user drags; `usable` is the axis length minus gap.
fn tile_splitter(
    ui: &mut egui::Ui,
    vertical: bool,
    length: f32,
    thickness: f32,
    usable: f32,
    ratio: f32,
    path: &[bool],
) -> Option<f32> {
    let size = if vertical {
        egui::vec2(thickness, length)
    } else {
        egui::vec2(length, thickness)
    };
    // Stable id per tree path so nested splitters don't collide
    let id = ui.id().with(("tile_splitter", path));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
    let hover = resp.hovered() || resp.dragged();
    if hover {
        ui.ctx().set_cursor_icon(if vertical {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        });
    }
    let paint = ui.painter();
    let mid = if vertical {
        egui::Rect::from_center_size(rect.center(), egui::vec2(2.0, rect.height().min(48.0)))
    } else {
        egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width().min(48.0), 2.0))
    };
    let fill = if hover {
        egui::Color32::from_rgb(180, 140, 230)
    } else {
        egui::Color32::from_rgb(90, 70, 120)
    };
    paint.rect_filled(mid, egui::Rounding::same(1.0), fill);
    // Grip dots
    let ink = if hover {
        egui::Color32::from_rgb(235, 220, 255)
    } else {
        egui::Color32::from_rgb(140, 120, 180)
    };
    let c = rect.center();
    for i in -1..=1 {
        let p = if vertical {
            egui::pos2(c.x, c.y + i as f32 * 5.0)
        } else {
            egui::pos2(c.x + i as f32 * 5.0, c.y)
        };
        paint.circle_filled(p, 1.4, ink);
    }
    if resp.dragged() {
        let delta = if vertical {
            resp.drag_delta().x
        } else {
            resp.drag_delta().y
        };
        if usable > 1.0 && delta.abs() > 0.0 {
            return Some(((ratio * usable + delta) / usable).clamp(0.15, 0.85));
        }
    }
    None
}

fn edge_zone(rect: egui::Rect, pos: egui::Pos2) -> EdgeZone {
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    let rx = ((pos.x - rect.min.x) / w).clamp(0.0, 1.0);
    let ry = ((pos.y - rect.min.y) / h).clamp(0.0, 1.0);
    // ~28% edge bands so T/B/L/R landings are easy to hit; inner = MOVE
    let band = 0.28;
    let in_cx = rx >= band && rx <= 1.0 - band;
    let in_cy = ry >= band && ry <= 1.0 - band;
    if in_cx && in_cy {
        return EdgeZone::Center;
    }
    // Nearest edge wins (corners resolve cleanly)
    let d_w = rx;
    let d_e = 1.0 - rx;
    let d_n = ry;
    let d_s = 1.0 - ry;
    let mut best = EdgeZone::West;
    let mut best_d = d_w;
    if d_e < best_d {
        best = EdgeZone::East;
        best_d = d_e;
    }
    if d_n < best_d {
        best = EdgeZone::North;
        best_d = d_n;
    }
    if d_s < best_d {
        best = EdgeZone::South;
    }
    best
}

fn left_half(r: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(r.min, egui::pos2(r.center().x, r.max.y))
}
fn right_half(r: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(r.center().x, r.min.y), r.max)
}
fn top_half(r: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(r.min, egui::pos2(r.max.x, r.center().y))
}
fn bottom_half(r: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(r.min.x, r.center().y), r.max)
}
/// Mini rising line-chart mark for chart titles (font-independent).
fn paint_chart_glyph(ui: &mut egui::Ui) {
    let size = egui::vec2(14.0, 12.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let ink = egui::Color32::from_rgb(235, 225, 255);
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let pts = [
        egui::pos2(rect.left() + 0.5, rect.bottom() - 1.0),
        egui::pos2(rect.left() + 4.0, rect.center().y + 2.5),
        egui::pos2(rect.center().x + 0.5, rect.center().y - 0.5),
        egui::pos2(rect.right() - 0.5, rect.top() + 1.0),
    ];
    for w in pts.windows(2) {
        ui.painter().line_segment([w[0], w[1]], stroke);
    }
}

/// Shared Files toolbar chrome height (Open / Recent aligned row).
const FILES_TOOL_BTN_H: f32 = 22.0;

/// Files panel Open… control — painted folder + “Open…” (matches chart tool icons).
fn files_open_button(ui: &mut egui::Ui) -> egui::Response {
    let text = "Open…";
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(13.0),
            egui::Color32::PLACEHOLDER,
        )
    });
    let pad = ui.spacing().button_padding;
    let icon = 14.0_f32;
    let gap = 5.0_f32;
    let size = egui::vec2(
        pad.x * 2.0 + icon + gap + galley.size().x,
        (pad.y * 2.0 + galley.size().y.max(icon)).max(FILES_TOOL_BTN_H),
    );
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    paint_files_tool_chrome(ui, rect, &resp);
    let painter = ui.painter();
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad.x, rect.center().y - icon * 0.5),
        egui::vec2(icon, icon),
    );
    paint_open_file_in(painter, icon_rect);
    let text_pos = egui::pos2(
        icon_rect.right() + gap,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(
        text_pos,
        galley,
        egui::Color32::from_rgb(235, 225, 255),
    );
    resp
}

/// Files header New chart control — plain text “New chart” only (no +/- / sparkline icon).
fn files_new_chart_button(ui: &mut egui::Ui) -> egui::Response {
    let text = "New chart";
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(13.0),
            egui::Color32::PLACEHOLDER,
        )
    });
    let pad = ui.spacing().button_padding;
    let size = egui::vec2(
        pad.x * 2.0 + galley.size().x,
        (pad.y * 2.0 + galley.size().y).max(FILES_TOOL_BTN_H),
    );
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    paint_files_tool_chrome(ui, rect, &resp);
    let painter = ui.painter();
    let text_pos = egui::pos2(
        rect.left() + pad.x,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(
        text_pos,
        galley,
        egui::Color32::from_rgb(235, 225, 255),
    );
    resp
}

/// Files panel Recent control — same height/chrome as Open… + dropdown chevron.
fn files_recent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(13.0),
            egui::Color32::PLACEHOLDER,
        )
    });
    let pad = ui.spacing().button_padding;
    let icon = 14.0_f32;
    let gap = 5.0_f32;
    let chevron = 8.0_f32;
    let size = egui::vec2(
        pad.x * 2.0 + icon + gap + galley.size().x + gap + chevron,
        (pad.y * 2.0 + galley.size().y.max(icon)).max(FILES_TOOL_BTN_H),
    );
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    paint_files_tool_chrome(ui, rect, &resp);
    let painter = ui.painter();
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad.x, rect.center().y - icon * 0.5),
        egui::vec2(icon, icon),
    );
    paint_recent_clock_in(painter, icon_rect);
    let text_pos = egui::pos2(
        icon_rect.right() + gap,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(
        text_pos,
        galley,
        egui::Color32::from_rgb(235, 225, 255),
    );
    // Dropdown chevron
    let cx = rect.right() - pad.x - chevron * 0.5;
    let cy = rect.center().y;
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    painter.line_segment(
        [egui::pos2(cx - 3.0, cy - 1.5), egui::pos2(cx, cy + 1.5)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(cx + 3.0, cy - 1.5), egui::pos2(cx, cy + 1.5)],
        stroke,
    );
    resp
}

/// Shared Open/Recent button fill + purple stroke.
fn paint_files_tool_chrome(ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
    let visuals = ui.style().interact(resp);
    let fill = if resp.hovered() {
        egui::Color32::from_rgb(56, 48, 88)
    } else {
        visuals.weak_bg_fill
    };
    let painter = ui.painter();
    let rounding = egui::Rounding::same(4.0);
    painter.rect_filled(rect, rounding, fill);
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(130, 100, 180)),
    );
}

/// Clock mark for Recent dropdown (Files toolbar).
fn paint_recent_clock_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    let c = rect.center();
    let r = (rect.width().min(rect.height()) * 0.42).max(4.0);
    painter.circle_stroke(c, r, stroke);
    // Hands
    painter.line_segment([c, egui::pos2(c.x, c.y - r * 0.55)], stroke);
    painter.line_segment([c, egui::pos2(c.x + r * 0.4, c.y + r * 0.15)], stroke);
}

/// Open-folder glyph (document flap + body) for Files → Open.
fn paint_open_file_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    // Folder body
    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.top() + 4.0),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
    );
    painter.rect_stroke(body, egui::Rounding::same(1.5), stroke);
    // Tab
    painter.line_segment(
        [
            egui::pos2(body.left(), body.top()),
            egui::pos2(body.left() + 2.0, rect.top() + 1.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(body.left() + 2.0, rect.top() + 1.5),
            egui::pos2(body.left() + 6.5, rect.top() + 1.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(body.left() + 6.5, rect.top() + 1.5),
            egui::pos2(body.left() + 8.0, body.top()),
        ],
        stroke,
    );
    // Open flap (slanted top edge inside body)
    painter.line_segment(
        [
            egui::pos2(body.left() + 1.0, body.top() + 3.0),
            egui::pos2(body.right() - 1.0, body.top() + 1.0),
        ],
        stroke,
    );
}

/// Compact red × for Files close column (font-independent).
fn paint_close_x_glyph(ui: &mut egui::Ui) -> egui::Response {
    let size = egui::vec2(12.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let ink = egui::Color32::from_rgb(220, 70, 70);
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let pad = 2.5_f32;
    let painter = ui.painter();
    painter.line_segment(
        [
            egui::pos2(rect.left() + pad, rect.top() + pad),
            egui::pos2(rect.right() - pad, rect.bottom() - pad),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(rect.right() - pad, rect.top() + pad),
            egui::pos2(rect.left() + pad, rect.bottom() - pad),
        ],
        stroke,
    );
    resp
}

/// Side length matching draggable chart title chrome (15pt label + 3px margins ≈ 22).
const CHART_TOOL_BTN: f32 = 22.0;

/// Painted icons for the chart header toolbar (font-independent).
#[derive(Clone, Copy)]
enum ChartToolIcon {
    Gear,
    SavePng,
    Export,
    Clear,
    Duplicate,
    PopOut,
    PopIn,
    Close,
}

/// Equal-size chart toolbar button; `active` uses purple toggle fill.
fn chart_tool_button(ui: &mut egui::Ui, active: bool, icon: ChartToolIcon) -> egui::Response {
    let size = egui::vec2(CHART_TOOL_BTN, CHART_TOOL_BTN);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&resp);
    let fill = if active {
        egui::Color32::from_rgb(90, 70, 140)
    } else if resp.hovered() {
        egui::Color32::from_rgb(56, 48, 88)
    } else {
        visuals.weak_bg_fill
    };
    let painter = ui.painter();
    let rounding = egui::Rounding::same(4.0);
    painter.rect_filled(rect, rounding, fill);
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(130, 100, 180)),
    );
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 14.0));
    match icon {
        ChartToolIcon::Gear => paint_gear_in(painter, icon_rect),
        ChartToolIcon::SavePng => paint_save_png_in(painter, icon_rect),
        ChartToolIcon::Export => paint_export_in(painter, icon_rect),
        ChartToolIcon::Clear => paint_clear_in(painter, icon_rect),
        ChartToolIcon::Duplicate => paint_duplicate_in(painter, icon_rect),
        ChartToolIcon::PopOut => paint_popout_in(painter, icon_rect, true),
        ChartToolIcon::PopIn => paint_popout_in(painter, icon_rect, false),
        ChartToolIcon::Close => paint_tool_close_in(painter, icon_rect),
    }
    resp
}

/// Picture frame + mountain mark (Save PNG of plot).
fn paint_save_png_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    let frame = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.top() + 1.5),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 1.5),
    );
    painter.rect_stroke(frame, egui::Rounding::same(1.5), stroke);
    // Sun
    painter.circle_filled(
        egui::pos2(frame.left() + 3.5, frame.top() + 3.5),
        1.4,
        ink,
    );
    // Mountain polyline
    let base_y = frame.bottom() - 1.5;
    let pts = [
        egui::pos2(frame.left() + 1.5, base_y),
        egui::pos2(frame.left() + 4.5, frame.center().y + 0.5),
        egui::pos2(frame.center().x + 0.5, base_y - 1.0),
        egui::pos2(frame.right() - 1.5, frame.top() + 4.0),
        egui::pos2(frame.right() - 1.5, base_y),
    ];
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], stroke);
    }
}

/// Paint a gear mark into `rect` (hub + rim + six teeth).
fn paint_gear_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    let c = rect.center();
    let r_outer = (rect.width().min(rect.height()) * 0.32).max(3.5);
    let r_inner = r_outer * 0.4;
    painter.circle_stroke(c, r_inner, stroke);
    for i in 0..6 {
        let a = (i as f32) * std::f32::consts::TAU / 6.0;
        let dir = egui::vec2(a.cos(), a.sin());
        let a0 = c + dir * (r_outer - 1.2);
        let a1 = c + dir * (r_outer + 1.5);
        painter.line_segment([a0, a1], stroke);
    }
    painter.circle_stroke(c, r_outer, stroke);
}

/// Download arrow into a tray (Export CSV).
fn paint_export_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.4_f32, ink);
    let cx = rect.center().x;
    let top = rect.top() + 1.0;
    let mid = rect.center().y + 0.5;
    let tray_y = rect.bottom() - 2.0;
    // Vertical stem + chevron
    painter.line_segment(
        [egui::pos2(cx, top), egui::pos2(cx, mid + 1.5)],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(cx - 3.5, mid - 1.5),
            egui::pos2(cx, mid + 2.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(cx + 3.5, mid - 1.5),
            egui::pos2(cx, mid + 2.0),
        ],
        stroke,
    );
    // Tray
    painter.line_segment(
        [
            egui::pos2(rect.left() + 1.5, tray_y - 3.0),
            egui::pos2(rect.left() + 1.5, tray_y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(rect.left() + 1.5, tray_y),
            egui::pos2(rect.right() - 1.5, tray_y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(rect.right() - 1.5, tray_y),
            egui::pos2(rect.right() - 1.5, tray_y - 3.0),
        ],
        stroke,
    );
}

/// Eraser / clear mark (Clear series).
fn paint_clear_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.4_f32, ink);
    // Slanted eraser body
    let a = egui::pos2(rect.left() + 2.0, rect.bottom() - 3.5);
    let b = egui::pos2(rect.left() + 5.5, rect.top() + 2.5);
    let c = egui::pos2(rect.right() - 2.0, rect.top() + 5.0);
    let d = egui::pos2(rect.right() - 5.5, rect.bottom() - 1.0);
    painter.line_segment([a, b], stroke);
    painter.line_segment([b, c], stroke);
    painter.line_segment([c, d], stroke);
    painter.line_segment([d, a], stroke);
    // Band across eraser
    painter.line_segment(
        [
            egui::pos2(rect.left() + 3.8, rect.center().y + 0.5),
            egui::pos2(rect.right() - 3.5, rect.center().y - 1.0),
        ],
        stroke,
    );
}

/// Two overlapping rects (Duplicate chart).
fn paint_duplicate_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    let back = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 3.5, rect.top() + 1.0),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 3.5),
    );
    let front = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.top() + 3.5),
        egui::pos2(rect.right() - 3.5, rect.bottom() - 1.0),
    );
    painter.rect_stroke(back, egui::Rounding::same(1.5), stroke);
    painter.rect_stroke(front, egui::Rounding::same(1.5), stroke);
}

/// Window + corner arrow (`out` = pop-out, else pop-in).
fn paint_popout_in(painter: &egui::Painter, rect: egui::Rect, out: bool) {
    let ink = egui::Color32::from_rgb(220, 205, 245);
    let stroke = egui::Stroke::new(1.3_f32, ink);
    let win = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.top() + 3.0),
        egui::pos2(rect.right() - 4.0, rect.bottom() - 1.0),
    );
    painter.rect_stroke(win, egui::Rounding::same(1.0), stroke);
    // Title bar tick
    painter.line_segment(
        [
            egui::pos2(win.left() + 1.0, win.top() + 2.5),
            egui::pos2(win.right() - 1.0, win.top() + 2.5),
        ],
        stroke,
    );
    let tip = if out {
        egui::pos2(rect.right() - 1.0, rect.top() + 1.0)
    } else {
        egui::pos2(win.center().x + 1.0, win.center().y + 1.0)
    };
    let base = if out {
        egui::pos2(rect.center().x + 0.5, rect.center().y - 0.5)
    } else {
        egui::pos2(rect.right() - 1.0, rect.top() + 1.0)
    };
    painter.line_segment([base, tip], stroke);
    // Arrow head
    if out {
        painter.line_segment(
            [tip, egui::pos2(tip.x - 3.5, tip.y)],
            stroke,
        );
        painter.line_segment(
            [tip, egui::pos2(tip.x, tip.y + 3.5)],
            stroke,
        );
    } else {
        painter.line_segment(
            [tip, egui::pos2(tip.x - 3.0, tip.y)],
            stroke,
        );
        painter.line_segment(
            [tip, egui::pos2(tip.x, tip.y - 3.0)],
            stroke,
        );
    }
}

/// Compact × for Remove chart.
fn paint_tool_close_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(230, 110, 110);
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let pad = 2.5_f32;
    painter.line_segment(
        [
            egui::pos2(rect.left() + pad, rect.top() + pad),
            egui::pos2(rect.right() - pad, rect.bottom() - pad),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(rect.right() - pad, rect.top() + pad),
            egui::pos2(rect.left() + pad, rect.bottom() - pad),
        ],
        stroke,
    );
}

/// Map Options stroke style → egui_plot LineStyle.
fn stroke_to_line_style(s: StrokeStyleKind) -> LineStyle {
    match s {
        StrokeStyleKind::Solid => LineStyle::Solid,
        StrokeStyleKind::Dashed => LineStyle::dashed_dense(),
        StrokeStyleKind::Dotted => LineStyle::dotted_dense(),
    }
}

/// Order line/step/area vertices by ascending X so unsorted rows do not zigzag.
fn sort_plot_points_by_x(pts: &mut Vec<[f64; 2]>) {
    if pts.len() < 2 {
        return;
    }
    // Already monotonic (typical CSV / DAQ) — skip the sort.
    let mut sorted = true;
    for w in pts.windows(2) {
        if w[0][0] > w[1][0] {
            sorted = false;
            break;
        }
    }
    if sorted {
        return;
    }
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
}

/// Sparkline in time order: bucket Y after sorting by dataset X (matches main chart).
fn sparkline_by_x(x: &[f64], values: &[f64], out_len: usize) -> Vec<f64> {
    let n = values.len().min(x.len());
    if n == 0 || out_len == 0 {
        return vec![0.0; out_len];
    }
    let already = x.windows(2).all(|w| w[0] <= w[1]);
    if already {
        return sparkline(&values[..n], out_len);
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap_or(std::cmp::Ordering::Equal));
    let ordered: Vec<f64> = order.into_iter().map(|i| values[i]).collect();
    sparkline(&ordered, out_len)
}

/// Expand samples into stair-steps for ChartKind::Step.
fn to_step_points(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() * 2);
    out.push(pts[0]);
    for w in pts.windows(2) {
        out.push([w[1][0], w[0][1]]);
        out.push(w[1]);
    }
    out
}

/// Apply ChartDrawOptions (kind / width / stroke / markers / bars / area) to one series.
fn add_styled_series(
    plot_ui: &mut PlotUi,
    name: &str,
    color: [u8; 3],
    pts: &[[f64; 2]],
    draw: &ChartDrawOptions,
) {
    if pts.is_empty() {
        return;
    }
    let color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);
    let style = stroke_to_line_style(draw.stroke);
    match draw.kind {
        ChartKind::Bar => {
            let bars: Vec<Bar> = pts
                .iter()
                .filter(|p| p[1].is_finite())
                .map(|p| Bar::new(p[0], p[1]).fill(color32))
                .collect();
            if bars.is_empty() {
                return;
            }
            let mut chart = BarChart::new(bars).name(name.to_string()).color(color32);
            if draw.bar_width > 0.0 {
                chart = chart.width(draw.bar_width as f64);
            }
            plot_ui.bar_chart(chart);
        }
        ChartKind::Points => {
            let points: PlotPoints = pts.iter().copied().collect();
            plot_ui.points(
                Points::new(points)
                    .name(name.to_string())
                    .color(color32)
                    .radius(draw.marker_size.max(1.0))
                    .filled(true),
            );
        }
        ChartKind::Line | ChartKind::Step | ChartKind::Area => {
            let owned;
            let use_pts: &[[f64; 2]] = if draw.kind == ChartKind::Step {
                owned = to_step_points(pts);
                &owned
            } else {
                pts
            };
            let points: PlotPoints = use_pts.iter().copied().collect();
            let mut line = Line::new(points)
                .name(name.to_string())
                .color(color32)
                .width(draw.line_width)
                .style(style);
            if draw.kind == ChartKind::Area {
                line = line.fill(0.0);
            }
            plot_ui.line(line);
            if draw.show_markers {
                let points: PlotPoints = pts.iter().copied().collect();
                plot_ui.points(
                    Points::new(points)
                        .name(format!("{name} ·"))
                        .color(color32)
                        .radius(draw.marker_size.max(1.0))
                        .filled(true),
                );
            }
        }
    }
}

/// Mini 3×2 table/grid mark (font-independent; matches chart glyph ink).
fn paint_table_glyph(ui: &mut egui::Ui, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    paint_table_glyph_in(ui.painter(), rect);
}

fn paint_table_glyph_in(painter: &egui::Painter, rect: egui::Rect) {
    let ink = egui::Color32::from_rgb(235, 225, 255);
    let stroke = egui::Stroke::new(1.4_f32, ink);
    let r = rect.shrink(1.0);
    // Header band makes the spreadsheet shape read clearly at rail size
    let y_hdr = egui::lerp(r.top()..=r.bottom(), 0.38);
    let header = egui::Rect::from_min_max(r.min, egui::pos2(r.right(), y_hdr));
    painter.rect_filled(
        header,
        egui::Rounding {
            nw: 1.0,
            ne: 1.0,
            sw: 0.0,
            se: 0.0,
        },
        egui::Color32::from_rgba_unmultiplied(235, 225, 255, 55),
    );
    painter.rect_stroke(r, egui::Rounding::same(1.0), stroke);
    // Vertical dividers → 3 columns
    let x1 = egui::lerp(r.left()..=r.right(), 1.0 / 3.0);
    let x2 = egui::lerp(r.left()..=r.right(), 2.0 / 3.0);
    painter.line_segment([egui::pos2(x1, r.top()), egui::pos2(x1, r.bottom())], stroke);
    painter.line_segment([egui::pos2(x2, r.top()), egui::pos2(x2, r.bottom())], stroke);
    // Header rule + body mid-row
    painter.line_segment([egui::pos2(r.left(), y_hdr), egui::pos2(r.right(), y_hdr)], stroke);
    let y_body = egui::lerp(y_hdr..=r.bottom(), 0.5);
    painter.line_segment(
        [egui::pos2(r.left(), y_body), egui::pos2(r.right(), y_body)],
        stroke,
    );
}

/// Min/max Y across series points (pads flat ranges so dual-axis map stays invertible).
fn series_y_range(series: &[(String, [u8; 3], Vec<[f64; 2]>)]) -> Option<(f64, f64)> {
    let mut mn = f64::INFINITY;
    let mut mx = f64::NEG_INFINITY;
    for (_, _, pts) in series {
        for p in pts {
            if p[1].is_finite() {
                mn = mn.min(p[1]);
                mx = mx.max(p[1]);
            }
        }
    }
    if !(mn.is_finite() && mx.is_finite()) {
        return None;
    }
    if (mx - mn).abs() < 1e-12 {
        Some((mn - 1.0, mx + 1.0))
    } else {
        Some((mn, mx))
    }
}

/// Linear map of Y from one axis range into another (dual-Y remapping).
fn map_y(y: f64, from: (f64, f64), to: (f64, f64)) -> f64 {
    let span = (from.1 - from.0).abs().max(1e-12);
    let t = (y - from.0) / span;
    to.0 + t * (to.1 - to.0)
}

/// Remap each series' Y values from `from` range into `to` range (display space).
fn remap_series_y(
    series: &[(String, [u8; 3], Vec<[f64; 2]>)],
    from: (f64, f64),
    to: (f64, f64),
) -> Vec<(String, [u8; 3], Vec<[f64; 2]>)> {
    series
        .iter()
        .map(|(name, color, pts)| {
            let mapped = pts
                .iter()
                .map(|p| [p[0], map_y(p[1], from, to)])
                .collect();
            (name.clone(), *color, mapped)
        })
        .collect()
}

fn format_axis_tick(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let a = v.abs();
    if a == 0.0 {
        "0".into()
    } else if a >= 1.0e4 || a < 1.0e-3 {
        format!("{v:.3e}")
    } else if a >= 100.0 {
        format!("{v:.1}")
    } else if a >= 10.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.3}")
    }
}

fn fit_bounds_for_series(series: &[&(String, [u8; 3], Vec<[f64; 2]>)]) -> Option<PlotBounds> {
    let mut b = PlotBounds::NOTHING;
    for (_, _, pts) in series {
        for p in pts {
            if p[0].is_finite() && p[1].is_finite() {
                b.extend_with(&egui_plot::PlotPoint::new(p[0], p[1]));
            }
        }
    }
    if !b.is_valid() {
        return None;
    }
    // Match egui_plot default margin_fraction (5%).
    b.add_relative_margin_x(egui::vec2(0.05, 0.05));
    b.add_relative_margin_y(egui::vec2(0.05, 0.05));
    Some(b)
}

/// Next free "Chart N" across existing titles (split/duplicate/new never collide).
fn unique_chart_title(charts: &[ChartState]) -> String {
    let used: HashSet<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    let mut n = 1usize;
    loop {
        let candidate = format!("Chart {n}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

/// Rename later duplicates so every chart title is unique (session restore).
fn ensure_unique_chart_titles(charts: &mut [ChartState]) {
    let mut seen = HashSet::<String>::new();
    for i in 0..charts.len() {
        if seen.insert(charts[i].title.clone()) {
            continue;
        }
        let fresh = unique_chart_title(charts);
        charts[i].title = fresh.clone();
        seen.insert(fresh);
    }
}

/// Wheel / pinch zooms **X only** (never past fit-all); Y always fits points in that X window.
/// Plain scroll-over-plot zooms (does not pan — Plot.allow_scroll is off).
fn apply_wheel_zoom_x_and_locked_auto_y(
    plot_ui: &mut PlotUi,
    fit: Option<&PlotBounds>,
    series: &[&(String, [u8; 3], Vec<[f64; 2]>)],
) {
    // Double-click reset sets auto bounds before our Set mods — don't fight it.
    if plot_ui.response().double_clicked() {
        return;
    }

    let mut bounds = plot_ui.plot_bounds();
    let mut x_auto = plot_ui.auto_bounds().x;

    // Stale X window (empty-tile zoom / default 0..1) with real series at unix time → blank
    // plot and frozen axis. If current X misses all points, snap back to fit-all.
    if let Some(fit) = fit {
        if fit.is_valid_x() {
            let x0 = bounds.min()[0];
            let x1 = bounds.max()[0];
            let overlaps_fit = x0 <= fit.max()[0] && x1 >= fit.min()[0];
            let any_in_window = series_has_point_in_x(series, x0, x1);
            if !x_auto && (!overlaps_fit || !any_in_window) {
                plot_ui.set_auto_bounds(egui::Vec2b::TRUE);
                return;
            }
        }
    }

    // --- X-only wheel / pinch ---
    if plot_ui.response().contains_pointer() {
        let (mut zoom, scroll) =
            plot_ui.ctx().input(|i| (i.zoom_delta_2d(), i.smooth_scroll_delta));
        // Ctrl/Cmd-scroll → zoom_delta; plain wheel → scroll_delta. Map plain wheel to zoom.
        if zoom == egui::Vec2::splat(1.0) && scroll != egui::Vec2::ZERO {
            // Match egui scroll_zoom_speed default (~0.01): positive scroll.y zooms in.
            let factor = (0.01_f32 * scroll.y).exp();
            if (factor - 1.0).abs() > 1e-6 {
                zoom = egui::vec2(factor, 1.0);
            }
        }
        // Force X-only: ignore any Y component from pinch / trackpad.
        zoom.y = 1.0;
        if (zoom.x - 1.0).abs() > 1e-6 {
            if let Some(fit) = fit {
                let at_fit_x = bounds.width() >= fit.width() * 0.995;
                if zoom.x < 0.999 && at_fit_x {
                    // Zooming out at floor — stay / return to full auto.
                    plot_ui.set_auto_bounds(egui::Vec2b::TRUE);
                    return;
                }
                if let Some(center) = plot_ui.pointer_coordinate() {
                    bounds.zoom(zoom, center);
                    if bounds.width() >= fit.width() * 0.995 {
                        plot_ui.set_auto_bounds(egui::Vec2b::TRUE);
                        return;
                    }
                    x_auto = false;
                }
            } else {
                plot_ui.zoom_bounds_around_hovered(zoom);
                return;
            }
        }
    }

    // Full auto X → egui fits both axes to the full series (Y ≡ visible when X is fit-all).
    if x_auto {
        return;
    }

    // Locked auto Y: fit Y to series points inside the (possibly just-zoomed) X window.
    let x0 = bounds.min()[0];
    let x1 = bounds.max()[0];
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (_, _, pts) in series {
        for p in pts {
            let (x, y) = (p[0], p[1]);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            if x < x0 || x > x1 {
                continue;
            }
            y_min = y_min.min(y);
            y_max = y_max.max(y);
        }
    }
    if y_min.is_finite() && y_max.is_finite() {
        if (y_max - y_min).abs() < 1e-12 {
            y_min -= 1.0;
            y_max += 1.0;
        }
        let mut y_fit = PlotBounds::from_min_max([x0, y_min], [x1, y_max]);
        y_fit.add_relative_margin_y(egui::vec2(0.05, 0.05));
        bounds.set_y(&y_fit);
        plot_ui.set_plot_bounds(bounds);
    } else if let Some(fit) = fit {
        // Zoomed window lost all points (e.g. after file reload with new X scale) — refit.
        if fit.is_valid() {
            plot_ui.set_auto_bounds(egui::Vec2b::TRUE);
        }
    }
}

/// True if any series sample falls inside inclusive X window.
fn series_has_point_in_x(
    series: &[&(String, [u8; 3], Vec<[f64; 2]>)],
    x0: f64,
    x1: f64,
) -> bool {
    let (lo, hi) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    for (_, _, pts) in series {
        for p in pts {
            let x = p[0];
            if x.is_finite() && x >= lo && x <= hi {
                return true;
            }
        }
    }
    false
}

fn icon_toggle(ui: &mut egui::Ui, on: &mut bool, glyph: &str, tip: &str) {
    let fill = if *on {
        egui::Color32::from_rgb(90, 70, 140)
    } else {
        egui::Color32::from_rgb(40, 34, 54)
    };
    let sense = egui::Sense::click();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), sense);
    ui.painter()
        .rect_filled(rect, egui::Rounding::same(6.0), fill);
    ui.painter().rect_stroke(
        rect,
        egui::Rounding::same(6.0),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(130, 100, 180)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(230, 220, 245),
    );
    if resp.clicked() {
        *on = !*on;
    }
    resp.on_hover_text(tip);
}

/// Table rail button — same chrome as `icon_toggle`, painted 3×2 grid mark.
fn icon_toggle_table(ui: &mut egui::Ui, on: &mut bool) {
    let fill = if *on {
        egui::Color32::from_rgb(90, 70, 140)
    } else {
        egui::Color32::from_rgb(40, 34, 54)
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, egui::Rounding::same(6.0), fill);
    ui.painter().rect_stroke(
        rect,
        egui::Rounding::same(6.0),
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(130, 100, 180)),
    );
    paint_table_glyph_in(ui.painter(), egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 14.0)));
    if resp.clicked() {
        *on = !*on;
    }
    resp.on_hover_text("Table");
}

/// Mini line-chart sparkline (not bar/histogram glyphs).
fn draw_sparkline(ui: &mut egui::Ui, channel_id: Uuid, spark: &[f64]) {
    // Cap to leftover row width so the spark never expands the Channels column
    let w = ui.available_width().clamp(0.0, 56.0);
    if w < 20.0 {
        return;
    }
    let pts: PlotPoints = spark
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.is_finite().then_some([i as f64, *v]))
        .collect();
    let id = egui::Id::new(("spark", channel_id));
    Plot::new(id)
        .height(14.0)
        .width(w)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show_axes([false, false])
        .show_grid([false, false])
        .show_background(false)
        .show_x(false)
        .show_y(false)
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new(pts)
                    .width(1.25_f32)
                    .color(egui::Color32::from_rgb(160, 190, 255)),
            );
        });
}

/// Turn an egui OS drop into a filesystem path (path, file:// URL, or temp from bytes).
fn resolve_dropped_file(f: &egui::DroppedFile) -> Result<PathBuf, String> {
    if let Some(path) = f.path.clone() {
        return Ok(path);
    }
    // Some backends only fill `name` with a file:// URI or absolute path.
    let name = f.name.trim();
    if !name.is_empty() {
        if let Some(path) = path_from_uri_or_str(name) {
            return Ok(path);
        }
    }
    if let Some(bytes) = &f.bytes {
        let fname = if name.is_empty() {
            "dropped.csv".into()
        } else {
            std::path::Path::new(name)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "dropped.csv".into())
        };
        let dir = std::env::temp_dir().join("chart-bench-drops");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(fname);
        std::fs::write(&path, bytes.as_ref()).map_err(|e| e.to_string())?;
        return Ok(path);
    }
    Err("dropped file had no path or bytes".into())
}

fn path_from_uri_or_str(s: &str) -> Option<PathBuf> {
    let s = s.trim();
    if s.starts_with("file://") {
        let rest = s.trim_start_matches("file://");
        // file:///home/foo → /home/foo ; percent-decode lightly
        let decoded = percent_decode_path(rest);
        let p = PathBuf::from(decoded);
        if p.exists() {
            return Some(p);
        }
        // Try with leading slash restored if host-style file://localhost/…
        if let Some(idx) = rest.find('/') {
            let p = PathBuf::from(percent_decode_path(&rest[idx..]));
            if p.exists() {
                return Some(p);
            }
        }
        return None;
    }
    let p = PathBuf::from(s);
    p.exists().then_some(p)
}

fn percent_decode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = || {
                let hi = (b[i + 1] as char).to_digit(16)?;
                let lo = (b[i + 2] as char).to_digit(16)?;
                Some(((hi << 4) | lo) as u8)
            };
            if let Some(c) = h() {
                out.push(c as char);
                i += 3;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

/// Thousands separators for status-bar row counts.
fn fmt_status_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_unix_hms(t: f64) -> String {
    use chrono::{TimeZone, Utc};
    match Utc.timestamp_opt(t as i64, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%H:%M:%S").to_string(),
        _ => format!("{t:.0}"),
    }
}

fn format_unix_full(t: f64) -> String {
    use chrono::{TimeZone, Utc};
    match Utc.timestamp_opt(t as i64, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => format!("{t:.4}"),
    }
}

fn nearest_y(pts: &[[f64; 2]], x: f64) -> Option<f64> {
    let mut best = *pts.first()?;
    let mut best_d = (best[0] - x).abs();
    for p in pts.iter().skip(1) {
        let d = (p[0] - x).abs();
        if d < best_d {
            best_d = d;
            best = *p;
        }
    }
    Some(best[1])
}

/// Window / taskbar icon — chart mark (same art as Windows PE `assets/icons/app.ico`).
pub fn load_window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/icons/chart-bench-app-icon-256.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}
