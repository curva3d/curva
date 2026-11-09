# Spec: Milestone 1, Rotate, Scale, Mirror, Arc and Rectangle

**Status:** Approved by the owner in chat · 9 November 2026 ("keep going", after the plan of these commands)
**Scope comes from:** architecture section 10, Milestone 1, phase F1 commands.

## 1. Commands

| Command | Alias | Asks for | Does |
|---|---|---|---|
| `Rotate` | `ro` | Objects, centre, then an angle in degrees or two reference points | Turns objects on the plane of the last-used viewport |
| `Scale` | `sc` | Objects, origin, then a factor or two reference points | Resizes evenly in all directions |
| `Mirror` | `mi` | Objects, then the two ends of a mirror line | Adds a mirror image and keeps the originals, as Rhino does by default |
| `Arc` | `a` | Centre, start, end | Draws counterclockwise from start to end |
| `Rectangle` | `rec` | Two opposite corners | Draws a closed polyline on the plane |

Curves made flat on a plane use the plane of the viewport where their first point was picked.

## 2. Not in this piece of work

The `Copy` option of Rotate and Scale, uneven scaling (`Scale1D`, `Scale2D`), and other ways of starting an arc or rectangle.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover exact results for each command, turning on a chosen plane, invalid input leaving the model unchanged, and snapping to arcs.

**The owner's check:** he tries each command by hand and says whether it behaves like Rhino.
