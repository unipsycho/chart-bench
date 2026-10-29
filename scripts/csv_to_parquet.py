#!/usr/bin/env python3
"""Build sample .parquet files from CSVs (pyarrow).

Examples:
  uv run --with pyarrow scripts/csv_to_parquet.py \\
      samples/seattle_weather.csv -o samples/parquet/seattle_weather.parquet

  uv run --with pyarrow scripts/csv_to_parquet.py \\
      samples/tips_restaurant.csv samples/iris.csv \\
      -o samples/parquet/

Compression defaults to SNAPPY (csv-bench reads UNCOMPRESSED + SNAPPY).
Pass --uncompressed for UNCOMPRESSED pages.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path


def _require_pyarrow():
    try:
        import pyarrow  # noqa: F401
        import pyarrow.csv  # noqa: F401
        import pyarrow.parquet  # noqa: F401
    except ImportError as e:
        raise SystemExit(
            "pyarrow required. Try: uv run --with pyarrow scripts/csv_to_parquet.py ..."
        ) from e


def csv_to_parquet(csv_path: Path, out_path: Path, compression: str) -> None:
    import pyarrow.csv as pacsv
    import pyarrow.parquet as pq

    table = pacsv.read_csv(csv_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(table, out_path, compression=compression)


def main(argv: list[str] | None = None) -> int:
    _require_pyarrow()

    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("csv", nargs="+", type=Path, help="CSV path(s) to convert")
    p.add_argument(
        "-o",
        "--output",
        type=Path,
        required=True,
        help="Output .parquet file, or a directory when converting multiple CSVs",
    )
    p.add_argument(
        "--uncompressed",
        action="store_true",
        help="Write UNCOMPRESSED pages (default: SNAPPY)",
    )
    args = p.parse_args(argv)
    compression = "NONE" if args.uncompressed else "SNAPPY"

    csvs: list[Path] = list(args.csv)
    for path in csvs:
        if not path.is_file():
            raise SystemExit(f"missing CSV: {path}")

    out = args.output
    if len(csvs) == 1 and out.suffix.lower() == ".parquet":
        csv_to_parquet(csvs[0], out, compression)
        print(f"wrote {out} ({compression})")
        return 0

    if out.suffix.lower() == ".parquet" and len(csvs) > 1:
        raise SystemExit("with multiple CSVs, -o must be a directory")
    out.mkdir(parents=True, exist_ok=True)
    for csv_path in csvs:
        dest = out / f"{csv_path.stem}.parquet"
        csv_to_parquet(csv_path, dest, compression)
        print(f"wrote {dest} ({compression})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
