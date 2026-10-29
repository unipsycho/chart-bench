#!/usr/bin/env python3
"""Build sample .xlsx workbooks from CSVs (openpyxl).

Examples:
  uv run --with openpyxl scripts/csv_to_xlsx.py \\
      samples/seattle_weather.csv -o samples/excel/seattle_weather.xlsx

  uv run --with openpyxl scripts/csv_to_xlsx.py \\
      --sheet Seattle=samples/seattle_weather.csv \\
      --sheet Vega=samples/weather_vega.csv \\
      --sheet Temps=samples/temps_daily_min.csv \\
      -o samples/excel/weather_combo.xlsx
"""
from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path


def _require_openpyxl():
    try:
        from openpyxl import Workbook  # noqa: F401
    except ImportError as e:
        raise SystemExit(
            "openpyxl required. Try: uv run --with openpyxl scripts/csv_to_xlsx.py ..."
        ) from e


def add_csv_sheet(wb, sheet_title: str, csv_path: Path, *, replace_active: bool) -> None:
    if replace_active and wb.sheetnames:
        ws = wb.active
        ws.title = sheet_title[:31]
    else:
        ws = wb.create_sheet(title=sheet_title[:31])

    with csv_path.open(newline="", encoding="utf-8") as f:
        reader = csv.reader(f)
        for row in reader:
            ws.append(row)


def main(argv: list[str] | None = None) -> int:
    _require_openpyxl()
    from openpyxl import Workbook

    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "csv",
        nargs="*",
        type=Path,
        help="CSV path(s); sheet title = stem (single-file / multi-file mode)",
    )
    p.add_argument(
        "--sheet",
        action="append",
        default=[],
        metavar="NAME=PATH",
        help="Named sheet (repeatable). Overrides positional CSVs when set.",
    )
    p.add_argument("-o", "--output", type=Path, required=True, help="Output .xlsx path")
    args = p.parse_args(argv)

    sheets: list[tuple[str, Path]] = []
    for spec in args.sheet:
        if "=" not in spec:
            raise SystemExit(f"--sheet expects NAME=PATH, got {spec!r}")
        name, path_s = spec.split("=", 1)
        sheets.append((name.strip(), Path(path_s)))
    if not sheets:
        for csv_path in args.csv:
            sheets.append((csv_path.stem, csv_path))
    if not sheets:
        raise SystemExit("provide CSV path(s) and/or --sheet NAME=PATH")

    for _, path in sheets:
        if not path.is_file():
            raise SystemExit(f"missing CSV: {path}")

    wb = Workbook()
    # Drop the default empty sheet after first real sheet is written.
    first = True
    for title, path in sheets:
        add_csv_sheet(wb, title, path, replace_active=first)
        first = False
    # If somehow still only default blank, remove nothing; otherwise ok.

    args.output.parent.mkdir(parents=True, exist_ok=True)
    wb.save(args.output)
    print(f"wrote {args.output} ({len(sheets)} sheet(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
