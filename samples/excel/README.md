# Excel samples

Committed fixtures only — no generator scripts in this repo (keeps GitHub language
stats Rust). Files were built from CSV counterparts with openpyxl-style workbooks.

| File | Sheets |
|------|--------|
| `seattle_weather.xlsx` | single sheet (`seattle_weather`) |
| `weather_combo.xlsx` | `Seattle`, `Vega`, `Temps` |

Opening an `.xlsx` in csv-bench loads **every non-empty sheet** as its own dataset named `file.xlsx :: SheetName`.

To rebuild locally (optional, not shipped): create workbooks from the CSVs under
`samples/` — `seattle_weather.csv` → `seattle_weather.xlsx`; and a multi-sheet
`weather_combo.xlsx` with `Seattle` ← `seattle_weather.csv`, `Vega` ←
`weather_vega.csv`, `Temps` ← `temps_daily_min.csv`.
