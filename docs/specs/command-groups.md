# Spec: commands grouped by kind of geometry (NURBS, Mesh, SubD)

**Status:** Draft, for the owner's approval in chat. The owner asked on 20 November 2026 for the
commands to be organised by NURBS, mesh, SubD and so on, in the code **and in the menus**.
**Covers:** the command tabs in the top bar (Freeform specialization) and the folders the commands
live in (`crates/curva-commands/src/commands/`). Every command keeps its name, aliases, options
and behaviour; typing a command works exactly as before. Issue #422 (the command list at the size
limit) is done as part of step 1.

## 0. In short

| What changes | What you will notice |
|---|---|
| The top bar's tabs start with the kinds of geometry: **NURBS** (Curves, Surfaces, Solids), then **Mesh** and **SubD** | Mesh and SubD commands get tabs of their own; today they have none and must be typed |
| The rest of today's *Modify* tab splits into **Edit** (cut, join, move, deform) and **Organise** (groups, blocks, layers, properties) | Modify had 44 buttons mixing very different things |
| **Annotate** and **Layouts** tabs | Text, dimensions, hatches, layouts and printing get buttons; today they are typed or reached from panels |
| The code folders follow the same groups, each with its own list of commands and its own tab | Nothing visible. Each stream edits only its own groups, and the main command file stops growing |

## 1. Why

Today's tabs are Curves (34), Surfaces (36), Solids (21), Modify (44), Analyze (20) and Select
(17). The 18 mesh commands (Mesh, the mesh primitives, ReduceMesh, MeshRepair, MeshToNURB and the
rest) and SubD commands appear in no tab, nor do annotation, layout and printing commands. Modify
holds booleans, transforms, blocks, layers and properties together. The code is grouped like
Rhino's menus but not like the tabs, and every command is registered in one file of 800 lines.

## 2. The tabs

Left to right. A thin divider and a small label group the first five: **NURBS** over Curves,
Surfaces and Solids; Mesh and SubD beside them. The rest are shared by every kind of geometry.

| Tab | What it holds |
|---|---|
| **Curves** | As today |
| **Surfaces** | As today |
| **Solids** | As today, plus the booleans from Modify: BooleanUnion, BooleanDifference, BooleanIntersection, BooleanSplit, EditBoolean, BreakBoolean |
| **Mesh** *(new)* | Mesh, MeshBox, MeshSphere, MeshCylinder, MeshCone, MeshTorus, MeshPlane, ReduceMesh, TriangulateMesh, QuadrangulateMesh, ExtractMeshFaces, MeshRepair, FillMeshHoles, UnifyMeshNormals, RebuildMeshNormals, CullDegenerateMeshFaces, MeshToNURB |
| **SubD** *(new)* | SubDLevel today; the mesh stream adds the rest as they arrive |
| **Edit** | From Modify: Trim, Split, Join, Explode; Move, Copy, Rotate, Scale, Mirror, Array, ArrayLinear, Align, Distribute; Shear, Twist, Bend, Taper, Flow, CageEdit |
| **Organise** | From Modify: Group, Ungroup, Block, Insert, BlockManager, BlockEdit, ExplodeBlock; ChangeLayer, MatchProperties, Linetype, PrintWidth, SetUserText, GetUserText, Purge; Lock, Unlock, LockSwap, BringToFront, SendToBack |
| **Annotate** *(new)* | Text, TextObject, Dim, DimAligned, DimRotated, DimRadius, DimDiameter, DimAngle, DimOrdinate, Leader, Dot, Hatch, DimCurveLength, DimArea, DimVolume |
| **Layouts** *(new)* | Layout, Detail, TitleBlock, ScaleBar, SheetIndex, Revision, CopyLayout, Make2D, Print, ExportPDF |
| **Analyze** | As today |
| **Select** | As today |

Each button keeps its icon and tooltip. The tab remembered between sessions is kept by name, so
someone who left Curva on *Modify* comes back on *Edit*.

## 3. The code

- `commands/` gets one folder per group: `nurbs/curves`, `nurbs/surfaces`, `nurbs/solids`, `mesh`,
  `subd`, `edit`, `organise`, `annotate`, `layouts`, `analyze`, `select`, plus `view` and `files`,
  which have no tab. Today's `modify` folder is shared out between `edit`, `nurbs` (point and knot
  editing: Rebuild, ChangeDegree, InsertKnot, RemoveKnot, PointsOn, PointsOff, SetPt) and `solids`
  (booleans).
- Each folder gives its own part of the command list, and the list of buttons for its tab. The
  main command file only joins them in order (issue #422). The interface reads the tabs from there
  instead of keeping its own list.
- Ownership follows the folders: the mesh stream owns `mesh` and `subd` and their tabs' buttons.

## 4. Steps

Each step is its own pull request.

1. **The folders and the per-group lists** (#422). Code moves only: the command list and its order
   are pinned by a test first, and the tabs look as they do today.
2. **The tabs of section 2.** A test checks that every button names a real command, that no
   command has a button in two tabs, and that every mesh and SubD command has a button.

## 5. Not in this spec

New commands. The Architecture, Publisher and Analysis specializations' tabs, which will follow
the same pattern when they get commands. Menus that drop down from the tabs, or customising them.

## 6. Choices for the owner

- **One "Edit" tab for cutting, joining and transforming** (recommended), or Rhino's split into a
  *Transform* tab and leaving Trim, Split, Join and Explode in Curves and Surfaces.
- **Annotate and Layouts tabs now** (recommended): ten tabs instead of six, but nothing stays
  hidden behind typing.
