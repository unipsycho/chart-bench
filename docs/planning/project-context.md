# CSV Graphing — project context

> Snapshot of decisions; durable copy is root [`PROJECT.md`](../../PROJECT.md). Paths below are local to this repo.

## Home
- Local project folder (M17): `/home/mike/Documents/AAI-Projects/csv-bench`
- Inspiration: Favnyr (Rust + Slint file manager) — layout UX only; GPL-3.0 → clean-room
- Feature map: [favnyr-csv-feature-map.md](favnyr-csv-feature-map.md)
- Plot capability scope: [plot-capabilities-scope.md](plot-capabilities-scope.md)
- CSV header / channel detection: [csv-header-channel-detection.md](csv-header-channel-detection.md)
- Workflow vision (DnD + presets): [product-workflow-vision.md](product-workflow-vision.md)
- Data info / quality analysis: [data-info-panel-analysis.md](data-info-panel-analysis.md)
- Live / growing files: [live-file-reload.md](live-file-reload.md)
- OSS CSV detection / profiling landscape: [csv-detection-tools-landscape.md](csv-detection-tools-landscape.md)
- Competitive CSV/tabular graphing survey: [competitive-csv-graphing-survey.md](competitive-csv-graphing-survey.md)
- UI chrome / panel navigation: [ui-chrome-navigation.md](ui-chrome-navigation.md)
- External sample CSVs catalog: [csv-bench-sample-csvs.md](csv-bench-sample-csvs.md)

## Decisions
- Stack: **100% Rust**, **egui + eframe + egui_plot** (TopicBench-aligned), not Slint
- Layout: Favnyr-style nested splits/tabs/workspaces (port ideas, not Favnyr source)
- Formats: CSV, Parquet, XLS(X)
- Charts: cartesian / DAQ-style only — **no pie, 3D, or BI gallery**
- Layer A must-haves: multi-axes, clear legends, mouse-over readout, column previews, export (PNG/CSV + workspace)
- **Flexible multi-line CSV headers** + smart channel detection (unknown header depth; roles like name/node/units; find data start via stable repeating column width + data-likeness); user overrides required
- **Workflow:** fast file select → channel preview → DnD overlay onto charts (multi-file, multi-axes); auto-save session; reusable **Presets** with file slots + channel re-bind; history preview/revert (see product-workflow-vision.md)
- **Info panel:** records, time range, channel count, spacing regular vs sporadic, per-channel stats + missingness; heuristic quality flags (gaps, spikes, flatlines); trends/anomaly helpers local-only — no cloud ML for v1
- **Live files:** Follow mode for files being written — watch + debounced append ingest for CSV; full reload on truncate/replace; follow-tail vs keep zoom; pause/resume (see live-file-reload.md)
- **Time/X:** auto-scan for timestamp/time columns (name + value shape: ISO, epoch sec/ms/us); use as X when found; else row-index; user override (see csv-header-channel-detection.md Step F)

## Decisions (continued)
- egui can reproduce Favnyr drag zones via tree+flatten + hit-test overlays (center=tab, edge=split, outside=detach); not a Slint renderer port
- Prefer custom tree for Favnyr fidelity, or `egui_tiles`/`egui_dock` for faster MVP
- **Must-have interactivity:** multi-axes, clear legends, mouse-over readouts, previews, export/save to other files (see plot-capabilities-scope.md §0)
- Preset metaphor **recommended** over heavy “Project” for v1 (bookmarks = quick presets); Projects can layer on later
- **Chrome (proposed):** no fat TopicBench-style ribbon for v1 — compact top bar + panel toggles + Favnyr tiling/DnD + command palette/F1; optional thin File/View/Help menu; per-chart mini toolbar (see ui-chrome-navigation.md)

## Decisions (continued)
- Default layout: Files above Channels; single-line channel rows; chart default **Overlay + shared Y**; multi-axis auto range groups; Favnyr-ish icon rail + framed chrome (see csv-bench-ui-pass.md)
- Multi-row LabVIEW-style headers: prefer instrument/name row over title/operator for channel names

## Open
- Favnyr-style drag-to-split / detach for chart tiles (Duplicate exists; full gestures still next)
- Confirm Preset vs Bookmark naming; slot naming; channel mismatch UX
- Broader product-competitive survey still open
- Confirm export formats and anomaly threshold UX

See also root [`GAPS.md`](../../GAPS.md) and [`TODO.md`](../../TODO.md).
