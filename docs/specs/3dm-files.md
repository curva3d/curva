# Spec: Rhino .3dm files

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, point 1; decision 0019 (1).

## 1. Scope

Curva reads and writes Rhino's `.3dm` format in its own Rust code (a new crate, `curva-3dm`), with no
`unsafe` code and no C++ library. The format is the one McNeel publishes in openNURBS (its source is
the documentation; no code is copied). Curva reads files from Rhino 5 to Rhino 8 and writes the Rhino 7
version, which Rhino 7 and 8 open.

## 2. What comes across

| In the file | Read | Written |
|---|---|---|
| Units, tolerance | yes | yes |
| Layers: name, colour, visible, locked, print width, nesting (flattened to "Parent::Child" names) | yes | yes |
| Points, lines, polylines, arcs, circles, ellipses, NURBS and poly-curves | yes, exact | yes, exact |
| NURBS surfaces, planes, surfaces of revolution and extrusions | yes, as surfaces | yes |
| Polysurfaces and solids (B-reps, trimmed faces) | yes | yes |
| Extrusion objects (Rhino's light solids) | yes, as solids | not written (B-reps instead) |
| Object names, colours by layer or object (own colours kept), print widths, user text | yes | yes |
| Linetypes of layers and objects (the linetype table), lengths in millimetres | yes: Curva's seven by name, others as the document's own with their patterns | yes, the ones used, with their patterns |
| Groups, locked and hidden objects | yes | yes |
| Blocks: definitions, instances, linked definitions | yes | yes |
| Named views and construction planes | yes | yes |
| Meshes, SubD, point clouds | listed and skipped, with a count in the command bar | no |
| Text dots (`ON_TextDot`): point, words, secondary words, height, font | yes | yes |
| Hatches (`ON_Hatch`) and their patterns (the hatch pattern table): plane, outlines and holes, pattern, scale, turn | yes; a pattern Curva lacks comes in with the file | yes |
| Notes (`ON_Text`): words (from rich text), plane, height and alignment from their own or named annotation style, font | yes, from Rhino 6 to 8 | yes: words with their fields, plane, height and alignment, through one annotation style per height and alignment (fonts not yet) |
| Annotation styles (`ON_DimStyle`): name, font, text height, arrow size, text gap, extension lines, decimals, alignment | yes, the ones the annotation uses, kept by name | yes, each style used under its name (version 1.2 with a font; later settings take Rhino's defaults); a note of another height or alignment gets a style beside its own ("ISO 3.5 7 Centre"), folded back on reading |
| Linear dimensions (`ON_DimLinear`): points, dimension line, aligned or rotated, typed text | yes, with their style | yes, with a style of their annotation style's text height |
| Radius, diameter and angle dimensions (`ON_DimRadial`, `ON_DimAngular`), leaders (`ON_Leader`) | yes (looks from Curva's current style; leaders with a landing) | yes |
| Layouts (page views) with their size, and what is drawn on them (in the file's page unit) | yes | yes, in millimetres, with Rhino's four standard views beside them |
| Detail views: frame, standard view or perspective, target, scale, display mode (Rhino's standard modes), lock | yes | yes (view attributes version 1.3) |
| Scale bars and tables | — (Rhino has none) | as the plain drawing they make: lines, solid hatches and notes, grouped |
| Document user text (`DocumentText` field), layout user text (`LayoutUserText`, on the page's viewport) and the model's notes (`Notes` field) | yes | yes, with those fields kept for Rhino to work out (a field naming a layout names its page) |
| Materials, render settings | no | no |

## 3. Commands and menus

- **File → Open** takes `.3dm` as well as `.curva`; the model opens as a new, unsaved Curva model
  with the file's name.
- **Import** adds a `.3dm` (or `.curva`) to the open model, its layers merged by name.
- **Export** writes the model, or **ExportSelected** the selection, as `.3dm`.
- **SaveAs** offers `.3dm` beside `.curva`, with a note that Curva-only data (live booleans, History)
  is saved as plain geometry.
- A report after each read: what was brought in, and what was skipped and why.

## 4. How it is checked

- A set of test files saved from Rhino 7 and 8 (made by the owner, or from openNURBS' public examples
  where their licence allows) covering every row of §2, read and compared with known values (counts,
  lengths, areas, volumes, layer names).
- Round trips: Curva → `.3dm` → Curva gives the same model; `.3dm` → Curva → `.3dm` → Curva too.
- Files written by Curva open in Rhino (the owner's check, on a Rhino installation).
- Damaged or unknown files are refused with a message, never a crash (fuzz tests on the reader).
