#!/usr/bin/env python3
"""Generate a continuous multi-channel DAQ-style Parquet soak file.

Sensor / SCADA style: regular timestamps + numeric analog channels (not trip/event logs).
Writes under samples/large/ (gitignored). Re-run with uv:

  uv run --with numpy --with pyarrow python scripts/gen_daq_soak_parquet.py
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

# 16 named analog channels — plant / DAQ flavored
CHANNEL_NAMES = [
    "accel_x_g",
    "accel_y_g",
    "accel_z_g",
    "temp_c",
    "pressure_kpa",
    "flow_lpm",
    "voltage_v",
    "current_a",
    "rpm",
    "torque_nm",
    "vib_rms_mm_s",
    "level_pct",
    "humidity_pct",
    "strain_ue",
    "ai_spare_1",
    "ai_spare_2",
]


def synth_chunk(t0_ms: int, n: int, dt_ms: int, seed: int) -> dict[str, np.ndarray]:
    """Build one row-group of continuous sensor channels."""
    rng = np.random.default_rng(seed)
    i = np.arange(n, dtype=np.float64)
    t_s = (t0_ms + i * dt_ms) * 1e-3
    # Shared process dynamics + independent sensor noise (keeps size large under SNAPPY)
    base = (
        0.35 * np.sin(2 * np.pi * 0.17 * t_s)
        + 0.22 * np.sin(2 * np.pi * 1.3 * t_s)
        + 0.08 * np.sin(2 * np.pi * 47.0 * t_s)
        + 0.05 * np.sin(2 * np.pi * 0.003 * t_s)
    )
    ramp = 0.00002 * i
    out: dict[str, np.ndarray] = {
        # Epoch millis — written as TIMESTAMP(ms) so Chart-Bench picks time X.
        "timestamp": (t0_ms + (i.astype(np.int64) * dt_ms)).astype("datetime64[ms]"),
    }
    scales = [
        (0.15, 0.02),  # accel_x
        (0.12, 0.02),  # accel_y
        (1.00, 0.03),  # accel_z (1g bias)
        (45.0, 0.08),  # temp
        (101.3, 0.15),  # pressure
        (12.0, 0.25),  # flow
        (24.0, 0.05),  # voltage
        (3.5, 0.08),  # current
        (1800.0, 2.5),  # rpm
        (40.0, 0.4),  # torque
        (2.5, 0.05),  # vib
        (62.0, 0.2),  # level
        (48.0, 0.3),  # humidity
        (120.0, 1.5),  # strain
        (0.0, 0.05),  # spare1
        (0.0, 0.05),  # spare2
    ]
    phase = np.linspace(0.0, 2.5, len(CHANNEL_NAMES))
    for name, (bias, noise), ph in zip(CHANNEL_NAMES, scales, phase):
        sig = bias + ramp * (0.5 + ph) + base * (0.4 + 0.3 * ph) + np.sin(
            2 * np.pi * (0.41 + ph) * t_s + ph
        ) * (0.05 * abs(bias) + 0.02)
        out[name] = (sig + rng.normal(0.0, noise, size=n)).astype(np.float64)
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "samples"
        / "large"
        / "daq_multichannel_1khz.parquet",
    )
    ap.add_argument("--rows", type=int, default=8_000_000, help="Total sample rows")
    ap.add_argument("--dt-ms", type=int, default=1, help="Sample period in milliseconds")
    ap.add_argument("--chunk", type=int, default=250_000, help="Rows per Parquet row group")
    ap.add_argument("--seed", type=int, default=42)
    args = ap.parse_args()

    args.out.parent.mkdir(parents=True, exist_ok=True)
    fields = [pa.field("timestamp", pa.timestamp("ms"))] + [
        pa.field(n, pa.float64()) for n in CHANNEL_NAMES
    ]
    schema = pa.schema(fields)

    writer = pq.ParquetWriter(
        args.out,
        schema,
        compression="SNAPPY",
        use_dictionary=False,
        write_statistics=True,
    )
    written = 0
    t0 = 1_700_000_000_000  # fixed epoch ms start (continuous clock)
    chunk_i = 0
    try:
        while written < args.rows:
            n = min(args.chunk, args.rows - written)
            cols = synth_chunk(t0 + written * args.dt_ms, n, args.dt_ms, args.seed + chunk_i)
            table = pa.Table.from_pydict(cols, schema=schema)
            writer.write_table(table)
            written += n
            chunk_i += 1
            if chunk_i % 4 == 0 or written >= args.rows:
                print(f"  wrote {written:,} / {args.rows:,} rows", flush=True)
    finally:
        writer.close()

    size = args.out.stat().st_size
    print(f"OK {args.out}")
    print(f"rows={written} channels={len(CHANNEL_NAMES)} dt_ms={args.dt_ms}")
    print(f"bytes={size} (~{size / (1024**2):.1f} MiB)")


if __name__ == "__main__":
    main()
