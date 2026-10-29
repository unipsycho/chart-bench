# TODO

Actionable list (titles from the former Project tracker; Cursor task IDs dropped).

## Open

- [ ] Lock planning and todos into local csv-bench *(this lockdown — keep docs current)*
- [ ] Confirm Preset vs Bookmark naming + slot / channel-mismatch UX
- [ ] Confirm export formats (PNG / CSV / workspace) and anomaly threshold UX
- [ ] Implement Parquet ingest (replace stub)
- [ ] Command palette / F1 if not fully wired
- [ ] Session history preview / revert (per workflow vision)

## Done (recent)

- [x] Chart tiling UX: clear TOP/BOTTOM/LEFT/RIGHT split landings + explicit SPLIT cue; center MOVE; own-tile edge split; Overlay no longer hides multi-pane charts
- [x] Presets/session: persist all open files + per-chart L/R series bindings; remap by path/display/column after reload (UUID-safe)
- [x] Competitive survey of CSV and tabular graphing tools → `docs/planning/competitive-csv-graphing-survey.md`
- [x] Clear channel hover preview when pointer leaves Channels list; no hover/edge axis pan (screen-space crosshair; wheel zooms, drag pans)
- [x] Channels header: active chart label + dropdown; tile click syncs; unique Chart N names on new/split/duplicate; no hover/wheel axis pan (wheel zooms, drag pans)
- [x] Channels: ↑↓ nav; **Space** toggles selection, **Enter** adds to selection; **←/→** toggle left/right Y; **L/R** buttons (colored when on axis) replace +/+R; click selects only
- [x] Channels: Enter / Space / click = left Y toggle; **→** = right Y toggle (+R); ↑↓ nav; narrower left dock; section header glyphs; dock DnD top/bottom stack only; chart edge-split **duplicates**; chart **Pop-out** viewport; removed under-chart L/R checkbox strip
- [x] Channels: tighter row spacing; Enter / Space / click **toggle** on active chart
- [x] Right/left/bottom docks: full-column Favnyr drop target; chart titles draggable with landing preview
- [x] Dock drag affordance + Favnyr purple edge/center drop preview; painted move icons; table height cap; channel preview series; line sparklines; chart crosshair + per-series Y readout; active-chart series highlight in Channels
- [x] Scroll-wheel chart zoom (clamp to fit-all) + quieter plot grids
- [x] Channel list ↑↓ keyboard nav
- [x] Stack Info + Saved/Presets in one right column; dock stacks rearrangeable + session-persisted
- [x] Relaunch csv-bench GUI on M17
- [x] Fix channel names and Favnyr-ish UI layout pass → `docs/planning/csv-bench-ui-pass.md`
- [x] Launch csv-bench GUI on M17
- [x] Collect varied public CSV samples → `docs/planning/csv-bench-sample-csvs.md`, `samples/`
- [x] Mid-size open stress CSVs (~100k–500k) — local/gitignored; see `samples/README.md`
- [x] Clear painted Table icon on icon rail + Table section header
- [x] Build and run csv-bench on M17 → `docs/planning/csv-bench-m17-build.md`
- [x] Scaffold csv-bench egui workbench source
- [x] Decide UI chrome and navigation model → `docs/planning/ui-chrome-navigation.md`
- [x] Design live / growing file follow mode → `docs/planning/live-file-reload.md`
- [x] Survey CSV sniffing and channel-detection tools → `docs/planning/csv-detection-tools-landscape.md`
- [x] Design data info panel and local analysis → `docs/planning/data-info-panel-analysis.md`
- [x] Capture DnD workflow and preset vision → `docs/planning/product-workflow-vision.md`
- [x] Design multi-line CSV header and channel detection → `docs/planning/csv-header-channel-detection.md`
- [x] Scope egui_plot chart capabilities → `docs/planning/plot-capabilities-scope.md`
- [x] Map Favnyr features to a CSV graphing tool → `docs/planning/favnyr-csv-feature-map.md`

## Deferred / won't do (for now)

- [~] Favnyr-style **nested binary split tree** for chart tiles — not needed; edge SPLIT (duplicate) + center MOVE + Stacked/Side-by-side is enough. Dock sidebars stay top/bottom stack only.
