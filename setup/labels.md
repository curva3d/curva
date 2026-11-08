# Labels and milestones

## Workflow labels (architecture section 18)

| Label | Colour | Meaning |
|---|---|---|
| `agent-ready` | `0E8A16` | An agent may pick this up |
| `in-progress` | `FBCA04` | Claimed; the claiming pull request is linked |
| `needs-human` | `D93F0B` | Waiting for a decision from the owner |
| `blocked` | `B60205` | Waiting on something else; reason stated in the issue |
| `flaky-test` | `C5DEF5` | A quarantined test that passes and fails without code changes |

## Area labels

| Label | Colour | Meaning |
|---|---|---|
| `area: kernel` | `5319E7` | Geometry kernel and its evaluation |
| `area: infra` | `1D76DB` | Repository, CI, machines, agent tooling |
| `area: docs` | `0075CA` | Documentation, specs, decisions |
| `area: commands` | `006B75` | Freeform commands |

## Milestones

| Title | Description |
|---|---|
| `M0 — Groundwork and kernel evaluation` | Repository, CI, agent guardrails, and a tested choice of geometry kernel |
| `M1 — Freeform foundation` | Core, viewport, command framework, .curva file, F1 commands, history on curves |
| `M2 — Freeform surfaces and solids` | F2 and F3 commands, live booleans, blocks, 3DM import / export |
| `M3 — Layouts and drafting` | F4: layouts, details, Make2D, dimensions, hatches, smart object representations |
| `M4 — Architecture` | Levels, walls, slabs, openings, doors, windows, live plans and sections, IFC |
