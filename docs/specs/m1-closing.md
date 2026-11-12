# Spec: closing Milestone 1

**Status:** Approved by the owner in chat · 12 November 2026 ("Let's do it", to the proposed list).
**Scope comes from:** architecture section 10, Milestone 1 scope and success criteria, checked against
what Curva has after 0.1.1. Clipping planes, curvature and zebra analysis, and Import / Export are left
for Milestone 2, next to the surface tools and .3dm files.

## 1. What the owner will be able to do

| Piece | Does, as in Rhino |
|---|---|
| `Curve` | A control-point curve: click points, Enter to finish. Options `Degree` (1 to 11, default 3, remembered) and `Close`. The preview follows the cursor |
| `ScaleNU` | Pick a base point, then type a scale factor for X, then Y, then Z, along the construction plane's axes (Enter keeps 1). Option `Copy`. Live preview |
| `SelSrf`, `SelSolid` | Select every visible, unlocked surface (open) or solid (closed), adding to the selection |
| Selection filter | A row in the status bar, Points · Curves · Surfaces · Solids, each a checkbox, plus Disable. Clicking and boxing only pick the kinds ticked. `SelFilter` shows or hides the row; `SelFilterToggle` (or Disable) turns the filter off and on without losing the ticks. The `Sel…` commands ignore the filter, as in Rhino |
| Ghosted display | A third display mode beside Wireframe and Shaded: faces see-through at about half strength, so edges behind them show. Chosen in `SetDisplayMode` or from the viewport title |
| `Volume` | Pick closed solids, Enter: the command line shows each volume and the total, in the model's units, to six significant figures. Open surfaces are refused with a message |
| Autosave | Every 5 minutes, and only if something changed, Curva saves a copy to a recovery file in the user's app-data folder (not over the user's file). If Curva did not close normally, the next start offers to restore it. A normal close deletes the recovery file |

## 2. Behind the scenes

- **The `.curva` file format.** The architecture asks for two ways of storing the file to be compared: redb (a small embedded database) and a zip-style package. Today a `.curva` file is one JSON text. Measure all three on the test scene (5,000 curves and 500 solids): time to save, time to open, file size, and how a damaged file fails. Write the results up as a decision note for the owner to choose; files saved in today's format keep opening whatever is chosen.
- **Engine bugs reported upstream.** Prepare small reproducible cases for the two solidcore problems we know: a body turned then moved cannot be read back (status 1007), and Cap leaves the closed space empty (#67). **The issues are posted on solidcore's GitHub only after the owner has seen them**, since they go out under the owner's account.

## 3. Order of work

1. `Curve`, `ScaleNU`, `SelSrf`, `SelSolid`, `Volume`.
2. Selection filter.
3. Ghosted display.
4. Autosave and restore.
5. File format measurements and decision note.
6. Upstream bug reports, drafted for the owner.

## 4. Not in this piece of work

Clipping planes, curvature and zebra analysis, Import / Export, units and tolerance settings, picture
frames, ViewCaptureToFile, Material, history for more commands, recompute on all cores, and Rhai
(still held, decision 0014).

## 5. How it is checked

No Rhino tests (decision 0011). Automatic checks:
- A degree-3 `Curve` through n clicks has n control points and starts and ends on the first and last click; `Close` makes it closed and periodic.
- `ScaleNU` by (2, 1, 3) turns a 1 mm cube's bounding box into 2 × 1 × 3 mm, and is one undo step.
- `SelSrf` and `SelSolid` pick only their kind; with only Curves ticked, a box around everything picks only curves.
- `Volume` of a 10 mm cube is 1000 mm³, and of a cylinder of radius 5 and height 10 is 785.398 mm³.
- Ghosted pictures made with `--picture` show an edge behind a face that Shaded hides.
- Autosave writes the recovery file after a change, not without one, and restoring brings the same objects back.

**The owner's check:** tries it by hand and says whether it behaves like Rhino.
