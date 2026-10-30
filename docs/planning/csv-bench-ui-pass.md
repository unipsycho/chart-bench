# Chart-Bench UI + header naming pass (M17)

## Fixes

**OTSG2 / multi-row headers** — Title/operator rows were wrongly treated as Names, so channels showed as `col_N` / unit tokens (`kPa`). Detection now:

- Marks short meta rows as Title
- Marks unit rows (kPa, in.H2O, L/hr, …) as Units
- Uses the wide instrument-tag row as Names (`PT-200 INLET (kPa)`, …)
- Parses LabVIEW-style times (`9:24:34.354 AM 2019-10-30`)
- Stops treating arbitrary floats as timestamps

**Layout / Favourn-inspired chrome**

- Left icon rail (Files / Channels / Info / Table / Saved)
- Single browser pane: **Files stacked above Channels**
- Channel rows are **one line** (name · spark · `+` / `+R`) with a **vertical scroll**
- Framed main canvas + bottom summary (`N files · channels · series · rows`)
- Purple dark theme closer to Favourn’s chrome

**Charts**

- Default **Overlay** layout + **shared Y** (one plot; no forced L/R split)
- Top controls: Overlay / Stacked / Side-by-side, and **Y: shared | multi-axis**
- Multi-axis auto-groups by value range onto left/right
- **Duplicate** chart action (start of detach/tile workflow — full Favourn-style drag-split still TODO)

## Verify

```bash
cd /home/mike/Documents/AAI-Projects/chart-bench
cargo test --lib
cargo run --release
# Open samples/OTSG2-…Copy01.csv or samples/otsg2_header_sample.tsv
```

Binary: `target/release/chart-bench`
