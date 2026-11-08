# 0002 — Licence: MIT OR Apache-2.0

**Status:** Accepted · 8 November 2026 (architecture open decision 6)

**Decision:** Curva's code is licensed MIT OR Apache-2.0, at the user's choice.

**Why:** It's the standard pairing in the Rust world, so code can flow in from SolidCraft,
solidcore and Serpentine3D, and out to others, without legal friction. Apache-2.0 adds a patent
grant. It also fits the open-core plan: the app stays free, paid services are separate.

**Rejected:** GPL or AGPL. They would block reusing permissive code in some directions and make
commercial partnerships (e.g. manufacturers publishing objects) harder. It also rules out
AGPL kernels such as brepkit as dependencies.

**Note:** Library *content* (smart objects) gets its own content licence later (architecture 10b).
