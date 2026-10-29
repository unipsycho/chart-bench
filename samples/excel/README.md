# Excel samples

Generated from CSV counterparts via `scripts/csv_to_xlsx.py` (openpyxl).

| File | Sheets |
|------|--------|
| `seattle_weather.xlsx` | single sheet (`seattle_weather`) |
| `weather_combo.xlsx` | `Seattle`, `Vega`, `Temps` |

Opening an `.xlsx` in csv-bench loads **every non-empty sheet** as its own dataset named `file.xlsx :: SheetName`.

```bash
uv run --with openpyxl scripts/csv_to_xlsx.py \
  --sheet Seattle=samples/seattle_weather.csv \
  --sheet Vega=samples/weather_vega.csv \
  --sheet Temps=samples/temps_daily_min.csv \
  -o samples/excel/weather_combo.xlsx
```
