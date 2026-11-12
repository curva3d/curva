//! Frosted glass for the command bar (spec `docs/specs/ui-floating-panels.md`, section 2).
//!
//! The viewports are drawn into a texture the size of the window instead of straight onto the
//! screen, then copied to the screen. Glass panels blur their part of that texture in a few small
//! passes, the dual Kawase blur, and draw it under their tint, rounded to their corners. Where the
//! graphics card is a software one the blur is skipped and the tint alone shows.
//!
//! Everything happens in egui's paint callbacks: the viewports draw into the texture while egui
//! prepares the frame, which comes before the glass is prepared, as the viewports are painted on
//! a lower layer.

use egui_wgpu::wgpu;

/// Copies the scene texture to the screen, pixel for pixel.
const COPY_SHADER: &str = "
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var filtered: sampler;
@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fragment_main(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(scene, vec2<i32>(at.xy), 0);
}";

/// One step of the dual Kawase blur. `down` halves the picture, averaging a centre and four
/// diagonal taps; up doubles it, from eight taps around. `region` is the part of the source
/// read, in its texture coordinates; `texel` half a source pixel.
const BLUR_SHADER: &str = "
struct Step { region: vec4<f32>, texel: vec4<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var filtered: sampler;
@group(0) @binding(2) var<uniform> step: Step;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> Out {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    let uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return Out(vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0), step.region.xy + uv * step.region.zw);
}
fn tap(uv: vec2<f32>) -> vec4<f32> {
    let low = step.region.xy;
    let high = step.region.xy + step.region.zw;
    return textureSample(source, filtered, clamp(uv, low, high));
}
@fragment fn down(input: Out) -> @location(0) vec4<f32> {
    let h = step.texel.xy;
    var sum = tap(input.uv) * 4.0;
    sum += tap(input.uv - h) + tap(input.uv + h);
    sum += tap(input.uv + vec2<f32>(h.x, -h.y)) + tap(input.uv + vec2<f32>(-h.x, h.y));
    return sum / 8.0;
}
@fragment fn up(input: Out) -> @location(0) vec4<f32> {
    let h = step.texel.xy;
    var sum = tap(input.uv + vec2<f32>(-h.x * 2.0, 0.0));
    sum += tap(input.uv + vec2<f32>(-h.x, h.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(0.0, h.y * 2.0));
    sum += tap(input.uv + vec2<f32>(h.x, h.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(h.x * 2.0, 0.0));
    sum += tap(input.uv + vec2<f32>(h.x, -h.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(0.0, -h.y * 2.0));
    sum += tap(input.uv + vec2<f32>(-h.x, -h.y)) * 2.0;
    return sum / 12.0;
}";

/// Draws the blurred picture into a glass panel, faded out beyond its rounded corners.
const GLASS_SHADER: &str = "
struct Panel { rect: vec4<f32>, radius: vec4<f32> };
@group(0) @binding(0) var blurred: texture_2d<f32>;
@group(0) @binding(1) var filtered: sampler;
@group(0) @binding(2) var<uniform> panel: Panel;
@vertex fn vertex_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fragment_main(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let size = panel.rect.zw;
    let local = at.xy - panel.rect.xy;
    let colour = textureSample(blurred, filtered, local / size);
    // Distance outside a rounded rectangle, for its corners.
    let r = panel.radius.x;
    let q = abs(local - size * 0.5) - (size * 0.5 - vec2<f32>(r));
    let outside = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let inside = clamp(0.5 - outside, 0.0, 1.0);
    return vec4<f32>(colour.rgb, inside);
}";

/// How many times the blur halves the picture before growing it back: two gives a soft blur of
/// about 8 points at the window's scale.
const BLUR_LEVELS: usize = 2;

fn texture(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size[0].max(1),
                height: size[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn sampled_layout(device: &wgpu::Device, with_uniform: bool) -> wgpu::BindGroupLayout {
    let mut entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ];
    if with_uniform {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("curva glass"),
        entries: &entries,
    })
}

struct Stage<'a> {
    label: &'static str,
    shader: &'a wgpu::ShaderModule,
    fragment: &'static str,
    layout: &'a wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    /// Drawn in egui's pass, which has the window's depth buffer.
    on_screen: bool,
}

fn stage(device: &wgpu::Device, parts: &Stage<'_>) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(parts.label),
        bind_group_layouts: &[Some(parts.layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(parts.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: parts.shader,
            entry_point: Some("vertex_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: parts.shader,
            entry_point: Some(parts.fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: parts.format,
                blend: parts.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: parts.on_screen.then(|| wgpu::DepthStencilState {
            format: crate::gpu::DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// The window-sized texture the viewports draw into, with its depth buffer.
pub(crate) struct SceneTarget {
    pub(crate) size: [u32; 2],
    pub(crate) colour: wgpu::TextureView,
    pub(crate) depth: wgpu::TextureView,
    copy_group: wgpu::BindGroup,
    /// The frame it was last cleared in, so the first viewport of a frame clears it.
    pub(crate) cleared: Option<u64>,
}

/// The blur's textures for one glass panel's size.
struct BlurChain {
    /// The panel's size in pixels the chain was made for.
    size: [u32; 2],
    /// Halved, then halved again.
    levels: Vec<(wgpu::TextureView, [u32; 2])>,
    /// Back at half the panel's size: what the panel shows.
    result: wgpu::TextureView,
}

/// The pipelines and textures for drawing the viewports through a texture and blurring it.
pub(crate) struct Compositor {
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    copy_layout: wgpu::BindGroupLayout,
    step_layout: wgpu::BindGroupLayout,
    copy: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    pub(crate) target: Option<SceneTarget>,
    chain: Option<BlurChain>,
    /// The blur's passes, kept while the glass panel stays the same place and size.
    #[allow(clippy::type_complexity)]
    plan: Option<(
        ([f32; 4], f32, [u32; 2]),
        Vec<(bool, wgpu::BindGroup, wgpu::TextureView)>,
    )>,
    /// Ready for the glass to draw this frame.
    glass_group: Option<wgpu::BindGroup>,
    /// Whether to blur at all; off on software graphics.
    pub(crate) blur: bool,
}

impl Compositor {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let copy_layout = sampled_layout(device, false);
        let step_layout = sampled_layout(device, true);
        let module = |label, source: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let copy_shader = module("curva scene copy", COPY_SHADER);
        let blur_shader = module("curva blur", BLUR_SHADER);
        let glass_shader = module("curva glass", GLASS_SHADER);
        let make = |label, shader, fragment, layout, blend, on_screen| {
            stage(
                device,
                &Stage {
                    label,
                    shader,
                    fragment,
                    layout,
                    format,
                    blend,
                    on_screen,
                },
            )
        };
        Self {
            format,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("curva glass"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..wgpu::SamplerDescriptor::default()
            }),
            copy: make(
                "curva scene copy",
                &copy_shader,
                "fragment_main",
                &copy_layout,
                None,
                true,
            ),
            down: make(
                "curva blur down",
                &blur_shader,
                "down",
                &step_layout,
                None,
                false,
            ),
            up: make(
                "curva blur up",
                &blur_shader,
                "up",
                &step_layout,
                None,
                false,
            ),
            glass: make(
                "curva glass",
                &glass_shader,
                "fragment_main",
                &step_layout,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                true,
            ),
            copy_layout,
            step_layout,
            target: None,
            chain: None,
            glass_group: None,
            plan: None,
            blur: true,
        }
    }

    /// Makes sure the scene texture matches the window, and returns it.
    pub(crate) fn target(&mut self, device: &wgpu::Device, size: [u32; 2]) -> &mut SceneTarget {
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            let colour = texture(
                device,
                "curva scene",
                size,
                self.format,
                // Writable too, for the test that checks the blur.
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            );
            let depth = texture(
                device,
                "curva scene depth",
                size,
                crate::gpu::DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let copy_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("curva scene copy"),
                layout: &self.copy_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&colour),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.target = Some(SceneTarget {
                size,
                colour,
                depth,
                copy_group,
                cleared: None,
            });
        }
        self.target.as_mut().expect("made just above")
    }

    /// Copies a viewport from the scene texture to the screen, inside egui's pass.
    pub(crate) fn copy_to_screen(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(target) = &self.target {
            pass.set_pipeline(&self.copy);
            pass.set_bind_group(0, &target.copy_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    fn step_group(
        &self,
        device: &wgpu::Device,
        source: &wgpu::TextureView,
        values: [f32; 8],
    ) -> wgpu::BindGroup {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("curva blur step"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curva blur step"),
            layout: &self.step_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Blurs the scene behind a glass panel, `rect` in pixels as `[x, y, width, height]`, ready
    /// for [`Self::draw_glass`]. `radius` is its corners' radius in pixels.
    pub(crate) fn blur_behind(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        rect: [f32; 4],
        radius: f32,
    ) {
        if !self.blur {
            self.glass_group = None;
            return;
        }
        let Some(target) = &self.target else {
            self.glass_group = None;
            return;
        };
        let key = (rect, radius, target.size);
        if self.plan.as_ref().is_none_or(|(held, _)| *held != key) {
            let steps = self.plan_blur(device, rect, radius);
            self.plan = Some((key, steps));
        }
        let Some((_, steps)) = &self.plan else {
            return;
        };
        for (down, group, into) in steps {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("curva blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: into,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if *down { &self.down } else { &self.up });
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// The blur's passes for a glass panel at `rect`, made once for each place and size: down
    /// from the panel's part of the scene, down again, then back up twice. Each is whether it
    /// goes down, what it reads, and what it draws into. Also makes the glass's own bind group.
    fn plan_blur(
        &mut self,
        device: &wgpu::Device,
        rect: [f32; 4],
        radius: f32,
    ) -> Vec<(bool, wgpu::BindGroup, wgpu::TextureView)> {
        let Some(target) = &self.target else {
            return Vec::new();
        };
        let [tw, th] = target.size.map(|v| v as f32);
        let size = [rect[2].max(1.0) as u32, rect[3].max(1.0) as u32];
        if self.chain.as_ref().is_none_or(|chain| chain.size != size) {
            let usage =
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
            let mut levels = Vec::new();
            let mut level = size;
            for index in 0..BLUR_LEVELS {
                level = level.map(|v| (v / 2).max(1));
                levels.push((
                    texture(
                        device,
                        &format!("curva blur {index}"),
                        level,
                        self.format,
                        usage,
                    ),
                    level,
                ));
            }
            let half = size.map(|v| (v / 2).max(1));
            // Readable too, for the test that checks the blur.
            let result = texture(
                device,
                "curva blur result",
                half,
                self.format,
                usage | wgpu::TextureUsages::COPY_SRC,
            );
            self.chain = Some(BlurChain {
                size,
                levels,
                result,
            });
        }
        let chain = self.chain.as_ref().expect("made just above");
        let mut steps = Vec::new();
        let region = [rect[0] / tw, rect[1] / th, rect[2] / tw, rect[3] / th];
        let whole = [0.0, 0.0, 1.0, 1.0];
        let half_texel = |size: [u32; 2]| [0.5 / size[0] as f32, 0.5 / size[1] as f32];
        let mut source: (&wgpu::TextureView, [f32; 4], [f32; 2]) =
            (&target.colour, region, [0.5 / tw, 0.5 / th]);
        for (view, level) in &chain.levels {
            let [hx, hy] = source.2;
            let group = self.step_group(
                device,
                source.0,
                [
                    source.1[0],
                    source.1[1],
                    source.1[2],
                    source.1[3],
                    hx,
                    hy,
                    0.0,
                    0.0,
                ],
            );
            steps.push((true, group, view.clone()));
            source = (view, whole, half_texel(*level));
        }
        let ups: Vec<(&wgpu::TextureView, [u32; 2])> = chain
            .levels
            .iter()
            .rev()
            .skip(1)
            .map(|(view, level)| (view, *level))
            .chain(std::iter::once((
                &chain.result,
                size.map(|v| (v / 2).max(1)),
            )))
            .collect();
        for (view, level) in ups {
            let [hx, hy] = source.2;
            let group = self.step_group(device, source.0, [0.0, 0.0, 1.0, 1.0, hx, hy, 0.0, 0.0]);
            steps.push((false, group, view.clone()));
            source = (view, whole, half_texel(level));
        }
        self.glass_group = Some(self.step_group(
            device,
            &chain.result,
            [rect[0], rect[1], rect[2], rect[3], radius, 0.0, 0.0, 0.0],
        ));
        steps
    }

    /// Draws the blur made by [`Self::blur_behind`] into the glass panel, inside egui's pass.
    pub(crate) fn draw_glass(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(group) = &self.glass_group {
            pass.set_pipeline(&self.glass);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// A glass panel's paint callback: blurs what is behind it and draws that under its tint.
pub struct Glass {
    /// The panel, in points.
    pub rect: epaint::Rect,
    /// Its corners' radius, in points.
    pub radius: f32,
}

impl egui_wgpu::CallbackTrait for Glass {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<crate::LineRenderer>() {
            let scale = screen.pixels_per_point;
            let rect = [
                self.rect.min.x * scale,
                self.rect.min.y * scale,
                self.rect.width() * scale,
                self.rect.height() * scale,
            ];
            renderer
                .compositor
                .blur_behind(device, encoder, rect, self.radius * scale);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: epaint::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<crate::LineRenderer>() {
            renderer.compositor.draw_glass(pass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Compositor;
    use egui_wgpu::wgpu;

    /// A graphics device, or `None` on a machine without one.
    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
    }

    #[test]
    fn the_blur_softens_a_sharp_edge() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        let (width, height) = (128u32, 64u32);
        let mut compositor = Compositor::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let target = compositor.target(&device, [width, height]);
        // Black on the left half, white on the right: a sharp edge down the middle.
        let pixels: Vec<u8> = (0..height)
            .flat_map(|_| {
                (0..width).flat_map(|x| {
                    if x < width / 2 {
                        [0, 0, 0, 255]
                    } else {
                        [255; 4]
                    }
                })
            })
            .collect();
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        queue.write_texture(
            target.colour.texture().as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        compositor.blur_behind(
            &device,
            &mut encoder,
            [0.0, 0.0, width as f32, height as f32],
            0.0,
        );
        // Read the blurred result back: half the panel's size.
        let result = compositor.chain.as_ref().unwrap().result.texture().clone();
        let (rw, rh) = (width / 2, height / 2);
        let row = (rw * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curva blur test"),
            size: u64::from(row * rh),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            result.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(rh),
                },
            },
            wgpu::Extent3d {
                width: rw,
                height: rh,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mapped = slice.get_mapped_range().unwrap();
        let middle = (rh / 2 * row) as usize;
        let reds: Vec<u8> = (0..rw as usize).map(|x| mapped[middle + x * 4]).collect();
        // Far from the edge it stays black and white; near it, there are greys in between.
        assert!(reds[2] < 30 && reds[rw as usize - 3] > 225, "{reds:?}");
        let greys = reds
            .iter()
            .filter(|value| (40..=215).contains(*value))
            .count();
        assert!(greys >= 3, "the edge was not softened: {reds:?}");
    }
}
