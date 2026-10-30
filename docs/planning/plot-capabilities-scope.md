# Plot capabilities scope (egui_plot + chart-bench)

**Audience:** product review before building.  
**Stack assumption:** egui + eframe + **egui_plot** (TopicBench-aligned). Custom `Painter` only where egui_plot is too weak.  
**Related:** [favnyr-csv-feature-map.md](favnyr-csv-feature-map.md), [project-context.md](project-context.md).  
**Confidence:** API surface from egui_plot’s public design (crate split from egui); verify exact type names against the pinned crate version at scaffold time — docs.rs unreachable from the cloud agent that authored this.

---

## 0. Owner must-haves (locked)

These are **first-class product requirements**, not polish. They define Layer A success as much as “can draw a line.”

| Must-have | Meaning for chart-bench | egui_plot fit |
| --- | --- | --- |
| **Multi-axes** | Dual (or multi) Y scales on one plot and/or stacked plots with independent Y + **linked X**; series assignable to an axis | Linked plots + dual-Y transform/overlay; assign series → left/right axis |
| **Clear legends** | Readable series names (file + column), color swatch, click show/hide, not cramped or truncated by default | Built-in legend + our naming/style layer |
| **Mouse-over** | Hover → crosshair + tooltip/readout of X and each visible series’ Y at that X (nearest sample); works across linked tiles | Pointer coords + our readout UI on top of plot |
| **Previews** | Lightweight preview before full commit: e.g. column list sparklines / mini-plot on hover or select; drag-ghost preview when dropping a column onto a chart | Small `Plot` or painter sparkline widgets + DnD preview overlay |
| **Save out to other files** | Export plot image (PNG) and/or data (CSV of plotted / visible window); save workspace/session so charts can be reopened | Image capture of plot rect; CSV writer; TOML/JSON workspace |

**Out of scope reminder:** no pie / 3D / BI gallery.

---

## 1. What egui_plot gives you out of the box

Think of egui_plot as an **interactive 2D cartesian plot canvas** with first-class series items, not a BI chart gallery (no native pie/sankey/3D).

### 1.1 Native / first-class plot items (typical)

| Item | What it is | Good for |
| --- | --- | --- |
| **Line** | Polyline through `(x,y)` | Time series, continuous channels (default DAQ view) |
| **Points** | Scatter markers | Samples, outliers, sparse events |
| **BarChart** | Vertical (or horizontal) bars | Histograms (pre-binned), category counts, discrete X |
| **BoxPlot** | Box-and-whisker | Distribution summary per category / file |
| **Arrows** | Vector field / quiver-ish | Rare for Chart-Bench; defer |
| **Polygon** | Filled / stroked shapes | Confidence bands, ROI overlays |
| **HLine / VLine** | Infinite reference lines | Thresholds, triggers, “now”, zero line |
| **Text** | Anchored labels | Annotations at points |
| **PlotImage** | Image in plot coords | Spectrogram / heatmap as image (DIY) |

You can also implement **`PlotItem`** yourself for exotic series (candles, error bars, step-with-gap, etc.).

### 1.2 Canvas / interaction features (typical)

| Capability | Notes |
| --- | --- |
| Pan / zoom | Drag, scroll, boxed zoom (configurable allow_*) |
| Legend | Click-to-hide style patterns common in demos |
| Axis formatters | Custom tick labels (time, SI prefixes, degrees) |
| Linked axes | Share X and/or Y across multiple `Plot` widgets — **critical** for multi-tile linked cursors |
| Auto-bounds | Fit data; pin include_x/include_y; aspect locks |
| Cursor readout | Pointer → plot coords (build crosshair UI on top) |
| Coordinate transforms | Screen ↔ plot for custom overlays / click-to-select |

### 1.3 What egui_plot is *not*

- Not a spreadsheet chart wizard (Excel-style “recommended charts”).
- Not 3D / surface / volume.
- Not a full statistical suite (regressions, ANOVA UI).
- Not automatically fast for **tens of millions** of raw points — you supply decimated series.
- Pie / donut / sunburst / treemap / sankey / network / 3D: **out of scope** (owner: not wanted).

---

## 2. Chart types to consider (product menu)

Ratings for **Chart-Bench** (tabular CSV/Parquet/XLS, DAQ-ish, Favnyr tiling, TopicBench-like multi-series):

**MVP** = ship soon · **Soon** = after core line workflow · **Later** = nice · **Skip** = wrong product shape

### 2.1 Core cartesian (primary product)

| Chart | egui_plot path | Rating | Why |
| --- | --- | --- | --- |
| **Line** (multi-series) | `Line` | **MVP** | Default mental model for channels vs time/index |
| **Step line** | `Line` with stepped points *or* custom item | **Soon** | Digital / PLC-style signals |
| **Scatter** | `Points` | **MVP** | XY relationships, not only time |
| **Line + markers** | `Line` + `Points` same series | **Soon** | Sparse timestamps still readable |
| **Area / filled under line** | `Line` fill / polygon band | **Soon** | Cumulative, envelope |
| **Stacked area** | Multiple filled series + stacking math | **Later** | More BI than DAQ |
| **Dual Y-axis** | Two plots linked X *or* custom transform | **Soon** | Mixed units (temp + pressure) — users will ask early |
| **Secondary X** (rare) | Custom | **Later** | |

### 2.2 Distribution & discrete

| Chart | egui_plot path | Rating | Why |
| --- | --- | --- | --- |
| **Histogram** | Pre-bin → `BarChart` | **Soon** | Column distribution; easy win |
| **Bar (categorical)** | `BarChart` | **Soon** | Grouped by label column |
| **Grouped / stacked bars** | Multiple bar sets + offset math | **Later** | |
| **Box plot** | `BoxPlot` | **Later** | Compare files/batches |
| **Violin / density** | Custom | **Skip / Later** | Niche |

### 2.3 Scientific / DAQ specialty

| Chart | Path | Rating | Why |
| --- | --- | --- | --- |
| **Error bars** | Custom `PlotItem` or arrows+points | **Later** | Lab data |
| **Band / min–max envelope** | Polygon or dual lines | **Soon** | Decimated view of dense data (show range) |
| **Spectrogram / heatmap** | `PlotImage` or custom mesh | **Later** | Powerful but heavy UX |
| **FFT / PSD line** | Compute → `Line` | **Later** | Feature is compute+UI, plot itself is line |
| **XY with color by 3rd column** | `Points` + per-point color | **Later** | |
| **Polar / smith / Bode special** | Custom | **Skip** unless you target RF |

---

## 3. Capabilities beyond “chart type”

These matter as much as the chart menu for a serious desktop tool.

### 3.1 Series & data binding

| Capability | Recommendation |
| --- | --- |
| Multi-series on one plot | **MVP** |
| Drag column → add series | **MVP** (product differentiator with Favnyr-like DnD) |
| Merge series from multiple files | **MVP** |
| Color / style by file or series | **MVP** |
| Show/hide series (legend or checklist) | **MVP** — **must-have clear legends** |
| Per-series scale / offset / unit | **MVP** for axis assignment; offset math **Soon** |
| Derived series (expr: `a-b`, rolling mean) | **Later** (compute pipeline) |
| **Auto-detect time/X column** (else index) | **MVP** — **owner must-have**; see csv-header-channel-detection.md Step F |
| Manual X = time \| index \| chosen column | **MVP** override |

### 3.2 Axes, time, units

| Capability | Recommendation |
| --- | --- |
| Numeric X/Y | **MVP** |
| **Multi-axes (dual Y / series→axis)** | **MVP** — **owner must-have** |
| Time axis formatting (absolute / relative-to-start) | **MVP** — engineering data is usually timestamped |
| Log Y (and log X) | **Soon** |
| Linked X across tiles | **MVP** (TopicBench “combined graph” DNA) |
| Linked cursor / crosshair + readout | **MVP** — **owner must-have mouse-over** |
| Autoscale Y on visible X window | **Soon** |
| Manual Y limits / “stick scale” | **Soon** |
| SI prefixes / engineering notation | **Soon** |

### 3.3 Navigation & selection

| Capability | Recommendation |
| --- | --- |
| Pan / zoom / box zoom / reset | **MVP** (mostly free from egui_plot) |
| Zoom X only / Y only | **Soon** |
| Click point → highlight table row | **Soon** |
| Brush range → filter table / stats | **Later** |
| Measure tool (Δx, Δy between cursors) | **Soon** (DAQ users love this) |
| Annotations (vertical markers, notes) | **Later** |

### 3.4 Layout (multi-plot) — Favnyr-shaped

| Capability | Recommendation |
| --- | --- |
| Multiple plot tiles | **MVP** |
| Stacked plots, shared X | **MVP** |
| Tabs of plots in one leaf | **Soon** |
| Detach plot window | **Later** |
| Sync zoom across tiles | Via linked axes — **MVP/Soon** |

### 3.5 Performance & large files

| Capability | Recommendation |
| --- | --- |
| Decimate for draw (LTTB / minmax buckets) | **MVP** once files > ~50k–100k points |
| Level-of-detail by zoom | **Soon** |
| Show envelope (minmax) when zoomed out | **Soon** |
| Lazy load Parquet row groups | **Later** |
| Progressive load CSV | **Later** |
| Cap series count with warning | **MVP** polish |

### 3.6 Previews

| Capability | Recommendation |
| --- | --- |
| Column sparklines / mini preview in channel list | **MVP** — **owner must-have previews** |
| Hover/select column → larger peek plot | **MVP or Soon** |
| DnD ghost: preview series on target chart before drop | **Soon** (delight; same data path) |

### 3.7 Export & session

| Capability | Recommendation |
| --- | --- |
| Save / copy **PNG** of plot | **MVP** — **owner must-have save-out** |
| Export plotted or visible-window **CSV** | **MVP** — **owner must-have save-out** |
| Workspace remembers series bindings + axes + layout | **MVP** |
| SVG / PDF publication export | **Later** (plotters bridge candidate) |

---

## 4. Suggested product layers (how to talk about “done”)

### Layer A — first useful product (**includes owner must-haves**)

- Line + scatter; multi-series; multi-file overlay  
- **X = auto-detected time** when present, else index (manual override)  
- **Multi-axes** (at least dual Y + series→axis; linked X across tiles)  
- **Clear legends** (name, color, show/hide)  
- **Mouse-over** crosshair + multi-series readout  
- **Previews** (column sparklines / peek)  
- **Save out:** PNG of plot + CSV of plotted/visible data + workspace file  
- Pan/zoom; column drag onto chart  
- Downsampling so large CSV doesn’t die  

### Layer B — “Lab desktop”

- Log scale, step lines, histogram  
- Δ measure between cursors  
- Autoscale-Y-in-window, sticky scales  
- Area/envelope for dense data  
- Richer DnD ghost preview  

### Layer C — “Power / specialty”

- Expressions, FFT, heatmap/spectrogram  
- Box plots, error bars  
- Brush↔table sync, annotations  
- Detach windows, SVG/PDF export  

---

## 5. egui_plot vs custom Painter (decision rule)

| Situation | Use |
| --- | --- |
| Normal multi-series line/scatter/bars, interactive zoom | **egui_plot** |
| Need linked multi-plot dashboards | **egui_plot** (+ linked axes) |
| >1–2M points at 60 fps, GPU-ish thick lines | **Custom Painter** or mesh path *feeding the same data model* |
| Heatmap / spectrogram | **PlotImage** first; custom mesh if too slow |
| Pie / fancy BI | Don’t — or separate non-goal |

Keep **one series data model** (decimated views + full column handles) so you can swap render backends later without redoing DnD/workspace.

---

## 6. Open choices for you to mark

**Already decided:** multi-axes, clear legends, mouse-over, previews, save-out (PNG + CSV + workspace) = **Layer A**. No pie/3D/BI. **Auto-detect timestamp/time columns as X** (index fallback only).

Still open:

1. **Default chart on drop:** line only, or line+scatter toggle?  
2. **Multi-axes shape:** dual Y on one plot vs stacked plots with independent Y (or both)?  
3. **Preview shape:** sparklines in the column list, hover peek card, or both?  
4. **Measure cursors (Δ):** Layer A or B?  
5. **Histogram:** in first release or right after?  
6. **Heatmap / FFT:** in roadmap at all for v1 year?  
7. **Max series per plot** soft cap (e.g. 16 / 32 / unlimited)?  
8. **Export extras:** clipboard PNG? Parquet out? Excel out?  
9. **Time ticks:** absolute vs relative-to-start vs both by default?

---

## 7. Short verdict

**egui_plot is the right default** for chart-bench: interactive cartesian multi-series is its job, and that matches TopicBench + DAQ CSV workflows.  

Plan Layer A around **line/scatter + auto time-X + multi-axes + clear legends + mouse-over + previews + export (PNG/CSV/workspace) + linked tiles + DnD + downsampling**. Next ring: histogram/log/measure/envelope. Heatmap/FFT optional later. **Pie / 3D / BI = never.**
