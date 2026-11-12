# Spec: picking, snaps, undo and navigation

**Status:** Draft for the owner's approval · 12 November 2026. Second in the order of work (decision 0016).
**Covers:** feedback items 1, 8, 9, 17 and 18 (`docs/specs/feedback-2026-11-12.md`).

## 1. Clicking a surface or solid selects it (item 1)

Today only a click near an edge selects a surface or solid. From now on a click anywhere on its faces
selects it, as in Rhino. When objects overlap, the nearest one under the pointer wins, and curves and
points still win within their usual few pixels. A window box takes objects wholly inside it and a
crossing box objects it touches, faces included. The selection filter still applies.

## 2. Snaps only to what can be seen (item 8)

In Shaded and Ghosted views, a snap point hidden behind a face is not offered. Before showing a snap,
Curva checks that no face lies between the camera and that point. In Wireframe views everything can be
seen, so everything snaps, as in Rhino.

## 3. Sticky snaps and guidelines (item 9)

As in Rhino's SmartTrack and ArchiCAD's guide lines:
- **Holding a snap.** Resting the pointer on a snap point (an end, a midpoint, a centre, an intersection)
  for about half a second makes it sticky. It keeps a small marker in the accent orange.
- **Guidelines.** Each sticky point sends out soft, dashed, see-through lines along the construction
  plane's X and Y axes, and at 45°. When ortho is on, only X and Y.
- **New snap points from them.** Where two guidelines cross, where a guideline crosses an object, and
  midway between two sticky points, the pointer snaps. The marker says which kind it is, with symbols in
  ArchiCAD's style: a cross for an intersection, a triangle for a midpoint, a square for an end.
- **Letting go.** Resting on a sticky point again releases it and its lines. Escape, or the end of the
  command, releases them all. Up to eight can be held at once.
- **Visibility.** Only visible points can be held (§2).

## 4. Undo covers every change (item 18)

Ctrl+Z undoes the last change to the model (an object made, changed, deleted, renamed or moved to a
layer; a layer changed) together with the selection change that came with it. This holds even while
the command line has focus, which today swallows Ctrl+Z as text editing. Ctrl+Y and Ctrl+Shift+Z redo.
Camera moves are not undone by Ctrl+Z, as in Rhino (decision 0016): `UndoView` and `RedoView` step
through the active viewport's recent views, on Alt+Z and Alt+Shift+Z. Text in the command line is still
undone by Ctrl+Z when it holds typed text that has not been run.

## 5. Ctrl + right-drag zooms (item 17)

Holding Ctrl while dragging with the right button zooms the active viewport in and out about the point
under the pointer where the drag began, as in Rhino. Dragging up zooms in.

## 6. How it is checked

- Automatic:
  - a click inside a box's face, away from its edges, selects the box
  - the nearer of two overlapping objects wins
  - a snap point behind a face is not offered in Shaded, and is offered in Wireframe
  - sticky points and their guideline crossings and midpoints snap where expected, and resting again
    releases them
  - Ctrl+Z with the command line focused undoes the last model change and restores the selection
  - Ctrl + right-drag zooms
- **The owner's check:** tries picking and snapping on a crowded model by hand.
