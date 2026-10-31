# Competitive survey — CSV / tabular graphing tools

**Status:** Complete (M17, 2026-10-28). Web + GitHub spot-check.  
**Purpose:** Position **Chart-Bench** before locking MVP scope — strengths of peers, gaps vs our goals, differentiation.  
**Related:** [product-workflow-vision.md](product-workflow-vision.md), [csv-detection-tools-landscape.md](csv-detection-tools-landscape.md), [plot-capabilities-scope.md](plot-capabilities-scope.md), [`PROJECT.md`](../../PROJECT.md).

**Note:** Prefer official hosts. PlotJuggler README warns that **plotjuggler.com** has been impersonated — use [plotjuggler.io](https://www.plotjuggler.io) / [github.com/PlotJuggler/PlotJuggler](https://github.com/PlotJuggler/PlotJuggler).

---

## 1. Evaluation lens (chart-bench goals)

Score each tool against the product must-haves (not “who has the most chart types”):

| Criterion | What “good” means for us |
| --- | --- |
| **DnD channels** | Drag series onto charts; overlay is default |
| **Multi-file** | Overlay channels from many dumps in one session |
| **Dual / multi Y** | Left+right (or more) Y on one plot without ceremony |
| **Favnyr tiling** | Nested splits / tabs / rearrange / detach; layout as first-class |
| **Multi-line headers** | LabVIEW-style name/node/units; unknown header depth |
| **Presets** | Reuse layout + channel roles on *new* files (slots / rebind) |

Also noted when relevant: live/follow files, Parquet/XLS, speed on big series, license, desktop vs notebook/web.

**Legend in gap lines:** ✅ strong · ⚠️ partial · ❌ weak / absent · — N/A by design

---

## 2. Executive matrix

| Tool | DnD | Multi-file | Dual Y | Tiling | Multi-hdr | Presets | Fit to chart-bench |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **PlotJuggler** | ✅ | ✅ | ❌ (by design) | ⚠️ plots/layout | ❌ | ✅ layouts | Closest cousin — steal UX, differ on Y + headers |
| **VisiData** | — (keys) | ⚠️ | ⚠️ | — | ❌ | — | Explore/TUI inspiration, not a workbench |
| **Grafana + CSV** | ❌ | ⚠️ | ✅ | ⚠️ dashboard | ❌ | ✅ dashboards | Preset metaphor only |
| **Excel / LibreOffice** | ❌ | ⚠️ | ✅ | ❌ | ⚠️ | ⚠️ templates | Ubiquitous fallback; slow DAQ UX |
| **DIAdem / LabVIEW viewers** | ⚠️ | ✅ | ✅ | ⚠️ | ⚠️ TDMS-native | ✅ | Commercial DAQ gold standard |
| **Origin / OriginPro** | ❌ | ✅ | ✅ | ❌ | ⚠️ | ✅ batch | Publication science, not dump triage |
| **seaborn / plotly** | — | ✅ | ⚠️ | — | ❌ | — (code) | Analysis notebooks, not desktop DnD |
| **Timeplotters** | — | — | — | — | ❌ | — | CLI log viz; adjacent only |
| **Serial Studio / Teleplot** | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ❌ | ⚠️ | Live telemetry dashboards |
| **LabPlot / SciDAVis / AlphaPlot** | ❌ | ✅ | ✅ | ❌ | ❌ | ⚠️ | Origin-like OSS science desks |
| **Perspective / Tad** | ❌ | ✅ | ⚠️ | — | ❌ | — | Huge tables / pivots, not DAQ overlays |
| **Foxglove Studio** | ⚠️ | ✅ | ⚠️ | ✅ layouts | ❌ | ✅ | Robotics layouts; not CSV-first |
| **TopicBench** | ✅ sticky | topics | ✅ | ⚠️ | — | ⚠️ | Plot/egui DNA to reuse |

---

## 3. Tool dossiers

### 3.1 PlotJuggler (OSS) — primary competitor

| | |
| --- | --- |
| **Stack** | C++ / Qt, OpenGL; plugins (CSV, Parquet, MCAP, ULog, ROS, MQTT, UDP, …) |
| **License** | MPL-2.0 (Qt LGPL deps); closed plugins allowed |
| **Scale** | ~6.3k★ (2026-10); PJ3 classic timeseries; PJ4 multimodal rewrite |
| **Links** | [GitHub](https://github.com/PlotJuggler/PlotJuggler), [plotjuggler.io](https://www.plotjuggler.io) |

**Strengths**

- Best-in-class **drag signals → plot** UX; multi-dataset load; millions of points.
- **Saved layouts** (optionally with data source) — closest preset analog.
- Live streaming + transforms (Lua/Python in PJ4); plugin ecosystem.
- Mental model matches DAQ engineers (channels, time tracker, zoom).

**Gaps vs chart-bench**

| Criterion | Gap |
| --- | --- |
| Dual Y | **Explicitly rejected** ([#562](https://github.com/PlotJuggler/PlotJuggler/issues/562) “not going to happen… ever”; [#651](https://github.com/PlotJuggler/PlotJuggler/issues/651) DIAdem-style multi-Y). Workaround = stacked plots → wasted vertical space / X align pain. |
| Multi-line headers | CSV loader expects normal tabular CSV + time column pick — not LabVIEW multi-row name/node/units. |
| Favnyr tiling | Multiple plot widgets + layout file, not VS Code / Favnyr binary-split tree + dock DnD. |
| CSV-first DAQ dumps | Optimized for robotics/ROS/MCAP; CSV is one plugin among many. |
| Desktop chrome | Qt robotics tool — not Favnyr-like file-manager chrome. |

**Steal:** DnD overlay, layout-without-source for “same view, new file,” transform editor ideas, streaming follow mindset.  
**Differentiate:** dual/multi Y on one tile, multi-line header ParsePlan, Favnyr tiling + channel dock, CSV/XLS dump workflow first.

---

### 3.2 VisiData (OSS)

| | |
| --- | --- |
| **Stack** | Python TUI |
| **License** | GPL-3.0 |
| **Scale** | ~9.4k★ |
| **Links** | [visidata.org](https://www.visidata.org), [graph docs](https://www.visidata.org/docs/graph/) |

**Strengths:** Instant CSV explore; type weave (`#` `%` `@`); scatter/line via `.`; keyboard density unmatched.  
**Gaps:** Terminal Braille plots ≠ DAQ workbench; no DnD tiles, multi-file overlay session, dual-Y product UX, presets, multi-line DAQ headers. GPL if considering code reuse (ideas only).  
**Steal:** Channel list keyboard nav, type/date inference cues, “open → understand in seconds.”

---

### 3.3 Grafana + CSV / Infinity datasource

| | |
| --- | --- |
| **Stack** | Web dashboard; Infinity / CSV plugins |
| **License** | AGPL (Grafana) + plugin terms |
| **Links** | [Infinity CSV docs](https://grafana.com/docs/plugins/yesoreyeram-infinity-datasource/latest/data-formats/csv/) |

**Strengths:** Dual Y and rich panels; **dashboards = strong preset** story; time-series viz mature; team share.  
**Gaps:** Not a local dump opener — needs datasource plumbing; CSV time-field friction common; no Favnyr tiling or channel DnD; multi-line headers not a thing; overkill for offline LabVIEW CSVs.  
**Steal:** Named dashboard / variable rebinding as preset metaphor; panel legend + hover readout polish.

---

### 3.4 Excel / LibreOffice Calc

| | |
| --- | --- |
| **Stack** | Spreadsheet + chart objects |
| **License** | Commercial / MPL-ish LibreOffice |

**Strengths:** Everyone has it; secondary Y (LibreOffice: series → Options → secondary Y); multi-series XY; templates/workbooks as weak presets; can link external CSV.  
**Gaps:** No channel DnD workbench; multi-file overlay is copy/paste hell; charts are sheet objects not tiled sessions; multi-row headers need manual skip/reshape; large dumps crawl; no live follow; Favnyr tiling absent.  
**Steal:** Secondary-axis as “normal,” not power-user; people already expect dual Y for mixed units.

---

### 3.5 NI DIAdem / LabVIEW TDMS viewers (commercial DAQ)

| | |
| --- | --- |
| **Stack** | NI proprietary; TDMS/TDM native; CSV secondary |
| **License** | Commercial |

**Strengths:** Multi-Y / channel-oriented viewing; groups/channels match instrument mental model; reporting + batch; large measurement datasets; DIAdem is the **reference UX** engineers cite when asking PlotJuggler for multi-Y. LabVIEW built-in TDMS viewer for quick look.  
**Gaps:** Cost / lock-in; TDMS-first (CSV dumps from non-NI stacks underserved); Windows-centric culture; not Favnyr tiling; multi-line *CSV* headers still awkward vs TDMS metadata; not a lightweight Rust cross-platform tool.  
**Steal:** Channel browser + multi-Y overlay as first-class; “VIEW → REPORT” separation; group/channel naming.  
**Differentiate:** Treat **CSV/XLS dumps with ugly headers** as first-class citizens; Linux-friendly; lightweight.

---

### 3.6 Origin / OriginPro (commercial science)

| | |
| --- | --- |
| **Stack** | Windows science desk; CSV connectors / batch |
| **License** | Commercial |

**Strengths:** Deep analysis (fit, peak, stats); publication graphs; batch ops; multi-axis; projects.  
**Gaps:** Heavyweight / paid; not DnD channel triage; 3D/gallery charts we explicitly **avoid**; multi-line DAQ CSV not the hero path; tiling ≠ Favnyr.  
**Steal:** Batch “same graph, new book” ≈ our presets; analysis depth as *future* optional, not MVP.  
**Differentiate:** Speed-to-first-overlay measured in seconds, not worksheet setup.

---

### 3.7 seaborn / matplotlib / Plotly (Python ecosystem)

| | |
| --- | --- |
| **Stack** | Notebook / script; Plotly ~18.9k★ MIT; seaborn ~14k★ BSD |
| **Links** | plotly.py, mwaskom/seaborn |

**Strengths:** Dual axis doable; multi-file trivial in code; Plotly interactive hover/zoom; seaborn statistical defaults.  
**Gaps:** **No desktop DnD product**; presets = scripts/notebooks; multi-line headers DIY; tiling = browser tabs; not an engineer “drop dump → chart” app.  
**Steal:** Encoding defaults (temporal → X); hover readout quality; don’t fight code for *analysis*, own the *interactive dump* niche.

---

### 3.8 Timeplotters (`timeplot` / `splot`)

| | |
| --- | --- |
| **Stack** | Haskell CLI → image; [jkff/timeplot](https://github.com/jkff/timeplot) (~80★, quiet) |
| **Links** | [jkff.info/software/timeplotters](http://jkff.info/software/timeplotters/) |

**Strengths:** One-liner log → plot; event-duration / activity visualizations Excel struggles with; “curiosity in one minute” philosophy.  
**Gaps:** Non-interactive images; custom log input format (not raw multi-column CSV workbench); no DnD, tiling, dual Y product, presets, multi-line headers.  
**Steal:** Instant gratification ethos; duration/gap visualizations for info-panel quality flags later.

---

### 3.9 Serial Studio, Teleplot, related live dashboards

| Tool | Notes |
| --- | --- |
| **Serial Studio** | ~7.3k★; UART/BLE/MQTT/CAN dashboards; CSV export-ish; open-source telemetry |
| **Teleplot** | ~225★ MIT; browser plots from serial/UDP — “ridiculously simple” |

**Strengths:** Live follow; channel dashboards; low ceremony for streams.  
**Gaps:** File-dump / multi-file compare / multi-line headers / Favnyr presets secondary; dual Y and tiling vary.  
**Steal:** Follow-mode UX; sparkline / live strip mindset.

---

### 3.10 LabPlot, SciDAVis, AlphaPlot, QtiPlot (OSS “Origin-like”)

| | |
| --- | --- |
| **LabPlot** | KDE C++; scientific desktop ([invent.kde.org/education/labplot](https://invent.kde.org/education/labplot)) |

**Strengths:** Spreadsheet + plot; multi-axis; free; analysis tools.  
**Gaps:** Worksheet-centric, not channel DnD; weak multi-file dump overlay; no Favnyr tiling; multi-line DAQ CSV custom; UI heavier than “open dump and go.”  
**Steal:** Free science-desk feature checklist; ignore as primary UX model.

---

### 3.11 Perspective, Tad (tabular analytics)

| Tool | Notes |
| --- | --- |
| **Perspective** | ~11k★ Apache-2.0; streaming columnar + charts |
| **Tad** | ~3.5k★ MIT Electron; open CSV/Parquet, pivot/filter |

**Strengths:** Huge tables, Parquet, streaming analytics (Perspective); frictionless open (Tad).  
**Gaps:** BI/pivot orientation — not DAQ multi-Y overlay + tiled charts; multi-line headers; Favnyr presets.  
**Steal:** Fast Parquet open; “feel instant on big files.”

---

### 3.12 Foxglove Studio (robotics)

**Strengths:** Saved layouts; multi-panel; MCAP/ROS; preset-like layout sharing.  
**Gaps:** Robotics message viz first; CSV dumps / LabVIEW headers / dual-Y DAQ not the product center.  
**Steal:** Layout JSON + “open layout on new bag” → our file-slot presets.

---

### 3.13 TopicBench (internal adjacency)

**Strengths:** egui/eframe/egui_plot DNA; multi-topic graphs; stickies; dual-axis familiarity; keyboard-first.  
**Gaps:** Topic/MQTT product — not CSV ParsePlan / multi-file dump presets.  
**Steal:** Plot interaction, chrome density, packaging lessons — keep chart-bench a **sibling**, not a TopicBench clone with CSV.

---

## 4. Cross-cutting findings

1. **Nobody owns “LabVIEW-ish multi-line CSV → channel DnD → dual-Y tiles → rebind presets” as one product.** Pieces exist across PlotJuggler, DIAdem, Grafana, Excel.  
2. **PlotJuggler is the UX north star for DnD timeseries** and also the **anti-pattern for dual Y** (wontfix). Dual Y is a clean wedge vs the OSS leader.  
3. **Presets** are everywhere under other names (layouts, dashboards, Origin batch, Foxglove layouts) — our **file slots + channel match rules** should be clearer than “reload layout hoping names match.”  
4. **Multi-line headers** remain an OSS desert (see also [csv-detection-tools-landscape.md](csv-detection-tools-landscape.md)) — keep as primary differentiator.  
5. **Favnyr tiling** is rare in graphers (file managers / IDEs have it; science desks don’t). Dock + binary split is brandable.  
6. **Don’t compete on 3D/BI galleries** (Origin/Plotly Express) — owner already ruled those out.  
7. **Licenses:** study PlotJuggler UX, don’t link LGPL Qt; don’t copy VisiData/Favnyr GPL code — reimplement.

---

## 5. Recommendations for chart-bench differentiation

### Own these wedges (marketing + MVP priority)

1. **Dual / multi Y on one Favnyr tile** — PlotJuggler won’t; DIAdem charges; Excel can but isn’t a dump workbench. Make L/R axis assignment as easy as ←/→ (already shipping direction).  
2. **Multi-line CSV channel detection** (name/node/units, unknown depth) with visible ParsePlan overrides.  
3. **Files → Channels (sparklines) → DnD overlay** in &lt;30s, including **multi-file** compare.  
4. **Presets with slots + rebind** (“same graphs, new dumps”) — better than layout-only tools when channel sets drift.  
5. **Favnyr-style tiling** (nested split / rearrange / pop-out) as the shell, not an afterthought.

### Explicitly do *not* chase (for MVP)

- ROS/MCAP/3D multimodal (PlotJuggler 4 / Foxglove territory).  
- Full DIAdem reporting / Origin publication suites.  
- Notebook/script API surface (point users to Polars/Plotly for that).  
- Pie / 3D / BI chart menus.

### Borrow without cloning

| From | Borrow |
| --- | --- |
| PlotJuggler | DnD overlay, layout-sans-data-source flow, transform/live follow ideas |
| DIAdem | Multi-Y + channel browser expectations |
| Grafana / Foxglove | Named reusable views; clear “apply to new data” |
| VisiData / qsv | Keyboard channel nav; sniff/stats for Info panel |
| TopicBench | egui_plot interaction DNA |
| Timeplotters | “One minute to insight” bar for quality/gap views later |

### Competitive one-liner

> **chart-bench is the desktop dump workbench PlotJuggler refuses to be on dual-Y, DIAdem won’t be on ugly CSV headers, and Excel can’t be on tiled multi-file channel DnD — with presets that rebind next week’s run.**

### Suggested MVP lock (post-survey)

Keep investing in: dual-Y overlay polish, multi-line header ParsePlan, Favnyr binary split tree (still open in `GAPS.md`), preset slots + mismatch UX, Parquet beyond stub, live Follow. Treat PlotJuggler as the UX benchmark for DnD speed; treat DIAdem screenshots as the dual-Y acceptance test; treat LabVIEW multi-row CSVs as the ingest acceptance test.

---

## 6. Sources (spot-checked 2026-10-28)

- PlotJuggler README / FAQ / issues #562 #651; GitHub API metadata  
- VisiData graph docs; Grafana Infinity CSV docs  
- LibreOffice Calc chart guide (secondary Y)  
- OriginLab product pages; NI DIAdem / TestFlow DIAdem-alternatives context  
- Timeplotters manual (jkff.info); Teleplot / Serial Studio / Perspective / Tad GitHub  
- Local planning: product-workflow-vision, csv-detection-tools-landscape, PROJECT.md  

Scrapes / search JSON under `.firecrawl/competitive-survey/` on M17 (research cache; not product docs).
