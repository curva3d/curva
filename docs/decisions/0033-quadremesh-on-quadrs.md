# 0033: QuadRemesh now, on quadrs

**Status:** Decided by the owner in chat · 17 November 2026. Replaces the quad remeshing row of
decision 0032 ("nothing for now").

**What was decided.** QuadRemesh is an important command, so it comes with SubD modelling (stage C
of `docs/specs/mesh-and-subd.md`) instead of waiting until someone asks. It is built on **quadrs**
(MIT), pinned at 1.0.0, behind Curva's one mesh interface in `crates/curva-mesh`, like the other
mesh libraries. What Rhino's QuadRemesh has and quadrs lacks (hard edges, guide curves, symmetry,
adaptive size, SubD output) Curva adds on top, as listed custom operations.

**Why.** Quickest route to working quads. Its first pull request compares it with Rhino's
QuadRemesh on test models, with pictures for the owner, before anything is built on it.

**What was looked at (17 November 2026).** No well-maintained pure-Rust quad remesher exists. The
well-maintained ones (QuadriFlow, QuadWild, Blender's) are C++, which Curva does not use.
- quadrs: Instant Meshes method, experimental, one author, unchanged since March 2026.
- A Rust port of QuadriFlow inside SKPFORGE (MIT, September 2026): basic run only (no hard edges,
  borders or adaptive size), not a separate library.
- Mantis (MIT): its quad remesher is planned, not built.

**Rejected.** Curva's own QuadRemesh from the start, following QuadriFlow's published method: no
dependence on a young library, but two to three more weeks before the first quads. The owner
chose to start on quadrs.

**Revisit if** quadrs falls short of Rhino on the test models, or stops working with Curva. Then
Curva writes its own behind the same interface (QuadriFlow's method is the candidate), and no
command changes.
