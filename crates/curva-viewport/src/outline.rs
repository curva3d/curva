//! Outlines of curved shapes (#182; spec `views-and-styles.md` §4.1): a pass over the depth
//! picture, after surfaces are drawn and before lines, marking where the depth jumps: the rim of
//! a sphere, the sides of a cylinder, one object in front of another.
//!
//! The test: for each pixel and each of the four directions, the depth a step further on is
//! compared with where the surface's own slope would carry it. Depth values in a perspective or
//! flat view change evenly across any flat surface on screen, so a flat surface seen at however
//! steep an angle never counts; a jump well past the slope does. The outline is drawn on the near
//! side of the jump, two pixels wide (the default line's width), so it sits on the object, not on
//! what is behind it.
//! (Depth discontinuities as outlines: Saito and Takahashi, "Comprehensible rendering of 3-D
//! shapes", SIGGRAPH 1990.)

use egui_wgpu::wgpu;

/// How far back, as a share of the distance, the depth must step to count as an outline.
const JUMP: f32 = 0.01;

const SHADER: &str = "
@group(0) @binding(0) var depth: texture_depth_2d;
struct Look { colour: vec4<f32> };
@group(0) @binding(1) var<uniform> look: Look;

@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the view.
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

fn depth_at(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    return textureLoad(depth, clamp(p, vec2<i32>(0), size - 1), 0);
}

// Whether the depth jumps between `a` and `b`, further than the slope from `before` to `a`
// carries on: `b` lies well behind where the surface through `before` and `a` would be. One
// minus a depth value goes as one over the distance, so the jump is also measured against it:
// it must be a real step back in distance (a few hundredths of the distance), not the stored
// depth's limited precision, which grows with distance.
fn jumps(before: f32, a: f32, b: f32) -> bool {
    // Against the empty background every step is an outline, however steeply the surface
    // turns away just before it (a sphere's rim seen straight on).
    if (b >= 1.0) { return a < 1.0; }
    let slope = a - before;
    let past = (b - a) - slope;
    let near = max(1.0 - a, 0.000001);
    return past > 4.0 * abs(slope) && past > JUMP * near;
}

@fragment fn fragment_main(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(at.xy);
    // Only on objects: the background is never outlined, the object beside it is.
    if (depth_at(p) >= 1.0) { discard; }
    // Written out with no loops, and ending in a return: DirectX 12's older shader compiler
    // (FXC) refuses to unroll a loop that leaves early, and does not count `discard` as leaving
    // the function. Curva must run there (Windows CI caught both).
    let outline = near_step(p, vec2<i32>(1, 0)) || near_step(p, vec2<i32>(-1, 0))
        || near_step(p, vec2<i32>(0, 1)) || near_step(p, vec2<i32>(0, -1));
    if (!outline) { discard; }
    return look.colour;
}

// Whether a step back lies within two pixels of `p` in direction `step`: between `p` and the next
// pixel, or between the next and the one after.
fn near_step(p: vec2<i32>, step: vec2<i32>) -> bool {
    let before = depth_at(p - step);
    let here = depth_at(p);
    let next = depth_at(p + step);
    let after = depth_at(p + step * 2);
    return jumps(before, here, next) || jumps(here, next, after);
}";

/// Draws outlines from a view's depth picture onto its colour picture.
pub(crate) struct Outliner {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    look: wgpu::Buffer,
}

impl Outliner {
    /// Prepares the pass for a colour picture in `format`.
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curva outlines"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("curva outlines"),
            source: wgpu::ShaderSource::Wgsl(SHADER.replace("JUMP", &format!("{JUMP:?}")).into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva outlines"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("curva outlines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let look = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curva outlines look"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            look,
        }
    }

    /// Draws the outlines found in `depth` onto `colour`, in `ink`, within the view's area
    /// `[x, y, width, height]` in pixels.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        colour: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        area: [f32; 4],
        ink: [f32; 3],
    ) {
        let bytes: Vec<u8> = ink
            .iter()
            .chain(&[1.0])
            .flat_map(|value| value.to_le_bytes())
            .collect();
        queue.write_buffer(&self.look, 0, &bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curva outlines"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.look.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva outlines"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: colour,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let [x, y, width, height] = area;
        pass.set_viewport(x, y, width, height, 0.0, 1.0);
        pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
