# SQLite samples

Committed fixtures only — no generator scripts in this repo.

| File | Tables | Notes |
|------|--------|--------|
| `plant_logs.sqlite` | `process_log`, `vibration_log` | Two tables with ISO-8601 `ts` + numeric channels |

**`process_log`** (180 rows): `ts`, `pressure_kPa`, `temp_C`, `flow_m3h`  
**`vibration_log`** (120 rows): `ts`, `accel_x_g`, `accel_y_g`, `accel_z_g`, `rpm`

Mirrors the plant / vibration channel naming used under `samples/matlab/`. Intended
for **table-as-sheet** UX (each table ≈ one sheet/dataset), similar to multi-sheet
Excel. Ingest is wired in chart-bench (`open_path` + Open dialog filters).
