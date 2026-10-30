# Chart-Bench — project lockdown

**Repo:** https://github.com/unipsycho/chart-bench  
**Version:** 0.8.0  
**License:** MIT OR Apache-2.0

## Build / run

```bash
cd chart-bench
cargo run --release
```

Optional checks:

```bash
cargo test --lib
# Demo: click Demo CSV, or Open / drop files
# Follow growing file: ./scripts/grow_demo.sh samples/live_growing.csv  then Open + Follow
```

Binary: `target/release/chart-bench`.

## What this is

Desktop workbench for **CSV / XLS(X) / (soon) Parquet** graphing. DAQ-style time-series first: open dumps fast, preview channels, drag onto charts, save layouts as presets.

**Inspiration:** Favnyr (Rust + Slint file manager) — layout UX ideas only (GPL-3.0 → clean-room; no Favnyr source).

## Stack

| Choice | Decision |
|--------|----------|
| Language | **100% Rust** |
| UI | **egui + eframe** (TopicBench-aligned) |
| Charts | **egui_plot** — cartesian / DAQ only |
| Not using | Slint; pie / 3D / BI-gallery chart menus |
| Formats | CSV + XLSX now; Parquet stub → real support later |

Pinned crate versions in `Cargo.toml` (egui/eframe/egui_plot **0.29**).

## Favnyr UX goals

- Nested splits / tabs / workspaces (ideas, not a Slint port).
- Drag zones: center = tab, edge = split, outside = detach (tree + flatten + hit-test overlays).
- Visual chrome: icon rail, framed main window, clean borders (MVP has rail + framed canvas; full drag-to-split still open — see `GAPS.md`).
- Prefer custom tile tree for fidelity, or `egui_tiles` / `egui_dock` for a faster MVP.

## Product decisions (durable)

- **Workflow:** fast file select → channel preview → DnD onto charts (multi-file overlay, multi-axes); session autosave; reusable **Presets** with file slots + channel re-bind (Projects can layer later).
- **Headers:** flexible multi-line CSV (unknown depth; roles name/node/units); find data start via stable column width + data-likeness; user overrides required. LabVIEW-style: prefer instrument/name row over title/operator.
- **Time/X:** auto-scan timestamp columns (name + value shape); else row-index; user override.
- **Charts:** default **Overlay + shared Y**; stacked / side-by-side and multi-axis optional. Must-haves: multi-axes, clear legends, mouse-over readout, column previews, export.
- **Info panel:** records, time range, channel count, spacing regularity, per-channel stats / missingness; local heuristic quality flags — no cloud ML for v1.
- **Live files:** Follow mode (watch + debounced append); full reload on truncate/replace; follow-tail vs keep zoom; pause/resume.
- **Chrome:** compact top bar + panel toggles + command palette — not a TopicBench-sized ribbon. Files stacked above Channels; single-line channel rows with scroll.
- **Sniffing:** learn from OSS (qsv, CleverCSV, DuckDB, Polars, PlotJuggler); multi-line name/node/units headers likely custom.

## Preferences

- Broad competitive review of CSV/tabular graphing tools — done (`docs/planning/competitive-csv-graphing-survey.md`); use wedges there when locking MVP.
- egui reimplements Favnyr-style drag zones — not a Slint port.
- No pie, 3D, or BI-gallery charts.
- Want Favnyr-like visual chrome and drag-to-split / duplicate chart tiles.

## Doc map

| File | Role |
|------|------|
| `PROJECT.md` | This file — durable decisions |
| `GAPS.md` | Outstanding gaps |
| `TODO.md` | Actionable checklist |
| `README.md` | Quick start + MVP feature table |
| `docs/planning/` | Full planning reports |

Key planning reports: `favnyr-csv-feature-map.md`, `plot-capabilities-scope.md`, `csv-header-channel-detection.md`, `product-workflow-vision.md`, `data-info-panel-analysis.md`, `live-file-reload.md`, `csv-detection-tools-landscape.md`, `competitive-csv-graphing-survey.md`, `csv-bench-competitors.md` (technical competitor notes), `ui-chrome-navigation.md`, `csv-bench-m17-build.md`, `csv-bench-sample-csvs.md`, `csv-bench-ui-pass.md`, `project-context.md`.
