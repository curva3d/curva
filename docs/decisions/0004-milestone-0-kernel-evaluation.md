# 0004 — Add Milestone 0: test the geometry kernels before building on one

**Status:** Accepted · 8 November 2026 · Amends architecture sections 6, 10 and 11

**The problem:** The kernel is the engine that computes curves, surfaces and solids. It decides
how good Freeform can be. The architecture names solidcore as the primary kernel, but nobody has
tested it on the operations an architect actually uses. solidcore is at version 0.0.x and about a
month old.

**What we found (8 November 2026):** there are more pure-Rust candidates than the architecture lists:

| Kernel | Licence | Notes |
|---|---|---|
| solidcore | Apache-2.0 | Port of NVIDIA's SMLib. Right scope (fillets, offsets, sweeps, booleans). 0.0.x |
| arris | MIT / Apache-2.0 | 0.9.x. Sweeps, lofts, fillets on freeform faces. Refuses some freeform booleans for now, clearly rather than wrongly |
| monstertruck | check | A renamed, maintained fork of truck |
| truck | MIT / Apache-2.0 | Original; last release Sept 2024 |
| curvo | check | NURBS curves and surfaces library, not a full solid kernel |
| brepkit | AGPL | **Excluded** by licence (see 0002) |

**Decision:** Before M1, run Milestone 0: the same set of real modelling operations on each
candidate, compared with Rhino, with a picture gallery and a plain-language recommendation.
The spec is `docs/specs/m0-kernel-evaluation.md`.

**Likely outcome:** not a single winner, but a primary kernel plus a second one for specific
operations, which the kernel interface (architecture section 7) was designed to allow.

**Rejected:** Starting M1 on solidcore and finding its limits later, when far more code depends on it.
