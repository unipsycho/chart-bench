# MATLAB `.mat` samples

Committed fixtures only — no generator scripts in this repo (keeps GitHub language
stats Rust). Files are **Level 5** MAT (MATLAB 5) with numeric column vectors suitable
for upcoming time-series ingest.

| File | Variables | Notes |
|------|-----------|--------|
| `plant_process.mat` | `time_s`, `pressure_kPa`, `temp_C`, `flow_m3h` | ~120 s plant-style process (240 samples) |
| `motor_vibration.mat` | `time_s`, `accel_x_g`, `accel_y_g`, `accel_z_g`, `rpm` | ~5 s vibration / motor (500 samples) |

Synthetic data generated with SciPy `savemat` (no MATLAB license required to read).
Ingest is wired in chart-bench (`open_path` + Open dialog filters).
