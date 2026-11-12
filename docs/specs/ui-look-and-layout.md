# Spec: the look and layout, second round

**Status:** Draft for the owner's approval · 12 November 2026. Third in the order of work (decision 0016).
**Covers:** feedback items 5, 10, 11, 12, 13, 14, 15, 16, 21, 23 (pointers), 24 and 25
(`docs/specs/feedback-2026-11-12.md`). It builds on the floating panels (`ui-floating-panels.md`).

## 1. Colours, font, light and dark (items 12, 14, 16)

- **Two themes**, chosen by the system at start, with Light or Dark forced from Settings (decision 0016).
  Each covers the panels, text, controls, the 3D background, the grid and the previews.
- **Greys and text in the Claude app's manner:** warm, soft greys rather than blue-greys.

  | | Dark | Light |
  |---|---|---|
  | Window and 3D background | `#262624`, the 3D view shading to `#1f1f1d` at the bottom | `#f5f4ef`, shading to `#ecebe5` |
  | Panels | `#30302e`, edge `#45443f` | `#faf9f5`, edge `#e5e3da` |
  | Text | `#f5f4ef`, quieter text `#a6a39b` | `#1f1e1d`, quieter text `#6b6a64` |

  The values are a starting point, tuned by the owner's eye once it can be tried.
- **One accent, orange `#e0782f`** (decision 0016): selected objects, picked edges, faces and control
  points, previews, sticky snaps, the active tab, ticked toggles, focused fields and highlighted list rows.
- **Inter** for the interface (decision 0016), bundled with its SIL Open Font Licence in `licenses/fonts/`.
  The command history keeps a monospace font, so typed values line up.

## 2. A compact command bar and more air (items 10, 11)

- **The command bar** sits centred at the bottom, at most 720 points wide (narrower in a small window). It
  holds the history, the prompt and typed line, and under them the snaps, Ortho, Grid snap, Gumball and the
  selection filter when shown. It stays frosted glass.
- **The agent connection** moves to its own small panel at the bottom right (decision 0016): Agent access
  and the status. It is a panel like the others: dockable, hidden in focus mode, and in the layout file.
- **Space between panels and from the window's edges** triples, from 8 to 24 points.

## 3. Resizing panels and pointers (items 13, 23)

- Near any edge of a panel the pointer becomes a resize arrow, and that edge pulses gently in the accent.
  Dragging an edge resizes that side only; a corner, its two sides. A docked panel resizes from its inner
  edge (and, for a top or bottom panel, its ends). A floating one, from every edge and corner.
- Sizes are kept in the layout file and never go below a usable minimum.
- **Pointer set** for the whole interface, together with the command steps' pointers
  (`command-framework.md` §4): resize arrows on panel edges, a grab hand on grips and gumball handles, a
  crosshair for point steps, a pointer with a box for picking, and a text cursor in fields.

## 4. The File tab (item 15)

A first tab, **File** (decision 0016), before Curves: New, Open, Save, Save As, Import, Export, Settings and
About, with Ctrl+N, Ctrl+O, Ctrl+S and Ctrl+Shift+S. Import and Export show but stay disabled, saying "Rhino
files (.3dm) come later in Milestone 2", until there is a format to offer. **Settings** opens a small window:
theme (System, Light, Dark), units (§7), and grid shown or not. **About** gives the version, the licence
and the credits from `ATTRIBUTION.md`.

## 5. Icons on the command buttons (item 21)

The command buttons show icons with the command's name on hover. For now they are Serpentine3D's toolbar
icons, which are drawn in code rather than stored as pictures. They are redrawn in Rust with the same
shapes and credited in `ATTRIBUTION.md` (MIT). They take the theme's text colour, with the accent where
Serpentine3D uses its own. A command Serpentine3D has no icon for shows its name. Curva's own icon
language comes later.

## 6. A readable suggestion list (item 5)

The list of commands matching what is typed is as wide as its longest name plus padding, one name per row,
never cut. It sits above the typed text with the highlighted row in the accent, and shows up to ten rows,
scrolling past that.

## 7. Units: millimetres or metres (item 24)

A model is in millimetres or metres, saved in its file; new models start in millimetres. Typed values,
the cursor tag, the properties panel and measurements use the model's unit, and show it (`mm`, `m`).
Changing the unit in Settings asks, as Rhino does, whether to scale the model to keep its real size or
keep the numbers as they are.

## 8. A grid like Serpentine3D's (item 25)

As Serpentine3D draws it: a minor line every unit and a major line every 10, out to 100 each way. The
lines are faint and see-through (about 5% for minor and 11% for major in dark mode, dark lines in light
mode), with softened red and green axes, over the 3D view's top-to-bottom shading. Grid snap uses the
minor step.

## 9. Order of work

1. Themes, colours, Inter, the accent everywhere, the grid and background.
2. The compact command bar, the agent panel, the wider spacing and the suggestion list.
3. Resizing from edges and corners, and the pointer set.
4. The File tab, Settings, About and units.
5. Serpentine3D's icons on the buttons.

## 10. How it is checked

- Automatic:
  - both themes give every colour a value, and the accent is the same everywhere
  - the layout keeps 24 points between panels, and the command bar's width limit and centring hold
  - resizing from each edge changes that side only, and from corners their two sides
  - the suggestion list is as wide as its longest name
  - the unit round-trips through a file, and scaling on a unit change is exact
  - every command button names a real command
- **The owner's check:** both themes by eye, on Windows and over Remote Desktop.
