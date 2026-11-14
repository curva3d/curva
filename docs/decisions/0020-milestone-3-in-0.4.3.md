# 0020: Milestone 3 in 0.4.3

**Status:** Decided by the owner in chat · 14 November 2026. Points 7 and 8 replaced by 0021.

Milestone 3 ("Layouts and drafting", `docs/specs/milestone-3-in-0.4.3.md`) is planned. The owner chose:

1. **One release.** All of Milestone 3 ships as 0.4.3, released when everything is built and checked
   in the app, as Milestone 2 did. (Rejected: two releases, or one per area.)
2. **Smart object representations wait for Milestone 4.** Milestone 3 is Rhino-style drafting only.
   Plan symbols, detail levels, scale variants, Rhai or WebAssembly logic and `.curvaobj` all arrive
   with Architecture, where doors and windows need them. This changes the M3 row of
   `architecture.md` §10, which had representations arrive here; the object model still allows them
   (definitions with several representations), so nothing built now needs rewriting.
3. **Make2D exactly like Rhino's:** a one-off drawing with no link to the model. (Rejected: an Update
   command that redraws it, and live drawings.)
4. **Both ISO and ANSI.** Title blocks, annotation styles, hatch patterns and paper sizes ship as a
   metric ISO set (A0 to A4, ISO 128 line weights, mm) and an imperial ANSI set (ANSI A to E and
   Arch sizes, feet and inches). A new document takes the set that matches its units.

Decided by Claude, as the owner asked to keep going:

5. **Clean-up first.** The post-milestone clean-up pass (`architecture.md` §17) for Milestone 2 is
   done before Milestone 3 starts.
6. **Hidden lines are Curva's own**, built on solidcore's silhouettes and ray tracing, and flagged as
   custom (decision 0019, point 4).
7. **Drawing files in pure Rust:** PDF through `pdf-writer` and `subsetter` (MIT OR Apache-2.0, from
   the Typst project), DXF through the `dxf` crate (MIT), SVG written by Curva. Each is checked with
   `cargo-deny` before it is added. Printing to a printer goes through the system's print command
   with a generated PDF, so no `unsafe` code is needed.
8. **No DWG** in 0.4.3: no maintained pure-Rust DWG library exists.
9. **Order of work:** clean-up; layouts and detail views; text and annotation styles; dimensions,
   leaders and dots; hatches and section fills; PDF export and printing (so every later part can be
   checked on paper); hidden lines, the Technical and Hidden modes, Make2D; title blocks, scale bars,
   sheet index, revisions; SVG and DXF; `.3dm` round trip of all of it.

**Would make us revisit:** a pure-Rust DWG library that is maintained (point 8); solidcore gaining
hidden-line removal (point 6, checked before every release); the owner wanting an earlier release
once layouts work (point 1).
