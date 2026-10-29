# csv-bench — competitor notes (technical)

**Related:** [competitive-csv-graphing-survey.md](competitive-csv-graphing-survey.md) (feature-fit matrix and full dossiers).

Short differentiation lens vs nearby dump / timeseries tools.

## Lens (what “win” means)

Desktop dump workbench: **CSV / XLS / Parquet**, multi-file DnD channel graphing, **dual Y**, Favnyr-style tiling, named **presets** (slots + rebind). Stack: Rust + egui.

| Criterion | Good for us |
| --- | --- |
| DnD channels | Drag series → overlay charts |
| Multi-file | Compare dumps in one session |
| Dual / multi Y | L/R (or more) without ceremony |
| Favnyr tiling | Nested split / rearrange / detach |
| Multi-line headers | LabVIEW-ish name/node/units |
| Presets | Same layout on *new* files (slots / rebind) |

## Primary cousins (gaps to protect)

| Tool | License | Steal | Differentiate |
| --- | --- | --- | --- |
| **PlotJuggler** | MPL-2.0 | DnD speed, layouts, follow mindset | Dual/multi Y (PJ rejects), multi-line DAQ headers, dump-first CSV/XLS |
| **VisiData** | GPL-3.0 | Instant explore density | Full desktop charts / DnD / presets (ideas only — GPL) |
| **DIAdem / LabChart / Origin** | Commercial | Batch / report depth as aspirational UX | Stay lightweight OSS dump triage |
| **Serial Studio / Teleplot** | GPL / MIT | Live follow dashboards | File dumps + multi-line headers first |
| **Grafana / Foxglove** | AGPL / freemium | Dashboard-as-preset metaphor | Local multi-file opener, not cloud/ops |

**Alert:** Prefer [plotjuggler.io](https://www.plotjuggler.io) / [GitHub](https://github.com/PlotJuggler/PlotJuggler). **plotjuggler.com** has been reported as an impersonation site.

Full matrices and dossiers live in the competitive survey.
