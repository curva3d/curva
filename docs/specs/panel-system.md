# Spec: the panel system: titles, hiding, tabs, merging and a placement grid

**Status:** Draft for the owner's approval · 12 November 2026. Part of the look and layout work, third in
the order of work (decision 0016), done before that spec's other steps since they build on it.
**Covers:** feedback items 27 to 31 (`docs/specs/feedback-2026-11-12.md`). Extends the floating panels
(`ui-floating-panels.md`) and the layout model (`crates/curva-app/src/layout.rs`).

## 1. The idea

The interface is made of **pieces**: Layers, Properties, Command, Agent, the command tabs, and later
Layouts and anything else. Each piece lives in a **panel**. A panel holds one piece, or several as tabs.
Users can pull pieces out, put them together, move panels anywhere on a soft grid, and hide them.
Every piece works this way, now and later: a piece only says what it holds and its name. Where it is,
which panel it is in and whether it shows are the layout's, and are remembered between sessions.

## 2. Names and tabs (items 27, 29)

- **Every panel shows its name**, centred along its top. A panel with several pieces shows one tab per
  piece across its top instead, the active tab in the accent orange.
- The **name or tab is the grip:** dragging it moves the panel, or a single tab pulls that piece out of
  its panel (§4). This replaces the separate grip of `ui-floating-panels.md`.
- Tabs within a piece (the File, Curves, Surfaces… tabs of the command tabs piece) look and behave like
  panel tabs, with the same pull-to-show and hide (§3).

## 3. Hiding, one panel or all (item 28)

- Each panel has a small **hide** control at the end of its title (a V, like the focus mode tab). Hiding a
  panel slides it to its nearest edge, as focus mode does for all of them.
- **What stays visible** of a hidden panel, alone or in focus mode, is a **tab** along that edge, showing
  the panel's name (or its pieces' names) written along the panel's longest edge. A tall panel hidden to
  the right shows its name running upwards; a wide panel hidden at the bottom shows it across.
- **Hovering** the tab slides the panel out a little. **Clicking** it brings it fully out. **Dragging** it
  pulls the panel out to where it is dropped. Hiding it again, or focus mode again, slides it back.
- Focus mode (the arrow tab at the top centre, or F12) hides every panel at once and brings back only those
  that were showing, leaving individually hidden panels hidden.

## 4. Pulling pieces out and merging them (item 30)

- **Merging:** dragging a panel (by its name or a tab) over another panel lights the one underneath with an
  orange glow. Dropping it there adds its pieces as tabs. For example, dropping the Command panel on the
  Layers and Properties panel gives it a third tab, Command.
- **Pulling out:** dragging a single tab out of a panel makes that piece a panel of its own, dropped where
  it is let go, on an edge or on the grid (§5). For example, pulling Properties out of that panel leaves
  Layers and Command together.
- Dropping near a window edge docks the panel there, as now, and the place it would take lights up.
- A panel's look follows what it holds. The frosted glass belongs to the Command piece, so a panel shows
  glass while Command is its active tab and the solid look otherwise.
- **Reset layout** puts every piece back in its standard panel and place.

## 5. A soft placement grid (item 31)

Floating panels don't go just anywhere. As on a desktop, they settle on a soft grid:
- Dropped, a floating panel's corner moves to the nearest point of a grid of 24 points (the space between
  panels), and its size rounds to the grid.
- It also lines up with the window's edges and with other panels' edges and centres when within 12
  points, keeping the 24-point gap between neighbours.
- While dragging, the spot it will settle on shows as a faint outline, so the snap is never a surprise.
- Panels never overlap: dropped over free space they settle in the nearest free spot. Dropped onto
  another panel, they merge (§4).

## 6. Order of work

1. The model: pieces, panels holding one or several, docking, the grid, hiding; kept in the layout file,
   and older layout files still read.
2. Names and tabs on every panel, with tabs as grips.
3. Hiding one panel, the edge tabs with names along the longest edge, and focus mode remembering them.
4. Merging with the orange glow, and pulling tabs out.
5. The soft grid with its outline, alignment and no overlaps.

## 7. How it is checked

- Automatic, on the layout model:
  - merging adds the dropped pieces as tabs, and pulling a tab out makes a panel of it, leaving the rest
  - every piece is always in exactly one panel
  - hidden panels leave an edge tab of the right size, along the right edge, with the name along the
    longest side
  - focus mode brings back only the panels that were showing
  - drops settle on the 24-point grid, line up within 12 points, and never overlap
  - the layout file round-trips, and older files read
- **The owner's check:** rearranges panels by hand, merges and splits them, and hides them.
