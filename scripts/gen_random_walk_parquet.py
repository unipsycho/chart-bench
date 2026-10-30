#!/usr/bin/env python3
"""Generate a long-span 2-channel random-walk Parquet soak file.

Two independent walks with a slight start offset (not periodic/repeating).
Default: 10 Hz × 365 days (~315.36M rows). Writes under samples/large/
(gitignored). Re-run with uv:

  uv run --with numpy --with pyarrow python scripts/gen_random_walk_parquet.py

If disk/RAM is tight, drop to 1 Hz for the same year span:

  uv run --with numpy --with pyarrow python scripts/gen_random_walk_parquet.py \\
    --hz 1 --out samples/large/random_walk_2ch_365d.parquet
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

CHANNEL_NAMES = ["walk_a", "walk_b"]
SECONDS_PER_DAY = 86_400


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "samples"
        / "large"
        / "random_walk_2ch_365d.parquet",
    )
    # Prefer longest continuous year: 10 Hz × 365 d ≈ 315.36M rows
    ap.add_argument("--days", type=float, default=365.0, help="Wall-time span in days")
    ap.add_argument("--hz", type=float, default=10.0, help="Sample rate in Hz")
    ap.add_argument(
        "--rows",
        type=int,
        default=None,
        help="Total sample rows (overrides --days/--hz when set)",
    )
    ap.add_argument(
        "--dt-ms",
        type=int,
        default=None,
        help="Sample period in milliseconds (overrides --hz when set)",
    )
    ap.add_argument("--chunk", type=int, default=500_000, help="Rows per Parquet row group")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--step-std", type=float, default=0.05, help="Per-sample step σ")
    ap.add_argument("--start-a", type=float, default=0.0)
    ap.add_argument("--start-b", type=float, default=2.5, help="Slight offset vs walk_a")
    args = ap.parse_args()

    # Resolve period + row count from Hz/days unless explicit overrides given
    if args.dt_ms is None:
        if args.hz <= 0:
            raise SystemExit("--hz must be > 0")
        dt_ms = int(round(1000.0 / args.hz))
        if dt_ms < 1:
            raise SystemExit("--hz too high for millisecond timestamps")
    else:
        dt_ms = args.dt_ms
        if dt_ms < 1:
            raise SystemExit("--dt-ms must be >= 1")

    hz = 1000.0 / dt_ms
    if args.rows is None:
        if args.days <= 0:
            raise SystemExit("--days must be > 0")
        rows = int(round(args.days * SECONDS_PER_DAY * hz))
    else:
        rows = args.rows
    if rows < 1:
        raise SystemExit("row count must be >= 1")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    schema = pa.schema(
        [pa.field("timestamp", pa.timestamp("ms"))]
        + [pa.field(n, pa.float64()) for n in CHANNEL_NAMES]
    )

    writer = pq.ParquetWriter(
        args.out,
        schema,
        compression="SNAPPY",
        use_dictionary=False,
        write_statistics=True,
    )
    rng = np.random.default_rng(args.seed)
    # Carry walk state across chunks so the series never resets/repeats
    last_a = float(args.start_a)
    last_b = float(args.start_b)
    written = 0
    t0 = 1_700_000_000_000  # same epoch anchor as DAQ soak
    chunk_i = 0
    print(
        f"Generating {rows:,} rows @ {hz:g} Hz ({dt_ms} ms) → "
        f"~{args.days if args.rows is None else (rows - 1) * dt_ms / 1000.0 / SECONDS_PER_DAY:.2f} days",
        flush=True,
    )
    try:
        while written < rows:
            n = min(args.chunk, rows - written)
            # Independent gaussian steps → cumulative random walks
            steps_a = rng.normal(0.0, args.step_std, size=n)
            steps_b = rng.normal(0.0, args.step_std, size=n)
            walk_a = last_a + np.cumsum(steps_a)
            walk_b = last_b + np.cumsum(steps_b)
            last_a = float(walk_a[-1])
            last_b = float(walk_b[-1])

            i = np.arange(n, dtype=np.int64)
            cols = {
                "timestamp": (t0 + (written + i) * dt_ms).astype("datetime64[ms]"),
                "walk_a": walk_a.astype(np.float64),
                "walk_b": walk_b.astype(np.float64),
            }
            writer.write_table(pa.Table.from_pydict(cols, schema=schema))
            written += n
            chunk_i += 1
            if chunk_i % 4 == 0 or written >= rows:
                print(f"  wrote {written:,} / {rows:,} rows", flush=True)
    finally:
        writer.close()

    size = args.out.stat().st_size
    dur_s = (written - 1) * dt_ms / 1000.0 if written else 0.0
    print(f"OK {args.out}")
    print(f"rows={written} channels={len(CHANNEL_NAMES)} dt_ms={dt_ms} hz={hz:g}")
    print(f"duration_s={dur_s:.1f} (~{dur_s / SECONDS_PER_DAY:.2f} days)")
    print(f"bytes={size} (~{size / (1024**2):.1f} MiB)")


if __name__ == "__main__":
    main()
