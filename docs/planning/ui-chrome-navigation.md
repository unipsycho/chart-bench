# UI chrome — menu, ribbon, or panel-first?

**Question:** Given csv-bench’s scoped workflow, do we need a **menu bar** or **TopicBench-style ribbon**? How do users open/toggle panels? How does that compare to **Favnyr**?

**Related:** [product-workflow-vision.md](product-workflow-vision.md), [favnyr-csv-feature-map.md](favnyr-csv-feature-map.md).

---

## 1. Short answer

**You don’t need a full Office-style ribbon for the happy path.**  
csv-bench is **panel + drag-and-drop + presets**, closer to Favnyr/PlotJuggler than to a document editor full of tabs of commands.

**You do need a thin, discoverable chrome** for: Open, panel toggles, Follow, Export, Presets, Help/shortcuts — plus **keyboard** (TopicBench habit) and a **command palette** for power users.

**Recommendation:** **Favnyr-like tiling + TopicBench-like keyboard**, with a **compact top bar** (not a fat ribbon). Optional later: a **narrow “Tools” strip** if export/analysis actions pile up — still not a multi-tab ribbon unless you insist on TopicBench brand consistency.

---

## 2. What the product actually needs to expose

Group actions by frequency:

### Always / every session (primary)

| Action | Best home |
| --- | --- |
| Open / drop files | Files pane + top **Open** + OS drag onto window |
| Browse channels, preview | **Channels** pane (always on or one toggle) |
| Drag channel → chart | Canvas itself |
| Toggle Files / Channels / Info / Saved / Table | **Panel toggle strip** or View menu shortcuts |
| Follow live file / follow tail | Control on **file row** + chart toolbar |
| Legend / mouse-over | On-chart (no ribbon) |
| Multi-axis assign | Legend / series context menu / drop affordance |

### Frequent but not every click

| Action | Best home |
| --- | --- |
| Save / apply **Preset** | **Saved** pane + top “Presets” |
| Export PNG / CSV | Chart context menu + top **Export** |
| Header / parse assistant | Channels or Info → “Edit headers…” |
| History / revert | Saved pane / History |
| Split / new chart tile | Favnyr gestures + small layout buttons |

### Rare / settings

| Action | Best home |
| --- | --- |
| Theme, keybindings, paths | **Settings** dialog (gear) |
| About / license | Menu or palette |
| Full reload file | File row menu |

A **classic menu bar** can hold File / Edit / View / Help as a thin OS-native or egui menu — fine for discoverability and accessibility. A **ribbon** only pays off if you have **many peer commands** that need icons in labeled groups (TopicBench’s MQTT surface might; csv-bench’s surface is thinner).

---

## 3. Favnyr vs TopicBench chrome

| | **Favnyr** | **TopicBench** (your ribbon) | **csv-bench fit** |
| --- | --- | --- | --- |
| Primary metaphor | File manager tiles + sidebar | MQTT workbench + ribbon actions | Data workbench tiles + DnD |
| Top chrome | Relatively light; shortcuts heavy | Ribbon groups commands | Prefer **light** |
| Panels | Sidebar + split leaves | Many panels / stickies | Files, Channels, Info, Charts, Saved |
| Layout control | Drag zones, split, detach | Panel toggles in UI | **Favnyr-style** for charts |
| Brand consistency | N/A | Same family as csv-bench | Echo TopicBench **shortcuts / F1**, not necessarily the ribbon height |

**Don’t clone Favnyr’s file-manager menus** (Open with, trash, eject). **Do clone** split/tab/detach + rebound shortcuts.  
**Don’t require TopicBench’s full ribbon** unless you want visual kinship for marketing; **do** keep `?` / F1 help and panel toggles familiar.

---

## 4. Recommended chrome layout

```text
┌─ [Open] [Presets ▾] [Export ▾]  …spacer…  [Follow] [⚙] [?] ─┐
├─ Panel toggles:  Files  Channels  Info  Table  Saved  (icons) ─┤
├──────────┬──────────────────────────────────────┬────────────┤
│ Files    │         Chart tiles (DnD)            │ Info       │
│          │         + per-chart mini toolbar     │            │
│ Channels │         (zoom, follow-tail, axis)    │            │
└──────────┴──────────────────────────────────────┴────────────┘
```

### Pieces

1. **Compact top bar** (~one row of controls) — not multi-row ribbon.  
2. **Panel toggle bar** — show/hide docked panes (like View → Panels, but always visible as icon toggles).  
3. **Per-chart toolbar** — zoom reset, follow-tail, export-this-plot (contextual > global ribbon).  
4. **Command palette** (`Ctrl+K` / `Ctrl+Shift+P`) — every action searchable.  
5. **Optional classic menu** — File / View / Help for OS norms and screen readers; can be hidden in “compact mode.”  
6. **Context menus** — right-click series, file, channel (axis, hide, export, header edit).

### What to avoid in v1

- Multi-tab ribbon (Home / Insert / Data / View…) with empty groups.  
- Duplicating every DnD action as a ribbon button.  
- Modal-only workflows for Open/Preset (keep panes).

---

## 5. Opening / toggling panels — concrete map

| Panel | Default | Toggle |
| --- | --- | --- |
| Files | On | `Ctrl+1` / toggle icon |
| Channels | On | `Ctrl+2` |
| Info | On or tab-with-Channels | `Ctrl+3` |
| Table (raw rows) | Off until needed | `Ctrl+4` |
| Saved (presets/history) | Off or narrow | `Ctrl+5` |
| Header assistant | Modal or slide-over | From Channels |
| Settings | Modal | Gear |

Closed panels leave more room for charts — Favnyr users already think in “show/hide side chrome.”

---

## 6. When a ribbon *would* make sense

Reconsider a TopicBench-like ribbon if you later add lots of **global analysis tools** (FFT, filters, batch export, compare runs) that aren’t natural on a single chart toolbar. Until then, a ribbon is mostly **vertical space tax** on a tiling plot app.

**Middle path for family branding:** same **icon language + shortcut help sheet** as TopicBench, but **height of a toolbar**, not a ribbon.

---

## 7. Decision stake (proposed)

| Choice | Stake |
| --- | --- |
| Full ribbon | **No** for v1 |
| Classic menu bar | **Optional thin** File/View/Help |
| Compact top bar + panel toggles | **Yes** |
| Favnyr tiling / DnD layout | **Yes** |
| Command palette + F1 shortcuts | **Yes** |
| Per-chart mini toolbar | **Yes** |

---

## 8. Open for owner

1. Prioritize **TopicBench visual kinship** (worth a short ribbon-like strip) or **max chart pixels** (toolbar only)?  
2. Menu bar always visible on macOS/Linux, or compact-only?  
3. Default visible panes: Files+Channels+Info+Chart, or Files+Channels+Chart with Info as tab?
