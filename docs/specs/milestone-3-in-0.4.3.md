# Milestone 3 in 0.4.3 · 14 November 2026

**Status:** Draft for the owner's approval.

Milestone 3 is "Layouts and drafting" (`docs/architecture.md` §10: phase F4, layouts, detail views,
Make2D, dimensions, hatches, PDF and DXF), and becomes the shared views-and-sheets core that every
specialization uses. This lists every part of it, numbered, with the spec that covers it. The owner's
answers are in decision 0020. Curva 0.4.3 is released when all of it is built and checked in the app.

Rhino 8 is the target for every command's name, steps and options; Serpentine3D's drafting module
(`serpentine3d/commands/drafting.py`, MIT) is the starting point, and its command names stay as
aliases. Each command gets its row in `docs/commands/rhino-reference.md`, written from Rhino 8's help
pages before it is built.

## 0. Clean-up after Milestone 2

1. The clean-up pass `architecture.md` §17 asks for after each milestone: dead code removed,
   duplicates merged, docs brought up to date, all tests passing. Done first, in its own pull request.

## Layouts and detail views (`layouts.md`)

2. Layouts: paper sheets as tabs beside the model, with size, orientation, margins and units;
   Layout, CopyLayout, and a Layouts panel listing, ordering and renaming them.
3. Detail views: live windows into the model on a sheet, with view, scale, display mode, lock,
   border and their own layer visibility; Detail and its options; entering a detail to pan and zoom.
4. Clipping planes and section fills: ClippingPlane (Curva has none yet), on and off per view and per detail, and solids
   cut by them filled with their section hatch (Rhino 8's section styles; Serpentine's SectionHatch).
5. Title blocks, scale bars, sheet index and revisions: built-in ISO and ANSI title blocks with fields
   filled from the document and the sheet; ScaleBar, SheetIndex, Revision.

## Annotation (`annotation.md`)

6. Text: notes on sheets and in the model, multi-line, with font, height, alignment and fields;
   edited in place.
7. Dimensions: linear, aligned, rotated, radius, diameter, angle, ordinate, arc length and curve
   length; associative to the geometry they measure, through detail views too.
8. Leaders and dots.
9. Hatches: Hatch on closed curves or by picking inside regions, patterns from `.pat` files, scale,
   rotation, base point, solid fills and gradients; hatch patterns kept in the document.
10. Annotation styles: named styles for text, dimensions and leaders; built-in ISO (metric) and ANSI
    (imperial) sets; annotation scaling in the model and on sheets as Rhino does it.

## Make2D (`make2d.md`)

11. Make2D exactly as Rhino 8 does it: a one-off 2D drawing of the selected objects from a view,
    with hidden lines, tangent edges, silhouettes, clipping-plane cuts and Rhino's output layers. It
    has no link to the model afterwards.
12. A hidden-line display mode (Rhino's Technical and Hidden) for viewports and detail views, sharing
    Make2D's hidden-line code.
13. The hidden-line removal is Curva's own (solidcore has silhouettes and ray tracing but no hidden
    lines), so it is flagged as custom in `docs/kernel/custom-operations.md` and checked against
    solidcore before each release.

## Drawing output (`drawing-output.md`)

14. Print: sheets, the model or a view, to PDF, to an image, or to a printer, with scale, print widths
    and colour or black-and-white.
15. Export to PDF, SVG and DXF; import DXF.
16. Layouts, detail views, annotation, hatches and annotation styles kept in `.curva` files, and read
    from and written to `.3dm` files.

## Not in 0.4.3

- Smart object representations (plan symbols, detail levels, scale variants) and parametric objects
  in Rhai or WebAssembly come with Milestone 4 (decision 0020).
- Live plan, section and elevation views that follow the model come with Milestone 4, as
  `architecture.md` §5 says. Make2D stays one-off.
- DWG files: no maintained pure-Rust reader or writer exists; revisit in Milestone 4.
