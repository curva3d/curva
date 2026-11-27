//! A sheet's own objects drawn again over its details (#826), as Rhino draws a layout's objects on
//! top of its details: inside each detail's frame, what Bevy drew of the sheet that is not bare
//! paper (filled text and marks) goes over the detail's picture, and the depth there is cleared,
//! so the sheet's lines, drawn next, show over the detail too.

use egui_wgpu::wgpu;

use super::DEPTH_FORMAT;

const SHADER: &str = "
struct Over {
    // The view's top left corner on the scene texture, in pixels.
    corner: vec4<f32>,
    // The paper, as the picture holds it.
    paper: vec4<f32>,
};
@group(0) @binding(0) var sheet: texture_2d<f32>;
@group(0) @binding(1) var<uniform> o: Over;
struct Out {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};
@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fragment_main(@builtin(position) at: vec4<f32>) -> Out {
    let c = textureLoad(sheet, vec2<i32>(at.xy - o.corner.xy), 0);
    // Bare paper lets the detail show; anything else covers it, the edges of filled letters,
    // blended with the paper, partly.
    let away = abs(c.rgb - o.paper.rgb);
    let cover = clamp(max(away.r, max(away.g, away.b)) * 4.0, 0.0, 1.0);
    return Out(vec4<f32>(c.rgb, cover), 1.0);
}
";

/// The program putting a sheet's picture over its details.
pub(crate) struct SheetOver {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl SheetOver {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curva sheet over details"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("curva sheet over details"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva sheet over details"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("curva sheet over details"),
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
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
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

    /// Puts `sheet`, Bevy's picture of a sheet placed at `area` (left, top, width, height in
    /// pixels), over what is on the scene texture (`targets`: its colour and depth) inside each
    /// of `frames` (the same, in pixels), leaving bare `paper` (as the picture holds it) out.
    pub(crate) fn draw(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        sheet: &wgpu::Texture,
        [colour, depth]: [&wgpu::TextureView; 2],
        (area, frames): ([f32; 4], &[[u32; 4]]),
        paper: [f32; 3],
    ) {
        let numbers: [f32; 8] = [
            area[0], area[1], 0.0, 0.0, paper[0], paper[1], paper[2], 1.0,
        ];
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curva sheet over details"),
            size: std::mem::size_of_val(&numbers) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bytes: Vec<u8> = numbers.iter().flat_map(|n| n.to_ne_bytes()).collect();
        queue.write_buffer(&uniform, 0, &bytes);
        let view = sheet.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curva sheet over details"),
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
            label: Some("curva sheet over details"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: colour,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        for &[x, y, w, h] in frames {
            pass.set_scissor_rect(x, y, w, h);
            pass.draw(0..3, 0..1);
        }
    }
}
