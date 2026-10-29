# Live / growing files — watch & refresh

**Owner ask:** Read a file that is **still being written** and refresh the loaded data as it grows.  
**Related:** [product-workflow-vision.md](product-workflow-vision.md), [csv-header-channel-detection.md](csv-header-channel-detection.md), [data-info-panel-analysis.md](data-info-panel-analysis.md).

---

## Goal

Logger or exporter is appending to a CSV (or similar) on disk. User opens it in csv-bench **while it’s growing** and sees charts / info / channel previews **update** without manual re-open — with clear control so the UI doesn’t thrash.

---

## Modes

| Mode | Behavior |
| --- | --- |
| **Static** (default for finished files) | Load once; optional manual Reload |
| **Follow** (live) | Watch file; ingest new data; refresh dependent views |
| **Paused follow** | Keep watching but don’t apply updates until Resume |

Per-file toggle: **Follow** on the file row / Info panel. Presets can remember “slot X = follow.”

---

## Detecting change

1. **`notify` file watcher** ( Favnyr already uses `notify` in spirit — same idea): mtime / size events.
2. **Debounce** (e.g. 100–300 ms) so bursty writers don’t reload every line.
3. **Poll fallback** (e.g. 1 s) when watcher is flaky (network FS, some Windows locks).
4. Compare **size + mtime + inode/file-id**; if file was **replaced** (truncation or new inode), treat as **full reload** not append.

---

## Ingest strategies

### Append-only CSV (common logger case) — prefer

- Keep parse plan / headers fixed after first successful detect.
- On growth: read **only new bytes** from last file offset; parse new rows; append to columnar store.
- Update: record count, time range, stats (incremental where possible), decimated plot cache for the new tail.
- If new row **width ≠ W** → soft error / pause follow / offer redetect (don’t silently corrupt channels).

### Full reload

- Use when: Parquet (often rewritten), XLSX save-replace, truncation (`size < last_offset`), schema/header change suspected, or user hits **Reload**.
- Preserve chart bindings by channel identity (name/node/units); zoom: policy below.

### Parquet / XLS

- Growing Parquet is uncommon (usually rewrite); default **Reload on change** with debounce.
- XLSX: same — writers typically replace the file; full reload.

---

## Refresh what

| View | On append | On full reload |
| --- | --- | --- |
| Channel list / sparklines | Update tail / recompute preview sample | Rebuild |
| Info panel | Bump records, extend time range, refresh spacing/gaps | Recompute |
| Charts | Append points (or rebuild decimated LOD) | Rebind series; see zoom policy |
| Quality flags | Incremental scan on new window optional | Rescan |

---

## Zoom / follow-tail UX

| Policy | When |
| --- | --- |
| **Stick to live edge** | If user was already scrolled to the latest X (or “Follow tail” armed) → keep auto-panning to new max X |
| **Keep window** | If user zoomed into history → **don’t** yank the view; badge “+N new points” |
| **Pin Y** | Optional stick Y scale while following so autoscale doesn’t jump |

Clear toolbar: **Follow tail ●/○** independent of file Follow if needed.

---

## Writer locks & partial lines

- CSV mid-line append: buffer incomplete trailing line until newline.
- Windows exclusive lock: retry read with backoff; show “waiting for writer…” rather than failing hard.
- Don’t hold long exclusive locks ourselves (open read-share).

---

## Performance

- Debounce + append path for large logs (hours of 1 Hz or kHz).
- Cap follow rate (e.g. UI refresh ≤ 10–30 Hz even if file screams).
- Decimate live series same as static; rebuild LOD periodically or for visible window only.
- Info stats: cheap counters every update; expensive anomaly scan on timer or manual.

---

## Acceptance

1. Open growing CSV with Follow → record count and chart tail advance as rows append.
2. Zoom into the past → new data does not steal the viewport; indicator shows growth.
3. Arm follow-tail → view tracks latest X.
4. Writer truncates / replaces file → full reload; channels still bound if headers match.
5. Toggle Follow off → file can grow on disk without UI updates until Reload / Follow on.

---

## Open choices

1. Follow **on by default** for files that grew once while open, or always opt-in?
2. Max auto-follow file size before warning?
3. Should presets force Follow on specific slots (e.g. “live plant log”)?
