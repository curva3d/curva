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

**Rejected.** Writing Curva's own look on top of Bevy where Bevy's does the job: more code to carry
through each Bevy upgrade, for no gain the owner asked for.

**Revisit if** a Bevy part proves clearly worse for Curva's drawings than a small custom one (then it
becomes a listed custom part, with the reason).
