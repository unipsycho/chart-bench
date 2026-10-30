# Sample CSVs

Small demos and public-domain / open datasets for smoke-testing chart-bench.

- Click **Demo CSV** in the app, or **Open** any file here.
- `otsg2_header_sample.tsv` — multi-line LabVIEW-style header snippet.
- `live_growing.csv` — used with `../scripts/grow_demo.sh` for Follow mode.

- `excel/` — XLSX samples (single + multi-sheet); see `excel/README.md`.
- `parquet/` — Parquet samples (stub ingest until CG-33); see `parquet/README.md`.
- `matlab/` — Level-5 `.mat` time-series fixtures (ingest pending); see `matlab/README.md`.
- `mdf/` — ASAM MF4 fixtures (ingest pending); see `mdf/README.md`.
- `witsml/` — small WITSML 1.4.1.1 wellLog XML (ingest pending); see `witsml/README.md`.
- `sqlite/` — multi-table SQLite with timestamp + numeric channels (ingest pending); see `sqlite/README.md`.

Large soak-test files (100k–500k+ rows) may exist on a local machine but are **gitignored** so the public repo stays lean. See `docs/planning/chart-bench-sample-csvs.md` for provenance notes.
