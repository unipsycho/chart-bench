# Parquet samples

Committed fixtures only — no generator scripts in this repo (keeps GitHub language
stats Rust). Files were built from the CSV counterparts with **SNAPPY** compression
(chart-bench also loads **UNCOMPRESSED**). Other codecs (gzip / zstd / …) are rejected
with a clear error.

| File | Source CSV |
|------|------------|
| `seattle_weather.parquet` | `../seattle_weather.csv` |
| `tips_restaurant.parquet` | `../tips_restaurant.csv` |
| `iris.parquet` | `../iris.csv` |
| `timeseries_abc.parquet` | `../timeseries_abc.csv` |
| `wind_laurel.parquet` | `../wind_laurel.csv` |
| `stocks_apple_2014.parquet` | `../stocks_apple_2014.csv` |

To rebuild locally (optional, not shipped): with [pyarrow](https://arrow.apache.org/docs/python/)
installed, read each CSV and write Parquet with `compression="snappy"` (or
`"none"` for UNCOMPRESSED) into this folder, matching the table names above.
