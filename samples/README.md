# Sample files

Small demos and public-domain / open datasets for smoke-testing Chart-Bench.

- Click **Demo CSV** in the app, or **Open** / drop any file here.
- `otsg2_header_sample.tsv` — multi-line LabVIEW-style header snippet.
- `live_growing.csv` — used with `../scripts/grow_demo.sh` for Follow mode.

## Format folders

- `excel/` — XLSX samples (single + multi-sheet); see `excel/README.md`.
- `parquet/` — Parquet samples (UNCOMPRESSED + SNAPPY); see `parquet/README.md`.
- `matlab/` — Level-5 `.mat` time-series fixtures; see `matlab/README.md`.
- `mdf/` — ASAM MF4 fixtures; see `mdf/README.md`.
- `witsml/` — small WITSML 1.4.1.1 wellLog XML; see `witsml/README.md`.
- `sqlite/` — multi-table SQLite with timestamp + numeric channels; see `sqlite/README.md`.

## What’s in the flat CSV set

**Sensor / IoT / industrial**

- Multi-device temp/humidity/pressure/motion (`iot_multi_device.csv`)
- Indoor BME-style env (`room_env_bme.csv`)
- Predictive-maintenance style pressures + fail flag (`iot_predictive_ibm.csv`)
- Multi-gas air quality, `;` + European decimals (`air_quality_uci_semicolon.csv`)
- Raspberry Pi meteorological log, headerless + headed (`raspi_meteo_*`)
- LabVIEW-style multi-line header snippet (`otsg2_header_sample.tsv`)

**Weather / climate / atmosphere**

- Seattle daily, multi-city Vega weather, CORGIS station weather
- Daily min temps, wind (Laurel NE), sunspots
- Mauna Loa CO₂ (NOAA commented + cleaned)
- Global monthly temperature anomalies

**Other shapes for ingest stress**

- Earthquakes (geo), airport traffic, ag exports (wide)
- Stocks OHLC / simple series, Gapminder panel, iris, tips, wine chemistry
- Tiny synthetic multi-series (`timeseries_abc.csv`)
- Growing-file Follow demo (`live_growing.csv`)

## Large stress set (local only)

Soak CSVs (~100k–500k+ rows) may exist on a developer machine under `samples/` / `samples/large/` but are **gitignored** so the public repo stays lean (Beijing AQ, household power, IoT env long, Uber NYC, OTSG2 full dump, etc.).
