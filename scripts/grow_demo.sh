#!/usr/bin/env bash
# Append rows to a live CSV so Follow mode can be tested.
set -euo pipefail
TARGET="${1:-samples/live_growing.csv}"
mkdir -p "$(dirname "$TARGET")"
if [[ ! -f "$TARGET" ]]; then
  cat > "$TARGET" <<'HDR'
Sensor,Temp,Pressure
Node,TankA,TankA
Units,°C,kPa
HDR
fi
i=0
base=$(date +%s)
while true; do
  ts=$(date -u -d "@$((base + i))" +"%Y-%m-%dT%H:%M:%S" 2>/dev/null \
    || date -u -r "$((base + i))" +"%Y-%m-%dT%H:%M:%S")
  temp=$(awk -v i="$i" 'BEGIN{printf "%.2f", 20 + (i%50)*0.1}')
  press=$(awk -v i="$i" 'BEGIN{printf "%.2f", 100 + (i%20)*0.05}')
  echo "$ts,$temp,$press" >> "$TARGET"
  i=$((i + 1))
  sleep 0.5
done
