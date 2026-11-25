# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

**Tidy-up first (owner's rule, 19 November 2026, `AGENTS.md`):** every stream starts each session
by completing one open `tidy-up` issue in code it owns, before its next feature; with none open, it
goes straight to features. The open issues are listed in `docs/reviews/code-health-2026-11-19.md`.
*(Added to every status page at the owner's request.)*

## 24 November 2026

**Done**
- **Step 6, part 6: outlines as wide as the style says** (spec §2.1, Outlines: width).
  Outlines used to come in two widths only: thin (two pixels) or thick (three, for widths over
  0.5 mm). They now draw at the style's width in millimetres, turned into pixels for the screen
  (`OutlineLook::pixels`). They are never thinner than the default line's two pixels, and at most
  eight. Shaded's 0.25 mm still draws two pixels and Massing's 0.75 mm three, so nothing looks
  different in the built-in styles. The outline pass now looks up to eight pixels away for a step
  in depth, in a loop that always runs to the end, which DirectX's older compiler needs. New
  tests: on the graphics card, a sphere's outline drawn 2 or 6 pixels wide as the style says; the
  widths in pixels, the built-in styles' included.
- **Step 6, part 5: lines dashed, dotted and see-through as the style says.**
  - Hidden lines now take their style's pattern (solid, dashed or dotted) and opacity. Before, they were always dotted and solid.
  - Curves and edges take the style's pattern too. Objects' own line types still apply as before.
  - Each kind of line's pattern and opacity travel in the line table the graphics card already reads, so a change shows at once.
  - Sizes on screen: dashes 8 pixels with 4-pixel gaps; dots 3 pixels with 3-pixel gaps, as Technical's were.
  - A selected line keeps its pattern but is never see-through.
  - Nothing looks different in the built-in styles: Technical's hidden lines stay dotted.
  - New tests on the graphics card: curves solid, dashed or dotted as the style says; a see-through kind of line lets the background through. Also new: the table keeps each kind's pattern and opacity.
- **Step 6, part 4: lines thin and fade with distance** (spec §2.2 "by distance", §4.1). A style
  whose line rules say "by distance" now draws lines as they are up to its near distance, then
  thinner and fainter toward its far distance, and as thin and faint as it says from there on.
  The line program works this out for each line on the graphics card, so a change to the rule
  shows at once with nothing rebuilt. Distances are in metres, measured in the model's own unit
  (`Style::in_units`; the app passes the model's unit). A perspective view measures from its eye.
  A parallel view measures from the point it looks at, so a plan keeps its look while you zoom
  (its eye moves as it zooms). A selected line never fades. The built-in styles have no such
  rule, so nothing looks different until the Style panel turns it on. New tests: on the graphics
  card, lines thin and fade with distance only where the style says; the distance table in the
  model's units; distances from the eye in a perspective and from the target in a parallel view.

## 23 November 2026

**Done**
- **Step 6, part 3b: views remember a style by name** (spec §2.1: a view holds its own style).
  A viewport now holds a style name beside its display mode: a built-in name just sets the
  display mode, any other names one of the model's styles, looked up as the viewport is drawn,
  so editing it shows on the next frame with nothing rebuilt; if it is deleted, the viewport falls
  back to its display mode's built-in style. Choosing a display mode forgets the style; switching
  a viewport to another standard view keeps it. Saved views now keep the style they were saved
  with (`SavedView::style`, absent in older files) and bring it back when restored, and renaming
  a model's style renames it in the saved views and viewports drawing with it. A viewport's
  background and lines (dark edges for solid faces) now follow its style rather than its display
  mode. In the app (`styles.rs`): `Pane::set_style`, `set_display`, `style_name`;
  `Curva::pane_view_style`, `pane_style`, `set_pane_style`, `rename_style`. Nothing looks
  different with the built-in modes. New tests: a viewport takes a built-in style as its display
  mode and others by name; a viewport shows its model style's edits at once and falls back when
  it is deleted; saved views follow a renamed style.
- **Step 6, part 3a: styles saved in the model and in "My styles"** (spec §2.4, owner's decision
  C), for the Style panel the features stream is building. A model now keeps its own styles in
  its `.curva` file, each as its `.curvastyle` text so settings a later Curva adds survive;
  older files open with none. In `curva-document`: `Document::styles`, `model_style`,
  `style_named` (the model's copy, else the built-in style of that name), `save_style`,
  `delete_style` and `rename_style`, with `StyleError`; built-in names are refused (built-ins are
  only copied), names ignore case and spaces around them. Like saved views, a model's styles are
  not part of undo. The user's library is `style::Library`: `list`, `load`, `save` and `delete`
  over one file per style, named after the style with characters files cannot hold replaced; the
  name inside the file is the one that counts, a file that cannot be read is left out, and a
  half-written file never replaces a whole one. In the app, `styles::library()` is the library in
  `Curva/styles/` in the user's app data. Nothing on screen changes. New tests: built-ins only
  copied and names ignoring case; a view finding the model's style or else the built-in one;
  renaming keeping names unique; a model's styles saved in its file with unknown settings; an
  older file opening with none; the library saving, listing, replacing and deleting files and
  skipping unreadable ones; clashing file names; safe file names.
- **Step 6, part 2d: Curva's own drawer gives each kind of face its style's look too.** Each
  face vertex now carries its kind, packed into the top byte of its object number as lines' are,
  and each view sends a small face table with its line table (`Style::face_rows`): each kind's
  colour (its own or one), opacity and whether it shows, and whether faces are lit flat. One face
  program reads it, in an opaque pass and a see-through pass over the lines; the separate paper
  (Hidden), white (Massing) and ghosted programs are gone. A view whose NURBS faces are off but
  whose meshes are on now draws the meshes' faces. Nothing looks different in the six modes,
  except that locked objects keep their grey faces in Hidden, Technical and Massing (they were
  white), as Rhino 8 draws them with its default settings (measured: a locked box stays grey when
  a mode paints every surface one colour, and is opaque: seen from above it reads 125,125,125
  over a 157,163,170 background). New tests on the graphics card: meshes follow NURBS until set
  apart; a hidden kind draws nothing while others show; meshes alone still draw; a see-through
  kind lets the background through; flat faces take the style's colour unlit while locked ones
  keep their own.
- **Step 6, part 2c: faces of NURBS, meshes and SubD each drawn as the style says.** Every shaded
  shape now carries its kind of object (NURBS, mesh, SubD, or as given for locked objects and
  annotation's filled letters), and each view's style gives each kind whether it shows, one colour
  or its own, and how opaque (`Style::faces_of`). Bevy keeps one material for each colour and
  opacity actually shown, and a style change shows each part again in its kind's new look,
  skipping those that do not change. Meshes and SubD follow NURBS until set apart, as the style
  model says. Locked objects keep their grey but are as see-through as NURBS faces, so Ghosted
  still sees through them; a selected object whose kind has one colour keeps it (its lines show
  the selection), as Hidden's white faces did. Pictures drawn by Curva's own drawer still give
  every face NURBS' look. Nothing looks different. New tests on Bevy's pictures: meshes follow
  NURBS until set apart; a kind's faces hidden show what is behind while others show; a kind's
  opacity lets the background through, and a locked object keeps its own colour.
- **Step 6, part 2b: each kind of line drawn as the style says, on the graphics card.** Every
  line now carries its kind (curve, NURBS edge, seam, mesh wire, SubD crease or border, SubD cage
  edge, annotation, or as given for previews, markers, the grid and locked objects), packed into
  the top byte of the object number it already had, so lines take no more memory. Each view sends
  a small table, worked out from its style (`LineTable::from_style`): each kind's colour (its own
  or one), width (its own times a factor, or fixed millimetres, all times the style's line
  weight) and whether it shows, plus hidden lines' look. The line program looks each line up, so
  a style change will show at once with nothing rebuilt. Hidden's black ink is now just its
  table; the separate ink program is gone. Nothing looks different, except that locked objects
  stay grey in Hidden and Technical (they turned black; Rhino keeps them grey). In the style
  model, mesh wires and SubD lines can now take their object's colour as well as one colour or
  the edges', as they draw today. New tests on the graphics card: an edge takes its style's
  colour while a curve keeps its own; a kind turned off draws nothing while a preview always
  draws; a fixed width and the line weight set how thick lines draw; Hidden inks model lines
  and leaves previews their colour; and the tables of the built-in styles.
- **Step 6, part 2a: the style model and style files** (owner, 23 November 2026, after mock-ups of
  the Style panel: "let's build it… focus on the inherent code and make this well done; UI will
  change after the features stream is done revising the UI look"). The owner's decisions are in
  `views-and-styles.md` §2.5: three engines (Vector, Occlusion, Rendered); settings per object type
  (NURBS, Meshes, SubD, Curves, Points, Text, Lights, Clipping planes), Meshes and SubD starting as
  "Same as NURBS"; one style for 3D views, diagrams, renders and live plans, sections and
  elevations; a simple panel later, a few choices per section and the rest under More.
  **Announced, outside the viewport crate (decision B of the spec):** `curva-document` gains a
  `style` module, `ViewStyle`, where every setting of Rhino 8's display modes has a place (the
  mock-up's table maps them), plus style files (`.curvastyle`, plain text, one setting a line
  under its group's heading): unknown settings kept and written back, a bad value keeps its
  default and is reported with its line, nothing fails to read. The six display modes are
  built-in styles there; the viewport reads them through `Style::of`, and backgrounds come from
  the style. Nothing looks different (one colour, the edges' near-black, moved by less than a
  hundredth to fit a file's 0–255 steps). **Next:** lines carry their kind so a style's widths
  and colours are applied on the graphics card, then faces per object type, then the engines.
- **Step 6, styles as settings: part 1, the display modes as built-in styles** (owner, 23
  November 2026: "Let's start on custom display styles"). A view no longer carries five on/off
  switches (shaded, ghosted, hidden, hidden lines, massing); it carries a style
  (`curva_viewport::Style`) made of the layers of `views-and-styles.md` §2.1: Surfaces (shown or
  not, one colour or each object's, see-through, lit by the view, the sun, or flat), the model's
  lines (own colours or black ink), Outlines (colour, thin or thick) and hidden lines. The six
  display modes are built-in styles (`Style::SHADED` and the others) and look exactly as before;
  the line drawer, Bevy and pictures read the style's settings, not the mode's name. One change
  you can see: a picture script asking for `SetDisplayMode Wireframe` now gets a wireframe picture
  (it was shaded). **Announced, outside the viewport crate:** `crates/curva-app` gains
  `styles.rs` (each display mode's built-in style) and passes styles to views, details and
  pictures. Rhino 8's display-mode settings were exported for the next parts (surfaces' colour
  and transparency, lighting and shadows, edge, curve and mesh-wire widths and colours,
  silhouettes, background). **Next:** part 2, each setting adjustable (colours, opacity, line
  widths, lighting, background), drawn by the same code; part 3, the Style panel to edit styles
  live and save your own.
- **Two glass panels at once each frosted** (features stream's report, seen in `--ui-pictures`):
  with Settings and Layouts floating side by side only one showed frost. Every panel prepares
  its blur before any paints, and there was one blur for all, so the last won. Each panel now
  keeps its own blur, by place and size (the 32 most lately used kept). New test: two panels
  over black and white each show what is behind them.
- **Meshes' wires drawn as plain one-pixel lines** (owner, on the *Meshes* picture: "hard to
  understand what I'm looking at, is the display glitching?"). Not a glitch: each ring has a
  million triangles and every edge was drawn, piling up over the surface. Rhino 8 does the same
  (its Shaded mode shows mesh wires, thickness 1; its rings come out solid black), so the wires
  stay; they are now plain one-pixel lines, two corners a segment instead of six. *Meshes*: 33.7
  to 16.6 ms between frames (the screen's rate), Curva's lines on the card 27.7 to 10.6 ms; Rhino
  about 84 ms. New tests: a wire is one pixel tall over its face; wires show on Bevy's faces in
  perspective as much as with no face; wires are sent apart from other lines. Seen, not fixed:
  where a ring's outline crosses the grid, bits of grid line poke over its edge.
- **Large scenes drew no faces at all, fixed** (found taking pictures of the test scenes for the
  owner). Bevy packs meshes into shared buffers it grows to 512 MB, past the 256 MB Curva's device
  allows; once a scene's faces passed 256 MB (the *Meshes* and *Large* scenes, or any model that
  large without shared copies) Bevy drew nothing, background included, and a picture of a single
  mesh larger than that crashed Curva. Bevy's buffers are now kept within the device's limit and
  faces are handed over in meshes of at most a million triangles. New test: 120,000 triangles on
  a device allowing 8 MB buffers still draw (it fails without the fix). *Meshes* re-measured with
  its faces drawn: 33.7 ms between frames, Curva's mesh-edge lines 27.7 ms of it; one thread as
  fast as many (work 12.7–13.7 ms against 13.0–14.1 ms).
- **The *Meshes* scene measured for the first time** (`--scene meshes`, `docs/performance.md`).
  Meshes have been in the document and drawn since 18–19 November (#266, #267); an older line
  below still says the scene waited for them, and only the benchmark scene was missing. 16 mesh
  rings, 15.68 million triangles: 29–31 ms between frames (target under 16 ms), first frame
  12–17 s, the first snap 14 s. Most of the frame is the graphics card drawing the meshes' edges
  as lines (30.6 ms). Bevy on one thread is as fast as on many here (work 12.2–13.5 ms against
  13.5–13.7 ms, the same time between frames). Next to look at: drawing mesh edges cheaply, and
  whether Rhino 8 shows them in Shaded at all (to measure with its standard display modes). *These first figures were
  wrong, as Bevy drew no faces; corrected in the entry above.*
- **Tidy-up #303 done** (#620): the long functions in `viewports.rs`, `sheets.rs`, `main.rs` and
  `benchmark.rs` broken into named parts under 80 lines, each viewport's drawing moved to
  `pane_drawing.rs`; no change in behaviour (pictures byte-for-byte the same).
- **Step 5, Bevy's fixed cost per frame: part done** (`docs/performance.md`, "Bevy's fixed cost
  per frame"). Measured where an empty scene's 3 ms a frame goes: Bevy's drawing step 1.23 ms, of
  which 0.36 ms is the graphics driver taking the frame; Curva's own window, panels and lines
  about 1.4 ms. Bevy's schedules now run on one thread, and each view uses one light cluster:
  empty scene 3.38 to 2.82 ms a frame, *Medium* 4.00 to 3.73 ms, *Large* 7.00 to 6.87 ms. The
  earlier one-thread attempt broke agent pictures because a new view's first picture needs one
  more update on one thread; it now gets it. A new test checks that changes to a view already
  drawn (new faces, a new shape, a new look) still show on the very next picture.
- **Next:** what is left of Bevy's cost is the driver and Bevy's per-frame buffers; Curva's own
  1.4 ms (window, panels, lines) is the larger part now. Then step 6, customisable styles.

## 22 November 2026

**Done**
- **A less dense grid, and a grid colour for each display mode** (owner, testing 0.1.6, 22
  November 2026: "make the grid less dense, it looks ugly"; "each view mode can have a different
  color for the grid in the settings"). The grid now follows the model's unit as Rhino 8's
  "Large Objects" templates do (measured): in millimetres a line every 10 mm and a major one
  every 100 mm (was every 1 mm); metres and feet every unit, major every 10; inches every inch,
  major every foot. Settings → General → "Grid colour in each display mode" holds a colour for
  each mode (major lines in it, minor ones halfway to the background), with Standard to go back.
  By default Hidden, Technical and Massing now draw the grid as faint dark lines over their own
  light backgrounds, no longer the dark theme's. Grid snap stays at 1 unit.
- **Smooth lines, curves, edges of solids and outlines** (owner, 22 November 2026: "why do lines
  always have a jagged look?"). Planned in `display-pipeline.md` §2.5 but never built, so every
  slanting or curved line was a staircase of solid pixels. Each line now fades out over its
  outer half pixel; outlines of round shapes, found from the depth picture, give the pixels
  beside them a little ink, more in the notch of each step. Costs about 0.05 ms a frame (1.95 to
  2.00 ms, building scene in a release build). Not yet smoothed: the grid's thin lines.
- **Clay renamed Massing, and its ground plane replaced by a plain background** (owner, 22
  November 2026). The grey ground was a 3D plane that disappeared when looking from below the
  model, and no other mode has one: every display mode draws a simple two-colour gradient behind
  the model, as the approved styles plan's Background layer says (`views-and-styles.md` §2.1).
  Massing's now shades from light blue at the top to grey at the bottom; shadows fall on the
  model itself. `SetDisplayMode` option `Massing` (typed `m`).
- **Next (owner, 22 November 2026):** the speed steps 4 and 5 of `display-on-bevy.md`, before
  customisable styles (step 6).
- **Clay display mode, a look like SketchUp's** (#551, the owner's direction: white surfaces,
  black outlines thicker than inner lines, simple shadows, a light blue sky and a grey ground;
  plain, with no sketchy edges). Faces are white and lit by the sun, casting shadows on a grey
  ground at the model's height 0 that reaches to the horizon; the model's lines are thin and
  black, the outlines of shapes three pixels wide; the sky is light blue overhead and paler at
  the horizon. In the Display panel, `SetDisplayMode` (`Clay`), detail views and pictures. Named
  Clay rather than after SketchUp, and because `S` already means Shaded in `SetDisplayMode`.
  SketchUp's exact default colours were not measured (it is not installed here); the owner chose
  the colours above.
- **Picked parts of meshes and SubDs show, and a SubD's cage while it is edited** (#334,
  first part; #563 and this). Points, edges and faces picked with Ctrl+Shift were invisible;
  now they are drawn in the selection colour, as Rhino highlights them: points as dots, edges
  thick, faces tinted. On a SubD shown smooth they lie on the smooth surface (a point at its
  limit point, an edge along its run, a face as its patch), from the mesh stream's
  `edges_on_surface`, `limit_points` and `surface_with_faces` (#566); shown flat, on the cage.
  They are kept between frames, so a still highlight costs nothing. A SubD with its points on
  or a part picked shows its cage as thin dotted lines, as Rhino 8 does; with its points on,
  the cage's points are drawn and picked as control points (#564).
- **Dragging a SubD's cage points redraws it with no subdivision** (#334, item 4). Each smooth
  SubD keeps the mesh stream's `LiveSurface` (#566, #569): the subdivision's layout and weights
  are found once for the cage's shape, and every new set of cage points only re-applies the
  weights, for the surface and for the cage edges and creases on it. A new shape, new creases
  or a new level make it again; a SubD with face colours or shown flat draws as before. Even an
  ordinary change now subdivides once instead of three times (surface, edges, creases). Not yet
  timed in a release build.
- **Technical display mode, as in Rhino** (#164, second half; #562). As Hidden, with the lines
  faces hide drawn dotted, black dots on the white, as Rhino 8's Technical mode shows them. In
  the same places as Hidden. Not yet: the hidden parts of curved shapes' outlines (Rhino dots
  those too), which the depth picture cannot find; they need outlines worked out from the
  shapes themselves.
- **Hidden display mode, as in Rhino** (#164, first half; #560). Surfaces are flat
  white paper on a white background, hiding what is behind them, and every visible edge, curve
  and outline of a curved shape is drawn in black ink, as Rhino 8's Hidden mode looks (measured
  there on a box, cylinder and sphere). It is in the Display panel's list and in
  `SetDisplayMode`, works in detail views on sheets (`Hidden` as a detail's display), and
  snapping in it finds only what is in sight, as in Shaded. Previews, the selection and snap
  markers keep their colours. Outlines of curved shapes are now found from smaller depth steps
  (1% of the distance instead of 3%), so thin posts and tree trunks standing on a floor keep
  their outlines all the way down; shaded views look the same.

## 21 November 2026

**Done**
- **Meshes and SubDs show their wires in shaded views, as in Rhino** (#472). The owner saw
  meshes drawn with no lines at all. Read from Rhino 8's own settings and checked in pictures,
  its Wireframe, Shaded and Ghosted modes all draw every edge of a mesh, and every edge of a
  smooth SubD on its surface, thin and in the object's colour. Curva drew them in wireframe
  only; now it draws them in all three, in the object's colour. A body's edges and a SubD's
  creases and borders stay dark where faces show, as before.
- **The wheel zooms towards the mouse, as in Rhino** (#469, #501): the point under the mouse
  stays still, not the centre of the view.
- **The Display panel shows "Perspective" whole, and its edges resize properly** (#470). It
  opens wider (now 290 points; 180 before), also from a layout saved narrower. Dragging a panel's
  edge now moves it from where the drag began by the whole way since, so the edge stays under
  the pointer after the panel stops at its smallest; and the layout is written once when the
  edge is let go, not on every frame of the drag.
- **The view picker says and shows where each view looks from** (#471, the owner's choice of
  option 2 from a mock-up). Each view is a filled tile with a small line drawing of a house
  seen from its side (the plan from Top, the gable from Front and Back, only Front with a door,
  the long side from Left and Right, only the edges in sight in the two perspectives), the view
  shown in the accent. The caption reads "Look at the model from", the button "View: Top", and
  resting on a view says what it looks at ("Top: looking down, at the plan"). The Display panel
  now opens 290 points wide so the longer button fits.
- **In focus mode a panel at the top goes to the window's edge, as the others do** (#473). The
  Operations bar slid only to where the top bar had been, which had slid away itself, so it
  stayed floating; now hidden panels slide to the edges of the whole window in focus mode, and
  a top panel's tab moves aside from the focus tab at the top centre.
- **Picking faces no longer draws long yellow lines across the view** (#522): picked faces are
  tinted as one mesh, exactly to their corners.
- **Tidy-up** (#303, #505): the shortcut keys are split into three functions, in their own file.

## 20 November 2026

**Done**
- **SubDs show** (#324, spec `mesh-and-subd.md` Stage B, items 1 to 4). SubDs from `.3dm`
  files and SubD tools were in the model but invisible. Now a SubD shades as its smooth
  surface, as Rhino draws it, with its creases and open borders drawn on it and its outline
  following the view; shown flat, its cage's own faces shade flat with all its edges; a click
  on it picks it, as a solid when its cage is closed; and a print shows its lines. Each SubD is
  subdivided as finely as about 50,000 quads allow (a cube's cage four times, a cage of a few
  thousand faces twice), which takes about 60 ms for a cage of 384 faces. In wireframe a
  smooth SubD also draws its cage's edges on the surface, as Rhino does, using the mesh
  stream's `edge_curves` (#438); views showing faces leave them out, and creases and borders
  are drawn once, not twice. Faces coloured one by one show their colours, smooth (from the
  mesh stream's `surface_coloured`, #461) or flat, and the rest the object's colour, as in
  Rhino.
- **Snapping is under 1 ms a frame on the largest test scene** (#130). With a whole large
  scene in view, thousands of snap points were within reach of the cursor, and each was measured
  and sorted; now snapping searches outwards from the cursor and stops as soon as nothing nearer
  can exist, choosing the same point as before. *Large*: 1.19 ms to 0.14 ms a frame. The search
  structure is now also built in the background after each change, so the first snap after an
  edit rarely waits for it (it took half a second on *Large*). The benchmark now reports that
  first snap on its own instead of hiding it in the average.
- **Edits in a drawing full of dimensions no longer pause.** After any change, every dimension
  and hatch was worked out again, because their look follows the model's unit and styles; now they
  keep what they drew while those settings stay the same. On the owner's 3.6 MB drawing, each
  edit went from 0.43 s of redrawing to 0.043 s. Text with fields, scale bars and tables are
  still redrawn each time, as they read other things.
- **Large scanned meshes are quicker to work with** (#267, the speed left over from Stage A).
  A mesh's edges were each listed as their own line, so a scan of a million triangles held
  about 1.5 million lines; now its edges are joined end to end into chains of up to 256 points,
  some thousands for that scan, drawing exactly the same edges. On a test mesh of a million
  triangles, working it out went from 1.4 s to 0.75 s, and handing its lines to the window from
  140 ms to 45 ms.
- **Clicking to pick no longer slows down in big models.** A click looked at every point of
  every object; now it looks only at objects whose box is near the cursor, using the box tree
  snapping already had, and a click on a face skips meshes nowhere near it. On the same test
  mesh, a click went from about 450 ms to under 1 ms, and a click on a face away from its edges
  from 150 ms to 35 ms. The first click after a change no longer builds snapping's whole
  index (0.9 s on that mesh); picking builds only the small part it needs.

**Next**
- A click on the face of a scan still checks each triangle (35 ms for a million); a box tree
  per mesh would make it instant, if it shows up in use.
- Orbit-test the owner's larger project files once the features stream makes import faster.

## 19 November 2026

**Done**
- **Meshes show their vertex colours** (#267, Stage A item 3): a mesh that carries colours, per
  vertex or per corner, shows them instead of its layer's colour, on screen and in pictures; the
  selection colour still shows when it is picked. That finishes #267's Stage A, except speed for
  meshes with millions of edges.
- **The camera offers all the views the new view picker shows** (#356, for the features stream's
  picker). Bottom, Left and Back views as Rhino's, each with its own drawing plane; a two-point
  perspective that stays level, so upright lines stay upright and parallel on screen (dragging up
  and down does not tilt it); and an adjustable lens, read and set in millimetres or as Rhino's
  angle of view (50 mm is 39.6°), kept when the view is turned. Saved views keep the lens too
  (the field added by the features stream); views saved before open at 50 mm.
- **Edits in a drawing full of blocks no longer pause for a second or more.** After any change,
  every block was worked out again; now an unchanged block keeps what it drew while its
  definition and the settings it reads stay the same. On the owner's 3.6 MB drawing, each edit
  went from 1.5 s of redrawing to 0.33 s.
- **A drawing full of text opens about 19 s sooner.** Letters and arrowheads were each made
  into a flat face by the modelling engine, about 3 ms a letter: a 3.6 MB drawing of the owner's
  with 356 texts spent 19 s on them before its first frame. They are now filled in their plane by
  lyon (decision 0030); working out that drawing went from 24.6 s to 5.6 s.
- **Meshes show and can be picked** (#267, spec `mesh-and-subd.md` Stage A, items 1, 2 and 4).
  Meshes from `.3dm` files and mesh tools were in the model but invisible. Now they shade, from
  their own normals where they have them (an unwelded edge shows as a crease) or else smooth
  across welded edges, as Rhino does; their wires show in wireframe and are left out where faces
  show; a click on a mesh picks it as a solid when closed, a surface when open; and a print shows
  its borders. Still to do: vertex colours (item 3), and speed for meshes of millions of edges
  (each wire is listed as its own line for now).
- **The owner's own project files orbit** (asked by the owner through the features stream).
  The benchmark can now open a file (`--open`); the seven files that read in under a minute were
  orbited, all fine except one 3.6 MB drawing, whose text and block splines made 14.9 million
  points (27 ms a frame on the graphics card). Text, dimensions and hatches now keep only the
  points their shape needs, and splines nearly all; that drawing is now 1.1 million points and
  1.9 ms. What still slows it is the Layers panel, 15 ms a frame there, passed to the features
  stream. Figures in `docs/performance.md`; the files stay on the owner's machine, unnamed.

## 18 November 2026

**Done**
- **Curva uses about 1 GB less memory on *Large*** (#153). Copies of a body no longer each keep
  a mesh of their own just for clicking and snapping; picking moves the one shared mesh into place
  as it looks. Each object's points are also held once instead of twice. Peak memory in use went
  from 2.1–2.4 GB to 1.3–1.5 GB, peak reserved from 3.8 to 2.8 GB; frames are no slower
  (`docs/performance.md`). All existing picking and snapping tests pass unchanged.
- **The perspective view uses Rhino's standard lens** (owner, testing 0.1.3: the 3D view's camera
  looked different from Rhino's). Its lens was a 40° field of view, wider than Rhino's 50 mm on
  35 mm film (about 27° from top to bottom), so models looked more stretched toward the edges. Now
  50 mm, in Curva's lines and Bevy's surfaces alike; the starting view stands farther back (228
  instead of 150) so it frames what it did. Flat views keep their scale. Not yet as Rhino: in a view
  taller than it is wide, Rhino fits the 24 mm to the width; Curva always fits it to the height.
  New test: `the_perspective_lens_is_rhinos_50_mm`.
- **The grid shows again in 3D and flat views** (#344, reported by the owner). Since step 3c
  (#255) took out Curva's own surface drawing, views Bevy draws only got Curva's second part
  (lines, previews), and the grid was in the first part, so it was never drawn. Now it is drawn
  on Bevy's picture before the outlines, only where Bevy drew nothing, so surfaces lying on it
  or near it cover it as before. On the way: Bevy's background is a flat surface just inside
  the farthest distance, which the depth handed to Curva's lines took as a surface; it now counts
  as nothing drawn. New test: through the window's own path, the grid shows beside a face lying
  on it and not through it. The grid's place far from the origin is unchanged (#305).

## 17 November 2026

**Done**
- **Moved from V: to E:** (repository `E:\curva`, worktrees `E:\curva-<issue>`). The stack of
  steps 2 and 3 was brought up to date with main; the "weakened tests" failures on #253 and #255
  were the branches being behind main, gone after merging it.
- **Merged:** step 2 lines (#248) and clipping (#249), the Bevy guide (#256, `docs/bevy/guide.md`),
  the kernel's shared mesh and placement (#278), step 3, Bevy as the only 3D engine (#250, #253,
  #255).
- **Step 4, copies drawn once, merged** (#281): *Large* opens in 4.4–4.6 s instead of 74 s, and
  uses about 1.3 GB of graphics memory instead of 3.2 GB (two runs each, `docs/performance.md`).
  Placing a block instance used to remake its body in the engine, so every tree was meshed again;
  now an instance that moves, turns, scales evenly or mirrors shares its body, and Bevy holds one
  mesh per shape and draws every copy at its place.
- **Step 4, circles drawn from their axes, merged** (#299): *Large* now uses 800 MiB
  of graphics memory steady instead of 1312 MiB, under the spec's 1 GB. Most of what was left was
  the 40,000 circles, 128 segments each (328 MB); the copies' edges were only 38 MB. A whole,
  undashed circle is now one entry, and the graphics card works out its segments.
- **Step 4, far from the origin, merged** (#305): a window scene is measured from a point near
  the model (its middle, to the nearest metre), worked out in double precision before narrowing,
  and its camera, grid, previews and clipping planes are moved to match; shared meshes are
  measured from their own middle. New *Far* checks: a model 500 km out draws the same picture as
  at the origin, on Curva's lines (310 pixels differed before) and on Bevy's surfaces (576).
  *Far* runs as fast as *Large*. Custom part `far-origin` listed; `big_space` checked and not used.
  Drawing only: document coordinates, saved files, copy and paste, snapping and the coordinates
  shown stay exactly where they are (owner's rule, so pasted objects land alike across files).
- **Step 4, Bevy's settings in millimetres, merged** (#312): checked by a new test, a box casting its
  shadow drawn 10 mm and 10 km across gives the same picture. Shadow distances already followed
  the view; one gap fixed: a view turned Sunlit without moving kept shadow distances set for an
  earlier view (a second new test, which failed before the fix).
- **Step 4, far from the origin in pictures and sheets, merged** (#316): pictures for agents (and
  `--picture`, `--look-pictures`, styled pictures) and layout sheets' details are measured from
  the middle of the model too, so a far model's pictures are as sharp as the window. New test: a
  circle and a box 500 km out picture as they do near the origin. The picture device is no longer
  dropped when its thread ends (it panicked a test's thread on exit).
- **Step 5, snapping searches only what could win, merged** (#321, part of #130): only segments
  nearer the cursor than the nearest place found are searched for meeting points and
  perpendicular feet. *Large*: snapping 20.6 ms to about 4 ms a frame, 24 ms to 16.2 ms between
  frames (the screen's own rate). A test compares with searching everything at 4,900 cursor
  positions. #130 stays open: its aim is under 1 ms.
- **Step 5, selecting, merged** (#340): under 1 ms more than an ordinary frame, as the spec asks
  (*Large* about 0.2 ms with a solid selected, *Medium* 0.4–0.7 ms). Two causes: Bevy was handed a
  material for every part in every view at each change (12,000 on *Large*), now only for the
  parts of objects whose selection changed; and the benchmark itself listed every selectable
  object at each change (10 ms on *Large*), now once. New test: a new selection recolours only
  what changed, and rightly.

**Next**
- Step 4, still to do: picking and snapping on the shared shapes instead of placed copies
  (Curva's own memory: *Large* peaks at about 2.3 GB).
- Tried and set aside: running Bevy's schedules on one thread cut the processor's work per frame
  from 3.4 ms to 2.6 ms on an empty scene (4.4 to 3.8 ms on *Medium*), but agent pictures then
  come out wrong (`a_far_model_pictures_as_at_the_origin`: 61,166 pixels differ), so it is not
  merged. Probably the order Bevy's systems run in for the first picture of a scene; worth a look
  as part of the fixed cost. Leaving out the unused anti-aliasing and post-processing features
  saves only about 0.1 ms.
- Step 5, still to do: Bevy's fixed cost per frame; the *Meshes* scene, which waits for mesh
  objects in the document (#266, features) and drawing them (#267, display); snapping under 1 ms
  (#130); Rhino's figures beside every target.

**Heads-up for the features stream**
- `curva_kernel::Body::transformed` now gives a copy sharing the body when the map only moves,
  turns, scales evenly or mirrors (as `Body::mapped` does), instead of remaking it in the engine.
  Same shapes, same results; shears and uneven scales are unchanged. New: `Body::shared_mesh`,
  `Body::placement`.
- `curva_viewport::Drawing` has a new field `shapes`, and `SceneParts::shared` and
  `scene_shapes_without` are new; `SceneParts::of` is unchanged.
- `Drawing` and `SceneParts` also have a new field `circles` (whole circles drawn from their
  axes); `SceneParts::shared` leaves those circles out of its lines. Pictures and sheets pass
  `circles: Arc::default()` and keep drawing circles as segments.
- Far from the origin (#225): `SceneParts` has a new field `origin`; `ghost_lines`, `ghost_faces`,
  `grid_lines_in` and `scene_faces_without` take the origin to measure from, and
  `Camera::measured_from` and `narrow_from` are new. In the app, `clipping_for` and `clipped_view`
  take it too (sheets pass the origin, `[0.0; 3]`).

## 16 November 2026

**Done**
- **Plan for the display on Bevy approved** (`docs/specs/display-on-bevy.md`, #242): seven steps
  (faces, lines, Bevy only, copies and far coordinates, speed, styles as settings, plans and
  sheets), targets including "faster than Rhino" on every scene. Owner's decisions: Bevy becomes
  the only 3D engine at step 3; Zebra and Draft angle dropped for now; machines without a graphics
  card run Bevy on software graphics.
- **Decisions 0029 and 0030 merged:** Bevy draws every view, 3D and 2D (Vello not adopted, after
  the plan accuracy test #235); use Bevy as it ships, custom parts marked, listed
  (`docs/bevy/custom-parts.md`) and checked at every upgrade; `architecture.md` §6b on how Curva,
  solidcore and Bevy fit together.
- **Step 1, faces in Bevy, in review** (#223): each Bevy view has its own copy of the objects on
  its own render layer, sharing meshes, with Bevy's own materials and lights. The owner chose the
  Studio look for base Shaded (a light from the viewer, slightly above and to the left, one from
  above, Bevy's gradient environment reflected, shine 0.6), after pictures beside Rhino's Shaded;
  Sunlit (a sun with shadows) for presentation. Curva's own background gradient stays, as a flat
  backdrop. Selection, Wireframe, Ghosted. `--bevy` on the benchmark.
- **Rhino measured on the same scenes** (the owner's reference, `docs/performance.md`): all four
  views about 10 ms on *Medium*, 167 ms on *Large*; Curva 2.9–3.4 and 24.9–26.2 ms; Bevy faces
  5.7–5.8 ms on *Medium* (Bevy's fixed cost per frame, about 2.5 ms, to work on in step 5).
- **Pilot step 1 merged** (#228): Bevy running inside Curva on its own device. A clean build takes
  about twice as long (306 → 601 s, debug). The owner allowed the MIT-0 licence (`encase`).
- **Disk full on V:**, builds failed. Cleaned the display stream's old build folders and the main
  folder's stale debug cache (about 60 GB). From now on worktrees and spike build folders are
  removed when done.
- **The owner chose Bevy for the 3D views** ("go ahead with your recommendation"): decision 0028
  (#217, merged). Bevy draws faces, meshes and SubD; Curva keeps its CAD layer. 0027 marked
  partly replaced (its 3D sentence).
- **Pilot spec approved by the owner** (`docs/specs/bevy-pilot.md`, #218): one view behind a
  setting, the pass targets as written; Bevy's own lines considered and not used (no
  paper-millimetre weights, exact arcs, outlines, cuts, snapping or precision far away). Its six
  steps are issues #222 to #227.
- **#180 tidy-up merged** (#220): `lib.rs` and `gpu.rs` split into modules named by what they
  do, the nine functions over 80 lines broken into named steps, tests unchanged (119 pass). Speed
  unchanged: *Medium* 2.6–3.0 → 2.7 ms between frames, *Large* 23.1–23.2 → 23.3–23.4 ms, card time
  the same, two runs each on a quiet machine.
- **#199 viewport-lib and #198 Bevy tried inside Curva, reported, decided (0028), issues closed.** Both on
  Curva's device, from Curva's camera, on the same scenes (`spikes/common/scenes.rs`, shared by all
  renderer spikes), each measured twice on the RTX 3060. Branches `199-viewport-lib-spike` and
  `198-bevy-spike`, sources only, not merged. *Large* at 1920×1080: viewport-lib 38 ms per frame
  with lines kept (540 ms its normal way), 1 GB; Bevy 13.4 ms (21 ms with SSAO, TAA, shadows), 374 MB,
  first frame under 1 s. Curva today at 1280×800: *Large* 22 ms on the card, first frame 62 s, about
  2.1 GB; the difference is mostly instancing (step 11).
  - viewport-lib: GPL-3.0-only (clashes with decision 0025), one author, a crash with 4-sample
    smoothing plus lines, no splitting of big line blocks. Recommendation: borrow ideas only.
  - Bevy: MIT/Apache; SSAO and TAA need more than Curva's baseline device (allowed on recommended
    cards by decision 0026); metres assumed; single precision far away; no cuts, selection outline
    or usable picking. Recommendation: adopt with conditions (updated for meshes and SubD).
  - Updated for meshes and SubD (owner, 16 November): two more tests. Big one-off meshes
    (15.7 million triangles): Bevy 3.8 ms per frame and 569 MB, viewport-lib 11.9 ms and 1.2 GB;
    Curva's face format would need about 1.9 GB (120 bytes per triangle, no shared corners). SubD
    surface of 524,000 triangles changing every frame: Bevy 4 ms, viewport-lib 11 ms. On Curva's
    own route, meshes bring back steps 13 to 15 and add shared corners, textures, mesh wires and
    live edits.
  - Comparison of the three routes: adopt Bevy for the 3D views on conditions, Curva keeping its
    CAD layer (lines, outlines, cuts, styles, picking, precision). First a pilot the owner approves:
    Bevy draws one view's faces behind a setting, Curva's lines and outlines as a Bevy pass.
- Status order updated (#200, merged).

## 15 November 2026

**Done**
- **Snapping about twice as fast** (#130, first part, merged as #151): the objects and snap places sit in a tree
  of boxes made once per model change, so snapping looks only near the cursor and finds the same
  points. Snapping per frame: *Medium* 3.2 → 1.3 ms, *Large* 45 → 20.5 ms; frame gaps *Medium*
  4.8 → 2.6 ms, *Large* 47 → 23.5 ms. What is left on *Large* is crowding (zoomed all the way out,
  115,000 segments within the cursor's reach); #130 stays open for it.
- Step 4b, **selecting no longer freezes Curva** (#144, merged as #148): one selection change took 117 ms on
  *Medium* and 2.3 s on *Large*; now 5.7 ms and 57 ms, about an ordinary frame. Scene vertices
  carry their object's number and the graphics card looks it up in a selection table (one byte
  per object), drawing selected objects in the selection colour and lines a pixel wider, exactly
  as before (a test compares the pictures pixel for pixel). The benchmark measures it with
  `--select`. Cost: the card's time per frame is up 8–17%. A change on *Large* still costs about
  9 ms more than an ordinary frame (the spec aims under 1 ms): to look at.
- Step 4a, **Large opens** (#141, merged as #143): uploads of lines and faces go in blocks of at most 64 MB, and
  views showing the same lines share one copy instead of each sending its own. *Large* orbits at
  47 ms between frames (it crashed before); *Medium* is unchanged. Its first frame takes 58 s and
  it holds about 2.1 GB on the card, both from the 2,000 trees written out one by one (step 11).
  The benchmark's work figure now leaves out the opening frames. Fixed on the way: the card's timer
  dropped passes shorter than one tick of its clock, which made its test fail now and then.
- Step 1c, the baseline kept (#138, merged as #139): pictures made without a window (for agents and `--picture`)
  ask for their device as the window does, and a test draws a shaded face and lines of three
  widths with every optional extra off and on and checks the pictures are identical. Step 1 is
  complete.
- Step 1b, the graphics card's own time (#132, merged as #137): the benchmark reports it, read from the card's
  clock. *Medium* 1.5 ms, reduced *Large* 7.1 ms per frame; the processor is the bigger cost on both
  (`docs/performance.md`). The window now asks the card for its device through
  `curva_viewport::device_descriptor`: the baseline limits, plus optional extras only when present.
  Found on the way: building *Medium*'s 500 cylinders takes about 35 s, about 60 ms per extrusion in
  the engine (#136, for the features stream).
- Step 1a, benchmark scenes (#129, merged as #131): `--orbit-benchmark --scene medium|large|far`,
  plus `--splines` and `--trees`; the report says what the scene holds, how long it took to build and when the first
  frame showed. Baseline on the RTX 3060 in `docs/performance.md`:
  - *Medium* orbits at 4.8–4.9 ms between frames.
  - ***Large* does not open: Curva crashes.** All lines, and all faces, go into one block of
    graphics memory, and the card refuses a block over 256 MB; each part of *Large* alone is too
    big (curves 376 MB, solid copies 580 MB, trees 1,154 MB). The largest scene that opens holds
    roughly 35,000 circles' worth of lines. Steps 4, 11 and 13 remove this ceiling.
  - Reduced *Large* (20,000 circles, 5,000 splines, 4,000 solids) orbits at 20 ms between frames,
    of which **snapping takes 17.6 ms** (#130).
  - Measured on a 1280×800 display (the machine was reached over Parsec); the spec's targets are
    for 1920×1080.
- Spec for the new display pipeline approved and merged (`docs/specs/display-pipeline.md`, #127):
  15 steps (§7). Decisions: no Intel test machine, so the baseline is kept by design and by margins
  on the RTX 3060 (§5); the display stream makes the few changes outside the viewport that §8
  lists, announcing each here first; display modes live in the user folder, later to be tied to a
  Curva online account.
- Status split per stream and the display stream's rules in `AGENTS.md` (#125).

**In progress / next** (owner, 15–16 November 2026: decisions 0024, 0026 and 0028)
- **Recommended graphics card** (decision 0026): an RTX 3060, or an AMD or Intel card of the same
  class, with fallbacks for weaker machines. Speed targets are set on the RTX 3060 itself
  (`docs/specs/display-pipeline.md` §5 and §6); 2020 Intel integrated graphics are the fallback
  case.
- **3D direction** (owner, 16 November 2026, decision 0028, replacing the 15 November direction):
  Bevy draws the 3D views' faces, meshes and SubD, with its shading, shadows, ambient occlusion,
  smoothing and instancing; Curva keeps its CAD layer (paper-millimetre lines, exact arcs, dashes,
  outlines, cuts and fills, styles, picking and snapping, precision far from the origin). A pilot
  comes first. re_renderer and viewport-lib are not used as renderers; re_renderer's line and
  outline technique may still be borrowed for the CAD layer.
- **2D direction** (decision 0029, replacing 0027): Bevy draws plans, sections and sheets too,
  from the drawing description; Vello not adopted.
- Order of work (owner, 16 November 2026):
  1. **The display on Bevy** (`docs/specs/display-on-bevy.md`, approved): step 1 faces (#244),
     step 2 lines and clipping planes (#248, #249) in review; step 3a, Bevy draws every view, in
     review; then 3b pictures for agents through Bevy, 3c Curva's own surface drawing deleted.
  2. **Closed:** #180 (merged), #186, #198, #199 (decided by 0028; spike branches kept unmerged for
     reference), #197 (superseded; its Diagram-look comparison is in pilot step 5).
  3. **Retired:** steps 5 to 15 of `display-pipeline.md` (Bevy has instancing and culling; exact
     curves move into the line drawer); #153 to close once step 4 lands.
- **Shelved** (decision 0024): steps 14 (occlusion culling) and 15 (clusters, mesh shaders), back
  only if a measured real model is still too slow after steps 11 to 13. The styles prototype's
  third look (Textured) and fine-tuning of the looks wait until the drawing is correct.
- **Correct drawing:** seams no longer drawn in shaded views (#183, merged); outlines of curved
  shapes in shaded views (#189); still to do: outlines in Ghosted and Wireframe (then seams go
  there too), edges that stay whole where a surface is seen at a steep angle, and edges that
  follow the true shape when zooming in (spec steps 5 to 7).
- **Styles prototypes merged:** Diagram (#168: white surfaces, thin grey inner lines, bold
  outlines, soft shading and ambient occlusion) and Architectural (#173: a plan cut at 1.2 m with
  solid black cut walls and lines that thin and lighten with distance).

**Heads-up for the features stream** (changes outside the viewport, decision B)
- **Bevy steps 1 to 3a (#244, #248, #249, and 3a):** `curva_viewport::Drawing` has a new field
  `camera` (the view's camera); `Engine::Bevy` draws every model view and every detail on a sheet
  (`viewports.rs`, `sheets.rs`). The "3D engine" choice, `Pane::engine`, its three words in
  `en.txt` and the benchmark's `--bevy` are gone again. `curva --look-pictures <folder>` makes
  pictures of Bevy's looks; `save_bevy_picture` draws a picture through the window's own path.
- **Bevy pilot step 1 (#228):** `Pane` in `crates/curva-app/src/viewports.rs` has a new field
  `engine`, and the view's menu a "3D engine" choice; `curva_viewport::Drawing` has a new field
  `engine` (`Engine::Curva` keeps today's drawing, as `sheets.rs` does); `main.rs` stores a
  `curva_viewport::PilotSeed`; three new entries in `crates/curva-commands/locales/en.txt`. The
  viewport crate now depends on Bevy 0.20.1, which lengthens clean builds.
- **`curva_kernel::BodyMesh` has a new field `seams`** (one flag per edge): edges where one face
  meets itself, which the engine does not mark (`seam_edges` in `crates/curva-kernel/src/body.rs`).
  Shaded views leave them out; wireframe, previews and prints keep them until outlines of curved
  shapes are drawn there.
- Done: `crates/curva-app/src/benchmark.rs` and the `--orbit-benchmark` arguments in
  `crates/curva-app/src/main.rs` (#131); the window's graphics set-up in `main.rs` now uses
  `curva_viewport::device_descriptor` (#132).
- Done in step 4b (#144): `refresh_scene` in `crates/curva-app/src/viewports.rs` builds the scene
  with `curva_viewport::SceneParts` once per model change and hands the selection to the viewport
  through `Drawing::objects` and `Drawing::selection`; `sheets.rs` and `clipping.rs` give the new
  fields. Pictures for agents still draw the selection into colours, unchanged.
- **`curva_viewport::Vertex` and `MeshVertex` have a new field `object`** (#144). Code that makes
  them by hand sets `object: curva_viewport::NO_OBJECT`, as `clipping.rs` now does.

**Open `display` issues**
- #130: snapping now goes through an index of the objects (about twice as fast); what is left on
  *Large* is crowding: zoomed out, 115,000 segments lie within the cursor's reach.
- #136 (not display, for the features stream): extruding a circle takes about 60 ms.

**Where the display stands today** (history before this date is in `status/features.md`)
- Wireframe, Shaded and Ghosted modes, fixed in code. Solids are meshed once, to a tolerance set
  by their size, and kept until they change. Curves are cut into a fixed number of segments
  whatever the zoom (a circle always 128), so arcs look like polygons when zoomed in. Edges are not
  anti-aliased.
- Lines are drawn once as strips with their own width (`docs/performance.md`).
- Frosted glass panels blur the viewport texture (`crates/curva-viewport/src/glass.rs`).

**Known problems / risks**
- *Large* opens but needs about 2.1 GB on the graphics card; on integrated graphics, which share
  the computer's memory, a model that size could still fail until steps 11 and 13.
- No measurements on the target low-end machine (2020 Intel integrated graphics); there will be
  none (owner's decision A).
- Benchmarks are noisy when another session builds on the same machine: rerun any result that
  looks off.
