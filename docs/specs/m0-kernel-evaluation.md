# Spec — Milestone 0: geometry kernel evaluation

**Status:** Draft · waiting for the owner's approval of section 3
**Decision behind it:** `docs/decisions/0004-milestone-0-kernel-evaluation.md`

## 1. What this is for

Find out which pure-Rust kernel (or combination) can carry Curva's Freeform modelling, by running
the operations an architect actually uses, on every candidate, and comparing each result with Rhino.

## 2. Candidates

solidcore, arris, monstertruck, truck (as a baseline), and curvo (curves and surfaces only).
Anything AGPL or GPL is excluded (decision 0002). Agents may add a candidate if they find a
promising pure-Rust kernel with a permissive licence; say so in the report.

## 3. Test operations — **owner to review**

Each operation uses simple, fixed inputs so Rhino and every kernel get exactly the same thing.
The owner should check: *is anything missing that you use every week? Is anything here that
doesn't matter to you?*

### Curves
1. **Interpolated curve** through 6 points in 3D (Rhino: InterpCrv).
2. **Offset** a planar freeform curve, both sides, with a distance that creates a self-intersection.
3. **Intersect** two freeform curves; report the intersection points.

### Surfaces
4. **Loft** through 4 open freeform curves, normal style.
5. **Loft** through 3 closed sections of different shapes (circle → rounded rectangle → ellipse).
6. **Sweep1:** a closed profile along a 3D freeform rail.
7. **Sweep2:** a profile along two rails.
8. **Revolve** a freeform profile 360° and 270°.
9. **Network surface** from 3 × 3 crossing curves.
10. **Planar surface** from a closed curve with two holes in it.
11. **Extrude** a closed curve with nested holes into a solid (Serpentine fixed a bug on exactly this).
12. **Offset surface** of a doubly curved surface.

### Solids and booleans
13. **Box minus cylinder** (the simplest case; everything must pass this).
14. **Union of two boxes** that share a face exactly (a classic failure case).
15. **Lofted freeform solid minus a box** (freeform meets flat).
16. **Two freeform solids** intersecting (the hardest boolean).
17. **Split** a freeform surface with a curve projected onto it.
18. **Shell** a box with the top face removed.

### Fillets and chamfers
19. **Fillet all edges of a box** (tests the corner blends).
20. **Fillet the edge** between a flat face and a cylinder.
21. **Fillet an edge between two freeform faces.**
22. **Chamfer** the edges of a box.

### Measurement and output
23. **Volume, area and centre of mass** of results 13, 15 and 19, against Rhino.
24. **Display mesh** of result 16: triangle count and time taken.
25. **STEP export** of results 15 and 19, opened in Rhino: does it open cleanly, as a closed solid?

### Everyday operations added from Serpentine3D's command list
26. **Intersect** a doubly curved surface with a cylinder; report the intersection curves (Rhino: Intersect).
27. **Join and Cap:** join four extruded wall surfaces into one open polysurface, then cap it into a closed solid (Rhino: Join, Cap).
28. **Section and Contour** of result 15: one section plane, then contours at a fixed spacing (Rhino: Section, Contour).
29. **Merge coplanar faces** on result 14: the shared flat faces become one face; report the face count (Rhino: MergeAllCoplanarFaces).
30. **Pipe** along a 3D freeform curve with a tight bend, with a radius close to the bend's own radius (Rhino: Pipe).
31. **Boolean that splits a solid in two:** a box minus a slab that passes right through its middle; the result must be two separate closed solids (Rhino: BooleanDifference).

## 4. What is recorded for each operation and kernel

- **Outcome:** works · wrong result · refuses (says it can't) · crashes or hangs.
  A clear refusal is better than a wrong answer, and is scored that way.
- **Match with Rhino:** volume / area / centre of mass within tolerance; topology counts
  (faces, edges); closed or open.
- **Time** taken, and whether it can run alongside other work.
- **A picture** of the result next to Rhino's.

## 5. Rhino reference

Rhino results are captured once on the Windows machine through Rhino MCP and stored as reference
files. **Before automating Rhino, check its licence terms for scripted use** (architecture open
decision 7). If automation isn't allowed, the owner builds the 31 references by hand once,
from a script of exact steps the agent writes.

## 6. Deliverables

1. A test harness (outside Curva's product code) that runs all operations on all candidates.
2. A **comparison gallery**: one page, pictures side by side, failures highlighted.
3. A **recommendation report** in plain language: which kernel for which operations, the gaps,
   and what it would take to close them.
4. A decision note (`0006-kernel-choice.md`) for the owner to approve.

## 7. Out of scope

Speed tuning, interface work, file formats beyond STEP. This is a test, not the product.
