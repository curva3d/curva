# Spec: Milestone 1, history on curves

**Status:** Approved by the owner in chat · 9 November 2026 (the plan this was part of; he asked for development to continue without being asked)
**Scope comes from:** architecture section 10, Milestone 1: "curves built from other curves update when their inputs change".

## 1. What the owner will be able to do

- Type `History` to turn recording on or off. It starts off.
- While it is on, a new `Mirror` or `Offset` remembers the curve it came from, and updates when that curve changes.

## 2. How it behaves

- An offset keeps its distance and side.
- Editing a child directly, such as moving it, breaks its link and it stays where it is put, as in Rhino.
- Deleting the source leaves the child in place and forgets the link.
- A chain of children updates in one pass, and a change and its updates are one undo step.
- Links are saved in the file. Files saved earlier open unchanged.

## 3. Not in this piece of work

History for commands other than Mirror and Offset, the warning Rhino shows when a link breaks, and multicore recompute, which matters once recipes get expensive.

## 4. How it is checked

No Rhino tests (decision 0011). Automatic checks cover a child following its source, recording off, a deleted source, a broken link, chains, undo and saving.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
