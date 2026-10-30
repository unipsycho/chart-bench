#!/usr/bin/env bash
# No-cargo smoke test of header/time heuristics.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
rustc "$ROOT/tools/inspect_csv.rs" -o /tmp/chart-bench-inspect
echo "== multiline =="
/tmp/chart-bench-inspect "$ROOT/samples/demo_multiline.csv"
echo "== index =="
/tmp/chart-bench-inspect "$ROOT/samples/demo_index.csv"
echo "== semicolon =="
/tmp/chart-bench-inspect "$ROOT/samples/demo_semicolon.csv"
echo "ALL OK"
