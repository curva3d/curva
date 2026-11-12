# Spec: floating panels

**Status:** Approved by the owner in chat · 12 November 2026, with the recommendations taken (Curva's own
blur), command tabs in the top bar, drawing aids and agent access kept at the bottom, and panels that
users can move and dock elsewhere.
**Scope comes from:** the owner's description, and how SolidCraft (`tangency-labs/solidcraft`, Apache-2.0)
draws its floating cards and glass. Only the method is followed; no SolidCraft code is copied.

## 1. What the owner will see

The viewports fill the whole window. The interface floats above them in panels with rounded corners, a
small gap from the window's edges and a soft shadow. Clicks on a panel go to the panel; clicks anywhere
else go to the viewport, as now.

| Panel | Where it starts | Holds | Look |
|---|---|---|---|
| Command tabs | One long bar across the top | Tabs **Curves**, **Surfaces**, **Solids** and **Operations**, each a row of buttons for its most used commands (below) | Solid |
| Side panel | Tall, down the right | Tabs: Layers, Properties, and later Layouts (Milestone 3) | Solid |
| Command bar | Along the bottom | On the left: the command history, the prompt and the typed line, with its suggestions. Below it: the snaps (End, Mid, Cen, Int, Perp, Near), Ortho, Grid snap, Gumball, and the selection filter when shown. On the right: the agent (MCP) connection, with Agent access and whether an agent is connected | **Frosted glass**: what is behind shows through, blurred, under a light tint |

### The command tabs

Each button runs its command, as typing it would, and shows the command's name when hovered. A first
set, to be adjusted once the owner has tried it:

| Tab | Buttons |
|---|---|
| Curves | Line, Polyline, Curve, InterpCrv, Circle, Arc, Ellipse, Rectangle, Point, Offset, Fillet, Chamfer |
| Surfaces | PlanarSrf, ExtrudeCrv, Revolve, Loft, Sweep1, OffsetSrf |
| Solids | Box, Pipe, FilletEdge, ChamferEdge, Shell, Volume |
| Operations | BooleanUnion, BooleanDifference, BooleanIntersection, Trim, Split, Join, Explode, Move, Copy, Rotate, Scale, Mirror, Array |

Buttons show the command's name in text for now; icons come with their own piece of work.

### Moving and docking panels

Users can arrange the interface as they like:
- Each panel has a grip. Dragging it lifts the panel, and the places it can go light up as it nears
  them: the top, bottom, left or right edge of the window, or anywhere as a floating panel.
- Dropping it on a lit place docks it there. A docked panel stretches along its edge, as the command
  tabs do along the top; a floating one keeps its size and can be resized from its corners.
- The arrangement is remembered between sessions, per user (not in the model file). A **Reset layout**
  entry puts everything back as in the table above.
- Panels are built so that every panel, now and later, takes part in this: a panel only says what it
  holds, and the layout decides where it is and how big.

### Focus mode

- A small tab with a V-shaped arrow sits at the top centre. Clicking it (or `F12`) slides **all** the
  panels out of view, each towards the edge it is docked to; a floating panel goes towards its nearest
  edge. Clicking it again brings them all back.
- When hidden, a thin strip of each panel stays visible at its edge, so it can be found again.
- Hovering over a strip slides that panel out a little, as a hint; moving away slides it back.
- Clicking a strip brings that one panel fully into view, leaving the others hidden. Clicking the
  arrow tab, or the strip again, hides it.
- Typing still goes to the command line while the command bar is hidden: a typed letter brings the
  command bar out until the command ends.
- The slides take about 0.2 seconds and ease in and out.

## 2. How the glass is made

The command bar blurs the viewport behind it on the graphics card: the viewport is drawn, the part under
the bar is blurred in a few small passes (the "dual Kawase" blur SolidCraft uses), and the bar is drawn
over it with a light tint. The blur is Curva's own code (owner's choice). Where the graphics card cannot
do it (a software adapter, as over some remote desktops), the bar falls back to the same tint without
blur, so it still reads as lighter glass.

## 3. Order of work

1. The layout model: panels, the places they dock, and the default arrangement, remembered between
   sessions. The panels float over the viewports with rounded corners and the solid style, laid out as
   in section 1, with the command tabs.
2. Focus mode: the arrow tab, the slides, the strips, hovering and clicking.
3. Moving and docking panels by their grips, and Reset layout.
4. The frosted command bar, with its fallback.

## 4. Not in this piece of work

Icons for the command buttons, users choosing which buttons a tab holds, light and dark themes, and the
built-in assistant panel (postponed by the owner on 10 November 2026).

## 5. How it is checked

- Automatic checks on the layout logic: each panel's rectangle when shown, hidden and peeking, for several
  window sizes and every place a panel can dock, and which edge each one hides towards. A layout saved and
  read back is the same. A click on a panel never reaches the viewport.
- Each command button runs the command it names.
- The blur is checked by a picture: a sharp line behind the command bar is softened in it, and is
  unchanged where there is no bar.
- Drawing stays fast: orbiting with the panels shown costs no more than 1 ms a frame over today
  (`--orbit-benchmark`).

**The owner's check:** tries it by hand, including over Remote Desktop, and says whether it looks right.
