# Parquet samples

Generated from CSV counterparts via `scripts/csv_to_parquet.py` (pyarrow).
Default compression is **SNAPPY**; csv-bench also loads **UNCOMPRESSED**
(`--uncompressed`). Other codecs (gzip / zstd / …) are rejected with a clear error.

| File | Source CSV |
|------|------------|
| `seattle_weather.parquet` | `../seattle_weather.csv` |
| `tips_restaurant.parquet` | `../tips_restaurant.csv` |
| `iris.parquet` | `../iris.csv` |
| `timeseries_abc.parquet` | `../timeseries_abc.csv` |
| `wind_laurel.parquet` | `../wind_laurel.csv` |
| `stocks_apple_2014.parquet` | `../stocks_apple_2014.csv` |

```bash
uv run --with pyarrow scripts/csv_to_parquet.py \
  samples/seattle_weather.csv samples/tips_restaurant.csv samples/iris.csv \
  samples/timeseries_abc.csv samples/wind_laurel.csv samples/stocks_apple_2014.csv \
  -o samples/parquet/
```
