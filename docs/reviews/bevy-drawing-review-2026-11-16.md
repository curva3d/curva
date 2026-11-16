# Review: what Curva's drawing specs ask of Bevy

*Display stream, 16 November 2026, for decision 0029. Read against `views-and-styles.md`,
`live-plans.md`, `line-weights.md`, `layouts.md`, `annotation.md`, `make2d.md`, `drawing-output.md`,
`blocks.md` and `architecture.md` §5, §5b and §20, with Bevy 0.20.1's own code where a question
turned on it.*

## The main finding

Bevy's way of working is **one scene, many cameras**: an object's look (its material) belongs to the
object, and every camera sees the same look. Curva's specs ask the opposite in several places: **each
view has its own** style, layers on and off, sun, clipping planes and, later, choice of object
representation, and four viewports can show four different views of the same objects at once.

None of this is blocked, but it decides how step 2 of the move (#223, faces into Bevy) is built, so it
is settled there before any faces are drawn:

| What differs per view | How Bevy can do it | Checked |
|---|---|---|
| Which objects show (view layers, per-detail layers, representations) | Bevy's *render layers*: each object carries the set of on-screen view slots it shows in; each camera one slot | Yes: built for this |
| The sun | One light per view slot, on that slot's render layer: Bevy uses a light only in views whose layers it shares (`bevy_pbr` `prepare_lights`) | Yes, in Bevy's code |
| Ambient light | Bevy has a per-camera ambient override | Yes, in Bevy's code |
| Style: surface colour, opacity, lighting, zebra, draft, ghosted | **Not per camera in Bevy.** Two ways: (a) a copy of each object per view slot, sharing its mesh, each copy with that view's material; (b) one Curva material whose shader reads the view's style from a per-view table | To measure in step 2 |
| Clipping planes (enabled per view and per detail) | Same choice as style; plus the shadow and ambient-occlusion passes must clip too, or cut-away parts still cast shadows | To build in step 2 or 3 |

Recommendation for step 2: try (a) first, as it is plain Bevy and copies share their mesh on the card;
measure *Large* with four views. Move to (b) only if the per-copy cost on the processor is too high.

## Spec by spec

### Layers and views (`views-and-styles.md` §1.4, `layouts.md` §2)
Modelling layers and each view's own layers decide **what Curva hands each view**; no engine does
this. Bevy's render layers carry the result: turning a layer off in one view changes that slot's bit
on the layer's objects. Locking, Hide and Show stay Curva's. **No issue.**

### Line weights (`line-weights.md`, `views-and-styles.md` §2.1)
Two rules, one drawer:
- 3D views: paper millimetres at the screen's real size, whatever the zoom (today's rule).
- Plans, sections and sheets: paper millimetres at the drawing's scale, growing as you zoom in.

Curva's line drawer draws both, with smooth edges worked out exactly per pixel. **Issue to respect:**
Bevy mixes colours in linear light and applies tone mapping and temporal smoothing to everything it
draws. Lines, hatches, cut fills and text must not go through that, or they come out lighter and
softer than the print (the test showed exactly this, 0.24% of pixels off, until fixed). So **Curva's
lines are drawn after Bevy's own passes**, on the finished picture, mixed as printers mix them, and
hidden behind faces by Bevy's depth. Where Bevy's temporal smoothing is on, the lines use the
unshaken camera and a small depth margin, so they neither flicker nor show gaps.

### Dashes (`views-and-styles.md` §2.1, `drawing-output.md` §7, #170)
- Plans and sheets: dashes are laid out once along each line on the page, in paper millimetres,
  exactly as the PDF writer lays them out. **No issue.**
- 3D views: a dash must be measured along the whole curve on screen, and that length changes as the
  view moves. A circle drawn as 128 short segments has no dashes at all if each segment starts its
  own pattern. **Issue:** needs the line drawer to add up lengths along each curve every frame (a
  small pass on the graphics card). Planned with the line drawer (step 3).

### Cut fills, section fills, hatches (`layouts.md` §3, `annotation.md` §5, `views-and-styles.md` §2.1)
- Plans and sections: fills and hatch lines come from the vector page. **No issue** (tested).
- 3D views with clipping planes: the cut face of a solid has to be filled. Bevy has no clipping
  planes or capping. **Answer:** the cap is the kernel's exact cut outline (`Body::section`, once #193
  is fixed), filled and hatched by Curva like any page fill, so it matches the print. The clipping
  itself is part of the per-view material work above.
- Large hatches (brick over a whole façade) are many lines: drawn once per change and kept on the
  card, not redrawn each frame.

### Crisp (vector) shadows (`views-and-styles.md` §4.2)
Plans, sections and elevations: the kernel's exact shadow shapes go on the vector page as fills,
under the lines; Bevy draws them like any fill. **No issue.** Soft shadows in 3D views are Bevy's
shadow maps, one sun per view (above), with distances set in millimetres (pilot step 4). Soft
shadows on a printed sheet: a picture made at print resolution (below).

### Styles (`views-and-styles.md` §2, §4.1)
- Shaded, Ghosted (Bevy has order-independent transparency), Diagram's ambient occlusion and the
  Textured paper and gradient fit Bevy's passes.
- **Issue:** outlines and folds found from the picture (Diagram) need, besides depth and normals
  (Bevy has both), a hidden picture of object numbers, which Bevy does not make. Curva adds it, as it
  does today.
- **Issue:** colours must show as set. Bevy's default tone mapping shifts colours (white walls come
  out off-white). Architectural styles use no tone mapping, or one set so that a layer's colour is the
  colour on screen.

### Live plans and sections (`live-plans.md`)
The vector page stays the single source; only "drawn by Vello" becomes "drawn by Bevy". To keep the
§6 targets (switch in one frame, edits under 50 ms), each object's part of the page becomes its own
small mesh, so an edit replaces only what changed (marks already will carry their object, step 2 of
that spec). Picking and snapping stay Curva's. **Screen against PDF:** met at 8 samples per pixel on
the recommended card; with 4 samples, a few edge pixels differ slightly (0.002%), still nothing
visible.

### Smart objects and representations (`architecture.md` §5b, Milestone 4)
A door shows a plan symbol in plan, its 3D model in elevation and 3D, a simpler symbol at 1:200, a
lighter model far away.
- Plan and section symbols are 2D drawings on the vector page. **No issue.**
- Several 3D representations of one object in different views: each representation is its own
  entity, shown in the view slots that chose it (render layers). Bevy's *visibility ranges* also
  switch detail by distance on their own. **No issue**, provided the step 2 mirror allows one object
  to be several entities, which it will.

### Annotation and text (`annotation.md`)
Text on screen is drawn from the letters' outlines as fills (Curva does this today); the PDF keeps
real, searchable text. **Small issue:** very small text when zoomed far out can shimmer; checked when
sheets move (the "text" item of decision 0029).

### Printing raster details (`drawing-output.md` §2)
Shaded details on a sheet print as a picture at the print resolution. An A1 sheet at 300 dpi is about
9,900 × 7,000 pixels, above the 8,192-pixel picture limit of the baseline device. **Answer:** render
in tiles (Bevy cameras can draw part of a view). Planned with sheets.

### Precision far from the origin (`architecture.md` §20)
Pilot step 4: positions relative to an origin near the camera. Plans and sheets are in paper
millimetres, small numbers. **No issue.**

### Machines without a graphics card (decision 0026)
Vello had a processor-only version; Bevy has none. Bevy runs on Windows' software graphics (WARP)
and Linux's, slowly. **Open:** measured before sheets move; if too slow, those machines get a
simpler fallback.

## Order this suggests

1. **Step 2, faces** (#223): settle per-view styles (copies or one Curva material) by measuring;
   render layers for view layers; one sun per view.
2. **Step 3, one line drawer** (#224): Curva's lines after Bevy's passes, both width rules, dashes
   along whole curves, edges mixed as printers mix them, depth from Bevy.
3. **Plans and sections** (`live-plans.md` step 5) on the same drawer, from the vector page, per-object
   meshes.
4. Steps 4 to 6 of the pilot spec as written (precision and millimetres, effects, measuring), then
   sheets with the four open checks (dashes, text, a large sheet, no graphics card).
