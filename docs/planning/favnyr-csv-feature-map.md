# Favnyr → CSV Graphing Tool Feature Map

**Status:** Research complete (source-backed).  
**Repo:** https://github.com/GlitchAwakened/Favnyr @ `89c2f99` (v0.3.7, shallow clone 2026-10-27 on M17).  
**Build smoke:** `cargo check -p favnyr-gui` succeeded (Rust 1.97.1 / Slint 1.18.1). No interactive UI smoke (headless check only).  
**No PR** (research only).

---

## Executive summary

**Favnyr is a portable file manager** (Windows/Linux), not a charting app. Stack: **Rust workspace + Slint 1.18 UI** (`favnyr-core` UI-free domain; `favnyr-gui` Slint + bridge). Layout is a **binary split tree** (`LayoutNode`) flattened to absolute rects because Slint cannot recurse components — VSCode-style nested H/V splits, tabs per leaf, detachable views (separate process), named TOML workspaces.

**Transferability:** **High for interaction/layout patterns, low for crates and domain.** Steal the workspace + binary-tree tile model, drag-to-split/tab/detach gestures, sidebar + dense panels, and session persistence. Do **not** port Slint or file-manager domain (thumbnails, shell menus, trash, archive recipes). Charting is **net-new** (`egui_plot` or equivalent).

**Verdict for a 100% Rust CSV/Parquet/XLS graphing tool aligned with TopicBench:** keep Favnyr’s **layout algorithm & UX**, implement shell in **egui + eframe** (TopicBench adjacency). Treat Slint as Favnyr’s choice, not the Project’s.

---

## Favnyr stack & architecture notes

| Item | Finding (source) |
| --- | --- |
| Product | Lightweight multi-view **file browser** (`crates/favnyr-gui/Cargo.toml` description; README) |
| Workspace | Cargo workspace `crates/favnyr-core`, `crates/favnyr-gui` — root `Cargo.toml` |
| Version / MSRV | Package `0.3.7`; `rust-version = "1.92"`; toolchain pin `1.97.1` (`rust-toolchain.toml`) |
| UI framework | **Slint `=1.18.1`** (+ `slint-build`) — not egui/iced |
| Windowing / render | Via Slint backends (winit / femtovg / software; pulled by `slint`) |
| Tiling / dock | **Custom** — no `egui_dock` / `egui_tiles`. Rust binary tree + flatten (`crates/favnyr-core/src/layout.rs`) |
| Persistence | TOML: session `workspace.toml`, named workspaces dir, `config.toml`, themes — `crates/favnyr-core/src/paths.rs`, `workspace.rs`, `config.rs` |
| Domain crates | `image`, `notify`, `trash`, `sysinfo`, `zip`, `dirs`, `arboard` / Wayland clipboard; Windows `windows` crate for Shell/OLE |
| Plotting | **None** |

### Crate layout

```
Favnyr/
  Cargo.toml                 # workspace + shared deps (slint, serde, toml, …)
  crates/favnyr-core/        # UI-free: layout, workspace, fs, themes, thumbnails, …
  crates/favnyr-gui/         # bin `favnyr`: Slint UI + Rust bridge
    src/ui/*.slint           # main_window, panel, tabs, sidebar, …
    src/bridge.rs            # AppState ↔ Slint callbacks (large)
    src/detached_view.rs     # tear-off view → new process
```

### Core layout model

From `crates/favnyr-core/src/layout.rs`:

- **`LayoutNode`**: `Leaf { panel: usize }` | `Split { dir: SplitDir, ratio, first, second }` — binary tree.
- **`SplitDir`**: `Row` (side-by-side) | `Column` (stacked). UI avoids ambiguous “horizontal/vertical” wording.
- **Flatten:** `LayoutNode::compute(area, gap) → Layout { views: ViewGeom[], splitters: SplitterGeom[] }` — absolute pixel rects for Slint `Repeater`s (no recursive Slint components).
- **Ops:** `split_leaf`, `remove_view` (collapse sibling = merge-away pane), `move_view_beside`, `swap_views`, `set_ratio`, `equalize` / `restore_ratios`.
- **Cap:** `MAX_PANELS = 32` in `crates/favnyr-gui/src/bridge.rs`.

### Document / view model

From `crates/favnyr-core/src/workspace.rs`:

- **`WorkspaceState`**: `panels: Vec<ViewState>` + optional `layout: LayoutNode` + `active_panel` + closed-tab history + sidebar collapse + optional `workspace_name`.
- **`ViewState`** (serialized as panel): stretch, tabs, active_tab, columns, tab_bar_mode, vbar_width.
- **`TabState`**: path, sort, preview/hidden, zoom, list|grid arrangement.
- Named workspaces: one TOML each under `workspaces/`; slots 1–9 for Ctrl+N load (`shortcuts.rs`).

### Drag / split encoding

- Tree lives in Rust (`AppState.layout: Rc<RefCell<LayoutNode>>` in `bridge.rs`).
- Slint paints flat `PanelBox` / `SplitterView` lists; drag zones (center = tab, edges = split, outside = detach) in `main_window.slint`.
- File DnD: modifiers Ctrl=copy / Shift=move (`file_drag.rs`); Space+folder-drag opens for split/tab drop (README pro tips).
- Detach: spawn sibling process with `--detached-view-state` + TOML `ViewState` (`detached_view.rs`).

### Dependencies excerpt (workspace)

Pinned in root `Cargo.toml`: `slint`/`slint-build` 1.18.1, `serde`, `toml`, `tracing*`, `notify`, `image` (pure Rust decode), `dirs`, `sysinfo`, `trash`, `open`, `zip`, `arboard`, `wl-clipboard-rs`. GUI adds platform Shell bindings on Windows.

---

## Feature inventory table

**Legend — CSV-tool rating:** Keep = reuse idea as-is · Adapt = keep idea, change semantics · Drop = not useful for tabular graphing.

| Feature | Favnyr behavior (cited) | CSV-tool rating | Notes |
| --- | --- | --- | --- |
| Nested tiled splits | Binary `LayoutNode` H/V splits, flatten to rects; up to 32 panels (`layout.rs`, `bridge.rs` `MAX_PANELS`) | **Keep** | Primary shell: Files \| Columns \| Chart(s) \| Table. Port algorithm to egui, not Slint. |
| Split gestures | Menu/shortcuts side-by-side / stacked; edge-drop creates split (`main_window.slint` `panel-split`, `strings.slint` drop-split) | **Keep** | Same affordances for chart/table tiles. |
| View “merge” | Closing/removing a view collapses its `Split` into the sibling (`remove_view` / `collapse_leaf` in `layout.rs`) | **Adapt** | Pane collapse = Keep. Prefer **series merge onto one chart** as the high-value “merge” for CSV. |
| Move / swap tiles | `move_view_beside`, `swap_views` (`layout.rs`); layout drag zones in GUI | **Keep** | Reposition chart tiles without losing series bindings. |
| Equalize splits | Double-click splitter / `equalize` + undo ratios (`layout.rs`, `PanelSplitterAbs` in `panel.slint`) | **Keep** | Nice polish; low cost if tree model exists. |
| Tabs per leaf | Each `ViewState` has `tabs[]` + `active_tab`; drag tab between views / to edges (`workspace.rs`, tab UI in `tabs.slint`) | **Keep** | Multiple charts or tables in one leaf. |
| Detach / floating | Tear-off view → new OS window via new process + `DetachedView` (`detached_view.rs`, detach icon/strings) | **Adapt** | Dual-monitor charts useful; prefer egui multi-viewport / second `eframe` window over Favnyr’s process-spawn model. |
| Named workspaces | Saved layouts + tabs; panel Ctrl+Q; slots Ctrl+1..9; dirty policies (`workspace.rs`, `shortcuts.rs`, README) | **Keep** | Session = files + tile tree + series bindings. |
| Session vs config | Layout/tabs in data TOML; window size & prefs in `config.toml` (`workspace.rs` header, `paths.rs`) | **Keep** | Same split for the CSV tool. |
| Left sidebar / Places | Shortcuts, favorites, drives, network; collapsible sections (`sidebar.slint`, `places.rs`) | **Adapt** | → file tray / recent datasets / open workbooks; drop drive eject / MTP. |
| Favorites | Shared across workspaces, tree containers (`favorites.rs`) | **Adapt** | Pin frequent CSV/Parquet paths or workspaces. |
| Columns (list) | Per-view show/hide/resize/reorder (`columns.rs`, `ViewState.columns`) | **Adapt** | Map to table column chrome + chart-channel metadata, not file-list columns. |
| List / grid + zoom | Item arrangement + Ctrl+wheel zoom (`ItemArrangement`, tab zoom fields) | **Drop** | File-browser presentation; table virtualization replaces this. |
| Type-to-filter / ext filter | In-view name filter (README; filter UI in `file_list.slint`) | **Adapt** | Column/channel name filter in the browser panel. |
| File DnD between views | Copy/move/link with progress; cross-app DnD (`fs/ops.rs`, `transfer.rs`, `windrag.rs`) | **Adapt** | → drag **column → chart** / series between charts; not filesystem copy. |
| Drag modifiers | Ctrl copy, Shift move, bare = choose (`file_drag.rs`) | **Adapt** | e.g. Alt-drag copy series; bare = move series. |
| Keyboard shortcuts | Fully rebound map (`shortcuts.rs`); Tab cycles views | **Keep** | Match TopicBench keyboard-first habit. |
| Themes | TOML themes, light/dark roles, per-workspace theme override (`theme/`, README) | **Adapt** | Keep theming hooks; DAQ-readable contrast over media chrome. |
| i18n | 8 languages (`i18n/*.toml`) | **Drop** for MVP | Add later if productized. |
| Thumbnails / previews | Images, audio covers, 3D, video/PDF via tools (`thumbnail/`) | **Drop** | Irrelevant to tabular graphing. |
| Shell / Open with / recipes | Custom openers, 7-Zip/email recipes (`openers.rs`, `archives.rs`) | **Drop** | OS open for source files only if needed. |
| Properties / checksums | Prop panel + MD5/SHA (`properties.rs`, `checksum.rs`) | **Drop** / light Adapt | Maybe file metadata later; not MVP. |
| Annotations (color/note) | Folder colors & notes (`annotations.rs`) | **Drop** for MVP | Optional “mark series” later. |
| Terminal / trash / eject | Desktop FM features (`actions.rs`, `eject.rs`, `trash`) | **Drop** | |
| Recursive folder size | Background walk (`folder_stats` bridge) | **Drop** | |
| Built-in charts | Absent | **N/A → Add** | Supply via `egui_plot` (or custom painter). |
| Plugin model | Absent (hard-coded openers) | **Drop** for MVP | |
| Network / telemetry | Explicitly none (README) | **Keep** as principle | Local-first desktop tool. |
| Collaborative features | Absent | **Drop** for MVP | |

---

## CSV / Parquet / XLS-specific feature proposals

These remain **net-new** relative to Favnyr’s file-manager shell (inspired by TopicBench graph workflows + DAQ practice). No Favnyr contradiction found.

### Data & channels
- **Unified table model:** arrow-like columnar in-memory (or `polars` / `arrow`) after load; CSV / Parquet / XLS(X) as import adapters.
- **Channel/column browser:** dtype, null %, min/max/sparkline; multi-select; search/filter by name.
- **Time/index axis:** auto-detect timestamp columns; fallback to row index; optional secondary X.
- **Units & scale:** per-series scale (linear/log), offset, unit label; dual Y axes.
- **Derived channels:** simple expressions (`a - b`, rolling mean) — post-MVP ok.

### Multi-file
- **File tray:** open/close, reload-on-change, sheet picker for XLS.
- **Join/align:** overlay series from different files on shared X (time or sample); optional time-offset calibration.
- **Color by file** + legend grouping.

### Chart interaction (core UX)
- **Drag column → empty tile** creates a chart; **drag column → existing chart** adds a series (merge).
- **Drag series between charts** / onto “new chart” drop zone; **Alt-drag** copies (mirrors Favnyr modifier spirit).
- **Chart types:** line (default), scatter, step, area, bar (categorical), histogram; heatmap later.
- **Linked cursors / crosshair** across tiles; shared X zoom when “link axes” enabled (TopicBench-like).
- **Box zoom, pan, wheel zoom, reset;** Y-autoscale per series or global.
- **Table ↔ chart selection sync:** click row ↔ highlight point; brush range → filter table.

### Layout (Favnyr-shaped)
- Port **binary split tree + flatten** (or `egui_tiles` / `egui_dock` if faster to ship — see recommendation). Favnyr proves the tree+flat-rect approach works without a docking crate.
- Leaves: Files | Columns | Chart(s) | Table; tabs inside leaves; optional detach for dual monitors.
- “Merge charts” = union series + axis policy (semantic Adapt of pane-close merge).

### Export / session
- Export PNG/SVG of chart; export visible table slice CSV.
- Workspace file: paths + layout + series bindings (not a full data copy) — same spirit as Favnyr TOML workspaces.

### Non-goals (MVP)
- Full spreadsheet editing, SQL IDE, cloud sync, realtime MQTT (TopicBench covers streaming), Favnyr FM domain (trash, thumbnails, shell recipes).

---

## Chart framework evaluation (Rust-first)

Constraint: **100% Rust**, prefer **egui** (TopicBench). Favnyr’s Slint choice does **not** force Slint for this Project.

| Option | Fit | Pros | Cons | Verdict |
| --- | --- | --- | --- | --- |
| **`egui_plot`** | Excellent | Same IMM stack as TopicBench; interactive; simple series API | Limited exotic types; large-N needs downsampling | **Primary recommendation** |
| **Custom `egui::Painter` / mesh lines** | High | DAQ-grade dense waveforms | Reinvent axes/legend | Hot paths if `egui_plot` stalls |
| **`plotters` + egui bridge** | Medium | Publication-quality static charts | Weaker live interaction | Optional export/reports |
| **Slint + custom plot** | Medium (Favnyr-parity UI) | Matches Favnyr chrome literally | No TopicBench reuse; plot ecosystem thinner; second skill stack | **Drop** unless Project abandons egui |
| **`plotly` / webview** | Poor | Familiar API | Not 100% Rust desktop feel | **Drop** |
| **`iced` + plots** | Low | Retained UI | No TopicBench or Favnyr reuse | **Drop** |
| **Qt/GTK plots** | Low | Mature | Dual stack | **Drop** |
| **`charming` (ECharts)** | Low–med | Fancy charts | Often JS/webview | Avoid for core |
| **`polars` / `arrow` / `calamine` / `csv`** | Required | Idiomatic tabular I/O | N/A | **Keep** as data layer |

**Performance:** multi-million-row → plot **decimated** series (minmax / LTTB); virtualize table rows.

---

## egui recommendation for this Project

**Use egui + eframe end-to-end** for the CSV graphing tool. Favnyr uses Slint; that is a **pattern source**, not a stack mandate. TopicBench adjacency + existing plot habits outweigh UI-framework parity with Favnyr.

Suggested crate set (MVP):
- `eframe`, `egui`, `egui_plot`
- **Docking:** either (a) **port Favnyr’s `LayoutNode` binary tree + flatten** into a small egui module (closest UX fidelity), or (b) adopt **`egui_tiles` / `egui_dock`** for speed. Do **not** expect to “match Favnyr’s docking crate” — Favnyr has none.
- Data: `polars` *or* `arrow` + `parquet`; `csv`; `calamine` (XLSX)
- Persistence: `serde` + `toml`/`ron`/`json` for workspaces (TOML matches Favnyr ops familiarity)

**Transfer from Favnyr (ideas, not code license assumptions — Favnyr is GPL-3.0):** nested splits, tabbed leaves, edge-drop split, center-drop tab, detach, named workspaces, equalize splitter, rebound shortcuts, local-first TOML state.

**Transfer from TopicBench:** combined multi-series time plots, sticky/watch panes, keyboard shortcuts, dense panels — re-skin “MQTT topic” → “column/channel.”

**Do not** introduce iced or a webview chart layer unless a later requirement forces HTML export. **Do not** adopt Slint solely to look like Favnyr.

**License note:** Favnyr is GPL-3.0-or-later. Copying substantial Favnyr source would viral-GPL the tool; reimplement the layout/UX from this map instead of vendoring `layout.rs`.

---

## Suggested MVP scope (ordered)

1. **Shell:** egui app with dock/tile layout — Files panel, Columns panel, one Chart leaf, one Table leaf; save/load layout stub (binary tree or `egui_tiles`).
2. **Loaders:** CSV + Parquet; XLSX sheet pick; columnar preview (first N rows) + schema.
3. **Plot path:** pick X + one+ Y columns → `egui_plot` line; legend; basic zoom/pan.
4. **Drag interactions:** drag column name onto chart to add series; drag to empty tile to spawn chart (Favnyr edge/center drop semantics).
5. **Multi-file:** two files open; overlay series; color-by-file.
6. **Linked X** optional toggle across chart tiles; crosshair readout.
7. **Workspace save** (paths + bindings + layout) — Favnyr-like named sessions.
8. **Polish:** downsampling, histogram/scatter, export PNG, expression channels; optional detach window; equalize splits.

**Defer:** GPL-safe clean-room only for any Favnyr-inspired layout code; plugins; realtime sources; i18n; FM domain features.

---

## Appendix A — Research evidence

| Check | Result |
| --- | --- |
| `git clone --depth 1 https://github.com/GlitchAwakened/Favnyr.git` | OK on M17 → `/tmp/favnyr-audit/Favnyr` |
| Commit | `89c2f99d7518e3b66d982690813d02074627e3c8` (tag line: Point the Nix recipe at 0.3.7) |
| `cargo check -p favnyr-gui` | Success (~1m42s) |
| Interactive UI run | Not required; skipped |

## Appendix B — Key source map

| Concern | Path |
| --- | --- |
| Workspace deps / Slint pin | `Cargo.toml` |
| Layout tree + flatten | `crates/favnyr-core/src/layout.rs` |
| Workspace / tabs / panels | `crates/favnyr-core/src/workspace.rs` |
| Paths (config/workspace/themes) | `crates/favnyr-core/src/paths.rs` |
| Shortcuts | `crates/favnyr-core/src/shortcuts.rs` |
| Themes | `crates/favnyr-core/src/theme/` |
| GUI bridge / `MAX_PANELS` | `crates/favnyr-gui/src/bridge.rs` |
| Main window / drop zones | `crates/favnyr-gui/src/ui/main_window.slint` |
| Panel + splitter | `crates/favnyr-gui/src/ui/panel.slint` |
| Detached views | `crates/favnyr-gui/src/detached_view.rs` |
| DnD modifiers | `crates/favnyr-gui/src/file_drag.rs` |
| Product overview | `README.md` |
