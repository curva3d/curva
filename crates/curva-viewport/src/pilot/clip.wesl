// Bevy's standard surface, cut by a view's clipping planes (Curva's custom Bevy part `clip-planes`):
// a point on the far side of any plane, where n·p + d > 0, is not drawn. Otherwise as Bevy's own
// `pbr.wesl` draws it, forward.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Clips {
    planes: array<vec4<f32>, 4>,
    // x: how many of the planes are in use.
    count: vec4<u32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> clips: Clips;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    for (var i = 0u; i < clips.count.x; i++) {
        let plane = clips.planes[i];
        if (dot(plane.xyz, in.world_position.xyz) + plane.w > 0.0) {
            discard;
        }
    }
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
