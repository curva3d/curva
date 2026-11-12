# Spec: command parity with Rhino for NURBS modelling

**Status:** Approved by the owner in chat · 12 November 2026. Sixth in the order of work (decision 0017), and part of the same
version as the other specs from the feedback on 0.3.0.
**Covers:** the owner's addition: "add all the commands we are missing from our list so we have command
parity with Rhino (for NURBS), and the command symbols that are still missing."

## 1. Scope

Every Rhino 8 command for **NURBS modelling**: drawing curves, editing curves, making surfaces, editing
surfaces, solids, solid editing, transforming, selecting, analysing and measuring, construction planes
and views, and the object utilities they lean on (Group, Lock, History).

Not in scope (other specializations or later milestones): meshes and SubD, rendering and materials,
annotation and dimensions, layouts and printing, file formats (Milestone 2), scripting, and
Grasshopper.

## 2. The list

`docs/commands/rhino-parity.md` lists every in-scope Rhino command by group, from the Rhino 8 help, with
its status in Curva: `done`, `partly`, `missing`, or `blocked` (the geometry engine cannot do it yet,
with the upstream report). The existing `docs/commands/rhino-reference.md` keeps the details of each
command Curva has; every command moved from `missing` gets its row there.

## 3. How each command is built

Through the command framework's workflow (`AGENTS.md`, "Writing or completing a command"): Rhino's steps
and options from its help page, a recipe, tests for each option, an icon (Serpentine3D's where it has
one, otherwise Rhino's idea in Curva's style and colours), its row in the reference, and its place in a
Commands tab where it belongs. A command the engine cannot do is marked `blocked`, with a report sent
upstream, and does not hold back the others.

## 4. Order

By group, the most used first: curve drawing and editing, surface making, solid making and editing,
transforms, surface editing, analysis, selection and utilities.

## 5. How it is checked

- A test reads `rhino-parity.md`: every listed command marked `done` or `partly` exists in Curva, and no
  in-scope group is left without its commands listed.
- Every command has an icon (`gumball-and-icons-0.3.0.md`).
- **The owner's check:** tries the new commands against Rhino, group by group.
