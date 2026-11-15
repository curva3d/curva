# Spec: views and visualisation styles

**Status:** Approved by the owner in chat · 15 November 2026 (display stream, #160), with the
decisions in §8.
**Covers:** the owner's direction of 15 November 2026. Replaces "Display modes as editable files"
(`display-pipeline.md` §3) and its steps 8 to 10. The rest of `display-pipeline.md` (detail that
follows the zoom, smooth edges, speed on large scenes, the machines Curva runs on) is unchanged.
**Rules:** 100% pure Rust, no C++ libraries. Designed for an RTX 3060-class graphics card (decision
0026); on weaker machines heavier effects lower their quality instead of being missing.

## 0. In short

- **Styles instead of display modes.** A visualisation style gives a view its character: a bold
  diagram, an architectural drawing with thick cut lines and lines fading with distance, a textured
  presentation. The same style works in a 3D view, in a live 2D plan or section, on a sheet, and in
  a PDF or SVG export, and looks the same in each.
- **One engine, one set of settings.** Every style, built-in or made by a user, is made of the same
  layers with the same settings. No style has different options because something else draws it.
- **Everything is live.** Every change shows in the view as you make it.
- **Views are saved items.** A view is a 3D camera or a live 2D plan or section, with its own style,
  its own layers on and off, and its own sun. Any viewport can show any view; sheets place views.
  The same plan can exist twice, one as a submission drawing and one as a client presentation.

What Rhino gets wrong and Curva must not: no live preview while editing a display mode; different
modes with different settings because different engines draw them; turning a layer off in one
view turns it off in every view.

## 1. Views

### 1.1 What a view is

A view is a saved item in the model, listed in a **Views** panel. It holds:

| Part | What it is |
|---|---|
| Name | e.g. "Ground floor – submission", "Ground floor – client", "Street perspective" |
| Kind | **Perspective** (a camera with a lens), **Parallel** (an axonometric or elevation), **Plan** (looking down, cut at a height) or **Section** (looking sideways, cut along a line) |
| Camera or cut | For perspective and parallel views, where the camera is and where it looks. For plans and sections, the cut (§1.3) |
| Style | Which visualisation style draws it (§2) |
| Layers | Which layers are on in this view (§1.4) |
| Sun | This view's sun (§1.5) |
| Locked | A locked view cannot be moved by orbiting, panning or zooming, so a submission view is not nudged by accident |

### 1.2 Where views show

- **Viewports show views.** Each viewport's title menu lists the views; picking one shows it there.
  Orbiting or zooming in a viewport changes the view it shows (unless the view is locked). The four
  viewports open on four standard views: Top, Front, Right and Perspective.
- **Sheets place views.** A detail on a sheet shows a view at a scale. Change the view and every
  viewport and sheet showing it changes with it.
- **Views panel:** New (from the current viewport, or a new plan or section), Duplicate, Rename,
  Delete, Lock. Duplicating a plan and giving the copy another style is how one floor gets a
  submission drawing and a client drawing.
- Named views (`NamedView`) become views; older files' named views open as views.

### 1.3 Plans and sections

- A **plan** looks straight down, cut at a **cut height** (default 1.2 m above its level) and shows
  down to a **view depth** (default: the level's floor). What lies above the cut is not drawn.
- A **section** looks sideways along a line drawn in plan, cut there, showing a set **depth**
  beyond the cut.
- Both are live: edit the model and the plan or section updates at once.
- What is cut and what is seen beyond the cut get their own style layers (§2.1), so cut lines can
  be thick and black and everything beyond thin and fading.
- Levels (named heights, as in architecture tools) come with the architecture specialisation; until
  then a plan's height is a number.

### 1.4 Layers: for modelling, and per view

There are two kinds of layer state (owner's decision A):

- **Modelling layers:** one on/off state for modelling, used by the viewports while you work,
  as the Layers panel works today.
- **View layers:** each saved view keeps its own layers on and off. On a sheet and in an export,
  a view always draws with its own layers, whatever the modelling layers say. "Walls" can be off
  while modelling and on in the printed plan.
- When a viewport shows a saved view, the Layers panel says at its top which state it is editing,
  **Modelling** or **This view: <name>**, and starts on the view's layers; one click switches. So a
  submission drawing is never changed by accident while modelling, nor the other way round.
- A new layer is on in the modelling layers and in every view.
- **Copy layer state:** a command copies one view's (or the modelling) layers on and off to other
  views, chosen in a list.
- **Locking stays model-wide:** a locked layer cannot be edited anywhere, as locking is about
  editing, not about how a drawing looks.
- **Hide and Show stay model-wide:** they are a modelling tool for getting objects out of the way,
  not part of a drawing.

### 1.5 The sun

- Each view has its own sun: **compass direction** and **height above the horizon**, set by
  dragging a sun handle in the view, or by typing the two angles. Shadows update as you drag.
- Later, with the architecture specialisation: "from location, date and time" as another way to set
  the same sun, as Rhino's Sun panel does.

## 2. Styles

### 2.1 A style is a stack of layers

Each layer draws one kind of thing with its own look. Every layer has an on/off switch.

| Layer | Draws | Settings |
|---|---|---|
| Background | Behind the model | One colour, a gradient of two colours (direction), or paper (a texture and its colour) |
| Surfaces | The faces | Colour from the object, its layer, or one colour for all; opacity 0–100%; lighting: soft studio light, sun-lit, or flat |
| Ambient occlusion | Soft darkening where surfaces meet and where objects stand on others | Strength, spread (how far it reaches), colour |
| Shadows | Shadows cast by this view's sun | Strength, colour, softness |
| Outlines | Where an object ends against what is behind it | Width, colour |
| Cut lines | Where a plan or section cuts through an object | Width, colour; cut fill: a colour or a hatch |
| Edges | The edges of faces and folds in surfaces | Width, colour |
| Hidden lines | Edges behind other surfaces | Shown or not; solid or dashed; colour, opacity, width |
| Curves | Ordinary curves and points | Width, colour |
| Finish | Over the whole picture | Paper grain, texture, a soft gradient, a darkened border (vignette); each with a strength |

Widths are in millimetres on paper, as layer and object print widths are. A width can be **fixed**
(e.g. 0.35 mm) or **follow the object** (its print width, times a factor).

### 2.2 Rules: settings that change

Any width, colour or opacity can follow a rule instead of a fixed value:

- **By distance:** from a near value to a far value between two distances, e.g. lines 0.5 mm black
  at 5 m fading to 0.05 mm, 20% grey at 50 m. Shown as a near/far slider pair with a small preview,
  never as a formula.
- **By layer:** e.g. "Trees: thin green", "Glass: surfaces 20% opaque". Later, by building element
  (walls, slabs, doors) once the architecture specialisation brings them.
- **Cut or beyond:** in plans and sections, what is cut and what is seen beyond the cut are set
  apart.

### 2.3 Built-in styles

| Style | Looks like |
|---|---|
| Wireframe | Lines only, as today's Wireframe |
| Shaded | Lit faces with dark edges, as today's Shaded |
| Ghosted | Faces at half strength, edges behind dimmed, as today's Ghosted |
| Diagram | Light surfaces, thick black outlines, thin black edges, soft ambient occlusion: a bold architectural diagram |
| Architectural | Thick black cut lines with a solid fill; lines beyond thinning and lightening with distance; white background |
| Textured | Paper texture, a soft gradient over the view, gentle shadows and ambient occlusion, warm grey lines |
| Technical | Black on white, hidden lines dashed |

Built-in styles cannot be changed, only copied. Zebra and Draft angle stay analysis overlays on top
of any style.

### 2.4 Files

- **A model carries its styles** (owner's decision C): every style its views use is saved inside
  the `.curva` file, so the model looks as intended whoever opens it, on any machine. Editing a
  style edits the model's copy.
- **My styles:** the user's own library, one plain-text file per style
  (`<style name>.curvastyle`) in `Curva/styles/` in the user's app-data folder. Choosing a style from
  the library for a view copies it into the model. **Save to my styles** puts a model's style into
  the library; **Export** makes a file to share. Built-in styles live inside the program and are
  copied into a model the same way when used.
- If a model's style and a library style share a name but differ, the model's own copy is always
  the one drawn; the Style panel marks the difference and offers to update either from the other.
- Unknown settings are kept when saving; a bad value falls back to its default and the Style panel
  says which line is wrong; a bad file never stops Curva starting.
- Later the owner wants the library tied to a Curva online account; style files are self-contained
  so they can be synced without changing their format.

## 3. Making a style: intuitive, live

- **Style panel:** the current view's style at the top (a list with a small picture of each style),
  then its layers as a stack, each with an eye to switch it and a row of settings in plain words.
- **Every change is live** in every viewport showing a view with that style, while dragging a
  slider, not after letting go.
- **Start from any style, then "Save as new style".** Editing a built-in style offers to save a copy
  at the first change, so the built-in one is never lost.
- **Undo inside the panel** steps back through style changes; **Reset** returns a layer to the
  style it was copied from.
- **Pictures of each style** in the list are drawn from the user's own model, small, so choosing a
  style is choosing a look, not reading names.
- No node graph to begin with: the layers and rules above cover the styles the owner described,
  and stay readable. A node view for power users could come later, reading the same files.

## 4. How it is drawn

### 4.1 Live views

On the graphics card, with the shared drawing programs of `display-pipeline.md`:

- **Outlines and folds** are found from the picture as it is drawn: where depth jumps, where the
  surface turns sharply, and where one object meets another (each object's number is drawn into a
  hidden picture, as the selection table already numbers them). Cheap on every machine.
- **Lines fading with distance:** each line's width and colour are worked out from its distance in
  the line program itself.
- **Cut lines and fills** in plans and sections: the cut is the plan or section plane; where it
  cuts a solid, the outline is drawn as a cut line and the inside filled.
- **Ambient occlusion:** worked out from the depth picture (screen-space ambient occlusion), at half
  resolution on slower machines.
- **Shadows:** a shadow picture from the view's sun (shadow mapping); its resolution drops on slower
  machines, and the shadows soften rather than vanish.
- **Background, paper and finish:** a final pass over the whole picture.

### 4.2 Plans, sections and exports

Printed sheets and PDF or SVG exports need real lines, not pixels. There, the exact edges, outlines
and cut lines come from the geometry engine, with the hidden-line method `Make2D` already uses, and
each line gets its width, colour and dashes from the same style rules, so the export matches what
was on screen. Surfaces, ambient occlusion, shadows, gradients and paper have no line form; they
go into the PDF or SVG as a picture underneath the lines, at print resolution (owner's decision).

### 4.3 Checked by

- A style drawn live and the same style exported: the lines land in the same places within a pixel
  (pictures compared by machine).
- Every built-in style has a reference picture of the test building; a change that alters one is
  shown side by side in its pull request.
- Each view keeps its own layers: turning a layer off in one view leaves the others unchanged
  (test).
- Style files: saved and read back unchanged; unknown settings kept; bad values fall back.
- Speed: *Medium* with the Diagram, Architectural and Textured styles, measured on the RTX 3060 with
  the margins of `display-pipeline.md` §5.

## 5. Prototype first (owner's decision)

Before building the rest, three small pull requests draw the look only, with no editor yet, and the
owner judges pictures:

1. Outlines and folds found from the picture, object numbers drawn to a hidden picture: the
   **Diagram** look.
2. Lines fading with distance, and cut lines and fills from a cutting plane: the **Architectural**
   look.
3. Ambient occlusion, paper and gradient: the **Textured** look.

Each comes with pictures of the same small test building (walls, slabs, roof, a few trees) in that
style beside Shaded. If the character is not there, the approach changes before anything else is
built.

## 6. Steps after the prototype

These replace steps 8 to 10 of `display-pipeline.md`; the other steps keep their order.

1. Views in the model: the views list, named views and older files brought in, saved in the file.
2. Viewports show views; the Views panel; locked views.
3. Layers per view, the Layers panel editing the current view, and Copy layer state.
4. Styles as settings over the shared drawing programs; Wireframe, Shaded and Ghosted rebuilt on
   them, looking as before.
5. Style files and the Style panel with live editing.
6. Sun per view, shadows and ambient occlusion as style layers.
7. Plans and sections with cut height and depth.
8. Exports and sheets drawing styles with real lines.
9. The remaining built-in styles.

## 7. Changes outside the viewport crate

Views and per-view layers live in the model, so this reaches further than `display-pipeline.md`
§8 foresaw:

- `curva-document`: views as items (camera or cut, style, layer state, sun, locked) and the styles
  they use, saved in the file; named views and older files brought in; modelling layers kept as today, view layers
  added.
- `curva-app`: Views panel, Style panel, the Layers panel editing the current view, viewport title
  menus listing views, sheets placing views.
- `curva-commands`: `SetDisplayMode` becomes `SetStyle` (the old name kept as an alias); commands for
  views (`NewView`, `Plan`, `Section`, `CopyLayerState`).
- Print and export: lines drawn by style.

## 8. Decisions

**Made by the owner (15 November 2026):** styles replace display modes, with Wireframe, Shaded and
Ghosted as built-in styles; a prototype with pictures comes first; paper and gradients go into
exports as pictures under the lines; layers on and off per view only; the sun set per view by
angles, place and time later; plans and sections with cut height and depth from the start; views
are saved items shown in any viewport or sheet; ambient occlusion is a style layer; one engine and
one set of settings; everything live.

**Confirmed by the owner (15 November 2026):**
- **A. Modelling layers and view layers** (§1.4): the modelling viewports have one layer state for
  modelling; each saved view has its own, which wins on sheets and in exports. Hide and Show stay
  model-wide, as modelling tools.
- **B. The display stream builds the model and interface parts** (§7), in small pull requests
  announced in `status/display.md` before each and merged quickly, as these touch files the
  features stream works in.
- **C. A model carries its styles** (§2.4), so it always looks as intended whoever opens it; the
  user's folder is a library to choose from. This replaces, for styles, the earlier choice to
  remember display modes by name.

## 9. Not covered

Materials and textures on objects, a rendered photo-realistic style, the Blender / Cycles bridge,
sun from location and time, levels, building elements (walls, slabs) as rule targets: all come
later, and the style layers are made so they can be added without changing existing style files.
