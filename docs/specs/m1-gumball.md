# Spec: Milestone 1, gumball

**Status:** Approved by the owner in chat · 9 November 2026 (the plan this was part of; he asked for development to continue without being asked)
**Scope comes from:** architecture section 10, Milestone 1.

## 1. What the owner will be able to do

- Select objects, and see a gumball at the middle of the selection whenever no command is running.
- Drag an arrow to move along X, Y or Z, or an arc to turn around that axis. The result is previewed while dragging, drawn like every other preview, and is one undo step.
- Turn the gumball off with the checkbox under the command line.

## 2. Not in this piece of work

Scale handles, which need uneven scaling. Typing an exact value during a drag, snapping a gumball drag, and aligning the gumball to a construction plane or object.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover finding each handle, the gumball keeping its size on screen, a drag moving only along its axis, the direction of a turn, and turning about the gumball's centre.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
