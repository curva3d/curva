# Spec: live booleans

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, point 3; decision 0019 (2).

## 1. What changes

BooleanUnion, BooleanIntersection and BooleanDifference (of one solid) keep the solids they were made
from. (As built: BooleanSplit, which makes two results from the same inputs, and a difference of
several solids at once stay plain, since an input belongs to one result only.) The inputs disappear from view as before, but they are kept inside the result, and the result
remembers how it was made (Curva's History, `curva-document/src/history.rs`).

## 2. Working with them

- **EditBoolean** (a new command): shows the result's inputs and lets them be moved, scaled, edited
  with the gumball or any command; the result updates as they change. Running it again on the result
  hides them. (As built: a toggle rather than an editing mode ended by Enter, so any command can be
  used meanwhile.) Moving or turning the result, with a command or the gumball, moves its inputs too,
  so it stays live.
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
