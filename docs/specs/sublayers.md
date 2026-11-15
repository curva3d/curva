# Spec: sublayers

**Status:** Asked for by the owner in chat · 15 November 2026 ("We need to add sublayers to curva then").
**Why now:** Make2D's layers (`make2d.md` §1) are a tree in Rhino, and drawings need layer trees.

Written from Rhino 8's `commands/layer.htm`. Its naming-conventions and layers pages refused to load
(HTTP 403), so the full-name form and what deleting a parent does follow Rhino's known behaviour,
noted below.

## 1. What a sublayer is

- A layer may have a **parent** layer; layers without one are top-level. Any depth.
- **Full name:** the names down the tree joined by `::` (`Make2D::Visible::Curves`), as Rhino writes
  them. Names must differ among layers with the same parent, as Rhino requires.
- **On and Lock:** each layer keeps its own setting. A layer shows only when it and all its parents
  are on, and can be picked only when none of them is locked. While a parent is off or locked, its
  children show it (half-drawn tick in the panel) and cannot be turned on or unlocked on their own,
  as in Rhino.
- **Current layer:** any layer can be current; making it current turns it and its parents on and
  unlocks them. The current layer's parents cannot be turned off or locked.
- **Deleting** a layer deletes its sublayers with it (Rhino's behaviour). As now, nothing is deleted
  when any of them holds objects or is current.
- **Per detail:** hiding a parent in a detail hides its children there too.

## 2. The Layers panel

- Layers show as a tree under their parents, indented, with a triangle to expand or collapse
  (Rhino's Expand and Collapse Sublayers). Whether a layer is expanded is saved with it.
- **New sublayer** (button, and in a layer's menu) adds a child to the current layer.
- **Drag** a layer's handle onto another to make it a sublayer, or onto the panel's top line to make
  it top-level; **Move up one parent** in the layer's menu.
- **Select objects** and **Select sublayer objects** in the layer's menu.

## 3. Elsewhere

- `ChangeLayer` and other commands that take a layer name take a full name (`Walls::Exterior`) or, when
  only one layer has it, a plain name.
- `.3dm` files keep the tree both ways (Rhino's parent layer id).
- The assistant's layer list gives each layer's full name and parent.
- Make2D puts its output under `Make2D` > `Visible` / `Hidden` > `Curves` / `Silhouettes`.

## 4. Checks

Tests for visibility and locking through parents, the current layer's parents, full names and lookup,
reparenting (no loops), deleting a tree, the `.3dm` round trip of a tree, and Make2D's tree. In the app:
make a tree, collapse it, turn a parent off and see its children's objects hide.
