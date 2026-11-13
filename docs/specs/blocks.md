# Spec: blocks

**Status:** Draft, for the owner's approval · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, point 2; decision 0019 (3).

## 1. What they are

As in Rhino: a **block definition** is a named group of objects with a base point; a **block
instance** is a placed copy (position, rotation, scale) that draws the definition. Editing the
definition changes every instance. The object model was built for this from Milestone 1
(`architecture.md` §5b).

- **Embedded:** the definition is stored in the file.
- **Linked:** the definition is another `.curva` or `.3dm` file, read when the model opens and when
  that file changes on disk (checked when Curva gets focus), so every instance updates.

## 2. Commands (Rhino's names and steps)

| Command | What it does |
|---|---|
| Block | Makes a definition from objects: base point, name; the objects become one instance |
| Insert | Places an instance of a definition, or of a file (embedded or linked): point, scale, rotation |
| BlockManager | Opens the Blocks panel |
| BlockEdit | Edits a definition in place: the other objects dim, the definition's objects can be edited, Enter saves |
| ExplodeBlock | Turns instances back into plain objects (Explode on an instance does the same) |
| ReplaceBlock | Swaps instances to another definition |
| SelBlockInstanceNamed | Selects the instances of a definition |

## 3. The Blocks panel

A panel like Layers: each definition with a thumbnail, its name, how many instances, and whether it
is embedded or linked (with its file); buttons to insert, edit, rename, delete (only when unused),
update a linked one now, embed a linked one, and select its instances. Nested blocks show as a tree.

## 4. Behaviour

- Instances snap at their definition's snap points, pick as one object, and take the instance's
  layer and print width for objects on the definition's "by parent" layer (Rhino's rule).
- Nested blocks work; a definition cannot contain itself (refused with a message).
- Booleans and other solid commands treat an instance as its geometry, after asking whether to
  explode it (Rhino's prompt).

## 5. Files

Saved in `.curva` with definitions once and instances as references; `.3dm` blocks import and export
as blocks (`3dm-files.md`). Older files open unchanged.

## 6. Checks

Tests for each command and option, nested blocks, the self-reference refusal, a linked file changing
on disk, undo, and files round-tripping; in the app, a chair made a block, placed six times, edited
once.
