# Spec: line drawing and line weights

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** feedback on 0.4.0, points 1 and 2; decision 0018 (1, 2).

## 1. What Rhino does

Rhino gives every layer and every object a **Print Width** in millimetres. An object's width is
`By Layer` unless set; a layer's is `Default` unless set. `Default` is Rhino's thin line. Rhino shows the
widths in Print Display mode, and always in print. Widths are paper millimetres: a 0.5 mm line is
0.5 mm thick on screen at the screen's real size, whatever the zoom.

## 2. Drawing lines once, at any width

0.4.0 draws every curve and edge three times, a pixel apart, to make them 2 pixels wide. That triples
the line work on the graphics card and cannot give other widths.

From 0.4.1 each line segment is drawn **once as a thin quad** (two triangles) turned to face the
screen, widened in the vertex shader by its own width in pixels, with smooth (antialiased) edges. One
draw for all the lines of a view, as before. The work per segment is the same for any width, so the
cost is that of the 1-pixel lines of 0.3.0, and drawing many weights costs nothing more than drawing
one. Joins between segments of a curve are rounded by overlapping the ends, so thick polylines have no
gaps at corners.

The grid stays 1 pixel. Highlighted (selected) lines draw one pixel wider than their weight.

**Checked by:** a benchmark test draws 200,000 segments; the frame time in the line pass is no more than
that of 0.4.0 with one pass (recorded in `docs/performance.md`).

## 3. Weights in the model

- **Layers** have a *Print width*: `Default`, or a number of millimetres (0.00 – 5.00, two decimals).
- **Objects** have a *Print width*: `By layer` (the default), `Default`, or millimetres.
- Rhino's list of common widths is offered in a drop-down: 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4,
  2.0 mm, and any value can be typed.
- Saved in the file; older files open with `Default` layers and `By layer` objects.
- Undoable like any change. The AI Assistant (MCP) can read and set both.

## 4. How they show

- `Default` draws 2 pixels thick (as 0.4.0).
- A width in millimetres draws at the screen's real size: millimetres × the screen's pixels per
  millimetre (from the window's scale factor, 96 dpi × scale), never thinner than 1 pixel. A 0.25 mm
  line is about 1 pixel on a 100 % screen and 2 on a 200 % screen; a 1 mm line is about 4 or 8.
- Every view shows them, in every display mode, as decision 0018 asks.
- Edges of solids and surfaces take the object's width; curves take theirs.

## 5. Where they are set

- **Layers panel:** a *Width* column beside the colour, showing `Default` or e.g. `0.35`; clicking it
  opens the list with a field.
- **Properties panel:** *Print width* for the selection: `By layer`, `Default`, or millimetres; a mixed
  selection shows `Varies`.
- Commands, as in Rhino: none needed beyond Properties.

## 6. Checks

- Unit tests: the width an object resolves to (By layer → layer → Default), files round-trip, older
  files open.
- In the app: set 0.13, 0.5 and 1.4 mm on three curves and a box, on a layer and on objects, and look
  at each display mode at 100 % and 200 % scale.
