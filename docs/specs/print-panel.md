# Plan: reorganising the print panel

**Status:** Draft for the owner's discussion · 21 November 2026 (features stream, #491).
**Covers:** the owner's note of 21 November 2026: "print ui panel is not good, let's research how
Archicad and Vectorworks and Rhino do it and suggest how to organise the print ui panel".
**Builds on:** `drawing-output.md` §1 (approved: Rhino 8's Print dialog as a panel).

## 0. In short

Today's panel is a list of layouts with ticks, two choices (colour or black; PDF or printer), a
Print button and a small preview. It is missing what an architect reaches for every week (saved
sets, one PDF or one per sheet, line width scale, file names) and it slowed the app on large files
(#474, fixed separately: the preview now draws once, on its own thread).

The proposal: **a big preview on the left, the settings in four short sections on the right, and
saved print sets at the top**: Rhino's settings, Vectorworks' and Archicad's saved sets.

## 1. How the others do it

| | Rhino 8 | Archicad | Vectorworks | What Curva takes |
|---|---|---|---|---|
| Shape | One dialog: destination, size, scale, colour, visibility, line types, margins, with a preview that fills most of it | Print for the current layout; **Publisher** for many layouts and views at once | Print for the current sheet; **Publish** for many sheets, views and worksheets, from this file or others in a folder | One panel that does both: one layout or many |
| Choosing what | Current, or **multiple layouts** ticked in a list; Print from the Layouts panel | **Publisher Sets**: a saved tree of layouts and views | **Publish list**, saved as **sets** in the file (with order, output and colour) | **Print sets**, saved in the model |
| Where it goes | Printer, PDF, image (raster or vector) | Printer, plotter, PDF, DWG, files in a folder | Print, PDF, DWG, image, per item | PDF (one file, or one per layout), printer, image; later DWG per item |
| How | Colour as displayed / print colour / black and white; line width scale; default line width; hidden objects | Pen set, colour, scale, area | Colour, black and white, resolution | Colour, greyscale, black; line width scale; layers |
| File names | One name | Per item, from the layout's number and name | Prefix and per-item name | From number and name, with a prefix |

Sources: [Rhino 8 Print](https://docs.mcneel.com/rhino/8/help/en-us/commands/print.htm),
[Rhino Print dialog FAQ](https://www.wiki.mcneel.com/_export/xhtml/print_dialog),
[Archicad Publisher Sets](https://help.graphisoft.com/AC/27/INT/_AC27_Help/030_Interaction/030_Interaction-3.htm),
[Vectorworks 2025: batch publishing](https://app-help.vectorworks.net/2025/eng/VW2025_Guide/ConnectCAD/..%5CPrintPublish%5CBatch%20publishing.htm).

## 2. The proposed panel

It opens wide (a large panel size from #488), because the preview is the point of it.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Print set: [ Licensing – all sheets ▾ ]  Save set  ·  Save as…            │
├───────────────────────────────────────┬──────────────────────────────────┤
│                                       │ 1 WHAT                           │
│                                       │   ☑ A-101 Ground floor   1:50   │
│        page preview, large            │   ☑ A-102 First floor    1:50   │
│        zoom with the wheel,           │   ☐ A-201 Sections       1:100  │
│        ◀ 2 / 7 ▶ under it             │   All · None · Open layout only  │
│                                       │ 2 WHERE                          │
│                                       │   ◉ PDF  ○ Printer  ○ Image      │
│                                       │   ◉ One file  ○ One per layout   │
│                                       │   Name: [Proj]_[number]_[name]   │
│                                       │ 3 HOW                            │
│                                       │   Colour ◉ As drawn ○ Grey ○ Black│
│                                       │   Line widths × [1.00]           │
│                                       │   ☐ Print hidden layers          │
│                                       │ 4 PAPER (printer and image only)  │
│                                       │   Size, fit or 1:1, margins, DPI │
│                                       ├──────────────────────────────────┤
│                                       │            [ Print 2 layouts ]   │
└───────────────────────────────────────┴──────────────────────────────────┘
```

- **Print sets** (top): a saved choice of layouts and settings, kept in the model, as Archicad's
  Publisher Sets and Vectorworks' publish sets. "Licensing – all sheets", "Client – plans only". The
  set's name is the panel's title; changing anything shows it as changed until saved.
- **1 What:** the layouts in the Layouts panel's order and folders, each with its number, name and
  main scale. A folder ticks all its layouts. All, None, and "Open layout only".
- **2 Where:** PDF (one file, or one per layout, named from a pattern of project, number and name),
  printer (the system dialog picks the printer, as today), or image (PNG at a DPI). DWG per layout
  later, with `export-formats.md`.
- **3 How:** colour as drawn, greyscale or black; line width scale (Rhino's); hidden layers printed
  or not.
- **4 Paper:** only shown for the printer and images: fit to paper or 1:1, margins, DPI. A layout
  printed to PDF is always 1:1 on its own paper, so the section stays hidden then.
- **Preview** (left, large): the page exactly as it will print, zoomable, stepping through the
  ticked layouts. Drawn on its own thread and kept until something changes (#474), with "Drawing the
  preview…" while it draws, so it never slows the viewport.
- **One button** that says what it will do: "Print 2 layouts", "Save 7 PDFs".

## 3. Steps

1. The layout and preview: wide panel, large preview, the four sections (today's choices moved in).
2. One file or one per layout, and the name pattern.
3. Print sets saved in the model.
4. Line width scale, greyscale, hidden layers, paper section, image output.

## 4. Decisions for the owner

1. **One panel for one and many** (the proposal), or Rhino-style Print plus a separate Archicad-style
   Publisher. *Recommended*: one panel; sets make the "many" case easy without a second tool.
2. **Preview on the left, large**, or on top as today. *Recommended*: left and large.
3. **Print sets in the first step**, or after the layout change. *Recommended*: after (step 3), so the
   new layout lands first.

### Owner's answers (26 November 2026)

1. **One panel** for one layout and many, as recommended.
2. **Preview on the left, large**, as recommended.
3. **Print sets after the layout change** (step 3), as recommended.
