# Data info panel & local analysis

**Goal:** Beside charts, always offer a clear **Info / Quality** view of what’s loaded: size, time span, channels, spacing regularity, missingness, and (where cheap and trustworthy) trends / anomalies / oddities. All **local** — no cloud ML required for v1.

**Related:** [product-workflow-vision.md](product-workflow-vision.md), [plot-capabilities-scope.md](plot-capabilities-scope.md), [csv-header-channel-detection.md](csv-header-channel-detection.md).

---

## 1. Where it lives in the UI

| Surface | Role |
| --- | --- |
| **Info panel** (side or bottom leaf) | Selection-aware: file, channel, or current chart’s series |
| **File row badge** | Tiny health dots (OK / gaps / irregular / errors) |
| **Channel list** | Inline min/max/null% + sparkline (already in preview vision) |
| **Chart mouse-over** | Point values (separate from panel; panel shows aggregate) |

Default: **Info** pane next to Channels; updates when selection changes.

---

## 2. Tier A — always-on summary (MVP / must-have)

Cheap, deterministic, every load / selection.

### Per file (or slot)

| Metric | Notes |
| --- | --- |
| **Records** | Row count (data rows only) |
| **Channels** | Count after header detect |
| **Time range** | Min–max of time/X column if present; else “index 0..N-1” |
| **Duration** | max−min time |
| **Columns / width** | Parsed width `W` |
| **Parse plan** | Header depth, roles, delimiter, confidence |
| **File size / load time** | Ops comfort |
| **Approx memory** | Optional |

### Spacing / sampling (owner ask: consistent vs sporadic)

On the X/time column (or index):

| Metric | How (simple) |
| --- | --- |
| **Median Δx** | Median of consecutive diffs |
| **Δx CV** | std/mean or IQR/median of diffs |
| **Classification** | `regular` / `mostly regular` / `sporadic` / `unknown` from CV + outlier-gap rate |
| **Largest gap** | Max Δx (+ timestamp if any) |
| **Gap count** | Diffs > k × median (e.g. k=3 or 5) |
| **Duplicate timestamps** | Count of Δx == 0 |

Show one plain sentence: *“~1.0 s spacing (regular); 3 gaps > 5 s.”*

### Per channel

| Metric | Notes |
| --- | --- |
| **dtype** | f64 / i64 / bool / string / time |
| **null / NaN / empty %** | Missingness |
| **min / max / mean / median** | Numeric only |
| **std / p05 / p95** | Quick spread |
| **constant?** | min==max (after nulls) |
| **finite count** | Non-null samples |

---

## 3. Tier B — quality flags (“obscure / missing / weird”)

Still classical stats — no neural nets. Run async on sample or full column after load.

| Flag | Idea |
| --- | --- |
| **Missing block** | Runs of nulls longer than threshold |
| **Clipped / flatline** | Long run at min or max or constant |
| **Spike / outlier** | Robust z-score (MAD) or IQR rule on values or on Δy |
| **Step change** | Large sustained level shift |
| **Stringy numeric** | Column mostly numbers but some junk tokens |
| **Unit mismatch hint** | Same name different units across files (preset apply) |
| **Sparse channel** | Very low finite % vs siblings |
| **Monotonic time break** | Time goes backwards |

UI: short list of **issues** with “show on chart” (zoom to range / mark band).

---

## 4. Tier C — trends & light “fuzzy” analysis (Soon / Later)

“Fuzzy” here = **heuristic / rule-based / optional small models locally**, not marketing AI.

| Capability | Approach | Tier |
| --- | --- | --- |
| **Trend** | Linear regression slope + R² on decimated series; label rising/falling/flat | Soon |
| **Smooth vs noisy** | Residual variance after short rolling mean | Soon |
| **Seasonality / period** | Autocorr peak on Δ-regular series (optional) | Later |
| **Change points** | Simple PELT-lite or sliding mean shift | Later |
| **Anomaly score** | Rolling MAD z-score timeline (plottable helper series) | Soon |
| **Cross-channel correlation** | Pearson on overlapping X (for “these two move together”) | Later |
| **Natural-language blurb** | Template sentences from metrics (*not* LLM-required): “Temp rises ~0.2 °C/min; 12 outliers; 1.2% missing.” | Soon |
| **Local LLM summary** | Optional later if user runs Ollama etc. — **not** MVP dependency | Later / optional |

Be honest in UI: **“Heuristic quality checks”**, not “AI certified clean.”

---

## 5. Scope rules

**Do:**
- Local-only computation; cancelable / progressive for big files.
- Use **decimated** views for trend/outlier scan when N is huge; refine on zoom if needed.
- Tie flags back to **chart** (vertical bands, mark points).
- Persist last summary in session/preset sidecar for instant reopen.

**Don’t (MVP):**
- Require network or paid cloud ML.
- Auto-delete/alter data; analysis is read-only.
- Spam false-positive anomaly walls — severity + thresholds user can tune.

---

## 6. Info panel wireframe (content blocks)

```text
┌─ Info ─────────────────────────────────────┐
│ File: pump_run_03.csv          [regular ●] │
│ Records: 125,432   Channels: 18            │
│ Time: 2024-10-01 08:00 → 14:32 (6h 32m)    │
│ Spacing: median 1.000s · CV 0.02 · gaps 3  │
│ Headers: 3 rows (name, node, units) ✓      │
├─ Selected channel: TankA / Temp (°C) ──────┤
│ n=125430  missing=0.00%  min=18.1 max=41.2 │
│ mean=27.4  std=3.1  trend: rising mildly   │
│ Flags: 2 spikes  (view)                    │
├─ Chart series (5) ─────────────────────────┤
│ … per-series one-liners …                  │
└────────────────────────────────────────────┘
```

---

## 7. Acceptance (vision)

1. Open file → Info shows records, channel count, time range, spacing class without opening a chart.
2. Select channel → min/max/missing/spark stats update.
3. Irregular timestamps → “sporadic” + largest gaps listed.
4. Injected null streak + spikes in fixture → flags appear; click zooms chart.
5. Big file → summary appears quickly (sample/decimate), then refines without blocking DnD.

---

## 8. Open choices

1. Default Info target: **file** vs **channel** vs **chart selection**?
2. Anomaly thresholds: fixed defaults vs per-preset?
3. Auto-add “anomaly highlight” series to chart, or flags-only until clicked?
4. Correlation matrix in Tier C — want it in year-one roadmap?
5. Template NL summary on/off by default?
