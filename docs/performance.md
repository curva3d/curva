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
