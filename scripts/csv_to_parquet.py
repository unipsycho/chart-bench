#!/usr/bin/env python3
"""Build sample .parquet files from CSVs (pyarrow).

Examples:
  uv run --with pyarrow scripts/csv_to_parquet.py \\
      samples/seattle_weather.csv -o samples/parquet/seattle_weather.parquet

  uv run --with pyarrow scripts/csv_to_parquet.py \\
      samples/tips_restaurant.csv samples/iris.csv \\
      -o samples/parquet/
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


def csv_to_parquet(csv_path: Path, out_path: Path) -> None:
    import pyarrow.csv as pacsv
    import pyarrow.parquet as pq

    table = pacsv.read_csv(csv_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(table, out_path)


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
    args = p.parse_args(argv)

    csvs: list[Path] = list(args.csv)
    for path in csvs:
        if not path.is_file():
            raise SystemExit(f"missing CSV: {path}")

    out = args.output
    if len(csvs) == 1 and out.suffix.lower() == ".parquet":
        csv_to_parquet(csvs[0], out)
        print(f"wrote {out}")
        return 0

    # Directory mode (multi CSV or -o without .parquet)
    if out.suffix.lower() == ".parquet" and len(csvs) > 1:
        raise SystemExit("with multiple CSVs, -o must be a directory")
    out.mkdir(parents=True, exist_ok=True)
    for csv_path in csvs:
        dest = out / f"{csv_path.stem}.parquet"
        csv_to_parquet(csv_path, dest)
        print(f"wrote {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
