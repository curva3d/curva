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
| DXF | The model or a layout in 2D or 3D: curves as lines, arcs, polylines and splines; text and dimensions as DXF annotation; hatches; blocks; layers with colour, line type and line weight | Curves, text, dimensions, hatches, blocks and layers, into the model or onto a layout |

- Export and Import with these types work through the existing Export, ExportSelected, Import and
  Open commands (as `.3dm` does), with Rhino's options for each format.
- DXF R2000 (AC1015) by default, as Rhino's default scheme, with R12 and R2018 as options.
- **Libraries** (decision 0020, point 7): `pdf-writer` and `subsetter` for PDF, `dxf` for DXF, SVG
  written by Curva. Each passes `cargo-deny` before it is added; their notices go into
  `THIRD-PARTY-LICENSES.html`.
- **No DWG** in 0.4.3 (decision 0020, point 8).

## 4. `.3dm` files

Curva's `.3dm` reader and writer (`3dm-files.md`) gain Rhino's drawing objects: layouts (pages)
with their paper size, detail views with camera, scale and lock, per-detail layer visibility, text,
dimensions of every kind, leaders, dots, hatches with their patterns, annotation styles, section
styles, and text fields. A drawing made in Curva opens in Rhino with its sheets, and the other way
round; the round-trip tests in `curva-3dm` cover each object.

## 5. Checks

Tests for each format: a known drawing exported, read back (PDF with a pure-Rust reader in tests
only, SVG as XML, DXF with the `dxf` crate) and compared; a printed line measured at its print
width; fonts embedded; a DXF from Rhino and one from another CAD program imported. In the app: print
a layout with a shaded and a Technical detail to PDF and check it at 100% against a ruler; open a
Curva drawing's `.3dm` in Rhino (`docs/releasing.md`).
