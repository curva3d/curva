//! Bevy's depth handed to Curva's lines: for a view Bevy draws, Bevy's depth (reversed, four
//! samples per pixel) is turned into Curva's own depth where the view sits on the scene texture,
//! so Curva's lines, outlines and previews are hidden behind Bevy's surfaces exactly as behind
//! Curva's own.
// CUSTOM BEVY PART: cad-lines

use egui_wgpu::wgpu;

use crate::camera::Lens;

/// Reads Bevy's depth at each pixel of the view, works out how far along the view's direction it
/// lies, and writes the depth Curva's own camera gives that distance (`Camera::matrix`).
const HANDOVER_SHADER: &str = "
struct Handover {
    // The view's top left corner on the scene texture, in pixels.
    corner: vec4<f32>,
    // Bevy's lens: x 1 if flat; y its nearest and z its farthest distance; w where its backdrop
    // begins: Curva's background, not a surface.
    bevy: vec4<f32>,
    // Curva's: x 1 if flat; y nearest, z farthest, w the distance to the target.
    curva: vec4<f32>,
    // x: for a flat view, the depth range Curva's matrix spreads over.
    range: vec4<f32>,
};
@group(0) @binding(0) var bevy_depth: texture_depth_multisampled_2d;
@group(0) @binding(1) var<uniform> h: Handover;
@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
// How far along the view a surface at Bevy's depth `d` lies; past the backdrop's distance where
// nothing is drawn: Bevy clears to 0, the far end of its reversed depth.
fn distance_at(d: f32) -> f32 {
    if (d <= 0.0) { return h.bevy.w; }
    if (h.bevy.x > 0.5) { return h.bevy.z + d * (h.bevy.y - h.bevy.z); }
    return h.bevy.y / d;
}
@fragment fn fragment_main(@builtin(position) at: vec4<f32>) -> @builtin(frag_depth) f32 {
    let p = vec2<i32>(at.xy - h.corner.xy);
    // Nothing drawn here, or only Bevy's backdrop, so the grid shows there.
    if (distance_at(textureLoad(bevy_depth, p, 0)) >= h.bevy.w) { return 1.0; }
    // The farthest surface among the samples of this pixel and the four beside it, leaving out
    // the backdrop. A line drawn on a face slanting away takes its depth where it crosses each
    // pixel, up to half a pixel from the samples there, so one sample alone could lie in front of
    // the line and break it into dashes (#663). The farthest a pixel away is never in front: as a
    // graphics card's slope-scaled depth bias, of about a pixel's worth of the face's slope.
    let last = vec2<i32>(textureDimensions(bevy_depth)) - 1;
    var steps = array<vec2<i32>, 5>(
        vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1)
    );
    var distance = 0.0;
    for (var k = 0; k < 5; k = k + 1) {
        let q = clamp(p + steps[k], vec2<i32>(0), last);
        for (var s = 0; s < i32(textureNumSamples(bevy_depth)); s = s + 1) {
            let here = distance_at(textureLoad(bevy_depth, q, s));
            if (here < h.bevy.w) { distance = max(distance, here); }
        }
    }
    if (h.curva.x > 0.5) {
        return clamp(distance / h.range.x + 0.5 - h.curva.w / h.range.x, 0.0, 1.0);
    }
    let near = h.curva.y;
    let far = h.curva.z;
    let a = far / (near - far);
    let b = near * far / (near - far);
    return clamp((b - a * distance) / distance, 0.0, 1.0);
}
";

/// The program that hands Bevy's depth to Curva's lines.
pub(crate) struct Handover {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl Handover {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curva depth handover"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("curva depth handover"),
            source: wgpu::ShaderSource::Wgsl(HANDOVER_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva depth handover"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("curva depth handover"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    /// Writes into `depth`, over `area` (left, top, width, height in pixels), the depth Curva's
    /// camera `lens` gives what Bevy drew with depth `bevy_depth`.
    pub(crate) fn write(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        bevy_depth: &wgpu::Texture,
        depth: &wgpu::TextureView,
        area: [f32; 4],
        lens: &Lens,
    ) {
        let flat = if lens.parallel { 1.0 } else { 0.0 };
        let (near, far) = (lens.near as f32, lens.far as f32);
        // The target is half-way through a flat view's depth range (`Camera::matrix`).
        let distance = (lens.near + lens.far) as f32 / 2.0;
        // Just short of Bevy's backdrop (`ViewScene::place_backdrop`).
        let backdrop =
            (lens.near + (crate::camera::BACKDROP_DEPTH - 0.01) * (lens.far - lens.near)) as f32;
        let numbers: [f32; 16] = [
            area[0],
            area[1],
            0.0,
            0.0,
            flat,
            near,
            far,
            backdrop,
            flat,
            near,
            far,
            distance,
            far - near,
            0.0,
            0.0,
            0.0,
        ];
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curva depth handover"),
            size: std::mem::size_of_val(&numbers) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bytes: Vec<u8> = numbers.iter().flat_map(|n| n.to_ne_bytes()).collect();
        queue.write_buffer(&uniform, 0, &bytes);
        let view = bevy_depth.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..wgpu::TextureViewDescriptor::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curva depth handover"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva depth handover"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(area[0], area[1], area[2], area[3], 0.0, 1.0);
        pass.set_scissor_rect(
            area[0] as u32,
            area[1] as u32,
            area[2] as u32,
            area[3] as u32,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
