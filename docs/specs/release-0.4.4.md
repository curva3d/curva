# Curva 0.4.4 · plan, 19 November 2026

**Status:** Approved by the owner in chat · 19 November 2026 ("make the plan include everything").
Decision 0038.

Everything open on 19 November 2026 ships in 0.4.4, except three items that wait on other
people's projects (section 9). Curva 0.4.4 is released when every other item is built and checked
in the app, with the hand checks and library checks in `docs/releasing.md`. There is no fixed
date; each stream works through its own part and reports progress in its status page.

Every item is an issue in the GitHub milestone **0.4.4**, so its progress shows there. Issues
opened later for these items join the milestone; new work that is not on this list goes into the
next update unless the owner adds it.

## 1. Already in (merged since 0.4.3)

These go into the 0.4.4 release notes as they are.

- **Meshes and SubD:** meshes and SubDs as objects, saved in `.curva`, read and written in `.3dm`,
  shown shaded and pickable; they move, stretch, deform and sit in stretched blocks. STL, OBJ and
  PLY in Open, Import and Export. SubD Weld and Unweld; QuadRemesh part 1, keeping hard edges; the
  `Mesh` command with Rhino's Coarse/Default/Smooth settings shared with export; meshes that follow
  their surface with History.
- **Files:** SketchUp files open (`curva-skp`); STEP export of surfaces, polysurfaces and solids;
  more `.3dm` files read correctly (loosely joined solids, weighted sum surfaces, three faces at an
  edge, surfaces of revolution, zero-length lines named in the report).
- **Speed:** big drawings open 2 to 8 times faster (every core, decision 0037); the *Large* plan in
  26 s instead of 122 s; text-heavy drawings about 19 s sooner; edits in block-heavy drawings
  0.33 s instead of 1.5 s; about 1 GB less memory on *Large*; real project files orbit smoothly.
- **Views and interface:** Rhino's 50 mm lens; Bottom, Left and Back views; two-point perspective;
  an adjustable lens kept in saved views; the view picker with all eight views; the file path row,
  no bottom bar or corner text, a glow when merging panels.

## 2. Owner's decisions

1. **#405** Spec: one list of file formats (draft, for approval).
2. **#404** AGENTS.md: tidy-up first (owner's rule of 19 November 2026), to merge.
3. **#206** A second shadows spec, duplicating #207 (merged 16 November): to close.

## 3. Meshes and SubD (mesh stream, unless noted)

4. **#272** Mesh booleans on manifold-rust (in progress).
5. **#269** STL and OBJ read and write in their own crate (in progress).
6. **#270** Mesh primitives, What, Properties, Join and Explode for meshes.
7. **#273** Check and MeshRepair for meshes.
8. **#274** ReduceMesh, TriangulateMesh, QuadrangulateMesh, ExtractMeshFaces, MeshToNURB.
9. **#329** QuadRemesh on quadrs: the rest of the interface and the basic command.
10. **#387** (features) Command framework: a step that opens the mesh panel.
11. **#333** (features) Command framework: picking points, edges and faces of meshes and SubDs, the
    remaining parts.
12. **#267** (display) Draw meshes: the rest (vertex colours; speed for meshes of millions of edges).
13. **#324** (display) Draw SubDs: smooth or flat surface, creases and borders.
14. **#334** (display) SubD editing in the viewport: cage display, part picking and highlight, live
    redraw.
15. **#407** (display) Live preview of meshing while the mesh panel is open.

## 4. Live plans (`live-plans.md`)

16. **#317** (features) The *Large* plan fast enough (owner's decision: before step 4).
17. **#203** (features) Profile and speed up building a plan (in progress).
18. **#204** (features) Kept per-object results and incremental hidden lines.
19. **#205** (display) Plans in viewports, editing on plans, previews held until the update arrives.
20. **#235** (display) Test: can Bevy draw plans and sheets as accurately as the print?

## 5. Display (display stream)

21. **#153** Draw repeated objects once (instancing) (in progress).
22. **#225** Bevy pilot step 4: precision far from the origin, millimetres.
23. **#226** Bevy pilot step 5: effects on recommended cards.
24. **#227** Bevy pilot step 6: measure and report.
25. **#170** Objects' own colours and the document's linetypes, with patterns in paper millimetres.
26. **#164** Technical and Hidden views.
27. **#284** Section hatch patterns on cut faces.
28. **#296** Snaps report the object and place they landed on (for dimension ties, #294).
29. **#182** Bug: seams drawn as lines on spheres and cylinders.
30. **#130** Snapping takes most of each frame on large scenes.

## 6. Other features (features stream)

31. **#294** Dimensions follow the objects they were snapped to (in progress, decision 0035).
32. **#328** Kernel: a flat bicubic surface fails to become a solid.
33. **#136** Extruding a circle into a solid takes about 60 ms.
34. **#389** `.3dm`: solids with sliver plane faces come in (pull request open).

## 7. Tidy-up (from `docs/reviews/code-health-2026-11-19.md`)

35. **#400** CI fails on new files over 800 lines or functions over 80 (in progress, PR #406).
36. **#391** A file format registry in `curva-document` (in progress, spec #405).
37. **#392** `curva-dxf`'s `lib.rs` split into export, import, curves and units.
38. **#393** `curva-3dm`'s `read/mod.rs` and `model.rs` under 800 lines.
39. **#394** `curva-kernel`'s `hidden.rs` under 800 lines.
40. **#395** (mesh) `.3dm` mesh and SubD reading and writing in functions under 80 lines.
41. **#396** `curva-app`'s long functions under 80 lines, with tests for their logic.
42. **#397** `curva-document`'s `fields.rs` call under 80 lines.
43. **#398** `title_block.rs` placed and `analyze/what.rs` make under 80 lines.
44. **#401** The shared helpers at the end of `commands/mod.rs` moved into `support.rs`.
45. **#178** The icon drawing and the long interface panels (in progress).
46. **#303** (display) The long functions in `viewports.rs`, `sheets.rs`, `main.rs` and `benchmark.rs`.

## 8. Release

47. The hand checks in `docs/releasing.md`, the check of custom kernel operations against the
    latest solidcore (decision 0019) and of custom Bevy parts against the latest Bevy
    (decision 0030); the release notes `docs/releases/v0.4.4.md`; the tag.

## 9. Waiting on other projects: in 0.4.4 only if they are ready in time

- **#67** Cap leaves the closed space empty: blocked on the geometry engine.
- **#381** Delete the patched `solidcore-ops` copy: when solidcore merges the fix
  (tangency-labs/solidcore#3358).
- **#349** Go back to quadrs from crates.io: when its author releases the repeatability fix.

If one is not ready, 0.4.4 ships without it and it moves to the next update; the release notes say
so.
