# Status

Curva is built by three streams of work, each in its own sessions. Each stream writes only its own
status page, so two sessions never edit the same file:

- **Features** (commands, NURBS geometry, documents, files, and the interface outside the viewport):
  [`status/features.md`](status/features.md). It also holds the project's history up to
  15 November 2026.
- **Display** (the viewport: drawing, display modes, speed on large scenes):
  [`status/display.md`](status/display.md).
- **Mesh** (mesh and SubD objects, from 16 November 2026):
  [`status/mesh.md`](status/mesh.md).
- **Testing** (from 25 November 2026: tests every command against real Rhino 8 and files the
  differences as `rhino-parity` issues for the other streams; changes no code they own):
  [`status/testing.md`](status/testing.md).

Read all three before starting work; write only yours. Who owns what is in `AGENTS.md`.

**Releases renumbered on 22 November 2026 at the owner's request:** the 0.4 series is now 0.1 and the four before it 0.0.1–0.0.4 (table in [`docs/releases/README.md`](docs/releases/README.md)); the next release after them was **0.1.6**.

**0.1.6 was published on 23 November 2026 at the owner's request, with what was on `main`** (notes:
[`docs/releases/v0.1.6.md`](docs/releases/v0.1.6.md)): the new interface, SubD and mesh commands measured
against Rhino 8, and display modes as styles. Open items from the owner's first impressions stay in
the GitHub milestone **0.1.5**, now for the next update; work on those first. New work goes into the next update unless the owner adds it.
