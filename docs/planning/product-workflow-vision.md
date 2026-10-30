# Chart-Bench — product workflow vision

**Goal:** Go from **files → channel preview → charts** in seconds, via drag-and-drop. Overlay channels (same file or many files), multi-axes as needed, then **reuse the same layout/views on the next similar dump** with minimal friction. Auto-save everything; make history/revert/preview of past work obvious.

**Related:** [plot-capabilities-scope.md](plot-capabilities-scope.md), [csv-header-channel-detection.md](csv-header-channel-detection.md), [favnyr-csv-feature-map.md](favnyr-csv-feature-map.md).

---

## 1. The happy path (what “simple” means)

```text
1. Drop / open files          → appear in File tray
2. Auto header/channel detect → Channels list with previews (sparklines)
3. Drag channel → chart       → overlays series (or creates chart tile)
4. Drag more (any file)       → same chart or new tile; assign axis if needed
5. Tweak layout               → splits/tabs (Favnyr-like); linked X, legends, mouse-over
6. Walk away                  → auto-saved session
7. Next week, new dump        → open Preset / Project → swap files → same charts
```

If step 7 takes more than ~30 seconds of thinking, the preset model failed.

---

## 2. Core objects (mental model)

Keep the vocabulary small:

| Object | What it is |
| --- | --- |
| **File** | One CSV / Parquet / XLS sheet on disk (with a saved **ParsePlan** for headers/channels) |
| **Channel** | One plottable series from a file (name/node/units + column) |
| **Chart** | One plot leaf: series list, axis assignments, style, view bounds (optional) |
| **Layout** | Tile tree (splits/tabs) + which chart/table/files panes exist |
| **Session** | What’s open *right now* (files + layout + charts + UI state) — auto-saved |
| **Preset** (working name) | A **reusable recipe**: layout + chart bindings described in terms of **slots / channel roles**, not one-off absolute paths only |

Engineers don’t need to see all six words in the UI. They see: **Files**, **Channels**, **Charts**, **Saved**.

---

## 3. Interaction principles

1. **Select fast** — recent files, folder drop, multi-select open; remember last folder.
2. **Preview before commit** — channel list always shows dtype + sparkline (or peek); header detect visible/fixable.
3. **Drag-and-drop first** — channel → chart overlays; channel → empty tile creates chart; series between charts moves/copies (modifier).
4. **Overlay is default** — dropping on an existing chart *adds* a series (multi-file OK).
5. **Multi-axes without ceremony** — drop suggests axis; or “pin to right axis” from legend/context; dual-Y is normal, not advanced.
6. **Auto-save always** — local session continuous; never “did I hit Save?”
7. **History is visible** — timeline or version list: preview old session snapshot, restore, or fork.
8. **Presets are for repetition** — same graphs, new files.

---

## 4. Presets / bookmarks / projects — options

Owner is unsure of the best metaphor. Three shapes that fit the same backend:

### Option A — **Presets** (recommended default)

- Save **Preset** = layout + charts + **file slots** + **channel match rules**.
- Slots: e.g. `Primary log`, `Compare log` (not hard-wired paths).
- On apply: user drops new files into slots (or “Replace all from folder”); channels re-bind by rules:
  - exact name/node/units match, then fuzzy name, then “ask.”
- Best when: same test, many runs.

### Option B — **Bookmark** (lightweight)

- One-click bookmark of current session (paths + layout + bindings).
- “Open bookmark but pick different files…” → same as slot replace wizard.
- Best when: occasional saves, less structure.

### Option C — **Project** (heavier)

- Named project with member files, presets inside, notes.
- Best when: multi-week campaigns; can grow later *on top of* A.

**Recommendation:** Ship **Session (auto) + Preset (slots + channel match)**. Call bookmarks a “quick preset.” Add Project folder later if needed. Avoid forcing “project” ceremony on day one.

### Apply preset flow (intuitive)

```text
Saved → My Pump Overlay
  [Slot: Run A]  (current: pump_2024-10-01.csv)   [Replace…]
  [Slot: Run B]  (current: pump_2024-10-02.csv)   [Replace…]
  Channel binding: 58 matched, 2 missing → [Review]
  [Open]
```

Missing channels: highlight in legend/chart as “unbound” with one-click remap.

---

## 5. Auto-save, revert, preview past work

| Mechanism | Behavior |
| --- | --- |
| **Live session** | Debounced auto-save to app data (`session.toml` or similar) |
| **Restore on launch** | Reopen last session (files + layout) unless user opted out |
| **History** | Periodic snapshots (e.g. on preset save, on file replace, every N minutes, or on idle) |
| **Preview past** | Selecting a history entry shows **read-only thumbnail / layout ghost** or opens as “Preview” without destroying current; **Restore** / **Fork** |
| **Revert** | One step undo stack for layout/series ops + “Restore snapshot…” for bigger jumps |

Keep history **local** and boring (no cloud sync in MVP).

---

## 6. UI chrome (minimal)

Favnyr-inspired tiling, egui implementation:

| Pane | Job |
| --- | --- |
| **Files** | Open/recent; slot badges when a preset is active; parse status |
| **Channels** | Group by file; filter; sparkline preview; drag handle |
| **Charts / layout** | DnD targets; multi-axes; legends; mouse-over readout |
| **Saved** | Presets list; session history |

No deep menus required for the happy path.

---

## 7. Non-goals (for this vision)

- Pie / 3D / BI gallery (already out).
- Realtime MQTT (TopicBench’s job) unless later.
- Heavy “project management” / Gantt / tasks.
- Collaborative cloud editing.

---

## 8. Acceptance (vision-level)

1. Open 2 CSVs with multi-line headers → channels previewed without hand-editing headers (or one fix saved).
2. Drag 3 channels from file A and 2 from file B onto one chart → overlay + dual axis as needed; legend clear; mouse-over works.
3. Save as preset with 2 slots → next day replace both files → charts repopulate with matched channels.
4. Kill app mid-work → relaunch restores session.
5. Open history entry → preview → restore earlier layout.

---

## 9. Open decisions for owner

1. Prefer UI word **Preset**, **Bookmark**, or **Template**?
2. File slots: free-form names vs fixed “File 1 / File 2”?
3. On channel mismatch: always show review dialog, or auto-bind high confidence silently?
4. History retention: last 20 snapshots vs time-based (7 days)?
5. Should “Replace from folder” auto-pair by filename pattern (date suffix)?
