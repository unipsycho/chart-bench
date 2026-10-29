# csv-bench sample CSVs

Public demos live flat under `samples/` (see also `samples/README.md`).

## What’s in the set (small / mid demos)

**Sensor / IoT / industrial**

- Multi-device temp/humidity/pressure/motion (`iot_multi_device.csv`)
- Indoor BME-style env: temp, hum, press, gas, lux, CO₂ (`room_env_bme.csv`)
- Predictive-maintenance style pressures + fail flag (`iot_predictive_ibm.csv`)
- Multi-gas air quality, `;` + European decimals (`air_quality_uci_semicolon.csv`)
- Raspberry Pi meteorological log, headerless + headed copies (`raspi_meteo_*`)
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

Soak CSVs (~100k–500k+ rows) may exist on a developer machine under `samples/` but are **gitignored** so the public repo stays lean (Beijing AQ, household power, IoT env long, Uber NYC, OTSG2 full dump, etc.).
