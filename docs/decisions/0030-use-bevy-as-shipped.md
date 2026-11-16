# 0030: Use Bevy as it ships; anything custom is marked and checked at every upgrade

**Status:** Decided by the owner in chat · 16 November 2026, for the move to Bevy (decisions 0028,
0029).

**What was decided.** Bevy is treated as solidcore is (decision 0019):
- **Use as much of Bevy as possible**: its materials, lights, shadows, transparency, ambient
  occlusion, smoothing, instancing, culling, render layers, cameras and visibility ranges, rather than
  Curva's own versions of them.
- **Anything Curva adds because Bevy cannot do it yet is a custom part**: marked in the code with
  `// CUSTOM BEVY PART: <name>`, listed in `docs/bevy/custom-parts.md` with what Bevy lacks, and
  kept in step by a test.
- **At every Bevy upgrade and before every release**, each custom part is checked against the new
  Bevy. If Bevy now ships it and it works at least as well (tests pass, pictures and speed no worse),
  Curva's part is deleted.

**What follows for the move.**
- Faces use Bevy's own materials and lights. Shaded views will look somewhat different from today's
  (Bevy's lighting rather than Curva's simple formula); copying today's formula exactly would be a
  custom part, so it is not done.
- Per-view looks (style, sun) use plain Bevy: one copy of each object per on-screen view, sharing its
  mesh, on that view's render layer, with that view's material and light.
- Expected custom parts (the review of 16 November 2026): Curva's CAD lines, plans and sheets drawn
  from the vector page, clipping planes with cut faces, the object-number picture for Diagram
  outlines, the moving origin for models far away, CAD picking and snapping, and the glue that runs
  Bevy inside Curva's window.

**The aim: less code to maintain.** The owner's goal is a simpler Curva, built on existing
solutions, with as little code of Curva's own as possible. Expected effect of the move (rough):

| | Before | After the move to Bevy |
|---|---|---|
| Curva's own drawing of surfaces, shading, see-through views, outlines | about 4,200 lines of Curva's own graphics code | mostly deleted; Bevy does it |
| Planned speed work (`display-pipeline.md` steps 11 to 15: repeated shapes, smarter uploads, skipping hidden objects) | still to write | never written; Bevy has it |
| Shadows, ambient occlusion, smoothing, big meshes, SubD display | still to write | Bevy has them |
| Lines in 3D (paper millimetres) | exists | kept, moved into Bevy's frame |
| Plans and sheets on screen | not started | about 200 lines of glue over lyon and kurbo |
| Running Bevy inside Curva's window, handing it the scene | none | a few hundred lines |

**Agreed workflow** (owner, 16 November 2026):
1. **Bevy first.** Before writing any drawing code, check what Bevy ships; use it if it does the job.
2. **Then existing libraries.** Where Bevy lacks something, use a maintained library before writing
   Curva's own: **lyon** turns lines and fills into triangles with their true widths; **kurbo**
   (already in Curva's build, through egui) handles curves and cuts lines into dashes. Licences are
   checked with `cargo-deny` as for any dependency.
3. **No add-ons from outside Bevy** for drawing (such as `bevy_prototype_lyon`): they often lag a
   Bevy version behind and would hold up every upgrade. A small part of Curva's own is cheaper to
   carry.
4. **Only then Curva's own code**, as small as possible, marked `// CUSTOM BEVY PART`, listed in
   `docs/bevy/custom-parts.md`, and checked at every Bevy upgrade and before every release.
5. **One drawing description** for plans and sheets, shared by the screen and every export (PDF,
   SVG, DWG, DXF; Illustrator takes the PDF and SVG with their layers), keeping CAD meaning: exact
   arcs and curves, layers, line types, hatches as patterns, real text and dimensions, blocks, the
   object each mark came from. Its spec change comes to the owner for approval before live plans
   are built.

How Curva, solidcore and Bevy fit together: `docs/architecture.md` §6b.

**Rejected.** Writing Curva's own look on top of Bevy where Bevy's does the job: more code to carry
through each Bevy upgrade, for no gain the owner asked for. Vello for 2D beside Bevy (decision
0029): two engines and two line drawers, while the drawing description is needed either way.

**Revisit if** a Bevy part proves clearly worse for Curva's drawings than a small custom one (then it
becomes a listed custom part, with the reason).
