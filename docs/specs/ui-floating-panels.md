# Spec: floating panels

**Status:** Draft for the owner · 12 November 2026. Asked for by the owner in chat, to follow the close of
Milestone 1: "modern and clean", floating panels with rounded corners, a frosted-glass command bar, and
a focus mode.
**Scope comes from:** the owner's description, and how SolidCraft (`tangency-labs/solidcraft`, Apache-2.0)
draws its floating cards and glass.

## 1. What the owner will see

The viewports fill the whole window. The interface floats above them in three panels with rounded
corners, a small gap from the window's edges and a soft shadow:

| Panel | Where | Holds | Look |
|---|---|---|---|
| Top bar | One long bar across the top | The file's name, the snaps (End, Mid, Cen, Int, Perp, Near), Ortho, Grid snap, Gumball, Agent access, and the selection filter when shown | Solid |
| Side panel | Tall, down the right | Tabs: Layers, Properties, and later Layouts (Milestone 3) | Solid |
| Command bar | Along the bottom | The command history, the prompt and the typed line, with its suggestions | **Frosted glass**: what is behind it shows through, blurred, under a light tint |

The panels sit over the viewports rather than taking space from them, so the model can be seen right
to the window's edges. Clicks on a panel go to the panel; clicks anywhere else go to the viewport, as now.

### Focus mode

- A small tab with a V-shaped arrow sits at the top centre. Clicking it (or a key, `F12` suggested)
  slides **all** the panels out of view, each towards its nearest edge: the top bar up, the side
  panel right, the command bar down. Clicking it again brings them all back.
- When hidden, a thin strip of each panel stays visible at its edge, so it can be found again.
- Hovering over a strip slides that panel out a little, as a hint. Moving away slides it back.
- Clicking a strip brings that one panel fully into view, leaving the others hidden. Clicking the
  arrow tab, or the strip again, hides it.
- Typing still goes to the command line while the command bar is hidden: a typed letter brings the
  command bar out until the command ends.
- The slides take about 0.2 seconds and ease in and out.

## 2. How the glass is made

The command bar blurs the viewport behind it on the graphics card: the viewport is drawn, the part
under the bar is blurred in a few small passes (the "dual Kawase" blur SolidCraft uses), and the bar
is drawn over it with a light tint. Where the graphics card cannot do it (a software adapter, as over
some remote desktops), the bar falls back to the same tint without blur, so it still reads as
lighter glass.

## 3. Decisions for the owner

1. **The blur's code.** (a) Write Curva's own blur, following SolidCraft's method. (b) Adapt
   SolidCraft's code, about 1,400 lines and Apache-2.0, with its notice in `ATTRIBUTION.md`. Decision
   0009 left reuse of SolidCraft's code undecided, so (b) would settle it for this part. Recommended:
   (a). The part Curva needs is small: one surface, the command bar.
2. **The top bar.** The list in section 1 moves what is now under the command line up to the top. Should
   it also have buttons for common commands, as Rhino's toolbars do? Recommended: not yet. Keep it
   clean, and add toolbars as their own piece of work.

## 4. Order of work

1. The panels float over the viewports with rounded corners and the solid style.
2. Focus mode: the arrow tab, the slides, the strips, hovering and clicking.
3. The frosted command bar, with its fallback.

## 5. Not in this piece of work

Moving or docking panels elsewhere, light and dark themes, toolbars of command buttons, and the
built-in assistant panel (postponed by the owner on 10 November 2026).

## 6. How it is checked

- Automatic checks on the layout logic: each panel's rectangle when shown, hidden and peeking, for
  several window sizes, and which edge each one hides towards. A click on a panel never reaches the
  viewport.
- The blur is checked by a picture: a sharp line behind the command bar is softened in it, and is
  unchanged where there is no bar.
- Drawing stays fast: orbiting with the panels shown costs no more than 1 ms a frame over today
  (`--orbit-benchmark`).

**The owner's check:** tries it by hand, including over Remote Desktop, and says whether it looks right.
