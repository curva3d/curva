# Spec: Milestone 1, layers and properties panels

**Status:** Approved by the owner in chat · 9 November 2026 (the plan this was part of; he asked for development to continue without being asked)
**Scope comes from:** architecture section 10, Milestone 1.

## 1. What the owner will be able to do

A panel on the right of the window holds:

- **Layers:** choose the current layer, rename, pick a colour, turn on or off, lock, delete and add. Objects are drawn in their layer's colour.
- **Properties:** for the selection, how many are selected, the kind and length of a single curve, and which layer it is on, with a list to move it to another.

Hidden layers are not drawn. Locked layers are drawn grey and cannot be selected, but can still be snapped to, as in Rhino. The current layer cannot be hidden or locked, and a layer in use cannot be deleted.

Copies, mirror images, arrays, offsets, cut pieces, joined curves and fillets stay on the layer of the curve they came from.

## 2. Not in this piece of work

Sub-layers, per-object colour, line types and print settings. Layer commands on the command line.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover each layer rule, hidden and locked drawing and picking, colours, older files opening on the default layer, and new curves keeping their source layer.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
