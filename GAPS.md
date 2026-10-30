# Outstanding gaps

Living list of product / engineering gaps. Pair with `TODO.md` for action items.

## UX / Favnyr tiling

- Chart tiles: **edge drop = SPLIT** (duplicate into Stacked / Side-by-side) with explicit TOP / BOTTOM / LEFT / RIGHT landing preview + seam/chevron; **center = MOVE**. Own-tile edge split works (needed for single-chart Overlay). Multi-pane auto-leaves Overlay so tiles stay visible. Nested binary split tree **deferred** — not required for docks or charts.
- Dock columns: full-column drop targets with **top/bottom** (or L/R on bottom strip) stack bands only — mid “split here” insert removed. Empty column takes the whole drop. Docks move only (no duplicate).
- Left Files/Channels column min width lowered (~140) so names can sit in a thinner column.
- Section headers show glyphs (Files/Channels/Info/Saved/Table + Charts ▤). Chart **Pop-out** opens an independent egui/OS viewport.
- Under-chart series checkbox / L·R strip removed — plot fills the chart area; axis toggle via Channels **←/→** and **L/R** buttons.
- Channels: tighter rows; ↑↓ nav; **Space**/Enter = **selection**; **←** = left Y toggle; **→** = right Y toggle; **L/R** buttons colored when on that axis. ↑↓/hover faded **preview**; plotted channels highlighted for the focused chart.
- Chart hover: vertical + horizontal crosshair + legend-style Y readout for each plotted series at that X.
- Sparklines are mini **line** plots (~48 samples), not unicode bars.
- Icon rail + framed chrome shipped; Info+Saved share one scrollable right column.
- Chart wheel-zoom clamps to fit-all; plot grids dimmed via quieter plot text color.
- Channels header shows/chooses the active chart; chart titles stay unique on new/split/duplicate; hover/wheel does not pan axes (`allow_scroll(false)` + scroll→zoom). Channel hover preview clears when the pointer leaves the Channels list. Chart crosshair is screen-space (VLine/HLine plot items were expanding auto_bounds on hover, including empty charts).
- Command palette / F1 mentioned in chrome design — not confirmed as implemented end-to-end.

## Presets / session persistence

- **Fixed:** presets and sessions store all open file slots + per-chart series with **L/R axis**, color, and stable remapping keys (`file_path`, `channel_key` / display name, `raw_column`). On restore/apply, channels rebind after files reopen (new UUIDs). Chart layout + Y-axis mode ride along with presets.
- Still open: Preset vs Bookmark naming; friendly UX when some channels fail to rematch after a schema change.

## Data formats

- **Parquet** — typed ingest (`parquet` crate, no Arrow): **UNCOMPRESSED / SNAPPY / GZIP / ZSTD / LZ4 / BROTLI**; full-row load with progress (refuses files above ~10 GiB).
- **MATLAB `.mat`** — Level-5 numeric vectors via `matfile` (time-like var → X, else index).
- **MDF/MF4** — ASAM MF4 via `falcon_mdf` (channel groups → datasets). Classic MDF3 (`.mdf`) listed in Open filters but not covered by sample fixtures.
- **WITSML** — wellLog XML mnemonics + `logData` rows via `quick-xml`.
- **SQLite** — each time-series table opens as `file :: table` (same multi-sheet UX as XLSX).
- Export formats beyond chart CSV / session — confirm PNG + workspace export UX.
- Broader dialect / encoding edge cases beyond current sniffer (learn from qsv / CleverCSV / DuckDB / Polars).

## Product / naming

- Confirm **Preset vs Bookmark** naming; file **slot** naming; channel-mismatch UX when rebinding presets.
- History preview / revert for session (vision doc) — depth of implementation TBD.

## Competitive / research

- **Competitive survey** of CSV and tabular graphing tools — **done** → `docs/planning/competitive-csv-graphing-survey.md`. Wedges to protect: dual/multi Y (PlotJuggler wontfix), multi-line CSV headers, Favnyr tiling, slot-based presets. Do not chase ROS/3D/BI galleries.

## Analysis / quality

- Anomaly threshold UX for info-panel heuristics (gaps, spikes, flatlines).
- Optional LLM assist deferred; keep local-only for v1.

## Samples / verification

- OTSG2 multi-row LabVIEW headers improved; keep expanding real dump coverage under `samples/`.
- Mid-size soak CSVs (~100k–500k) may exist locally but are gitignored from the public tree (see `samples/README.md`).
