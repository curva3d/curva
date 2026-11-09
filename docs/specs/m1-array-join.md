# Spec: Milestone 1, Array and Join

**Status:** Approved by the owner in chat · 9 November 2026 ("A, keep going", after the plan of these commands)
**Scope comes from:** architecture section 10, Milestone 1, phase F1 commands.

## 1. Commands

| Command | Alias | Asks for | Does |
|---|---|---|---|
| `Array` | `ar` | Objects, how many in X, Y and Z, then the spacing of each direction used | Copies objects into rows, columns and levels, with a live preview of what is typed |
| `Join` | `j` | Curves | Makes curves that meet end to end into one curve |

Lines and open polylines join into a polyline, closed if they form a loop. Smooth curves join into a NURBS curve. Pieces are found in any order and flipped as needed.

## 2. Not in this piece of work

Joining a straight curve to a smooth one, or joining arcs, which needs a mixed curve kind. Picking array spacing with clicks. `ArrayPolar` and `ArrayCrv`.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover array positions and limits, joined lengths adding up, flipped pieces, closing a loop, and undo.

**The owner's check:** he tries each command by hand and says whether it behaves like Rhino.
