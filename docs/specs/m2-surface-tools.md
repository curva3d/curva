# Spec: Milestone 2, surface tools

**Status:** Approved by the owner in chat · 11 November 2026, by choosing "More surface tools now" (Cap, FilletEdge, ChamferEdge, Shell, Pipe and OffsetSrf) over autosave and the rest of Milestone 1.
**Scope comes from:** architecture section 10, Milestone 2, and the phase F2 Surfaces and Solid editing lists. The engine passed these operations in Milestone 0 (`evaluation/reports/solidcore.tsv`: cases 12, 18, 19, 20, 22, 27 and 30).

## 1. What the owner will be able to do

| Command | Does, as in Rhino |
|---|---|
| `Pipe` | A round tube along curves, with a typed radius and a live preview. Options: `Cap` (closed ends, on by default) and `Thick` (a wall of a second radius) |
| `OffsetSrf` | A copy of a surface or solid's faces at a distance, with a live preview. Option `Solid` joins the copy to the original into a thick solid |
| `Cap` | Closes the flat open ends of surfaces, making solids where it can |
| `FilletEdge` | Rounds the picked edges of a solid by a radius |
| `ChamferEdge` | Bevels the picked edges of a solid by a distance |
| `Shell` | Hollows a solid to a wall thickness, opening the picked faces |

- **Picking edges and faces.** FilletEdge and ChamferEdge ask for edges; clicking near an edge picks it and draws it highlighted, Shift adds, Ctrl removes, and a box picks every edge inside it. Shell asks for faces the same way: clicking a face picks it. Enter finishes picking.
- Each command is one undo step and shows a live preview of its result before it commits.
- Each result goes on the layer of the object it came from. As with the booleans, a solid that is filleted, chamfered, shelled or capped is replaced by the result.

## 2. Order of work

1. Pipe, OffsetSrf and Cap, which need no new picking. Cap is blocked on the engine (#67).
2. Picking edges, then FilletEdge and ChamferEdge.
3. Picking faces, then Shell.

## 3. Not in this piece of work

Variable-radius fillets, fillets that run on along tangent edges by themselves, BlendSrf, ExtendSrf, and the rest of the F2 lists.

## 4. How it is checked

No Rhino tests (decision 0011). Automatic checks compare areas and volumes with exact answers: a pipe of radius r and length L has volume πr²L; a 10 mm cube shelled to 1 mm with its top open keeps 10³ − 8²·9 mm³; a fillet of radius r on one edge of a box removes (1 − π/4)·r² times the edge's length. Undo and the previews are checked too.

**The owner's check:** tries it by hand and says whether it behaves like Rhino.
