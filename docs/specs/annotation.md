# Spec: annotation

**Status:** Approved by the owner in chat · 14 November 2026.
**Covers:** `milestone-3-in-0.4.3.md`, points 6 to 10; decision 0020.

Rhino 8's annotation is the target: its objects, commands, options and styles. Annotation works the
same in the model and on layouts.

## 1. Annotation styles

A named **annotation style** holds everything about how annotation looks: font, text height, text
gap, arrowheads and their size, extension line offsets and extensions, dimension units, precision
and rounding, alternate units, tolerance display, leader settings, and whether it scales with model
space. Every text, dimension and leader uses a style and can override any of its settings, as in
Rhino.

- **Built-in sets** (decision 0020, point 4):
  - **ISO**, for metric documents: text 2.5, 3.5, 5 and 7 mm (ISO 3098), filled arrows or ticks,
    dimensions in mm.
  - **ANSI**, for imperial documents: text 3/32", 1/8" and 3/16", architectural feet-and-inches and
    decimal-inch styles.
  - A new document gets the set matching its units; the other can be added at any time.
- **Annotation scaling** as Rhino does it: on a layout, sizes are paper sizes. In the model, a style
  set to model-space scaling multiplies its sizes by the model-space scale, and details show
  annotation at the size it would print.
- **AnnotationStyle** (Rhino's DimStyle; Serpentine's DimStyle is an alias) opens the styles in a
  panel: add, copy, rename, delete unused, edit every setting with a live preview.
- Text height is measured on capital letters, as drafting standards do (taken from Serpentine).

## 2. Text

- **Text** places a note: pick a point (or two corners for a wrapping box), type in place beside it,
  click away or press Escape to finish. Multi-line, with left, centre and right alignment, nine
  justification points, rotation, bold, italic, underline, font and height per run, as Rhino's
  text editor.
- **Fields:** Rhino 8's text fields, as its help page `information/text_fields.htm` lists them:
  `%<FileName(...)>%`, `%<Date("Format", "Region")>%`, `%<DateModified(...)>%`, `%<PageName()>%`,
  `%<PageNumber()>%`, `%<NumPages()>%`, `%<PageWidth()>%`, `%<PageHeight()>%`, `%<PaperName()>%`,
  `%<DetailScale("DetailID","Format")>%`, `%<Area("ObjectID")>%`, `%<CurveLength("ObjectID")>%`,
  `%<Volume("ObjectID")>%`, `%<UserText("ObjectID","Key")>%`, `%<DocumentText("Key")>%`,
  `%<LayoutUserText(...)>%`, `%<LayerName(...)>%`, `%<ObjectLayer(...)>%`, `%<ObjectName(...)>%`,
  `%<ObjectPageName(...)>%`, `%<ObjectPageNumber(...)>%`, `%<ModelUnits()>%`, `%<Notes()>%`,
  `%<PointCoordinate(...)>%`, `%<BlockName(...)>%`, `%<BlockDescription(...)>%`,
  `%<BlockInstanceCount(...)>%`, with arithmetic (`%<NumPages()-1>%`); a field that cannot be
  worked out shows `####`. Updated when what they read changes. (This list first named a
  `DocumentName()` field, which Rhino does not have; `FileName` is Rhino's.)
- **Editing:** double-click text to edit it in place; its words, style and every override are also in
  the Properties panel. Serpentine's AnnotEdit is an alias.
- **Text in the model** lies on the construction plane, or faces the view if set to (Rhino's
  `Orientation` option). TextObject (curves, surfaces or solids from text) stays as it is.
- Fonts are the bundled Inter plus any font installed on the computer, read with `skrifa`.

## 3. Dimensions

| Command | What it measures |
|---|---|
| Dim | Horizontal or vertical, chosen by the direction you drag, or aligned with an option |
| DimAligned | Parallel to the two points |
| DimRotated | At a given angle |
| DimRadius, DimDiameter | Arcs and circles |
| DimAngle | Between two lines, or three points |
| DimOrdinate | X or Y from an origin |
| DimArcLength | The length of an arc |
| DimCurveLength | The length of a curve |
| DimRecenterText | Puts moved dimension text back in its default place |

Each takes Rhino 8's steps and options (`rhino-reference.md`). Grips move the dimension line, the
text and the measured points.

- **Associative:** a dimension that snapped to an object's points follows the object when it moves
  or changes, as in Rhino.
- **Through a detail:** a dimension placed on a layout that snapped to model geometry inside a detail
  measures the model in model units, follows the geometry, and moves with the detail's view (taken
  from Serpentine). A dimension on bare paper measures the paper.
- Dimension text can be overridden with typed text that includes the measurement (`<>`), as in Rhino.

## 4. Leaders and dots

- **Leader:** a polyline or curved arrow from a point to a note, with Rhino's options; the note is
  text, so it takes fields and styles.
- **Dot:** Rhino's annotation dot, a label bubble at a 3D point that stays the same size on screen,
  with its font height and secondary text.

## 5. Hatches

- **Hatch:** fills closed planar curves, or regions picked by clicking inside boundaries (Rhino's
  `Boundary` and region picking, with Serpentine's popup preview), with a pattern, scale, rotation
  and base point. Hatches with boundaries kept follow their curves when they change (Rhino's
  `Boundary` setting).
- **Patterns:** solid fill, gradients (two colours or more, linear or radial, as Rhino 8), and line
  patterns. The ISO set ships the common ISO 128-50 and architectural patterns (concrete, brick,
  insulation, earth, steel, timber); the ANSI set ships ANSI31 to ANSI38 and AR-series patterns.
  `.pat` files load more (HatchPatterns, Serpentine's command; Rhino keeps patterns in the document
  as well). Patterns used are saved in the file.
- **Editing:** a selected hatch shows its pattern, scale, rotation and colour in Properties;
  SetHatchBasePoint (Rhino's command) moves its origin; Explode turns it into curves or a surface.
- **SelHatch** selects every hatch (already planned by Rhino's selection set).

## 6. Selection, snaps and the rest

Annotation picks as one object, has snap points (text corners and insertion point, dimension
points, leader points), takes layer, colour and print width like any object, and works with Move,
Copy, Rotate, Scale, Mirror, Delete, Undo, groups and blocks. Mirror keeps text readable (Rhino's
`MirrorText` setting off by default).

## 7. Prior work checked

Checked 14 November 2026 (decision 0021, point 7):

| Need | Found | Licence, activity | Verdict |
|---|---|---|---|
| Multi-line text layout (wrapping, alignment, mixed fonts, right-to-left) | `parley` 0.13 (Linebender) | MIT OR Apache-2.0, updated November 2026, built on `skrifa`, which Curva already uses | **Use** |
| Text layout, alternative | `cosmic-text` 0.19 | MIT OR Apache-2.0 | Not chosen: `parley` shares Curva's font stack |
| Hatch regions and islands | `i_overlay` 9.1 | MIT OR Apache-2.0, updated October 2026 | **Use** for finding regions to fill and clipping pattern lines to them |
| Curve flattening for patterns | `kurbo` 0.13 | MIT OR Apache-2.0 | Not needed: the kernel samples curves |
| `.pat` files | no maintained crate | — | Curva reads them: a short, documented text format |
| Dimensions, leaders | no Rust library | — | Curva's own, following Rhino |

## 8. Files

Styles, text, dimensions, leaders, dots and hatches are saved in `.curva` files and read from and
written to `.3dm` files as Rhino's annotation objects (`drawing-output.md` §5).

## 9. Checks

Tests for every command and option; associative dimensions following a moved and a reshaped object;
a dimension through a detail; each text field; annotation scaling in the model and on paper;
built-in styles and patterns in both sets; region hatching with islands; a hatch following its
boundary. In the app: place every kind of dimension, edit text in place, hatch by region, and check
sizes on a printed PDF with a ruler (`docs/releasing.md`).
