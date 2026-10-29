#!/usr/bin/env bash
# No-cargo smoke test of header/time heuristics.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
rustc "$ROOT/tools/inspect_csv.rs" -o /tmp/csv-bench-inspect
echo "== multiline =="
/tmp/csv-bench-inspect "$ROOT/samples/demo_multiline.csv"
echo "== index =="
/tmp/csv-bench-inspect "$ROOT/samples/demo_index.csv"
echo "== semicolon =="
/tmp/csv-bench-inspect "$ROOT/samples/demo_semicolon.csv"
echo "ALL OK"
