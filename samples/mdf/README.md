# ASAM MDF samples

Committed fixtures only — no generator scripts in this repo.

| File | Channels | Notes |
|------|----------|--------|
| `vehicle_drive.mf4` | `VehicleSpeed`, `EngineRPM`, `ThrottlePos`, `CoolantTemp` | MF4.1 synthetic drive cycle (~40 s) |
| `power_bus.mf4` | `BusVoltage`, `BusCurrent`, `BusPower` | MF4.1 synthetic DC bus (~60 s) |

Built with [asammdf](https://github.com/danielhrisca/asammdf) as valid **MF4** files.
No redistributable public `.mdf` / `.mf4` with a clear sample license was pulled in —
these small synthetics avoid license ambiguity. Classic MDF3 (`.mdf`) not included;
MF4 is the preferred ASAM format for new ingest work.

Ingest is wired in chart-bench (`open_path` + Open dialog filters).
