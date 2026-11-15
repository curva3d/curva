# 0024: Trim the display plan, and try existing renderers first

**Status:** Decided by the owner in chat · 15 November 2026.

**What was decided.**
- Steps 14 (occlusion culling) and 15 (Nanite-style clusters and stand-ins, mesh shaders) of
  `docs/specs/display-pipeline.md` are **shelved**. They come back only if a real model, measured with
  `--orbit-benchmark`, is still too slow after steps 11 to 13 (instancing, off-screen culling, memory limit).
- Two short evaluations come before more drawing code is written by hand:
  **Vello** for 2D views and sheets (#185), and **re_renderer** for 3D lines (#186). Each ends with
  a plain-language report and a recommendation for the owner.
- Ideas may be learned from Blender, xeokit and other viewers. Code from GPL or AGPL projects is
  never copied.

**Why.** The owner found the display plan very complicated and asked whether an existing project
could do the job. None can do all of it within Curva's rules:
- Blender is GPL and C/C++. Linking it would make Curva GPL.
- xeokit is JavaScript and AGPL.
- Godot, Filament, VTK and OpenCascade's viewer are C++, and none of them draws like a CAD
  program (line weights, cut fills, plans that match the printed sheet).

Pure-Rust pieces can still take over parts of the work. Vello could draw plans and sections as the
same vector paths the PDF uses. re_renderer already draws lines, points, outlines and picking on
wgpu and egui. Steps 14 and 15 are game-engine techniques that add the most complexity for the
least gain in architectural models. The Nanite-style approach also needs graphics-card features
that 2020 Intel integrated graphics lack.

**Rejected.** Adopting a whole engine (Bevy, Blender, Godot, Filament): wrong licence, not Rust, or
no CAD drawing, and each would mean rebuilding Curva around it. Keeping all fifteen steps as planned.

**Revisit if** a measured real model is too slow after steps 11 to 13, or one of the evaluations
shows a library can replace more than expected.
