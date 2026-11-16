# Bevy in Curva: what Bevy 0.20 offers, and how Curva uses it

*Display stream, 16 November 2026. Read before any drawing work (decision 0030: Bevy first, then
existing libraries, then Curva's own code). Sources: Bevy's release notes for 0.17 to 0.20, its
examples, and Bevy 0.20.1's own source (`~/.cargo/registry/src/*/bevy_*-0.20.1`), which is the
authority where they differ: the examples on Bevy's main branch are already past 0.20.*

## How Bevy is built, in brief

- **Two worlds.** Bevy keeps the scene in its *main world* (entities: meshes, materials, lights,
  cameras). Each update copies what changed into the *render world* ("extract"), which prepares
  GPU data and draws. Since 0.15 the render world is retained: unchanged objects cost little.
- **Rendering is a schedule of systems** (since 0.19; the older render graph and its `Node` trait
  are gone). The 3D frame is the `Core3d` schedule, with sets `Core3dSystems::Prepass`,
  `MainPass`, `EarlyPostProcess`, `PostProcess`. A new pass is a system added to `Core3d` in one of
  those sets, with a `RenderContext` and the view's textures (`ViewTarget`, the depth).
- **Shaders are WESL** (since 0.20): `import bevy_pbr::render::forward_io::VertexOutput;`,
  `@if(DEF)`, `@group(constants::MATERIAL_BIND_GROUP)`. Plain WGSL still loads.
- **GPU-driven drawing** is the default where the device allows it: meshes in shared buffers,
  multi-draw indirect, culling and visibility ranges on the GPU (0.19), sparse uploads of only the
  transforms that changed. Many copies of one mesh with one material are drawn together on their
  own; there is no separate instancing API to call.

## Curva's needs, and what serves each

| Curva needs | Bevy 0.20 offers | Curva uses |
|---|---|---|
| Bevy inside Curva's egui window, on Curva's device, drawing only when a view asks | `RenderCreation::manual(device, queue, ...)`; Curva runs `SubApps::update()` itself; no window plugin needed | Yes (custom part `bevy-host`) |
| Several views at once | One camera per view; `Camera::viewport` draws a camera into a rectangle of its target; `Camera::order` | One camera per view |
| Bevy's picture inside Curva's window texture | `RenderTarget::TextureView(handle)` with `ManualTextureViews`: a camera draws into a texture made outside Bevy. With `CameraOutputMode::Write { blend_state: None, .. }` (the default) it writes only its viewport, leaving the rest | **Planned (step 5):** every view's camera draws straight into Curva's scene texture in its own viewport, replacing a picture per view and the copy |
| Different looks per view | `RenderLayers` on meshes, lights and cameras; a light only lights views that share its layer; `AmbientLight` per camera; `EnvironmentMapLight` per camera | Yes: a copy of each object per view on that view's layer, sharing meshes |
| Surfaces, highlights | `StandardMaterial` (roughness, reflectance, double-sided) | Yes |
| Reflections that make round shapes read (Rhino-like) | `EnvironmentMapLight::hemispherical_gradient`, with `rotation` for Z up; `GeneratedEnvironmentMapLight` filters any map at run time | Yes, the Studio look |
| Shadows | `DirectionalLight` with cascaded shadow maps (`CascadeShadowConfigBuilder`, distances to be set in millimetres); `ContactShadows` (0.19) for short contact shadows | Sunlit look; contact shadows to try for presentation styles |
| Ambient occlusion | `ScreenSpaceAmbientOcclusion` (needs `DepthPrepass`, `NormalPrepass`, `Msaa::Off`) | Step 5 / styles |
| Edge smoothing | `Msaa`; `TemporalAntiAliasing`; SMAA; FXAA | `Msaa::Sample4` (Curva's depth handover reads four samples) |
| Ghosted (see-through) | `AlphaMode::Blend`; order-independent transparency (`OrderIndependentTransparencySettings`, needs `Msaa::Off`) | Blend; OIT to measure with styles |
| Clipping planes | None | Custom part `clip-planes`: `ExtendedMaterial<StandardMaterial, _>` with a few WESL lines |
| Lines in paper millimetres, dashes along curves, exact hidden lines | Gizmos (pixel widths, simple dashes, debug-oriented); `WireframePlugin` (mesh triangle edges) | Neither fits; custom part `cad-lines` |
| Curva's lines hidden by Bevy's surfaces | `Camera3d::depth_texture_usages` (make the depth readable); `ViewDepthStencilTexture` in the render world; or a system in `Core3d` after `MainPass` with the depth bound | Depth handed over to Curva's line pass (`gpu/depth.rs`). A Curva line pass inside `Core3d` would skip the handover; considered when lines are reworked for dashes |
| Repeated objects drawn once | Automatic batching of one mesh + one material; `VisibilityRange` for detail by distance | Step 4: one shared mesh per shape from the kernel |
| Skip what is off screen or hidden | Frustum culling (on); `OcclusionCulling` (needs `DepthPrepass`); `NoCpuCulling` to leave culling to the GPU | Frustum culling; `NoCpuCulling` to measure in step 5 against Bevy's fixed cost per frame |
| Models far from the origin | Single precision; no moving origin in Bevy (`big_space` crate outside Bevy) | Custom part `far-origin`, step 4; check `big_space` first |
| Curva's own background | Unlit `StandardMaterial` on a backdrop mesh with vertex colours | Yes |
| Card time for the benchmark | `RenderDiagnosticsPlugin` (per-pass GPU time in `DiagnosticsStore`) | Step 5, replacing Curva's own card timer |
| Device lost, out of memory | `RenderErrorHandler` with `RenderErrorPolicy::{Recover, StopRendering, Ignore}` per error type | To set in step 5 (default: validation errors ignored, others ask the app to exit, which Curva's embedded Bevy does not do) |
| Grid | `InfiniteGrid` (0.19, a fading full-screen grid) | Not used: Curva's grid lies on the construction plane, is snapped to and drawn in paper millimetres |
| Gumball | `TransformGizmoPlugin` (0.19): moves, turns, scales a Bevy entity's `Transform` | Not used: Curva's gumball edits the model through commands, with Rhino's handles, typed numbers and Curva's snaps |
| Camera control | `PanOrbitCamera` (0.20, with `MeshPickingPlugin`) | Not used: Curva's Rhino-style camera and its saved views drive Bevy's camera |
| Picking | `MeshPickingPlugin` (faces only) | Not used: custom part `cad-picking` (edges, points, page lines, snaps) |
| Pictures for agents and prints | Any camera into an `Image` or `TextureView`; `Screenshot` | Bevy through the window's own path (`bevy_picture`) |

## Rules this gives

1. **Change only what changed.** Each inserted component or changed asset makes Bevy redo work;
   hand Bevy a camera, light or material only when it differs (as `ViewScene` does).
2. **Share meshes.** Equal contents share one mesh; copies of a shape will share one mesh with
   their own placement (step 4), which Bevy then draws together.
3. **One Bevy per program,** on Curva's device. Two at once crash some drivers (Windows' software
   graphics); the tests take turns.
4. **Before writing a pass, look for a Bevy component or a `Core3d` system set it can join.**
5. **Bevy's colours are linear and tone-mapped by default;** Curva sets `Tonemapping::None` and
   `DebandDither::Disabled` so a colour shows as set, and draws its own lines after Bevy, mixed as
   printers mix them.
