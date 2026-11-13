# 0019: Milestone 2 in 0.4.2

**Status:** Decided by the owner in chat · 13 November 2026.

The owner asked that 0.4.2 hold everything left in Milestone 2 and the new logo
(`docs/specs/milestone-2-in-0.4.2.md`), and chose:

1. **Rhino files in pure Rust.** Curva writes its own `.3dm` reader and writer instead of binding
   McNeel's C++ library, keeping the rule against `unsafe` code.
2. **Live booleans keep their inputs.** The solids a boolean was made from are kept, hidden, inside
   the result; editing them updates it through History. Breaking the link leaves a plain solid.
3. **Blocks embedded and linked**, as in Rhino, with a Blocks panel.
4. **The engine's gaps are built in Curva**, each flagged as custom. Before every release each one is
   checked against the latest solidcore, and solidcore's is used wherever it fully covers the
   operation, so that Curva's own code shrinks over time. (The owner's words: "with time we can get
   rid of the custom curva commands.")

Decided by Claude, as the owner asked to keep going:

5. Mesh and SubD items of phase F3 stay out (they belong after Milestone 4, `architecture.md` §10d);
   SectionHatch belongs to Milestone 3.
6. Order of work: the engine's gaps that other parts need (surface splitting, trimming, intersections,
   isocurves), then the rest of the gaps, live booleans, blocks, `.3dm` files (which need blocks and
   trimmed surfaces), organising and analysis, the framework moves. 0.4.2 is released when all of it
   is built and checked in the app.
