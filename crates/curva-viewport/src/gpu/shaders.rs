//! The shader programs the graphics card runs, in WGSL.

use crate::NO_OBJECT;

/// How many objects one row of the selection table holds (spec `display-pipeline.md` step 4).
pub(crate) const PICK_ROW: u32 = 4096;

/// Put in front of the line and face shaders: the selection table, one byte per object, 1 where
/// the object is selected. It is a texture, not a storage buffer, so the OpenGL fallback reads it
/// too. `picked` looks an object up; `widened` makes a selected line a pixel wider, as
/// [`crate::line_width`] does.
pub(crate) fn table_wgsl() -> String {
    let [r, g, b] = crate::SELECTED_COLOUR;
    format!(
        "
@group(0) @binding(1) var picks: texture_2d<u32>;
const SELECTED: vec3<f32> = vec3<f32>({r:?}, {g:?}, {b:?});
const NO_OBJECT: u32 = {NO_OBJECT}u;
const MM_PER_PIXEL: f32 = {mm:?};
fn picked(object: u32) -> bool {{
    if (object == NO_OBJECT) {{ return false; }}
    let size = textureDimensions(picks);
    let at = vec2<u32>(object % size.x, object / size.x);
    if (at.y >= size.y) {{ return false; }}
    return textureLoad(picks, at, 0).r != 0u;
}}
fn widened(width: f32, chosen: bool) -> f32 {{
    if (!chosen) {{ return width; }}
    return select(width - 1.0, width + MM_PER_PIXEL, width > 0.0);
}}
",
        mm = crate::MM_PER_PIXEL,
    )
}

/// The grid: true one-pixel lines, two vertices a segment.
pub(crate) const GRID_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec3<f32> };
@vertex fn vertex_main(@location(0) p: vec3<f32>, @location(1) c: vec3<f32>) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), c);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.colour, 1.0);
}";

/// The model's lines (spec `line-weights.md` §2): each segment is one instance, drawn as a quad of
/// two triangles facing the screen, as wide as its own width, and lengthened by half that width at
/// each end so the segments of a curve overlap at their joins. `pixel.xy` is a pixel's size in
/// clip space, `pixel.z` the screen's pixels per millimetre.
pub(crate) const LINE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// Clipping planes (spec `layouts.md` §3): `pixel.w` of them, each keeping the side where
// dot(n, p) + d is not above zero.
fn clipped(p: vec3<f32>) -> bool {
    let count = u32(uniforms.pixel.w);
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (i < count && dot(uniforms.clips[i].xyz, p) + uniforms.clips[i].w > 0.0) {
            return true;
        }
    }
    return false;
}
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec3<f32>,
    @location(1) world: vec3<f32>,
};
fn pixels(width: f32) -> f32 {
    return max(select(-width, width * uniforms.pixel.z, width > 0.0), 1.0);
}
@vertex fn vertex_main(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>, @location(1) given_ca: vec3<f32>, @location(2) given_wa: f32,
    @location(6) object: u32,
    @location(3) b: vec3<f32>, @location(4) given_cb: vec3<f32>, @location(5) given_wb: f32,
    @location(7) object_b: u32,
) -> Out {
    // A selected object draws in the selection colour and a pixel wider (`line_width`).
    let chosen = picked(object);
    let ca = select(given_ca, SELECTED, chosen);
    let cb = select(given_cb, SELECTED, chosen);
    let wa = widened(given_wa, chosen);
    let wb = widened(given_wb, chosen);
    var pa = uniforms.matrix * vec4<f32>(a, 1.0);
    var pb = uniforms.matrix * vec4<f32>(b, 1.0);
    // Behind the camera: cut the segment where it crosses the near plane.
    if (pa.z < 0.0 && pb.z < 0.0) {
        return Out(vec4<f32>(2.0, 2.0, 2.0, 1.0), ca, a);
    }
    if (pa.z < 0.0) { pa = mix(pa, pb, pa.z / (pa.z - pb.z)); }
    if (pb.z < 0.0) { pb = mix(pb, pa, pb.z / (pb.z - pa.z)); }
    let px = uniforms.pixel.xy;
    var along = (pb.xy / pb.w - pa.xy / pa.w) / px;
    let length_px = length(along);
    if (length_px > 1e-6) { along = along / length_px; } else { along = vec2<f32>(1.0, 0.0); }
    let across = vec2<f32>(-along.y, along.x);
    // Corners 0-5 make two triangles: which end, and which side.
    var ends = array<f32, 6>(0.0, 0.0, 1.0, 0.0, 1.0, 1.0);
    var sides = array<f32, 6>(-1.0, 1.0, -1.0, 1.0, 1.0, -1.0);
    let end = ends[corner];
    let side = sides[corner];
    var at = pa;
    var colour = ca;
    var world = a;
    var half = pixels(wa) * 0.5;
    if (end > 0.5) {
        at = pb;
        colour = cb;
        world = b;
        half = pixels(wb) * 0.5;
    }
    let offset = across * side * half + along * (end * 2.0 - 1.0) * half;
    at = vec4<f32>(at.xy + offset * px * at.w, at.zw);
    return Out(at, colour, world);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    if (clipped(input.world)) { discard; }
    return vec4<f32>(input.colour, 1.0);
}";

/// Lit mostly from the camera, like a headlight, so whatever faces the viewer is brightest.
/// Both sides of a face are lit alike, as an open surface can be seen from either.
pub(crate) const FACE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// Clipping planes (spec `layouts.md` §3): `pixel.w` of them, each keeping the side where
// dot(n, p) + d is not above zero.
fn clipped(p: vec3<f32>) -> bool {
    let count = u32(uniforms.pixel.w);
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (i < count && dot(uniforms.clips[i].xyz, p) + uniforms.clips[i].w > 0.0) {
            return true;
        }
    }
    return false;
}
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) colour: vec3<f32>,
    @location(2) world: vec3<f32>,
};
@vertex fn vertex_main(
    @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>,
    @location(3) object: u32,
) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), n, select(c, SELECTED, picked(object)), p);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    if (clipped(input.world)) { discard; }
    let n = normalize(input.normal);
    let facing = abs(dot(n, uniforms.light.xyz));
    // A little light from above as well, so tops and sides of a box differ.
    let above = abs(n.z);
    return vec4<f32>(input.colour * (0.25 + 0.5 * facing + 0.2 * above), 1.0);
}";

/// As [`FACE_SHADER`], but see-through at half strength, for the Ghosted display mode.
pub(crate) const GHOST_FACE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// Clipping planes (spec `layouts.md` §3): `pixel.w` of them, each keeping the side where
// dot(n, p) + d is not above zero.
fn clipped(p: vec3<f32>) -> bool {
    let count = u32(uniforms.pixel.w);
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (i < count && dot(uniforms.clips[i].xyz, p) + uniforms.clips[i].w > 0.0) {
            return true;
        }
    }
    return false;
}
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) colour: vec3<f32>,
    @location(2) world: vec3<f32>,
};
@vertex fn vertex_main(
    @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>,
    @location(3) object: u32,
) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), n, select(c, SELECTED, picked(object)), p);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    if (clipped(input.world)) { discard; }
    let n = normalize(input.normal);
    let facing = abs(dot(n, uniforms.light.xyz));
    let above = abs(n.z);
    // Alpha 0.5: a ghosted face covers half of what is behind it.
    return vec4<f32>(input.colour * (0.25 + 0.5 * facing + 0.2 * above), 0.5);
}";
