#!/usr/bin/env bash
# Sync csv-bench from the Project agent store into the M17 project folder.
set -euo pipefail

DEST="${1:-/home/mike/Documents/AAI-Projects/csv-bench}"
STORE_ROOTS=(
  "$HOME/.local/state/cursor/agent-stores/cursor_agent_stores"
  "/cursor/stores"
)

SRC=""
for root in "${STORE_ROOTS[@]}"; do
  if [[ -d "$root" ]]; then
    found=$(find "$root" -type d -path '*/artifacts/csv-bench' 2>/dev/null | head -1 || true)
    if [[ -n "$found" && -f "$found/Cargo.toml" ]]; then
      SRC="$found"
      break
    fi
  fi
done

if [[ -z "$SRC" ]]; then
  echo "Could not find artifacts/csv-bench in agent stores." >&2
  exit 1
fi

mkdir -p "$DEST"
# Preserve local lockdown planning (PROJECT/GAPS/TODO + docs/) — store artifacts
# are app source/samples only; planning truth lives in this M17 tree.
rsync -a --delete \
  --exclude target \
  --exclude .git \
  --exclude docs \
  --exclude PROJECT.md \
  --exclude GAPS.md \
  --exclude TODO.md \
  "$SRC/" "$DEST/"
echo "Synced $SRC -> $DEST (lockdown docs preserved)"
cd "$DEST"
cargo build
echo "Build OK. Run with: cargo run --release"
