# Status

*Updated at the end of every work session. Newest first.*

## Current state — 8 November 2026

**Milestone:** M0, groundwork and kernel evaluation.

**Done**
- Architecture draft v0.9 written by the owner (`docs/architecture.md`).
- Agents work through the owner's GitHub account and merge their own work once checks pass (decisions 0005 and 0007).
- Starter repository prepared: rules, decision notes, M0 kernel evaluation spec, issue list.
- One-time GitHub setup finished: https://github.com/curva3d/curva, with `main` protected and the
  17 M0 tasks in https://github.com/curva3d/curva/milestone/1.
- The kernel test list grew from 25 to 31 operations, chosen by the owner after comparing it with
  Serpentine3D's commands (#19).
- The owner confirmed the 31-operation list is final and approved; #8 is released.
- Dev Drive and Rust toolchain set up (#2). The repository is at `V:\curva`.
- Rust workspace skeleton (#3).
- Automated checks on Linux, Windows and macOS, required on `main` (#4).
- Claude Code hooks: format and lint after edits, tests before finishing, risky commands blocked (#5).
- The 31 kernel test cases as neutral files, and a harness to run them (#8), in `evaluation/`.
- A check that flags weakened tests on every pull request (#6). It is deliberately not a required
  check, so a change the owner has agreed to can still be merged.

**In progress**
- Milestone 1, first slice (`docs/specs/m1-first-slice.md`, approved by the owner): 12 commands, viewport and window built. The owner is trying it by hand.

- First hand-testing round (9 November 2026): command autofill, quick clicks no longer lost, window and crossing box selection, Shift adds and Ctrl removes. Orbit no longer waits for the display (31 ms to 2 ms between frames); it may still feel laggy over Remote Desktop, whose display runs at 32 Hz. Whether that is the connection is not settled: the owner can compare with Rhino over the same link or use Parsec.

- Object snaps (End, Mid, Cen, Int, Perp, Near), ortho and grid snap (`docs/specs/m1-snaps-and-ortho.md`). `New` now asks before discarding a model. With 5,000 circles a frame takes about 2.4 ms and snapping about 1 ms (`--orbit-benchmark --curves 5000`). The owner is trying it by hand.
- Four viewports with Top, Front, Right and Perspective views, each on its own plane (`docs/specs/m1-four-viewports.md`). The owner is trying it by hand.
- Rotate, Scale, Mirror, Arc and Rectangle (`docs/specs/m1-transforms-arc-rectangle.md`). A curve can snap to itself while being drawn, circles sit on any plane, and Curva opens on Perspective.
- Live previews for every command that changes geometry, drawn one way (rule in `AGENTS.md`).
- NURBS curve storage (decision 0012).
- Array and Join (`docs/specs/m1-array-join.md`).
- Mixed curves (decision 0013), Explode, and 0 allowed as an Array count.
- Split and Trim: cutters, then click the part to remove, as Serpentine3D and Rhino do.
- Offset and Fillet (`docs/specs/m1-split-trim-offset-fillet.md`).
- `CPlane` for custom construction planes; typed coordinates now follow the plane, and `w` means world.
- Fillet now works on arcs and polyline segments as well as lines.
- Gumball with move and turn handles (`docs/specs/m1-gumball.md`).
- Layers and properties panels (`docs/specs/m1-layers-properties.md`).
- Selection, hide and show, measuring, Flip, CloseCrv and ArrayPolar.
- Display modes wait for surfaces, as there is nothing to shade yet.
- Ellipse, Chamfer, Rotate3D, Scale1D and Scale2D.
- Point objects, Divide, ArrayCrv and Orient.
- Rebuild.
- History on Mirror and Offset (`docs/specs/m1-history.md`).
- MCP server, first part (`docs/specs/m1-mcp-server.md`): `curva --mcp`.
- Rhai scripting is built on the local branch `m1-rhai-scripting` and waits for the owner: Rhai brings in `smartstring` (MPL-2.0, unmaintained) and `tiny-keccak` (CC0).

**Waiting for the owner**
- Nothing yet. The engine is solidcore, with Serpentine3D's commands as the base (decision 0010). SolidCraft
  reuse is parked (0009).

**Testing**
- Agents do not run tests through Rhino (decision 0011). Curva is verified by its own checks.
- solidcore ran 28 of the 31 cases; results and known gaps are in 0011 and `evaluation/reports/solidcore.tsv`.

**Handoff notes**
- `V:` is a file-based Dev Drive (`C:\DevDrives\curva.vhdx`), reconnected at startup by the
  scheduled task "Attach Curva Dev Drive". Reconnecting after a restart has not been tested yet.
- The old copy in `C:\Projects\curva` is out of date and no longer used.
- Agents merge their own pull requests once checks pass; the owner does not click merge.

**Known problems / risks**
- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
