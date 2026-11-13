# Spec: live booleans

**Status:** Draft, for the owner's approval · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, point 3; decision 0019 (2).

## 1. What changes

BooleanUnion, BooleanDifference, BooleanIntersection and BooleanSplit keep the solids they were made
from. The inputs disappear from view as before, but they are kept inside the result, and the result
remembers how it was made (Curva's History, `curva-document/src/history.rs`).

## 2. Working with them

- **EditBoolean** (a new command; also a double-click on a live boolean): shows the result's inputs,
  ghosted, and lets them be moved, scaled, edited with the gumball or any command. The result updates
  as they change. Enter or Escape ends editing and hides the inputs again.
- Inputs can themselves be live booleans, so a chain updates from the bottom up.
- **BreakBoolean** (a new command): drops the inputs and the link, leaving a plain solid. Exploding,
  splitting or otherwise editing the result's faces breaks the link too, as History does in Rhino,
  with a note in the command bar.
- Properties shows "Live boolean (union of 3)" and a button for each command.
- A boolean that can no longer be computed (the inputs moved apart) keeps its last good result,
  outlined in the warning colour, with the reason in the command bar.

## 3. In files

Saved with their inputs and recipe; older files open with plain solids. Exported to `.3dm` as plain
solids (Rhino has no live booleans), with the inputs on a hidden layer if `ExportInputs` is chosen.

## 4. Checks

- Tests: moving an input updates the result; chains; BreakBoolean; undo across an edit; a failing
  update keeps the last result; files round-trip.
- In the app: union two boxes, EditBoolean, drag one with the gumball, watch the union follow.
