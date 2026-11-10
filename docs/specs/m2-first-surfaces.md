# Spec: Milestone 2, first surfaces

**Status:** Approved by the owner in chat · 10 November 2026, with Sweep1 and the booleans added at his choice ("Bigger"). On 10 November 2026 the owner chose to start surfaces now and postpone the assistant panel (`m1-assistant-panel.md`).
**Scope comes from:** architecture section 10, Milestone 2, and the phase F2 Surfaces and Solids lists. The engine already passed these operations in Milestone 0 (`evaluation/reports/solidcore.tsv`: cases 4, 5, 6, 8, 10, 11, 13 to 16 and 24).

## 1. What the owner will be able to do

| Command | Does, as in Rhino |
|---|---|
| `ExtrudeCrv` | Pulls curves straight out into surfaces, with a live preview of the height. Options: `Solid` (caps closed curves into a closed solid), `BothSides` |
| `PlanarSrf` | Fills closed flat curves with a surface; a curve inside another becomes a hole |
| `Revolve` | Spins curves around an axis, by a full turn or a typed angle |
| `Loft` | Stretches a skin through two or more curves in order, open or closed |
| `Box` | A solid box from two corners and a height |
| `Sweep1` | Sweeps cross-section curves along one rail curve |
| `BooleanUnion` | Joins solids into one |
| `BooleanDifference` | Cuts solids away from others |
| `BooleanIntersection` | Keeps only where solids overlap |

- **Shaded** display mode in every viewport, with a menu to switch each viewport between Wireframe and Shaded as in Rhino. Surfaces show their edges in wireframe.
- Surfaces and solids can be picked, moved, copied, turned, scaled, mirrored, hidden, deleted, put on layers and moved with the gumball, like curves. Each command is one undo step.
- They are saved in `.curva` files and come back exactly.
- Snaps work on their edges and corners (End, Mid, Near).

## 2. Not in this piece of work

Sweep2, fillet edges, Cap, shells and other F2 commands come in the next pieces. Control points on surfaces, history on surfaces, and render materials are later too.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover each command's result (area and volume compared with the exact answer, such as a 10 × 20 × 5 box being 1000 cubic units, and a box minus a cylinder losing exactly the cylinder's volume), saving and reopening, undo, transforms, and the live previews. The 5,000-curve speed test gains a scene with 500 surfaces, to keep orbiting smooth.

**The owner's check:** tries it by hand and says whether it behaves like Rhino.
