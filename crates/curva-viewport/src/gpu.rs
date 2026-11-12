//! Drawing lines and shaded surfaces on the graphics card, on screen and into a picture file.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

const LINE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec3<f32> };
@vertex fn vertex_main(@location(0) p: vec3<f32>, @location(1) c: vec3<f32>) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), c);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.colour, 1.0);
}";

/// Lit mostly from the camera, like a headlight, so whatever faces the viewer is brightest.
/// Both sides of a face are lit alike, as an open surface can be seen from either.
const FACE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) colour: vec3<f32>,
};
@vertex fn vertex_main(
    @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>,
) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), n, c);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let facing = abs(dot(n, uniforms.light.xyz));
    // A little light from above as well, so tops and sides of a box differ.
    let above = abs(n.z);
    return vec4<f32>(input.colour * (0.25 + 0.5 * facing + 0.2 * above), 1.0);
}";

/// As [`FACE_SHADER`], but see-through at half strength, for the Ghosted display mode.
const GHOST_FACE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) colour: vec3<f32>,
};
@vertex fn vertex_main(
    @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>,
) -> Out {
    return Out(uniforms.matrix * vec4<f32>(p, 1.0), n, c);
}
@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let facing = abs(dot(n, uniforms.light.xyz));
    let above = abs(n.z);
    // Alpha 0.5: a ghosted face covers half of what is behind it.
    return vec4<f32>(input.colour * (0.25 + 0.5 * facing + 0.2 * above), 0.5);
}";

/// The depth buffer's format, which the window must be opened with.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
/// Bits of depth to ask the window for, matching [`DEPTH_FORMAT`].
pub const DEPTH_BITS: u8 = 24;

const LINE_VERTEX_SIZE: u64 = 24;
const FACE_VERTEX_SIZE: u64 = 36;
const UNIFORM_SIZE: u64 = 80;
/// Faces are pushed slightly back, so edges on them are drawn in front.
const FACE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 4,
    slope_scale: 2.0,
    clamp: 0.0,
};
const PICTURE_BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.11,
    g: 0.12,
    b: 0.14,
    a: 1.0,
};

// Written out by hand so the crate needs no unsafe casting of memory.
fn line_bytes(lines: &[Vertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(lines.len() * LINE_VERTEX_SIZE as usize);
    for vertex in lines {
        for value in vertex.position.iter().chain(&vertex.colour) {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes
}

fn face_bytes(faces: &[MeshVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(faces.len() * FACE_VERTEX_SIZE as usize);
    for vertex in faces {
        for value in vertex
            .position
            .iter()
            .chain(&vertex.normal)
            .chain(&vertex.colour)
        {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes
}

fn uniform_bytes(matrix: &[[f32; 4]; 4], light: [f32; 3]) -> Vec<u8> {
    matrix
        .iter()
        .flatten()
        .chain(&light)
        .chain(&[0.0])
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Vertices on the graphics card, and what they were made from, so unchanged ones are kept.
struct Uploaded<T> {
    buffer: wgpu::Buffer,
    count: u32,
    source: Arc<Vec<T>>,
}

fn upload<T>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &Arc<Vec<T>>,
    bytes: impl Fn(&[T]) -> Vec<u8>,
    smallest: u64,
) -> Uploaded<T> {
    let bytes = bytes(source);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva vertices"),
        size: (bytes.len() as u64).max(smallest),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, &bytes);
    Uploaded {
        buffer,
        count: source.len() as u32,
        source: Arc::clone(source),
    }
}

fn stale<T>(held: &Option<Uploaded<T>>, wanted: &Arc<Vec<T>>) -> bool {
    held.as_ref()
        .is_none_or(|uploaded| !Arc::ptr_eq(&uploaded.source, wanted))
}

/// How many views can be drawn at once.
pub const VIEW_SLOTS: usize = 4;

/// What each view keeps for itself on the graphics card.
struct ViewSlot {
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    grid: Option<Uploaded<Vertex>>,
    /// Kept per view, as shaded and wireframe views draw edges in different colours.
    scene: Option<Uploaded<Vertex>>,
    shaded: bool,
    ghosted: bool,
}

/// The graphics card's side of the viewport.
pub struct LineRenderer {
    lines: wgpu::RenderPipeline,
    faces: wgpu::RenderPipeline,
    ghost_faces: wgpu::RenderPipeline,
    slots: Vec<ViewSlot>,
    surfaces: Option<Uploaded<MeshVertex>>,
    overlay: Option<Uploaded<Vertex>>,
    /// The ghosted faces of a command's preview.
    preview_faces: Option<Uploaded<MeshVertex>>,
    /// Draws the viewports through a texture, which glass panels blur.
    pub(crate) compositor: crate::glass::Compositor,
}

struct PipelineParts<'a> {
    label: &'static str,
    shader: &'static str,
    stride: u64,
    attributes: &'a [wgpu::VertexAttribute],
    topology: wgpu::PrimitiveTopology,
    bias: wgpu::DepthBiasState,
    /// See-through: blended over what is already drawn, and not hiding what comes after.
    see_through: bool,
}

fn pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    parts: &PipelineParts<'_>,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(parts.label),
        source: wgpu::ShaderSource::Wgsl(parts.shader.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(parts.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: parts.stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: parts.attributes,
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: parts
                    .see_through
                    .then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: parts.topology,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(!parts.see_through),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: parts.bias,
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

impl LineRenderer {
    /// Prepares the drawing programs for a target image format, drawn with a depth buffer in
    /// [`DEPTH_FORMAT`].
    #[must_use]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curva camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let slots = (0..VIEW_SLOTS)
            .map(|_| {
                let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("curva camera"),
                    size: UNIFORM_SIZE,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("curva camera"),
                    layout: &layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniforms.as_entire_binding(),
                    }],
                });
                ViewSlot {
                    uniforms,
                    bind_group,
                    grid: None,
                    scene: None,
                    shaded: false,
                    ghosted: false,
                }
            })
            .collect();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva drawing"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let lines = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva lines",
                shader: LINE_SHADER,
                stride: LINE_VERTEX_SIZE,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                topology: wgpu::PrimitiveTopology::LineList,
                bias: wgpu::DepthBiasState::default(),
                see_through: false,
            },
        );
        let face_attributes = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3
        ];
        let faces = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva faces",
                shader: FACE_SHADER,
                stride: FACE_VERTEX_SIZE,
                attributes: &face_attributes,
                topology: wgpu::PrimitiveTopology::TriangleList,
                bias: FACE_DEPTH_BIAS,
                see_through: false,
            },
        );
        let ghost_faces = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva ghosted faces",
                shader: GHOST_FACE_SHADER,
                stride: FACE_VERTEX_SIZE,
                attributes: &face_attributes,
                topology: wgpu::PrimitiveTopology::TriangleList,
                bias: FACE_DEPTH_BIAS,
                see_through: true,
            },
        );
        Self {
            lines,
            faces,
            ghost_faces,
            slots,
            surfaces: None,
            overlay: None,
            preview_faces: None,
            compositor: crate::glass::Compositor::new(device, format),
        }
    }

    /// Turns the glass blur on or off; off where the graphics card is a software one, so glass
    /// panels show their tint alone.
    pub fn set_blur(&mut self, on: bool) {
        self.compositor.blur = on;
    }

    /// Sends one view's camera and grid, and anything that changed, to the graphics card.
    fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, drawing: &Drawing) {
        let Some(slot) = self.slots.get_mut(drawing.slot) else {
            return;
        };
        queue.write_buffer(
            &slot.uniforms,
            0,
            &uniform_bytes(&drawing.matrix, drawing.light),
        );
        slot.shaded = drawing.shaded;
        slot.ghosted = drawing.ghosted;
        if stale(&slot.grid, &drawing.grid) {
            slot.grid = Some(upload(
                device,
                queue,
                &drawing.grid,
                line_bytes,
                LINE_VERTEX_SIZE,
            ));
        }
        if stale(&slot.scene, &drawing.scene) {
            slot.scene = Some(upload(
                device,
                queue,
                &drawing.scene,
                line_bytes,
                LINE_VERTEX_SIZE,
            ));
        }
        // The views of one frame share these, so only the first of them uploads.
        if stale(&self.surfaces, &drawing.faces) {
            self.surfaces = Some(upload(
                device,
                queue,
                &drawing.faces,
                face_bytes,
                FACE_VERTEX_SIZE,
            ));
        }
        if stale(&self.overlay, &drawing.overlay) {
            self.overlay = Some(upload(
                device,
                queue,
                &drawing.overlay,
                line_bytes,
                LINE_VERTEX_SIZE,
            ));
        }
        if stale(&self.preview_faces, &drawing.preview_faces) {
            self.preview_faces = Some(upload(
                device,
                queue,
                &drawing.preview_faces,
                face_bytes,
                FACE_VERTEX_SIZE,
            ));
        }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, slot: usize) {
        let Some(slot) = self.slots.get(slot) else {
            return;
        };
        pass.set_bind_group(0, &slot.bind_group, &[]);
        let faces = self.surfaces.as_ref().filter(|faces| faces.count > 0);
        let draw_faces =
            |pass: &mut wgpu::RenderPass<'_>, pipeline, faces: &Uploaded<MeshVertex>| {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, faces.buffer.slice(..));
                pass.draw(0..faces.count, 0..1);
            };
        // Solid faces go first and hide the lines behind them.
        if slot.shaded
            && !slot.ghosted
            && let Some(faces) = faces
        {
            draw_faces(pass, &self.faces, faces);
        }
        pass.set_pipeline(&self.lines);
        for lines in [&slot.grid, &slot.scene].into_iter().flatten() {
            if lines.count > 0 {
                pass.set_vertex_buffer(0, lines.buffer.slice(..));
                pass.draw(0..lines.count, 0..1);
            }
        }
        // Ghosted faces go over the lines, so lines behind a face show through it, dimmed,
        // while lines on its near side stay in front.
        if slot.shaded
            && slot.ghosted
            && let Some(faces) = faces
        {
            draw_faces(pass, &self.ghost_faces, faces);
        }
        // A command's solid preview is ghosted in every display mode, over the model.
        if let Some(preview) = self.preview_faces.as_ref().filter(|faces| faces.count > 0) {
            draw_faces(pass, &self.ghost_faces, preview);
        }
        if let Some(overlay) = self.overlay.as_ref().filter(|lines| lines.count > 0) {
            pass.set_pipeline(&self.lines);
            pass.set_vertex_buffer(0, overlay.buffer.slice(..));
            pass.draw(0..overlay.count, 0..1);
        }
    }
}

/// One frame of the viewport, handed to the window's drawing.
pub struct Drawing {
    /// Which view this is, below [`VIEW_SLOTS`].
    pub slot: usize,
    /// This view's grid and axes.
    pub grid: Arc<Vec<Vertex>>,
    /// The objects' lines: curves, and the edges of surfaces and solids.
    pub scene: Arc<Vec<Vertex>>,
    /// The faces of surfaces and solids, drawn when the view is shaded.
    pub faces: Arc<Vec<MeshVertex>>,
    /// The live preview.
    pub overlay: Arc<Vec<Vertex>>,
    /// The live preview's surfaces and solids, ghosted ([`crate::ghost_faces`]).
    pub preview_faces: Arc<Vec<MeshVertex>>,
    /// The camera.
    pub matrix: [[f32; 4]; 4],
    /// The direction the camera looks, of length 1, which lights the faces.
    pub light: [f32; 3],
    /// Whether this view shows faces shaded, rather than only lines.
    pub shaded: bool,
    /// Whether shaded faces are see-through, for the Ghosted display mode.
    pub ghosted: bool,
    /// Where the view is in the window, in points.
    pub area: epaint::Rect,
    /// The window's frame number, so the first view drawn in a frame clears the scene texture.
    pub frame: u64,
    /// Behind the model.
    pub background: epaint::Color32,
}

impl egui_wgpu::CallbackTrait for Drawing {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer) = resources.get_mut::<LineRenderer>() else {
            return Vec::new();
        };
        renderer.update(device, queue, self);
        // Drawn into the scene texture now, and copied to the screen when egui paints.
        let size = screen.size_in_pixels;
        let target = renderer.compositor.target(device, size);
        let first = target.cleared != Some(self.frame);
        target.cleared = Some(self.frame);
        let (colour, depth) = (target.colour.clone(), target.depth.clone());
        let scale = screen.pixels_per_point;
        let low = (self.area.min * scale).max(epaint::Pos2::ZERO);
        let high = (self.area.max * scale).min(epaint::pos2(size[0] as f32, size[1] as f32));
        if high.x - low.x < 1.0 || high.y - low.y < 1.0 {
            return Vec::new();
        }
        // Written as stored, like every colour Curva draws: egui gives the window a texture that
        // is not converted to linear light.
        let channel = |value: u8| f64::from(value) / 255.0;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &colour,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // The first viewport of a frame clears the whole texture, gaps included.
                    load: if first {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: channel(self.background.r()),
                            g: channel(self.background.g()),
                            b: channel(self.background.b()),
                            a: 1.0,
                        })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(low.x, low.y, high.x - low.x, high.y - low.y, 0.0, 1.0);
        pass.set_scissor_rect(
            low.x as u32,
            low.y as u32,
            (high.x - low.x) as u32,
            (high.y - low.y) as u32,
        );
        renderer.draw(&mut pass, self.slot);
        Vec::new()
    }

    fn paint(
        &self,
        _info: epaint::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<LineRenderer>() {
            renderer.compositor.copy_to_screen(pass);
        }
    }
}

/// What a picture shows: lines, and faces to shade.
pub struct Picture {
    /// Lines, as for [`Drawing::scene`].
    pub lines: Vec<Vertex>,
    /// Faces, as for [`Drawing::faces`]; drawn shaded when there are any.
    pub faces: Vec<MeshVertex>,
    /// The camera.
    pub matrix: [[f32; 4]; 4],
    /// The direction the camera looks.
    pub light: [f32; 3],
    /// Whether faces are see-through, as in the Ghosted display mode.
    pub ghosted: bool,
}

/// Draws a picture into a PNG file without opening a window, so results can be checked by eye.
pub fn save_picture(
    picture: Picture,
    size: [u32; 2],
    path: &std::path::Path,
) -> Result<(), String> {
    let [width, height] = size;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|error| error.to_string())?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|error| error.to_string())?;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("curva picture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("curva picture depth"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = LineRenderer::new(&device, format);
    let empty = Arc::new(Vec::new());
    let shaded = !picture.faces.is_empty();
    let drawing = Drawing {
        slot: 0,
        grid: Arc::clone(&empty),
        scene: Arc::new(picture.lines),
        faces: Arc::new(picture.faces),
        overlay: Arc::clone(&empty),
        preview_faces: Arc::new(Vec::new()),
        matrix: picture.matrix,
        light: picture.light,
        shaded,
        ghosted: picture.ghosted,
        area: epaint::Rect::NOTHING,
        frame: 0,
        background: epaint::Color32::BLACK,
    };
    renderer.update(&device, &queue, &drawing);
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva picture"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva picture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(PICTURE_BACKGROUND),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.draw(&mut pass, 0);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| error.to_string())?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for line in mapped.chunks(row as usize) {
        pixels.extend_from_slice(&line[..(width * 4) as usize]);
    }
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Picture, save_picture};
    use crate::{MeshVertex, Vertex};

    const SIZE: [u32; 2] = [64, 64];

    /// The brightest red value along the middle few rows at the centre column, or `None` when
    /// this machine has no graphics adapter to draw with.
    fn centre_brightness(ghosted: bool) -> Option<u8> {
        // Drawn straight in clip space: a grey face across the middle at depth 0.5, and a
        // white line behind it at depth 0.8.
        let corner = |x: f32, y: f32| MeshVertex {
            position: [x, y, 0.5],
            normal: [0.0, 0.0, 1.0],
            colour: [0.3, 0.3, 0.3],
        };
        let faces = vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)];
        let lines = (-2..=2)
            .flat_map(|step| {
                let y = step as f32 * 0.01;
                [
                    Vertex {
                        position: [-0.5, y, 0.8],
                        colour: [1.0; 3],
                    },
                    Vertex {
                        position: [0.5, y, 0.8],
                        colour: [1.0; 3],
                    },
                ]
            })
            .collect();
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let picture = Picture {
            lines,
            faces,
            matrix: identity,
            light: [0.0, 0.0, 1.0],
            ghosted,
        };
        let path = std::env::temp_dir().join(format!(
            "curva-ghost-test-{ghosted}-{}.png",
            std::process::id()
        ));
        if let Err(error) = save_picture(picture, SIZE, &path) {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return None;
        }
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).ok()?));
        let mut reader = decoder.read_info().ok()?;
        let mut pixels = vec![0; reader.output_buffer_size()?];
        reader.next_frame(&mut pixels).ok()?;
        let _ = std::fs::remove_file(&path);
        let (width, middle) = (SIZE[0] as usize, SIZE[1] as usize / 2);
        (middle - 3..=middle + 3)
            .map(|row| pixels[(row * width + width / 2) * 4])
            .max()
    }

    #[test]
    fn a_line_behind_a_face_shows_through_only_when_ghosted() {
        let (Some(shaded), Some(ghosted)) = (centre_brightness(false), centre_brightness(true))
        else {
            return;
        };
        assert!(ghosted > shaded + 40, "shaded {shaded}, ghosted {ghosted}");
    }
}
