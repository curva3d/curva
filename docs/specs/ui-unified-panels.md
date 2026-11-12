# Spec: one panel system for everything

**Status:** Approved by the owner in chat · 12 November 2026. Second in the order of work (decision 0017).
**Covers:** feedback items 15 to 25 (`docs/specs/feedback-0.3.0.md`). Builds on `panel-system.md`.

## 1. Every piece is a panel

The pieces are: the top bar, Commands (the command tabs and buttons), Layers, Properties, Command, AI
Assistant, Display (the viewport display mode) and Settings. All are drawn by the same code with
the same frame, glass, title font (Inter 13, the theme's quiet text), text size (Inter 14, the theme's
text) and controls. Settings opens as a panel, floating in the centre of Curva's window, and can
be docked or merged like any other. Any panel that pops up does so in the centre of the window.

## 2. The top bar

- Named **Curva**, always shown, fixed at the top, floating with the same 24-point gap as every panel.
  Only focus mode hides it. It cannot be moved, merged, closed or hidden on its own.
- Centred on its first row: the model's file name (or "Untitled") and "Curva".
- Second row, at the left: **File** and **Settings** menus. Centred under the name: the
  **specializations dial**: Freeform, Architecture, Publisher, Analysis, in bold. The selected one sits
  in the centre, the others either side, fading; clicking one or dragging/scrolling the row turns the
  dial, which loops. Architecture, Publisher and Analysis show "Coming in a later milestone" in the
  Commands panel.
- **Commands** becomes its own panel, docked under the top bar by default, holding the selected
  specialization's tabs and buttons.

## 3. Tabs

Tabs within a panel, and the Commands panel's tabs, can be dragged to reorder them. The order is kept in
the layout file.

## 4. Closing, hiding and restoring

- A **cross** at each panel's top-right (left of the hide V) closes it. A small panel in the centre asks
  "Close Layers?" (or every name, for a panel with tabs), says "You can bring it back from Settings →
  Interface", and offers Close and Cancel. Closing a panel with tabs closes all of them.
- The **hide V** points towards the edge the panel will slide to (down for a bottom panel, right for a
  right one, and so on).
- **Settings → Interface** lists every piece with a toggle to show or close it, and **Reset interface**.

## 5. Settings

Tabs: **General** (theme, glass or solid, units, grid, check for updates on opening), **Interface**
(§4), **AI Assistant** (`ai-assistant.md`), and **About** last (version, licence, credits). File → About
opens Settings at the About tab.

## 6. Smaller pieces

- **Display:** the viewport's display mode button (Wireframe, Shaded, Ghosted) is its own small panel,
  docked below the top-left panel by default, its label followed by a V pointing down.
- **Name and version** ("Curva 0.3.1") show in small quiet text at the window's bottom right.
- **Glass** is 25% more see-through: the tint's opacity drops by a quarter in both themes.

## 7. How it is checked

- Automatic, on the layout model: every piece in exactly one panel or closed; closing and restoring;
  tab reordering kept and saved; the top bar never movable; pop-up panels centred; the V's direction for
  each edge.
- **The owner's check:** arranging, closing and restoring panels; the dial.
