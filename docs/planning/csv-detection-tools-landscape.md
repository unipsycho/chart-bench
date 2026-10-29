# OSS / GitHub landscape — CSV detection, channels, profiling, viz hints

**Purpose:** Tools that already sniff CSV structure, infer types/channels, profile quality, or suggest how to view/graph data — so csv-bench can **borrow ideas** (and optionally deps) rather than inventing every heuristic.  
**Caveat:** Authored on a cloud agent **without live GitHub access**; URLs and feature claims are from known public projects and should be **spot-checked on M17** against current READMEs/tags.  
**Related:** [csv-header-channel-detection.md](csv-header-channel-detection.md), [data-info-panel-analysis.md](data-info-panel-analysis.md), [plot-capabilities-scope.md](plot-capabilities-scope.md).

**Relevance key:** ★★★ = study closely for csv-bench · ★★ = useful patterns · ★ = adjacent

---

## 1. What we’re looking for

| Capability | csv-bench need |
| --- | --- |
| Dialect sniff (`,`;` tab, quotes) | Robust open |
| Header depth / multi-row headers | Owner must-have (rare in OSS) |
| Type + time column inference | Auto X |
| Column/channel profiling | Info panel |
| Missing / anomaly heuristics | Quality flags |
| Viz recommendation (“plot this as line vs bar”) | Optional later |
| Fast Rust or embeddable logic | Stack fit |

**Honest gap:** Very few open tools handle **multi-line DAQ headers** (name/node/units) well. Most assume **one header row**. That part is likely **custom** for csv-bench; everything else has strong priors.

---

## 2. CSV dialect & schema sniffing

| Project | Stack | What it does | ★ | Notes for us |
| --- | --- | --- | --- | --- |
| **[qsv](https://github.com/jqnatividad/qsv)** | Rust | Fork/evolution of `xsv`: `sniff`, `schema`, `stats`, `frequency`, `tojsonl`, excel, … | ★★★ | Best Rust CLI to study; sniff + stats map to ParsePlan + Info panel. May shell out or learn from algorithms. |
| **[xsv](https://github.com/BurntSushi/xsv)** | Rust | Classic fast CSV toolkit (index, stats, select) | ★★ | Simpler ancestor; less “sniff” than qsv |
| **[CleverCSV](https://github.com/alan-turing-institute/CleverCSV)** | Python | Research-grade **dialect detection** (better than stdlib `csv.Sniffer`) | ★★★ | Port heuristics or call as optional; paper-backed |
| **[DuckDB `read_csv_auto`](https://duckdb.org/docs/data/csv/overview)** | C++ (Rust bindings exist) | Auto header, types, delimiter, sample-based | ★★★ | Excellent auto-CSV; embed via duckdb-rs **or** mirror behavior. Weak on multi-line headers. |
| **[Polars](https://github.com/pola-rs/polars)** | Rust | `Schemainfer`, CSV scan, dtypes | ★★★ | Likely ingest engine anyway; don’t reinvent type infer |
| **[Apache Arrow CSV](https://github.com/apache/arrow)** | C++/Rust | ConvertOptions, infer | ★★ | If on Arrow path |
| **Frictionless [Table Schema](https://github.com/frictionlessdata/frictionless-py)** / specs | Python | Infer + validate schema, dialects | ★★★ | Conceptual model for “ParsePlan as schema”; JSON Table Schema |
| **csvkit** ([wireservice/csvkit](https://github.com/wireservice/csvkit)) | Python | `csvstat`, sniff-ish utilities | ★★ | Good UX inspiration for stats CLI |
| **Python `csv.Sniffer`** | stdlib | Weak dialect sniff | ★ | Baseline to beat (CleverCSV does) |
| **Messytables** (legacy OKF) | Python | Type guessing for messy tables | ★★ | Historical ideas; project aged |
| **[sqlite-utils](https://github.com/simonw/sqlite-utils)** | Python | `insert` CSV with type detect | ★★ | Practical type rules |

---

## 3. Profiling, EDA, quality (Info panel cousins)

| Project | Stack | What it does | ★ | Notes |
| --- | --- | --- | --- | --- |
| **[ydata-profiling](https://github.com/ydataai/ydata-profiling)** (ex pandas-profiling) | Python | Full HTML profile: types, missing, correlations, warnings | ★★★ | Feature **checklist** for Info panel — don’t embed Python; reimplement summaries in Rust |
| **[Sweetviz](https://github.com/fbdesignpro/sweetviz)** | Python | Compare datasets, target analysis | ★★ | Compare two runs / preset slots |
| **[Great Expectations](https://github.com/great-expectations/great_expectations)** | Python | Declarative data tests | ★★ | “Expectations” ≈ quality flags + presets |
| **[Visidata](https://github.com/saulpw/visidata)** | Python TUI | Blazing keyboard CSV explore, type weave | ★★★ | Channel navigation UX inspiration |
| **[csvlens](https://github.com/YS-L/csvlens)** | Rust TUI | Fast CSV viewer | ★★ | Rust viewer patterns |
| **[Tad](https://github.com/antonycourtney/tad)** | Electron | Desktop pivot/filter CSV/Parquet | ★★ | Desktop “open & understand” UX |
| **[Datasette](https://github.com/simonw/datasette)** | Python | Publish SQLite; good explore metaphor | ★ | Less desktop DnD |
| **[D-Tale](https://github.com/man-group/dtale)** | Python | Flask UI on dataframes | ★ | Heavy web |
| **OpenRefine** | Java | Clustering, messy cleanup | ★★ | Header/messy-cell workflows — not graphing |

---

## 4. Auto “how should I chart this?” 

| Project | Stack | What it does | ★ | Notes |
| --- | --- | --- | --- | --- |
| **[Lux](https://github.com/lux-org/lux)** | Python | Intent-based viz recommendations on dataframes | ★★★ | Recommendation **ideas** (time→line, cat→bar); stay egui for render |
| **Voyager / CompassQL** ([vega-voyager](https://github.com/vega/voyager)) | JS | Explore + recommend encodings | ★★★ | Academic UX for “show me interesting charts” |
| **[Draco](https://github.com/uwdata/draco)** | — | Constraints for good Vega-Lite specs | ★★ | If we ever auto-pick chart type |
| **Vega-Lite** | JS | Type-driven encoding defaults | ★★ | Mental model: temporal field → x |
| **Plotly Express** | Python | `px.line` etc. with column roles | ★ | Convenience API shape |
| **RawGraphs** | Web | Chart type picker from tables | ★ | Manual, not sniff |

For csv-bench v1: **don’t need a recommender engine** — owner workflow is DnD channels. Still useful: **if column is temporal → default X**; numeric → series; low-cardinality string → don’t plot as line.

---

## 5. Time-series / engineering graphers (channels & CSV load)

| Project | Stack | What it does | ★ | Notes |
| --- | --- | --- | --- | --- |
| **[PlotJuggler](https://github.com/facontidavide/PlotJuggler)** | C++/Qt | Drag series, layouts, plugins, CSV/UDP/MQTT | ★★★ | Closest **product** cousin for DnD timeseries + layouts; study UX not code license alone (LGPL) |
| **[Foxglove](https://github.com/foxglove)** / Studio | — | Robotics viz; layouts | ★★ | Layout/preset inspiration |
| **[Teleplot](https://github.com/nesnes/teleplot)** | — | Live plots over serial/UDP | ★★ | Live follow mindset |
| **[Serial Studio](https://github.com/Serial-Studio/Serial-Studio)** | C++/Qt | Dashboard from streams + CSV | ★★ | Channel dashboards |
| **LabPlot** ([KDE LabPlot](https://invent.kde.org/education/labplot)) | C++ | Scientific desktop plotting | ★★ | Spreadsheet+plot |
| **SciDAVis / AlphaPlot / QtiPlot** | C++ | Origin-like OSS | ★ | Heavier science UI |
| **[Perspective](https://github.com/finos/perspective)** | C++/Rust/JS | Streaming columnar analytics + charts | ★★★ | Huge data + streaming; different UX |
| **Grafana** CSV plugin | — | Dashboards from CSV | ★ | Preset dashboards metaphor |
| **TopicBench** (your stack) | Rust/egui | Multi-topic graphs, stickies | ★★★ | Direct reuse of plot/interaction DNA |

**Multi-line header:** expect **custom** — PlotJuggler CSV is usually single header; DAQ exporters are the odd ones.

---

## 6. Rust-native pieces to lean on

| Crate / project | Use |
| --- | --- |
| **polars** / **arrow** | Load + dtypes + lazy |
| **qsv** (study or subprocess) | Sniff/stats reference |
| **calamine** | XLS/XLSX |
| **chrono** + **chrono-tz** | Timestamp parse |
| **notify** | Live file follow |
| **egui_plot** | Charts |
| **duckdb** (optional) | `read_csv_auto` via duckdb-rs if embed cost OK |

---

## 7. Recommended takeaways for csv-bench

1. **Dialect + types:** Follow **DuckDB/Polars/qsv/CleverCSV** ideas — sample rows, score delimiters, infer dtypes. Prefer **Polars or DuckDB** in-process over reimplementing sniffers from scratch.  
2. **Multi-line headers (name/node/units):** Treat as **product differentiator**; little OSS does this. Implement per [csv-header-channel-detection.md](csv-header-channel-detection.md).  
3. **Time auto-X:** Combine name cues + parse attempts (chrono); Vega/Lux “temporal → x” rule.  
4. **Info panel:** Steal the **metric list** from ydata-profiling / csvstat / qsv stats — implement in Rust.  
5. **DnD graph UX:** Study **PlotJuggler** (+ TopicBench) for layouts/series overlay; implement in egui.  
6. **Presets:** Grafana/Foxglove “layout” + PlotJuggler perspectives — map to our file-slot presets.  
7. **Live file:** `notify` + append parse; Serial Studio/Teleplot UX for “following.”  
8. **Viz recommendation engines (Lux/Voyager):** Optional later; not blocking if DnD is primary.

---

## 8. Suggested M17 verification pass

On M17 (GitHub works), clone or `gh` summary for:

```text
jqnatividad/qsv
alan-turing-institute/CleverCSV
pola-rs/polars
facontidavide/PlotJuggler
ydataai/ydata-profiling
lux-org/lux
vega/voyager
finos/perspective
YS-L/csvlens
saulpw/visidata
frictionlessdata/frictionless-py
```

Capture: license, last release, whether multi-header exists, any “sniff” CLI flags worth copying.

---

## 9. Quick license watchlist

Before depending on code (vs ideas): check each — **qsv** (MIT/Unlicense lineage), **Polars** (MIT), **CleverCSV** (BSD/MIT family — verify), **PlotJuggler** (LGPL — careful if linking), **ydata-profiling** (MIT), **DuckDB** (MIT). Prefer **reimplement heuristics** when licenses clash with product goals; Favnyr already taught GPL caution.
