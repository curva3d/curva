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
- Display modes: Wireframe and Shaded for each viewport, from `SetDisplayMode` or by clicking the viewport's title. Perspective starts shaded, as in Rhino.
- Ellipse, Chamfer, Rotate3D, Scale1D and Scale2D.
- Point objects, Divide, ArrayCrv and Orient.
- Rebuild.
- History on Mirror and Offset (`docs/specs/m1-history.md`).
- MCP server, first part (`docs/specs/m1-mcp-server.md`): `curva --mcp`.
- Rhai scripting is finished but held on the local branch `m1-rhai-scripting` (decision 0014), because Rhai 1.27.0 still requires `smartstring`.
- MCP connected to the open window, behind the Agent access checkbox, with approval of destructive actions.
- SelCrv, SelPt and BoundingBox.
- Names, SelName, SelLayer, SelDup, Isolate and ZoomSelected. Agent access starts on.
- SelLast, SelPrev, Area, SetPt, ProjectToCPlane, Zoom (window, in, out, target, extents, selected) and
  NamedView (saved in the file, outside undo). Agent pictures now show the window's last-used viewport.
- Agent approval errs on the safe side: an answer that reads like a guarded command (`save` in NamedView)
  also asks the user.
- FilletCorners, TweenCurves (closed curves are lined up first so tweens do not twist) and Orient3Pt.
  Option words can be shortened to any start no other option shares, as in Rhino (`c` for Close).
- Control points (`docs/specs/m1-control-points.md`): PointsOn (F10) and PointsOff (F11), clicking and boxing points, Move, the gumball and SetPt on picked points, InsertKnot (also InsertControlPoint), RemoveKnot, RemoveControlPoint (and Delete with points picked) and Smooth. The owner is trying it by hand. Known gap: pictures made with `--picture` do not show the point squares yet.
- Dir: arrows show which way the chosen curves run; click a curve to flip it, or type Flip for all.
- The built-in assistant panel is postponed by the owner (10 November 2026); its draft is in `docs/specs/m1-assistant-panel.md`.

**Milestone 2 started early, at the owner's choice (10 November 2026):** first surfaces, `docs/specs/m2-first-surfaces.md`, approved with Sweep1 and the booleans added.
- Surfaces and solids in the model and in files (older files still open), drawn by their edges, and moved, turned, scaled, mirrored, copied, hidden and put on layers like curves. `Box` makes the first solids. The properties panel shows a solid's volume or a surface's area.
- Shaded display: faces lit from the camera with a little light from above, dark edges on solids, depth in every view (flat views included). Pictures made with `--picture` and for agents are shaded too. Solids are meshed once and kept until they change.
- `ExtrudeCrv` (options Solid and BothSides, distance remembered) and `PlanarSrf`. As in Rhino, an outline and the closed curves inside it become one solid or surface with holes, and separate outlines make separate objects.
- `Revolve` (axis by two points, any angle up to a full turn; a closed curve turned part way is closed at its ends), `Loft` (through the curves in the order they were made, ends left open) and `Sweep1` (rail first, then cross sections; one curve selected beforehand is taken as the rail).
- `BooleanUnion`, `BooleanDifference` (solids to cut from, then solids to cut with) and `BooleanIntersection` (two sets). As in Rhino, the solids used are replaced by the result. Combining turned and moved solids works, and the result can be moved and combined again.
- The commands of `docs/specs/m2-first-surfaces.md` are all in. The owner is trying them by hand.
- Speed with solids (`--orbit-benchmark --solids 500`, RTX 3060): orbiting 500 cylinders takes 3.5 ms a frame (4.2 ms at worst); with 5,000 circles as well, 5.3 ms (7.3 ms). Meshing happens on all cores; opening a model of 500 cylinders spends about 3.5 seconds meshing them once.
- Surface tools (`docs/specs/m2-surface-tools.md`, approved 11 November 2026): `Pipe` (Cap and Thick options) and `OffsetSrf` (Solid option). **Cap is blocked** (#67): the engine sews caps on but leaves the space inside empty; three approaches tried, written up in the issue.
- Bodies are now stored in the engine's native format, which reads back every body tried, where the portable record failed for some (a thickened sheet; a box turned then moved).
- Measuring a solid's volume aims for six significant figures, and the properties panel measures once per change, as a curved solid takes about half a second.
- Picking edges: while a command asks for edges, clicking near an edge picks it (Shift adds, Ctrl removes, a window or crossing box picks many), and picked edges show thick and yellow. `FilletEdge` and `ChamferEdge` round or bevel them, with a live preview and the size remembered; the solid keeps its id, layer and name.
- Picking faces works the same way (a picked face is tinted yellow). `Shell` hollows a solid to a wall thickness, opening the picked faces, with a live preview.
- The surface tools spec is done apart from Cap (#67). The owner is trying them by hand.

**Closing Milestone 1** (`docs/specs/m1-closing.md`, approved 12 November 2026):
- `Curve` (control points, `Degree` remembered, `Close` makes a smooth closed curve), `ScaleNU` (three factors along the construction plane, `Copy`; solids too, round faces become NURBS), `SelSrf` and `SelSolid` (they add to the selection, as in Rhino; `SelCrv` and `SelPt` still replace it), and `Volume` (six significant figures). Telling a solid from a surface now reads whether every edge is shared by two faces instead of measuring the volume.
- Selection filter: `SelFilter` shows a row in the status bar, Points · Curves · Surfaces · Solids · Disable; clicking and boxing pick only the kinds ticked. `SelFilterToggle` (or Disable) turns it off and on and keeps the ticks. The `Sel…` commands ignore it, as in Rhino. Whether a body is a closed solid is worked out once, when it is meshed.

**First release, 0.1.0 (12 November 2026, the owner asked for it):** a preview to show where Curva is. Pushing a tag `v*` runs `.github/workflows/release.yml`, which builds Windows, Linux and macOS (Apple silicon), adds `THIRD-PARTY-LICENSES.html` (made by `cargo-about` from `about.toml` and `about.hbs`) and the notices in `licenses/`, and publishes a GitHub pre-release with the notes in `docs/releases/<tag>.md`. Builds are not signed; on Windows a console window opens beside Curva.

**0.1.1 (12 November 2026):** at the owner's request, releases come as a Windows installer (`.exe`, Inno Setup, per-user, no administrator rights), a Linux AppImage (built on Ubuntu 22.04 for older systems) and a macOS `.dmg` holding `Curva.app` (ad-hoc signed only). Packaging files are in `packaging/`. Pull requests that change the packaging build and check all three without publishing. The icon (`packaging/linux/curva.svg`) is a C drawn as an arc, with a thin yellow line joining its three yellow control points, chosen by the owner on 12 November 2026; Windows and macOS still use the default app icon.

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
- A pipe along a rail with a sharp corner (a polyline) is refused by the engine; Rhino mitres the corner. Smooth rails work.
- solidcore runs one operation at a time behind a single lock, so meshing many solids gains only about 2.3 times from all cores. Faster opening of large models needs that looked at upstream.
- solidcore cannot read back its own record of a body that has been turned and then moved (status 1007, "invalid input"). Curva avoids it by keeping each body as made plus one placement (`crates/curva-kernel/src/body.rs`). The bug should be reported upstream with a small reproducible case; booleans on turned bodies may meet it again.

- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
