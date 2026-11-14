# Spec: Make2D and hidden lines

**Status:** Approved by the owner in chat · 14 November 2026.
**Covers:** `milestone-3-in-0.4.3.md`, points 11 to 13; decision 0020 (3, 6).

## 1. Make2D, as Rhino 8 does it

**Make2D** draws the selected objects as 2D curves seen from a view, with lines hidden where other
objects cover them. The result is plain curves, with no link to the model afterwards (decision 0020,
point 3).

- **Steps and options:** Rhino 8's Make2D dialog, as the command's options and a small panel:
  - **View:** current view, current construction plane, or a four-view drawing (top, front, right
    and perspective, in US or European projection).
  - **Options:** show tangent edges, show tangent seams, hidden lines, scene silhouettes (outlines of
    objects against the background), clipping-plane intersections (with the section fill as a
    hatch), group the output, and maintain source layers.
  - **Output:** placed at the origin of the world top plane, or on the current layout when run
    from a detail, as Rhino does.
- **Layers:** Rhino's: a `Make2D` parent layer with `Visible` and `Hidden` sub-layers, each split into
  `Curves`, `Tangents`, `Silhouettes`, `Clipping plane intersections` and `Annotations`; with
  "maintain source layers", the source layer tree is repeated under each.
- **Annotation:** text and dimensions in the selection are copied into the drawing, flattened, as in
  Rhino.
- **What it draws:** edges of solids and surfaces, curves, silhouettes of curved faces, block
  instances (their contents), and clipping-plane cuts. Points, dots and hatches as Rhino draws
  them.

## 2. The hidden-line removal

solidcore finds silhouette curves on surfaces and traces rays, but has no hidden-line removal, so
Curva builds its own (decision 0020, point 6):

1. **Curves to draw:** edges, silhouettes from the view direction (solidcore's
   `surface_silhouette`), tangent edges, and the cuts from clipping planes.
2. **Projection** of each curve into the view plane, parallel or perspective.
3. **Splitting** each projected curve where it crosses the projected outlines of the faces that
   could cover it.
4. **Visibility** of each piece, tested at its midpoint by a ray from the eye (solidcore's ray
   tracer), so each piece is visible or hidden.
5. **Joining** the pieces back into curves on the right layers.

Source: Appel's quantitative invisibility and the classic split-and-test approach (Appel, "The notion
of quantitative invisibility and the machine rendering of solids", 1967; Hornung et al.). Curved
faces are split and tested at the model's tolerance, so the drawing is exact to that tolerance, not
to the screen.

- **Speed:** pieces are tested in parallel with rayon; a bounding-volume tree limits which faces each
  ray checks. Target: a 2,000-face model in under 5 seconds on the Windows machine. A progress bar
  shows and Escape cancels.
- **Custom:** listed in `docs/kernel/custom-operations.md` and checked against solidcore before each
  release, as decision 0019 requires.

## 3. Prior work checked

Checked 14 November 2026 (decision 0021, points 7 and 10):

| Found | Licence, activity | Verdict |
|---|---|---|
| solidcore 9342f32: `surface_silhouette`, ray tracer | Curva's engine | **Use** for steps 1 and 4 |
| `ogeom-hlr` 0.18 (part of `ogeom`, a B-rep kernel in Rust) | LGPL-2.1-only, first published September 2026 | **Not linked**: LGPL relinking duties don't fit an MIT OR Apache-2.0 program, and it needs its own kernel's B-rep. Its design is read as a reference |
| `i_overlay` 9.1 (polygon booleans) | MIT OR Apache-2.0, updated October 2026 | **Use** for step 3, splitting projected curves by face outlines where those are polygons |

## 4. Checks against Rhino

Rhino is the automatic reference (`architecture.md` §15): for a set of models (boxes, a cylinder
through a box, a sphere behind a box, a block, a clipped solid, a perspective view), Rhino's Make2D
output is saved as a reference, and Curva's is compared: same visible and hidden curves within
tolerance, same layers. Tests also cover every option, cancelling, and an empty selection.

## 5. Technical and Hidden display modes

Two new display modes, as in Rhino, for viewports and detail views, built on the same pieces:

- **Hidden:** visible edges and silhouettes only, on a plain background.
- **Technical:** visible edges, silhouettes, creases, intersections and section cuts in their print
  widths; hidden lines optionally dashed.

On screen they use a fast approximation (the GPU's depth buffer to hide lines, so orbiting stays
smooth), and switch to the exact hidden lines of §2 for printing and PDF, so paper is exact.

## 6. Not in 0.4.3

Make2D results that update when the model changes, and live plan and section views, come with
Milestone 4.
