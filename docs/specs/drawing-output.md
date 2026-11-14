# Spec: drawing output

**Status:** Draft for the owner's approval.
**Covers:** `milestone-3-in-0.4.3.md`, points 14 to 16; decision 0020 (7, 8).

## 1. Print

**Print** (Rhino 8's Print dialog, as a panel): choose what to print, where, and how.

- **What:** the current layout, chosen layouts or all of them (in the Layouts panel's order), the
  current view, or a window of it.
- **Where:** a PDF file (all chosen layouts in one PDF, or one file each), an image (PNG, at a chosen
  DPI), or a printer.
- **How:** scale (a layout prints 1:1 on its paper size; a view prints at a scale or fitted), print
  widths on or off with a scale for them, colour, greyscale or black and white, line types, margins,
  and which layers (hidden layers don't print, as in Rhino).
- **Preview:** the panel shows the page as it will print.
- **Printers:** Curva writes a PDF and hands it to the system's print command (Windows `print` verb,
  macOS and Linux `lp`), so no `unsafe` code is needed (decision 0020, point 7). The printer and
  paper are chosen in the system's dialog.
- **Serpentine aliases:** PrintSheets, Plot.

## 2. What goes on paper

- **Vectors:** curves, edges, Technical and Hidden details (with exact hidden lines, `make2d.md` §2),
  text (real text in the PDF, searchable, with the fonts embedded as subsets), dimensions, hatches
  (as fills and line patterns), section fills.
- **Raster:** shaded, rendered, ghosted and X-ray details are rendered at the print DPI and placed
  as images, as Rhino does; curves and annotation over them stay vectors.
- **Print widths and line types** from the object, its layer or the style, in millimetres on paper.
  "Hairline" prints as the thinnest line the device can draw.

## 3. Export and import

| Format | Export | Import |
|---|---|---|
| PDF | Layouts or views, as in §1; ExportPDF (Serpentine's command) is Print to a PDF | — |
| SVG | One file per layout or view, vectors with raster details embedded | — |
| DXF and DWG | The model or a layout in 2D or 3D: curves as lines, arcs, polylines and splines; text and dimensions as DXF annotation; hatches; blocks; layers with colour, line type and line weight | Curves, text, dimensions, hatches, blocks and layers, into the model or onto a layout |

- Export and Import with these types work through the existing Export, ExportSelected, Import and
  Open commands (as `.3dm` does), with Rhino's options for each format.
- DXF and DWG R2000 (AC1015) by default, as Rhino's default scheme, with later versions up to 2018 as
  options. Open takes DWG and DXF as Rhino does (into a new model).
- Each library passes `cargo-deny` before it is added; its notice goes into
  `THIRD-PARTY-LICENSES.html`. Each format is its own crate behind the format interface
  (`docs/reviews/code-health-2026-11-14.md`, fix 6): `curva-dxf` (DXF and DWG) and `curva-pdf` (PDF
  and SVG).

## 4. Prior work checked

Checked on crates.io on 14 November 2026 (decision 0021, point 7):

| Need | Found | Licence, activity | Verdict |
|---|---|---|---|
| PDF | `krilla` 0.8 (from the Typst project, built on `pdf-writer` and `subsetter`) | MIT OR Apache-2.0, updated June 2026, 2.5M recent downloads | **Use**: pages, vector paths, embedded and subset fonts, images, in one crate |
| PDF, lower level | `pdf-writer` 0.15; `printpdf` 0.12 | MIT OR Apache-2.0; MIT | Not needed directly: `krilla` covers them |
| DXF and DWG | `acadrust` 0.6.8 | MPL-2.0 (exception approved, decision 0021), updated October 2026, 117k recent downloads | **Use** for both |
| DXF only | `dxf` 0.6.2 | MIT, updated October 2026 | Fallback if `acadrust` stops being maintained |
| DWG read | `cadkit-dwg` 0.5, `ezdwg-core` 0.20 | MIT OR Apache-2.0; MIT; both a few weeks old | Fallback readers; too young to rely on |
| SVG writing | `svg` 0.18 | MIT OR Apache-2.0, last updated September 2024 | **Not used**: writing SVG paths is a few dozen lines; Curva writes it |
| SVG reading (tests) | `usvg` 0.50 | MIT OR Apache-2.0, updated November 2026 | **Use** in tests only, to read exports back |
| Printing | none in pure Rust | — | The system print command with a PDF, as decided |

## 5. `.3dm` files

Curva's `.3dm` reader and writer (`3dm-files.md`) gain Rhino's drawing objects: layouts (pages)
with their paper size, detail views with camera, scale and lock, per-detail layer visibility, text,
dimensions of every kind, leaders, dots, hatches with their patterns, annotation styles, section
styles, and text fields. A drawing made in Curva opens in Rhino with its sheets, and the other way
round; the round-trip tests in `curva-3dm` cover each object.

## 6. Checks

Tests for each format: a known drawing exported, read back (PDF with a pure-Rust reader in tests
only, SVG with `usvg`, DXF and DWG with `acadrust`) and compared; a printed line measured at its print
width; fonts embedded; a DXF and a DWG from Rhino and from AutoCAD imported. In the app: print
a layout with a shaded and a Technical detail to PDF and check it at 100% against a ruler; open a
Curva drawing's `.3dm` in Rhino (`docs/releasing.md`).
