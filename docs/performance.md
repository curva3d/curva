# Performance measurements

Measured with `curva --orbit-benchmark`, which opens the window on a scene, spins the view and prints
how long each frame took (on Windows, pipe it: `curva --orbit-benchmark --curves 5000 | more`).
Compare builds made the same way on the same machine: a local `cargo build --release` and the
release workflow's build give different absolute numbers.

## Lines drawn once at their own width (0.4.1, spec `line-weights.md` §2)

13 November 2026, NVIDIA GeForce RTX 3060 (Vulkan), both built locally with `cargo build --release`.
"Before" is 0.4.0, which drew each line three times, a pixel apart.

| Scene | Before: work per frame, frame gap | After: work per frame, frame gap |
|---|---|---|
| 5,000 circles and 200 cylinders | 9.29–9.46 ms, median 4.0–4.1 ms | 9.18–9.39 ms, median 3.9–4.1 ms |
| 20,000 circles | 8.30 ms, median 7.5 ms | 8.62 ms, median 7.6 ms |

The same within run-to-run noise: drawing lines as quads of any width costs no more than before.

## Solids held parsed (0.4.3, `docs/reviews/code-health-2026-11-14.md` fix 1)

14 November 2026, the Windows machine, `cargo run --release -p curva-kernel --example body_costs`: a
40 × 30 × 20 box with all twelve edges rounded (26 faces, 48 edges), moved. "Before" is 0.4.2, which
kept each solid as the engine's native text and read it again for every question; "after" holds the
engine's body once, shared between copies, and keeps what is worked out from it (record, mesh,
bounds, volume, area). Average of repeated calls on the same body:

| Question | Before | After: first time | After: again |
|---|---|---|---|
| is_solid | 4.29 ms | 1.54 ms | 0.000 ms |
| volume | 45.6 ms | 43.5 ms | 0.000 ms |
| bounds | 16.5 ms | 14.6 ms | 0.000 ms |
| mesh | 24.7 ms | 5.1 ms | 0.016 ms |
| edge_curves | 24.3 ms | 0.020 ms | 0.018 ms |
| a moved copy, then is_solid | 3.85 ms | 0.001 ms | 0.000 ms |

A copy of a body (what each undo step makes) now shares its faces instead of copying the text.
Bounds of a body that was moved, or scaled or mirrored along the axes, come from the kept box; a
turned body is still placed and measured exactly.
