# Kernel evaluation

The test bench for `docs/specs/m0-kernel-evaluation.md`. It is not part of Curva itself.

- `cases/` holds the 31 test operations, one file each, in a kernel-neutral form. Rhino and every
  kernel get exactly the same inputs.
- `harness/` runs the cases on a kernel and records the outcome, measurements, time and picture.
  A crash or a hang is recorded and the run carries on.

## Case files

Each line is `key: value`; lines starting with `#` are comments.

| Key | Meaning |
|---|---|
| `id`, `name`, `group` | The operation's number, description and section in the spec |
| `operation` | Neutral name of the operation |
| `rhino` | The Rhino command or commands for the reference result |
| `input` | A named input: `label kind parameters` |
| `input-points` | Control points for a `grid-surface` input, row by row |
| `option` | A setting, as `key=value` |
| `uses` | An earlier case whose result this one starts from |
| `measure` | Something to record about the result |

Input kinds: `points`, `line`, `interp-curve`, `circle`, `ellipse`, `rectangle`,
`rounded-rectangle`, `extruded-line`, `grid-surface`, `box`, `cylinder`. Circles and ellipses lie
flat unless they give a `normal`. Lengths are in millimetres.

## Adding a kernel

Implement the `Kernel` trait in `harness/src/lib.rs`: read a case, run it, and return measurements
or a refusal with its reason.
