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
| Object names, colours by layer or object, print widths, user text | yes | yes |
| Groups, locked and hidden objects | yes | yes |
| Blocks: definitions, instances, linked definitions | yes | yes |
| Named views and construction planes | yes | yes |
| Meshes, SubD, point clouds | listed and skipped, with a count in the command bar | no |
| Annotations, hatches, layouts, materials, render settings | skipped, counted (Milestone 3 and later) | no |

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
