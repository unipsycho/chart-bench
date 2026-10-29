# Parquet samples

Generated from CSV counterparts via `scripts/csv_to_parquet.py` (pyarrow).

| File | Source CSV |
|------|------------|
| `seattle_weather.parquet` | `../seattle_weather.csv` |
| `tips_restaurant.parquet` | `../tips_restaurant.csv` |
| `iris.parquet` | `../iris.csv` |

**Ingest status:** opening `.parquet` / `.pq` still hits the CG-33 stub (`src/ingest/parquet_stub.rs`) — clear error only. These files are for smoke tests once Parquet ingest lands.

```bash
uv run --with pyarrow scripts/csv_to_parquet.py \
  samples/seattle_weather.csv \
  -o samples/parquet/seattle_weather.parquet

uv run --with pyarrow scripts/csv_to_parquet.py \
  samples/tips_restaurant.csv samples/iris.csv \
  -o samples/parquet/
```
