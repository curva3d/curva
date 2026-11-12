# Spec: fixes after 0.3.0

**Status:** Approved by the owner in chat · 12 November 2026. First in the order of work (decision 0017).
**Covers:** feedback items 1 to 14 (`docs/specs/feedback-0.3.0.md`).

## What changes

1. **Ghosted snaps like Wireframe.** Only Shaded hides snap points behind faces.
2. **Escape clears the selection** when no command is running and the command line is empty.
3. **Opens filling its screen:** the window starts maximised on the screen it opens on.
4. **Snaps on curves and polylines.** Find why only bodies snap since 0.3.0 and fix it. New tests run
   the app's own aim path (not only the viewport crate) on a line, a polyline, an arc and a box, in
   each display mode, and the release checklist (`docs/releasing.md`) gains a hand check of snapping
   on curves.
5. **Escape keeps what a command made.** Escape ends the command; objects a command already put in the
   model (Copy's placed copies, any recipe that places as it goes) stay, as in Rhino, as one undo step.
   A command that has placed nothing changes nothing.
6. **Previews while the view turns.** Box and extrusion heights that follow the mouse along a line
   lock to the view they started in: turning the view keeps the height, and the next mouse move picks
   it up again from the new view.
7. **Thicker lines:** curves, polylines and body edges draw at twice today's width.
8. **The orange line on the top panel:** the resize handle's pulse drew along an edge it should not;
   only the edge being resized pulses, and only within the panel's own rectangle.
9. **Options in multi-step commands.** The options shown at a prompt can be clicked (they become
   buttons in the prompt line) and typed (full name or a unique start) at every step of every
   command. A test types and clicks each option of every recipe command.
10. **The pointer shows typed values.** While a number is typed for a size or distance, the tag beside
    the pointer shows that number, not the mouse's.
11. **Resizing panels selects nothing:** a drag that starts on a panel's edge or corner never reaches
    the viewport behind.
12. **FilletEdge and ChamferEdge previews:** while a preview shows, the original solid is drawn as
    see-through edges only, so the rounded or bevelled result is visible.
13. **Clipboard and common shortcuts:** Ctrl+C copies the selection, Ctrl+X cuts it, Ctrl+V pastes in
    place and selects the pasted objects, Ctrl+A selects all, Delete deletes; on macOS with Cmd.
    The clipboard is Curva's own, inside Curva.
14. **Dimensions while making:** commands making something with width and depth (Rectangle, Box, and
    the base of Box) show "width × depth" beside the pointer, in the model's unit, and "height" for the
    third step.

## How it is checked

- Automatic: each item above that can be tested headless has a test (snaps through the aim path in
  every mode; Escape keeping placed copies; options clicked and typed at every step; typed values in
  the tag; clipboard round trip; the resize drag not selecting).
- **The owner's check:** snapping on curves, previews while orbiting, Fillet/Chamfer previews.
