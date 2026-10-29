# Flexible CSV headers & smart channel detection

**Status:** Owner requirement — Layer A (ingest), not optional polish.  
**Related:** [project-context.md](project-context.md), [plot-capabilities-scope.md](plot-capabilities-scope.md).

---

## Problem

Real DAQ / export CSVs often look like this (row count and meaning vary by source):

```text
Sensor,Temp,Temp,Pressure,Flow
Node,TankA,TankB,TankA,Line1
Units,°C,°C,kPa,L/min
2024-01-01 00:00:00,21.1,22.0,101.3,3.2
2024-01-01 00:00:01,21.2,22.1,101.2,3.1
...
```

Or fewer / more header lines; sometimes a title row; sometimes blank rows; sometimes the first “header” is only names and units are missing. **We cannot assume a fixed header-row count.**

---

## Goals

1. **Detect how many leading rows are headers** (0..N) without hard-coding N.
2. **Interpret what each header row means** when possible (names, nodes/sources, units, …) — with user override.
3. **Build channels** from the stacked header cells (one channel per data column).
4. **Find where data starts** by analyzing row shape: when the table settles into repeating “data” rows with a stable column layout.
5. Always allow **manual correction** (header depth, row roles, skip rows, delimiter).

---

## Detection strategy (proposed)

Pipeline runs on a **prefix sample** (e.g. first 200–1000 lines + a short mid-file probe), then commits a parse plan.

### Step A — Tokenize rows

- Detect delimiter (`,`, `;`, `\t`, `|`) and quote rules.
- Parse each sampled line into cells; record **column count** per row.
- Note blank rows, comment prefixes (`#`, `//`) if present.

### Step B — Find the data region (where “columns start repeating”)

Owner cue: *find where the number of columns starts repeating itself as the actual data columns.*

Operationalize as:

1. **Stable width:** longest run of consecutive rows with the **same column count** `W` (and `W ≥ 2`).
2. Prefer runs that look like **data** (see Step C), not like short title rows.
3. The start of that dominant run is the **candidate first data row** `D`.
4. Rows `[0 .. D)` are the **header block** (minus blanks/comments handled as skip/noise).

Edge cases:

- Header rows may have **fewer** cells than data (trailing empties) — normalize by padding to `W` or treat short rows as titles above the real header grid.
- Some files have a **ragged** first header then align from row 2 — still use the stable-`W` run as truth for channel count.
- If multiple stable runs exist (e.g. metadata table then data), prefer the **longest** run and/or the run with highest “data-likeness.”

### Step C — Data-likeness score (per row)

For each row in the stable-width run, score:

| Signal | Data-like | Header-like |
| --- | --- | --- |
| Parse as number / bool / ISO date | High | Low |
| Mostly alphabetic tokens | Low | High |
| Repeated vocabulary across columns (“Units”, “Node”, …) | Low | High (role cue) |
| Empty cells ratio | Medium | Medium |
| Same string repeated in many columns | Low | Medium |

**First data row** ≈ first row in the stable-width region whose data-likeness exceeds a threshold **and** subsequent rows stay data-like. Header block = everything above that.

### Step D — Header row role detection

For each row in the header block, classify **role** (confidence + allow override):

| Role | Heuristics (examples) |
| --- | --- |
| **names** | Title-case / mixed identifiers; unique-ish; often last header row above data *or* first row |
| **nodes** / **sources** / **locations** | Row label in col0 (“Node”, “Source”, “Device”); values look like tags/IDs; lower uniqueness than names |
| **units** | Short tokens (`°C`, `kPa`, `psi`, `%`, `V`, `A`, `m/s`); row label “Units” / “Unit” |
| **dtype** / **format** | `float`, `int`, `string`, `bool` |
| **title** / **meta** | Single cell or 1–2 cells spanning; doesn’t match `W` |
| **unknown** | Keep raw; user assigns |

Col0 may be a **row label** (Sensor/Node/Units) rather than a channel — detect and strip from channel columns.

### Step E — Build channels

For each data column `j = 1..W` (or `0..W-1` if no label column):

- Collect cells from each header role → structured channel metadata:

```text
Channel {
  id,                 // stable internal id
  name,               // from names row (required-ish)
  node,               // optional
  units,              // optional
  display_name,       // e.g. "TankA / Temp (°C)" — configurable pattern
  source_file,
  column_index,
}
```

- **Display name pattern** (user preference): `{node}/{name} ({units})` with omissions when empty.
- Dedupe collisions: append ` (col j)` or node disambiguation.

### Step F — X / time column (**auto-detect; owner requirement**)

**Policy:** Scan for a timestamp / time column and **use it as X by default**. Most engineering files are time-based. Only if none is found → **point / row index** as X. User can override.

**Scan order (pick highest confidence):**

1. **Name cues** (header/channel name, case-insensitive): `time`, `timestamp`, `datetime`, `date`, `date_time`, `date time`, `ts`, `t`, `epoch`, `unix`, `gps_time`, `systime`, `sample_time`, …  
2. **Units cues**: `s`, `ms`, `us`, `ns`, `sec`, ISO-date-ish unit cells.  
3. **Value shape** on a sample of data rows (even without a great name):
   - Parse as **datetime** strings (ISO-8601, `YYYY-MM-DD HH:MM:SS[.fff]`, with/without `T`, timezone optional).
   - **Epoch numbers**: plausible Unix sec / ms / us ranges; mostly monotonic increasing.
   - Excel-style serial dates if from XLS path.
4. Prefer columns that are **mostly monotonic non-decreasing** with few backward steps.
5. If several candidates: prefer best name+parse score; if tied, leftmost.

**Result stored on file / ParsePlan:**

```text
XAxis {
  mode: Time | Index | Column,  // Index only when no time detected
  column_index: Option<usize>,
  time_parse: Iso | EpochSec | EpochMs | EpochUs | Custom(fmt),
  confidence: Low | Medium | High,
}
```

- Time axis → format ticks as absolute and/or relative-to-start (UI toggle).  
- Index axis → integer or float sample index; Info panel says “no timestamp detected.”  
- Persist detection + override in workspace/preset so Follow/reload doesn’t flip X.

---

## User overrides (required UX)

Auto-detect will be wrong sometimes — UI must expose:

1. **Header row count** (spinner) + live preview of parsed table.
2. **Per header row: role dropdown** (names / nodes / units / dtype / skip / title).
3. **Data start row** override.
4. **Delimiter / encoding** override.
5. **Channel rename** and units edit after detect.
6. **X axis column** picker (Time auto / Index / specific channel) + time format if needed.
7. **Re-detect** button after tweaks.

Preview panel should show: raw first N lines → interpreted header grid → first data rows → resulting channel list (ties to plot **previews** / sparklines).

---

## Other formats

| Format | Notes |
| --- | --- |
| **CSV / TSV** | Full multi-line header logic above |
| **XLS / XLSX** | Same idea on the sheet grid; “header rows” = leading sheet rows; often clearer blanks |
| **Parquet** | Schema already has names (+ sometimes metadata); multi-line headers N/A — map Arrow field names/metadata to Channel; optional sidecar CSV for rich headers |

---

## Implementation sketch (Rust)

- Ingest crate: sample → `ParsePlan { delimiter, header_rows, roles[], data_start, width }`.
- Persist `ParsePlan` in workspace so reopen doesn’t flip detection.
- Parsers: `csv` crate or Polars/Arrow with **pre-skip** + custom header composition (don’t rely on “first row only” APIs blindly).
- Soft-fail: if confidence low, open **Header assistant** modal instead of silent wrong parse.

---

## Acceptance examples

1. 3-line header (name/node/units) + numeric data → 3 header rows, roles correct, channels combine all three.
2. 1-line header only → depth 1, role names, data from row 1.
3. Title row + blank + 2 header rows + data → title skipped, depth detected, data start correct.
4. Header width &lt; data width (missing trailing commas) → still recovers `W` from data run.
5. User forces “2 header rows” when auto said 3 → respected and saved in workspace.
6. Column `Timestamp` with ISO values → X = time automatically; charts/Info use time range.
7. Epoch-ms column named `ts` → detected as time; ticks formatted sensibly.
8. No time-like column → X = index; UI indicates index mode.
9. Wrong auto pick → user sets X column; saved for that file/preset.

---

## Open choices

1. Default **display_name** pattern?
2. Treat col0 row labels as mandatory convention or optional?
3. Max auto header depth (e.g. cap at 10) before asking the user?
4. Should “Units” synonyms be locale-aware later, or English-first?
5. Default time tick display: absolute vs relative-to-start vs both?
