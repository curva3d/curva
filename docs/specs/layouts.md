# Spec: layouts and detail views

**Status:** Draft for the owner's approval.
**Covers:** `milestone-3-in-0.4.3.md`, points 2 to 5; decision 0020.

Rhino 8's layouts are the target. They become the shared views-and-sheets core (`architecture.md`
§5): Architecture's sheets in Milestone 4 are layouts with more kinds of view on them.

## 1. Layouts

A **layout** is a paper sheet: a name, a paper size and orientation, margins, and the units it is
read in (millimetres in a metric document, inches in an imperial one). It holds detail views and
anything drawn on the paper: text, dimensions, hatches, curves, title blocks.

- **Tabs:** the model and each layout are tabs along the bottom of the viewport area, as in Rhino.
  A layout opens filling the viewport area; the model tab gets its viewports back as they were.
  Taken from Serpentine: any viewport's title menu can also show a layout, so a sheet and the model
  can sit side by side.
- **Paper sizes:** ISO A0 to A4 and ANSI A to E and Arch A to E, portrait or landscape, or custom.
- **On the sheet:** pan and zoom work as in a viewport; snaps, the gumball, selection, Undo and every
  curve and transform command work on paper, in paper units.

| Command | What it does |
|---|---|
| Layout | New layout: name, paper size, orientation, and how many detail views to start with (Rhino's dialog, as prompts and in a small panel) |
| CopyLayout | Copies a layout with its details and annotation |
| Layouts | Opens the Layouts panel |

**The Layouts panel** (a panel in Curva's panel system): every layout with a thumbnail, renamed in
place, dragged into order (the order the sheet index and Print use), copied, deleted, and its
properties (size, orientation, margins) edited.

## 2. Detail views

A **detail** is a rectangle on a layout that shows the model live: its own camera (a standard view
or a saved one, parallel or perspective), scale, display mode and layer visibility.

- **Detail** (Rhino's command and options, recorded in `rhino-reference.md` from Rhino 8's help):
  adds a detail by two corners with the view and scale chosen at the prompts, and enables (enters),
  disables, locks, unlocks and scales details.
- **Entering a detail:** double-click it, as in Rhino. Inside, pan, zoom and model editing act on the
  model; double-click outside to leave. A locked detail cannot be panned or zoomed.
- **Scale:** typed as `1:50`, `2:1`, `1/4" = 1'-0"` or `1" = 20'`, from a list of the set's standard
  scales, or as Rhino's paper-length = model-length pair. The scale is kept while the frame is
  resized; changing the document's units keeps every detail at its scale.
- **Display mode** per detail: any of Curva's display modes, including the new Technical and Hidden
  (`make2d.md` §4).
- **Layer visibility per detail:** a layer can be hidden in one detail only (Rhino's per-detail layer
  visibility, in the Layers panel's detail column).
- **Border:** a detail's frame is an object on its layer, so it can be hidden by putting details on a
  non-printing layer, as in Rhino.
- **Properties panel:** a selected detail shows its view, projection, scale, display mode and lock.
- **Serpentine aliases:** DetailScale, DetailMode, DetailLock, DetailBorder and DetailDelete run the
  matching parts of Detail.

## 3. Clipping planes and section fills

Curva has no clipping planes yet. Milestone 3 adds them with Rhino 8's behaviour:

- **ClippingPlane:** places a clipping plane, a rectangle that cuts away everything on its back
  side, by corners on the construction plane (Rhino's steps and options). It moves, rotates and
  flips like any object; EnableClippingPlane and DisableClippingPlane switch it on and off.
- **Per view:** a clipping plane cuts the views and details it is enabled in (Rhino's
  `ClippingPlane` view list; Serpentine's EnableClippingInView and DisableClippingInView).
- **Section fills:** where a clipping plane cuts a closed solid, the cut face is filled with the
  object's section style: solid colour, a hatch pattern, or none, with the cut edge drawn at its own
  print width. The style comes from the object, else its layer, else the clipping plane (Rhino 8's
  section style order). **SectionStyle** sets it on objects, layers and clipping planes
  (Serpentine's SectionHatch is an alias).
- Fills appear in viewports, in details, in Make2D's output and in print.

## 4. Title blocks, scale bars, sheet index, revisions

- **TitleBlock** places one of the built-in title blocks (an ISO one per A size, an ANSI one per
  ANSI size) or a block from any file. Its text fields fill themselves from the document and the
  sheet: project name, drawing title, sheet number and count, scale of the main detail, date,
  author, revision. Fields are Rhino's text fields (`annotation.md` §2), so a title block made in
  Rhino with fields works in Curva too.
- **ScaleBar** places a scale bar for a detail's scale, which follows if that scale changes.
- **SheetIndex** places a table of every layout's number and name, updated when layouts change.
- **Revision** adds a row (number, date, description, initials) to the sheet's revision table,
  which the title block shows.

These are Serpentine's commands; Rhino has none of them, so their steps follow Serpentine's.

## 5. Files

Layouts, details, section styles and title blocks are saved in `.curva` files, and read from and
written to `.3dm` files as Rhino's layouts and details (`drawing-output.md` §4). Older files open
unchanged, with no layouts.

## 6. Checks

Tests for each command and option; a detail at 1:50 measured on paper; the scale kept through a
unit change and a resize; per-detail layer visibility; a clipping plane filling a cut solid with
each kind of section style; title block fields filled; the sheet index following a renamed layout.
In the app: make a layout with two details, enter one and pan, lock it, change its scale and
display mode, and print it to PDF (`docs/releasing.md` gains these hand checks).
