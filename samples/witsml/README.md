# WITSML samples

Committed fixtures only — no generator scripts in this repo.

| File | Contents |
|------|----------|
| `well_log_sample.xml` | Minimal WITSML **1.4.1.1** `wellLogs` / `wellLog` with time-indexed curves |

Mnemonics / channels: `TIME`, `DEPTH`, `ROP`, `WOB`, `SPP`, `FLOWIN` (21 rows of
comma-separated `logData`). Synthetic — not from a real well.

Schema namespace: `http://www.witsml.org/schemas/1series`. Intended for upcoming
WITSML XML ingest / mnemonic → channel mapping. Ingest is **not** wired yet.
