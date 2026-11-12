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
- Ghosted display: a third mode beside Wireframe and Shaded, from `SetDisplayMode` or the viewport title. Faces are drawn at half strength over the lines, so edges behind them show dimmed and edges in front stay sharp. Pictures made with `--picture` are ghosted when the script runs `SetDisplayMode Ghosted`; agent pictures follow the viewport.
- Autosave: every 5 minutes, if the model changed, a copy goes to `Curva/recovery` in the user's app-data folder (`%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS, `~/.local/share` on Linux), named by process so two Curvas do not clash, with the model's own file noted beside it. A normal close deletes it. At the next start a copy left behind is offered back; restored, it saves to its original file. Saving a file now writes beside it and renames, so a failed save never leaves a half-written file.
- File format measured (`docs/decisions/0015-curva-file-container.md`): on the test scene, a zip package is as fast as today's JSON (about 25 ms to save or read) and 13 times smaller; redb was slower and larger. The owner chose the zip package, still named `.curva`: files are now a zip holding `model.json`, and the older JSON files still open.
- Both solidcore bugs reported, with the owner's approval and without mentioning Curva: tangency-labs/solidcore#3265 (a turned box's record is refused) and #3266 (stitch with caps leaves no solid, which blocks Cap). Both checked at solidcore's latest code; texts in `docs/upstream/`.
- The closing spec is done. The owner is trying it by hand.

**Floating panels** (`docs/specs/ui-floating-panels.md`, approved 12 November 2026): the viewports fill the window under floating panels with rounded corners: command tabs at the top (Curves, Surfaces, Solids, Operations), Layers and Properties on the right, and a frosted command bar at the bottom with the drawing aids and, on the right, the agent connection. A focus mode slides them away, and users can move and dock panels. The blur is Curva's own, following SolidCraft's method.
- Steps 1 and 2: the panels float over the viewports with rounded corners and shadows, laid out by a layout model (`crates/curva-app/src/layout.rs`) that already allows any panel to dock to any edge or float; the arrangement is kept in `Curva/layout.json` in the user's app data. The command tabs run their commands (a running one is ended first). Focus mode: the arrow tab at the top centre or F12 slides the panels to their edges, leaving a strip; hovering a strip peeks, clicking it brings that panel back; typing brings the command bar out. The viewports sit in the area the panels leave free; drawing them under the panels comes with the glass (step 4).
- Step 3: each panel has a grip at its top-left. Dragging it moves the panel, the place it would land lights up, and dropping it near an edge docks it there (innermost on that edge), elsewhere it floats. Floating panels resize from their bottom-right corner. Right-clicking a grip offers Reset layout. Every change is saved at once.
- Step 4: the command bar is frosted glass. The viewports now draw into a window-sized texture during egui's prepare step and are copied to the screen; the glass blurs its part of that texture (dual Kawase, two levels, Curva's own code in `crates/curva-viewport/src/glass.rs`) and draws it under its tint, rounded to its corners. Viewports reach under the glass bar so the model shows through it. On a software graphics adapter the blur is off and the tint alone shows. Orbit benchmark (5,000 circles, 500 solids, RTX 3060): median frame gap unchanged at about 6.7 ms; work per frame up about 0.5 ms.
- The floating panels spec is done. The owner is trying it by hand.

**Released 0.2.0** (12 November 2026, the owner asked for it): Milestone 1 complete and the floating panels.

**Second round of interface work** (the owner's feedback on 0.2.0, `docs/specs/feedback-2026-11-12.md`; choices in decision 0016). Order: the command framework (`docs/specs/command-framework.md`), picking, snaps and undo (`picking-snaps-undo.md`), the look and layout (`panel-system.md`, then `ui-look-and-layout.md`), a Rhino-style gumball (`gumball-rhino.md`), then logo variations (`logo-variations.md`). **Specs approved by the owner on 12 November 2026**, with every panel frosted glass by default (Glass or Solid in Settings, kept light on the machine). Building the command framework first.
- Command framework, parts 1 and 2: typed `25`, `<30` and `25<30` for every point measured from a base; heights and extrusions follow the mouse along their direction (`Request::along`); a crosshair and a cursor tag for point steps; previews in the accent orange. Commands can now be written as recipes (`crates/curva-commands/src/recipe.rs`): options in Rhino's style, remembered between runs, steps that can change with a mode, one `make` for preview and result. Line (`BothSides`), Arc (centre, typed end angle, `StartPoint` three-point) and Box (`Center`, `BothSides`, Enter uses the width, height following the mouse) are on it. `docs/commands/rhino-reference.md` starts with them, and a test keeps it and the code in step. `AGENTS.md` says how to write a command.
- Command framework, part 3: ghosted solid previews. Commands can hand the surfaces and solids they would make to `Command::preview_bodies`; the viewport draws them see-through in the accent orange in every display mode (`curva_viewport::ghost_faces`), meshed once while they stay the same. Recipes do it from their `make`, and FilletEdge, ChamferEdge and Shell show the rounded, bevelled or hollowed solid.
- Rhino audit (framework step 3): `docs/commands/rhino-reference.md` now has a row for all 100 commands, written from the Rhino 8 help pages: Rhino's steps, its options, which Curva has, and the status. SelName, SelLayer, SelDup, SelFilter and SetDisplayMode could not be read there and are marked to complete from Rhino. A test checks that every command has a row.
- Curves tab on the framework (step 4): Line, Polyline (`Close`, `Undo`), Curve (`Degree`, `Close`, `Undo`), InterpCrv (`Close`, `Undo`), Circle (`2Point`, `3Point`, `Diameter`), Arc, Ellipse (`Diameter`, `Corner`), Rectangle (`Center`, `3Point`), Point, Offset (`Distance`, `ThroughPoint`, `BothSides`), Fillet (`Radius`) and Chamfer (`Distances`). A number typed at Offset's side prompt or at Fillet's and Chamfer's picks sets the distance or radius, as in Rhino. The framework gained finishing options (`Close`), `Undo`, whole-number and length options that refuse bad values, picks on curves, and results that keep their source's layer and history.
- Operations tab on the framework: Move (`Vertical`; picked control points still move instead of objects), Copy (`Vertical`, `InPlace`; each copy is placed at once, as in Rhino), Rotate, Scale and Mirror (`Copy`; Mirror also `XAxis` and `YAxis`), Array, Join, Explode, Split, and the three Booleans (`DeleteInput`). Their previews now show solids ghosted (outlines only past eight solids, to stay smooth). Trim stays as it was for now: it changes the model pick by pick, which the framework cannot state yet.
- Surfaces and Solids tabs on the framework: PlanarSrf, ExtrudeCrv (`Solid`, `BothSides`, `DeleteInput`; Enter uses the last distance), Revolve (`FullCircle`, `DeleteInput`), Loft, Sweep1, OffsetSrf (`Solid`, `DeleteInput`), Pipe (`Cap`, `Thick`, `Diameter`), FilletEdge, ChamferEdge and Shell (`DeleteInput`). The framework gained remembered defaults that Enter gives back, edge and face picks, a single-object step, and answers a recipe can send back to be asked again. Volume, a measurement, stays as it was. The remaining commands (off the tabs) move later.

**Picking, snaps, undo and navigation** (`docs/specs/picking-snaps-undo.md`): a click anywhere on a face selects its surface or solid, the nearest winning, with curves and points still winning within their few pixels; a crossing box also takes surfaces and solids whose faces it touches. In shaded and ghosted views a snap point hidden behind a face is not offered. Resting on a snap point for half a second holds it (up to eight): it sends out dashed guidelines along the plane's X and Y and at 45° (not with ortho), and the pointer snaps where guidelines cross, where they cross objects, and midway between held points, marked in ArchiCAD's symbols. Resting again lets go; the end of the command lets all go. Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z undo and redo even while the command line has focus (unless it holds typed text), and an undo brings back the selection as it was. `UndoView` and `RedoView` (Alt+Z, Alt+Shift+Z) step through each viewport's recent views. Ctrl + right-drag zooms about where the drag began.

**Panel system** (`docs/specs/panel-system.md`): the interface is now made of pieces (the command tabs, Layers, Properties, Command, Agent), each in a panel alone or with others as tabs. Every panel shows its name centred along its top, or a tab per piece with the showing one in the accent; the name or tab is the grip. A V at the end of the title hides the panel, leaving a tab on its edge with its name along it (running upwards on a side); hovering peeks, clicking brings it back. Dragging a panel over another lights it orange and merges them on drop; dragging one tab out makes that piece a panel of its own. Floating panels settle on a 24-point grid, line up with nearby edges within 12 points and never overlap. Panels sit 24 points apart; the command bar is compact and centred at the bottom, and the agent connection has its own panel in the bottom-right corner. Older layout files are read and brought up to date.

**Look and layout** (`docs/specs/ui-look-and-layout.md`): two themes in warm greys, chosen by the system or forced in Settings, covering the panels, text, controls, the 3D background and the grid; the accent orange for selected objects, picked control points, previews, held snaps, the active tab, focused fields and selected text; Inter for the interface (`licenses/fonts/Inter-OFL.txt`). Every panel is frosted glass, or solid if chosen in Settings. The grid follows Serpentine3D's: a faint line every unit and a stronger one every 10, out to 100, with softened axes. A File tab comes first: New, Open, Save, Save As (a new command), Import and Export (waiting for Rhino files in Milestone 2), Settings and About, with Ctrl+N, Ctrl+O, Ctrl+S and Ctrl+Shift+S. Settings holds the theme, glass or solid panels, the unit and the grid. A model is in millimetres or metres, kept in its file; changing the unit asks whether to scale the model or keep the numbers, and the cursor tag shows the unit. Panels resize from their edges and corners (a floating one from all, a docked one from its inner edge and, if shorter than its edge, its ends), the edge pulsing in the accent under a resize arrow; picking objects shows a small box beside the pointer. The command buttons show Serpentine3D's toolbar icons, redrawn in Rust, with names on hover. The list of matching commands is as wide as its longest name, ten rows before it scrolls. Still to come in this spec: the 3D background's top-to-bottom shading.

**First release, 0.1.0 (12 November 2026, the owner asked for it):** a preview to show where Curva is. Pushing a tag `v*` runs `.github/workflows/release.yml`, which builds Windows, Linux and macOS (Apple silicon), adds `THIRD-PARTY-LICENSES.html` (made by `cargo-about` from `about.toml` and `about.hbs`) and the notices in `licenses/`, and publishes a GitHub pre-release with the notes in `docs/releases/<tag>.md`. Builds are not signed; on Windows a console window opens beside Curva.

**0.1.1 (12 November 2026):** at the owner's request, releases come as a Windows installer (`.exe`, Inno Setup, per-user, no administrator rights), a Linux AppImage (built on Ubuntu 22.04 for older systems) and a macOS `.dmg` holding `Curva.app` (ad-hoc signed only). Packaging files are in `packaging/`. Pull requests that change the packaging build and check all three without publishing. The icon is a C drawn as an arc, with a thin yellow line joining its three yellow control points, chosen by the owner on 12 November 2026. Its source and the files made from it are in `packaging/icons/` (`curva.svg`, PNGs at 256 and 1024 px, `curva.ico`); it is in the Windows program and installer, the AppImage, the macOS app (an `.icns` made by the release workflow), the window's title bar and task bar, and the README.

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
- solidcore cannot read back its own record of some turned bodies (status 1007): a box away from the origin, turned about z by certain angles (17°, 29°, 37° and others), is refused; at the origin every angle works. Moving is not the cause, as first thought. Curva avoids it by keeping each body as made plus one placement (`crates/curva-kernel/src/body.rs`). Reported as tangency-labs/solidcore#3265.

- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
