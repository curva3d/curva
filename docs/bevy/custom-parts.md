# Custom Bevy parts

Drawing Curva does itself because Bevy cannot do it yet (decision 0030). The same rule as the
kernel's custom operations (`docs/kernel/custom-operations.md`, decision 0019): **use what Bevy ships
wherever it does the job; anything Curva adds is marked and checked at every Bevy upgrade.**

What Bevy offers for each of Curva's needs, and the rules that follow: `docs/bevy/guide.md`.

Each row has a `// CUSTOM BEVY PART: <name>` mark on the code that does it; a test
(`custom_bevy_parts_match_their_list`) fails if a mark has no row or a row no mark.

**At every Bevy upgrade, and before every release** (`docs/releasing.md`): read the new Bevy's
release notes and, for each row, check whether Bevy now covers the part. Where it does, switch to
Bevy's on a branch; if it works at least as well (the part's tests pass, and its pictures and speed
are no worse), delete Curva's code and the row, and say so in the release notes. Then update
"Checked at".

Glue that only connects Curva to Bevy (handing it the scene, the camera, the picture) is not a
custom part, as the kernel's interface to solidcore is not a custom operation. It is still kept as
small as Bevy allows.

| Part | Code | Used by | Bevy today | Checked at |
|---|---|---|---|---|
| bevy-host | `curva-viewport/src/pilot.rs` (`Pilot`) | Views drawn by Bevy | Bevy draws into a picture on a device it is given (`RenderCreation::manual`), but has no way to sit inside an egui window drawn by eframe and draw only when that window asks; Curva runs Bevy's update itself and copies the picture into the view. (`bevy_egui`, outside Bevy, puts egui inside Bevy's window, the other way round) | 0.20.1 |
| cad-lines | `curva-viewport/src/gpu/depth.rs` (`Handover`), `gpu/drawing.rs` (`lines_on_bevy`) | Lines, outlines, previews and the gumball on views Bevy draws | Bevy's gizmo lines have widths in pixels and simple dashes, and no paper millimetres, exact hidden lines or printer-like edges, so Curva draws its own lines on Bevy's picture. Bevy keeps its depth readable (`Camera3d::depth_texture_usages`); Curva turns it into its own depth there, so its lines hide behind Bevy's surfaces | 0.20.1 |
| clip-planes | `curva-viewport/src/pilot/clip.rs`, `pilot/clip.wesl` (`Clipping`) | Clipping planes on views Bevy draws | Bevy has no clipping planes; its own `ExtendedMaterial` lets Curva add a few lines to the standard surface that leave out what lies beyond a view's planes. The filled cut faces are Curva's own, from the engine's exact cuts (`curva-app/src/clipping.rs`). Not yet in shadows: a cut-away part still casts its shadow in the Sunlit look | 0.20.1 |
| far-origin | `curva-viewport/src/outlines.rs` (`Outlines::drawing_origin`), `lines.rs` (`narrow_from`), `pilot/scene/shapes.rs` (`placed`) | Every view of the window, Bevy's surfaces and Curva's lines alike | Bevy uses single precision and has no moving origin. The `big_space` crate (outside Bevy) gives Bevy's own entities a grid of origins, but Curva's lines, previews and cuts need the same origin, and Curva remakes its scene when the model changes anyway; so a window scene is measured from a point near the model, worked out in double precision before narrowing, and its camera moved to match (#225) | 0.20.1 |

## Planned (no code yet)

Parts the review of 16 November 2026 (`docs/reviews/bevy-drawing-review-2026-11-16.md`) expects
Curva to add. Each gets its row and mark when its code lands; before then, check Bevy first.

| Part | What Bevy lacks | Comes with |
|---|---|---|
| page-drawing | Plans, sections and sheets from the vector page: lines and fills as triangles with true widths, colours as printed. Bevy has 2D meshes but no vector paths | Live plans step 5 |
| object-numbers | A hidden picture of which object each pixel shows, for Diagram outlines. Bevy has depth and normal pictures, not object numbers | Diagram style |
| cad-picking | Picking and snapping on edges, points, curves and page lines. Bevy's mesh picking finds faces only | Kept from today's viewport |
