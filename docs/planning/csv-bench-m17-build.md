# csv-bench M17 build & smoke

**Host:** M17 (Omarchy / Hyprland, Wayland)  
**Date:** 2026-10-27  
**Result:** **PASS** — release binary built, GUI launched, smoke script OK.

## Paths

| Item | Path |
|------|------|
| Project store | `~/.local/state/cursor/agent-stores/cursor_agent_stores/bc-cdbe497d-a963-456d-ac13-59411966bbd7/files/` |
| Store artifact source | `…/files/artifacts/csv-bench/` |
| Store tarball | `…/files/media/csv-bench-src.tar.gz` |
| Working tree | `/home/mike/Documents/AAI-Projects/csv-bench` |
| Release binary | `/home/mike/Documents/AAI-Projects/csv-bench/target/release/csv-bench` (~12M) |
| UI screenshot | `…/files/media/csv-bench-m17-ui.png` (1261×1390) |

Synced from store `artifacts/csv-bench` → working tree (local folder was empty/stale).  
Removed `.cargo/config.toml` (cloud gitlab-index workaround) — not present on disk after sync cleanup.

## Toolchain / display

- `cargo 1.99.0` / `rustc 1.99.0`
- `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-1`, session Wayland
- crates.io downloads worked on M17

## Build notes (M17-only patches vs store artifact)

Store pins were for the stale gitlab crates index (egui **0.25**). Code comments already target egui **0.29** DnD APIs. To compile against live crates.io + rustc 1.99:

1. **Bump:** `eframe`/`egui`/`egui_plot`/`egui_extras` `0.25.0` → `0.29.0`
2. **Bump:** `toml` `0.8.10` → `0.8.23` (toml_edit serialize trait breakage)
3. **Bump:** `thiserror` `1.0.57` → `1.0.69` (naga/wgpu via egui-wgpu)
4. **Imports:** use `crate::model::{…}` re-exports (private submodule paths fail on current rustc)
5. **stats:** annotate `largest_gap: f64` (ambiguous float)
6. **main:** `Ok(Box::<CsvBenchApp>::default())` for eframe 0.29 create callback; `use eframe::egui`

Remaining: one deprecation warning (`ComboBox::from_id_source` → `id_salt`).

`cargo build --release` finished successfully after those changes.

## Smoke

- `scripts/smoke_inspect.sh` — **ALL OK** (multiline / index / semicolon samples)
- `cargo run --release` / binary launch — process stayed up, Hyprland client **title=`csv-bench`**, empty stderr log
- Screenshot captured with `grim` of the window region → `media/csv-bench-m17-ui.png` (empty chart UI: Open / Demo CSV / Files·Channels·Info)

## App ran?

**Yes.** GUI started on M17 Wayland; no launch errors. No PR opened (local working tree only; remotes not created).
