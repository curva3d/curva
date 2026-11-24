//! The shader programs the graphics card runs, in WGSL.

use crate::NO_OBJECT;

// The line program's table and its places, written out in its text, match the Rust side's.
const _: () = assert!(crate::line_table::LINE_ROWS == 18);
const _: () = assert!(crate::view_style::FADE_ROWS == 2);
const _: () = assert!(crate::line_table::HIDDEN_SLOT == 8);
const _: () = assert!(crate::view_style::FACE_ROWS == 8);
const _: () = assert!(super::upload::OBJECT_BITS == 0x00FF_FFFF);

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

/// The grid: true one-pixel lines, two vertices a segment, put at the far end of the depth
/// (just inside it, so it is not cut off there), so anything drawn, Curva's or Bevy's, covers it
/// (#344).
pub(crate) const GRID_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec3<f32> };
@vertex fn vertex_main(@location(0) p: vec3<f32>, @location(1) c: vec3<f32>) -> Out {
    let at = uniforms.matrix * vec4<f32>(p, 1.0);
    return Out(vec4<f32>(at.xy, at.w * 0.99999, at.w), c);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.colour, 1.0);
}";

/// What the line and circle shaders share (spec `line-weights.md` §2): `segment` draws one
/// corner of a segment as a quad of two triangles facing the screen, as wide as its own width,
/// and lengthened by half that width at each end so the segments of a curve overlap at their
/// joins. `pixel.xy` is a pixel's size in clip space, `pixel.z` the screen's pixels per
/// millimetre.
macro_rules! segment_wgsl {
    () => {
        "
struct Uniforms {
    matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32>, clips: array<vec4<f32>, 4>,
    // Each kind of line's look from the view's style (`LineTable::rows`): its colour and whether
    // to use it, then its width factor, fixed width (below zero for none) and whether it shows.
    lines: array<vec4<f32>, 18>,
    // Lines by distance (`Style::fade_rows`): the plane distances are measured from, then where
    // lines start to thin and fade, where they are thinnest, and how thin and faint there.
    fade: array<vec4<f32>, 2>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// How far a point is between where lines start to thin and fade (0) and where they are
// thinnest and faintest (1).
fn distance_part(p: vec3<f32>) -> f32 {
    let plane = uniforms.fade[0];
    let range = uniforms.fade[1];
    let away = dot(plane.xyz, p) + plane.w;
    return clamp((away - range.x) / max(range.y - range.x, 1e-6), 0.0, 1.0);
}
// Where hidden lines' look is in `lines`, after the eight kinds of line (`HIDDEN_SLOT`).
const HIDDEN_SLOT: u32 = 8u;
// A line's object word: its object's number in the low three bytes (all ones for none), its
// kind in the top byte (`upload::packed`).
const OBJECT_BITS: u32 = 0x00FFFFFFu;
// The Technical display mode's hidden lines are dotted: `dotted` turns this on.
const DOTTED: bool = false;
// One dot and one gap, in pixels.
const DOT_PERIOD: f32 = 6.0;
// Plain one-pixel lines, two corners a segment drawn as a line list, for meshes' wires:
// `thin` turns this on.
const THIN: bool = false;
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
    // Where along the line, in pixels, for dotting hidden lines (`dotted`).
    @location(2) along: f32,
    // How far across the line from its middle, in pixels, and half its solid width: its outer
    // half pixel fades out, so its edges are smooth rather than stepped.
    @location(3) @interpolate(linear) across: f32,
    @location(4) @interpolate(linear) half: f32,
    // How faint the line is where it is furthest (`distance_part`), 0 for none.
    @location(5) @interpolate(flat) fade: f32,
};
fn pixels(width: f32) -> f32 {
    return max(select(-width, width * uniforms.pixel.z, width > 0.0), 1.0);
}
// A line's own width as its kind's look sets it: fixed, or its own times a factor.
fn styled(width: f32, size: vec4<f32>) -> f32 {
    if (size.y >= 0.0) { return size.y; }
    return width * size.x;
}
fn segment(
    corner: u32,
    a: vec3<f32>, given_ca: vec3<f32>, given_wa: f32,
    b: vec3<f32>, given_cb: vec3<f32>, given_wb: f32,
    object: u32,
) -> Out {
    // The look the view's style gives this kind of line; hidden lines all take hidden lines'.
    let number = object & OBJECT_BITS;
    let slot = select(object >> 24u, HIDDEN_SLOT, DOTTED);
    let look = uniforms.lines[slot * 2u];
    let size = uniforms.lines[slot * 2u + 1u];
    if (size.z < 0.5) {
        return Out(vec4<f32>(2.0, 2.0, 2.0, 1.0), given_ca, a, 0.0, 0.0, 0.0, 0.0);
    }
    // Thinner with distance, as far as the style's rule says (`fade`).
    let ta = distance_part(a);
    let tb = distance_part(b);
    let thin = uniforms.fade[1].z;
    // A selected object draws in the selection colour and a pixel wider (`line_width`).
    let chosen = picked(select(number, NO_OBJECT, number == OBJECT_BITS));
    let ca = select(select(given_ca, look.rgb, look.w > 0.5), SELECTED, chosen);
    let cb = select(select(given_cb, look.rgb, look.w > 0.5), SELECTED, chosen);
    let wa = widened(styled(given_wa, size) * mix(1.0, thin, ta), chosen);
    let wb = widened(styled(given_wb, size) * mix(1.0, thin, tb), chosen);
    var pa = uniforms.matrix * vec4<f32>(a, 1.0);
    var pb = uniforms.matrix * vec4<f32>(b, 1.0);
    // Behind the camera: cut the segment where it crosses the near plane.
    if (pa.z < 0.0 && pb.z < 0.0) {
        return Out(vec4<f32>(2.0, 2.0, 2.0, 1.0), ca, a, 0.0, 0.0, 0.0, 0.0);
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
    // A thin line is just its two ends.
    let end = select(ends[corner], f32(corner), THIN);
    let side = select(sides[corner], 0.0, THIN);
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
    // How faint the line may get far away, worked out for each pixel from where it is; a
    // selected line never fades, so it stays easy to see.
    let fade = select(uniforms.fade[1].w, 0.0, chosen);
    // Half a pixel wider each side than the line, for its edge to fade out in.
    let edge = half + 0.5;
    let offset = across * side * edge + along * (end * 2.0 - 1.0) * half;
    at = vec4<f32>(at.xy + offset * px * at.w, at.zw);
    // Measured along the line from a fixed point on screen, not from the segment's start, so the
    // dots run on across the short segments of a curve instead of each starting with a dot.
    return Out(at, colour, world, dot(at.xy / at.w / px, along), side * edge, half, fade);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    if (clipped(input.world)) { discard; }
    if (DOTTED && fract(input.along / DOT_PERIOD) >= 0.5) { discard; }
    let faint = input.fade * distance_part(input.world);
    let cover = clamp(input.half + 0.5 - abs(input.across), 0.0, 1.0) * (1.0 - faint);
    if (cover <= 0.0) { discard; }
    return vec4<f32>(input.colour, cover);
}
"
    };
}

/// The model's lines: each segment is one instance, both its ends.
pub(crate) const LINE_SHADER: &str = concat!(
    segment_wgsl!(),
    "
@vertex fn vertex_main(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>, @location(1) ca: vec3<f32>, @location(2) wa: f32,
    @location(6) object: u32,
    @location(3) b: vec3<f32>, @location(4) cb: vec3<f32>, @location(5) wb: f32,
    @location(7) object_b: u32,
) -> Out {
    return segment(corner, a, ca, wa, b, cb, wb, object);
}"
);

/// Whole circles (#153): each circle is one instance, its centre and its two axes as long as
/// its radius, drawn as [`crate::CIRCLE_SEGMENTS`] segments worked out here, six corners each.
/// `CIRCLE_SEGMENTS` is put in front with the selection table ([`table_wgsl`]).
pub(crate) const CIRCLE_SHADER: &str = concat!(
    segment_wgsl!(),
    "
@vertex fn vertex_main(
    @builtin(vertex_index) index: u32,
    @location(0) centre: vec3<f32>, @location(1) x: vec3<f32>, @location(2) y: vec3<f32>,
    @location(3) colour: vec3<f32>, @location(4) width: f32, @location(5) object: u32,
) -> Out {
    let step = 6.283185307179586 / f32(CIRCLE_SEGMENTS);
    let piece = index / 6u;
    let ta = f32(piece) * step;
    let tb = f32(piece + 1u) * step;
    let a = centre + x * cos(ta) + y * sin(ta);
    let b = centre + x * cos(tb) + y * sin(tb);
    return segment(index % 6u, a, colour, width, b, colour, width, object);
}"
);

/// Faces, each kind as the view's style gives it (`Style::face_rows`): shown or not, in one
/// colour or its own (the selection colour when selected), and how opaque. One pass draws the
/// opaque ones, hiding what is behind them; `see_through` makes a pass drawing the others over
/// the lines, so lines behind them show through. Lit mostly from the camera, like a headlight,
/// so whatever faces the viewer is brightest, or flat where the style says (Hidden, #164). Both
/// sides of a face are lit alike, as an open surface can be seen from either.
pub(crate) const FACE_SHADER: &str = "
struct Uniforms {
    matrix: mat4x4<f32>,
    // The direction faces are lit from, and 1 after it where the style lights them flat.
    light: vec4<f32>,
    pixel: vec4<f32>, clips: array<vec4<f32>, 4>,
    lines: array<vec4<f32>, 18>,
    fade: array<vec4<f32>, 2>,
    // Each kind of face's look (`Style::face_rows`): its colour and whether to use it, then its
    // opacity and whether it shows.
    faces: array<vec4<f32>, 8>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// A face's object word: its object's number in the low three bytes (all ones for none), its
// kind in the top byte (`upload::packed`).
const OBJECT_BITS: u32 = 0x00FFFFFFu;
// This pass draws the see-through faces rather than the opaque ones: `see_through` turns it on.
const SEE_THROUGH: bool = false;
// A command's preview: half see-through in its own colour whatever the style, lit (`preview`).
const PREVIEW: bool = false;
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
    // How opaque, 0 where the face's kind is hidden.
    @location(3) @interpolate(flat) opacity: f32,
};
@vertex fn vertex_main(
    @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>,
    @location(3) object: u32,
) -> Out {
    let at = uniforms.matrix * vec4<f32>(p, 1.0);
    if (PREVIEW) { return Out(at, n, c, p, 0.5); }
    let number = object & OBJECT_BITS;
    let kind = object >> 24u;
    let look = uniforms.faces[kind * 2u];
    let how = uniforms.faces[kind * 2u + 1u];
    // A kind in one colour keeps it when selected: its selection shows by its lines.
    let chosen = picked(select(number, NO_OBJECT, number == OBJECT_BITS));
    let colour = select(select(c, SELECTED, chosen), look.rgb, look.w > 0.5);
    return Out(at, n, colour, p, select(0.0, how.x, how.y > 0.5));
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    if (clipped(input.world)) { discard; }
    var alpha = 1.0;
    if (PREVIEW) {
        alpha = input.opacity;
    } else {
        if (input.opacity <= 0.0 || (input.opacity < 1.0) != SEE_THROUGH) { discard; }
        if (SEE_THROUGH) { alpha = input.opacity; }
        if (uniforms.light.w > 0.5) { return vec4<f32>(input.colour, alpha); }
    }
    let n = normalize(input.normal);
    let facing = abs(dot(n, uniforms.light.xyz));
    // A little light from above as well, so tops and sides of a box differ.
    let above = abs(n.z);
    return vec4<f32>(input.colour * (0.25 + 0.5 * facing + 0.2 * above), alpha);
}";

/// [`FACE_SHADER`] drawing the faces the style makes see-through, over the lines.
pub(crate) fn see_through_faces() -> String {
    let on = FACE_SHADER.replace(
        "const SEE_THROUGH: bool = false;",
        "const SEE_THROUGH: bool = true;",
    );
    debug_assert_ne!(on, FACE_SHADER, "the face shader has no see-through switch");
    on
}

/// [`FACE_SHADER`] drawing a command's preview, ghosted in every display mode.
pub(crate) fn preview_faces() -> String {
    let on = FACE_SHADER.replace(
        "const PREVIEW: bool = false;",
        "const PREVIEW: bool = true;",
    );
    debug_assert_ne!(on, FACE_SHADER, "the face shader has no preview switch");
    on
}

/// `shader`, built on the shared line code, drawing dotted, for the Technical display mode's
/// hidden lines (#164): a dot and a gap every 6 pixels along each segment (`DOT_PERIOD`).
pub(crate) fn dotted(shader: &str) -> String {
    let dotted = shader.replace("const DOTTED: bool = false;", "const DOTTED: bool = true;");
    debug_assert_ne!(dotted, shader, "the shader has no dotted switch");
    dotted
}

/// The line `shader` drawing plain one-pixel lines, two corners a segment, for meshes' wires.
pub(crate) fn thin(shader: &str) -> String {
    let thin = shader.replace("const THIN: bool = false;", "const THIN: bool = true;");
    debug_assert_ne!(thin, shader, "the shader has no thin switch");
    thin
}
