//! Drawing lines and shaded surfaces on the graphics card, on screen and into a picture file.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

/// The grid: true one-pixel lines, two vertices a segment.
const GRID_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32> };
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
const LINE_SHADER: &str = "
struct Uniforms { matrix: mat4x4<f32>, light: vec4<f32>, pixel: vec4<f32> };
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec3<f32> };
fn pixels(width: f32) -> f32 {
    return max(select(-width, width * uniforms.pixel.z, width > 0.0), 1.0);
}
@vertex fn vertex_main(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>, @location(1) ca: vec3<f32>, @location(2) wa: f32,
    @location(3) b: vec3<f32>, @location(4) cb: vec3<f32>, @location(5) wb: f32,
) -> Out {
    var pa = uniforms.matrix * vec4<f32>(a, 1.0);
    var pb = uniforms.matrix * vec4<f32>(b, 1.0);
    // Behind the camera: cut the segment where it crosses the near plane.
    if (pa.z < 0.0 && pb.z < 0.0) {
        return Out(vec4<f32>(2.0, 2.0, 2.0, 1.0), ca);
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
    var half = pixels(wa) * 0.5;
    if (end > 0.5) {
        at = pb;
        colour = cb;
        half = pixels(wb) * 0.5;
    }
    let offset = across * side * half + along * (end * 2.0 - 1.0) * half;
    at = vec4<f32>(at.xy + offset * px * at.w, at.zw);
    return Out(at, colour);
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

/// Draft analysis: faces coloured by their angle to the horizontal, against world up: green with
/// more than 5° of draft, red undercut by more than 5°, yellow between, lit as usual.
const DRAFT_FACE_SHADER: &str = "
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
    let angle = degrees(asin(clamp(n.z, -1.0, 1.0)));
    var colour = vec3<f32>(0.95, 0.8, 0.2);
    if (angle >= 5.0) { colour = vec3<f32>(0.3, 0.8, 0.35); }
    if (angle <= -5.0) { colour = vec3<f32>(0.9, 0.25, 0.2); }
    let facing = abs(dot(n, uniforms.light.xyz));
    return vec4<f32>(colour * (0.45 + 0.55 * facing), 1.0);
}";

/// Zebra stripes: black and white bands from the reflection of the view in each face, so a break
/// in smoothness shows as a kink in the bands.
const ZEBRA_FACE_SHADER: &str = "
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
    var n = normalize(input.normal);
    let view = uniforms.light.xyz;
    if (dot(n, view) > 0.0) { n = -n; }
    let r = reflect(view, n);
    let band = fract(r.z * 5.0);
    let shade = select(0.08, 0.95, band < 0.5);
    return vec4<f32>(vec3<f32>(shade), 1.0);
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

/// A line vertex: position, colour and width.
const LINE_VERTEX_SIZE: u64 = 28;
/// A segment, both its ends, read as one instance by [`LINE_SHADER`].
const SEGMENT_SIZE: u64 = 2 * LINE_VERTEX_SIZE;
/// The corners of the two triangles of a segment's quad.
const SEGMENT_CORNERS: u32 = 6;
const FACE_VERTEX_SIZE: u64 = 36;
const UNIFORM_SIZE: u64 = 96;
/// Pixels per millimetre on a 96 dpi screen at 100 %.
const PIXELS_PER_MM: f32 = 96.0 / 25.4;
/// Lines are pulled slightly forward, so they win ties with what lies at their depth.
const LINE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: -2,
    slope_scale: -1.0,
    clamp: 0.0,
};
/// Faces are pushed slightly back, so edges on them are drawn in front.
const FACE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 16,
    slope_scale: 6.0,
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
        for value in vertex
            .position
            .iter()
            .chain(&vertex.colour)
            .chain(std::iter::once(&vertex.width))
        {
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

/// `pixel`: a pixel's width and height in clip space, and the screen's pixels per millimetre.
fn uniform_bytes(matrix: &[[f32; 4]; 4], light: [f32; 3], pixel: [f32; 3]) -> Vec<u8> {
    matrix
        .iter()
        .flatten()
        .chain(&light)
        .chain(&[0.0])
        .chain(&pixel)
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

/// Slots after the viewports' for the detail views on a sheet (spec `layouts.md` §2). A slot
/// holds buffers only once it draws something.
pub const DETAIL_SLOTS: usize = 12;

/// What each view keeps for itself on the graphics card.
struct ViewSlot {
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    grid: Option<Uploaded<Vertex>>,
    /// Kept per view, as shaded and wireframe views draw edges in different colours.
    scene: Option<Uploaded<Vertex>>,
    shaded: bool,
    ghosted: bool,
    zebra: bool,
    draft: bool,
}

/// The graphics card's side of the viewport.
pub struct LineRenderer {
    grid: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    faces: wgpu::RenderPipeline,
    ghost_faces: wgpu::RenderPipeline,
    zebra_faces: wgpu::RenderPipeline,
    draft_faces: wgpu::RenderPipeline,
    slots: Vec<ViewSlot>,
    surfaces: Option<Uploaded<MeshVertex>>,
    overlay: Option<Uploaded<Vertex>>,
    /// The ghosted faces of a command's preview.
    preview_faces: Option<Uploaded<MeshVertex>>,
    /// Draws the viewports through a texture, which glass panels blur.
    pub(crate) compositor: crate::glass::Compositor,
    /// The shaded background behind each viewport.
    background: Background,
}

/// The background: a quad over the whole viewport, shading from one colour at the top to
/// another at the bottom, drawn behind everything.
const BACKGROUND_SHADER: &str = "
struct Shaded { @builtin(position) position: vec4<f32>, @location(0) colour: vec3<f32> };
@vertex fn vertex_main(@location(0) at: vec2<f32>, @location(1) colour: vec3<f32>) -> Shaded {
    var out: Shaded;
    out.position = vec4<f32>(at, 1.0, 1.0);
    out.colour = colour;
    return out;
}
@fragment fn fragment_main(input: Shaded) -> @location(0) vec4<f32> {
    return vec4<f32>(input.colour, 1.0);
}
";

/// Bytes per background corner: a position on screen and a colour.
const BACKGROUND_VERTEX_SIZE: u64 = 5 * 4;

/// The program and the corners that draw the background.
struct Background {
    pipeline: wgpu::RenderPipeline,
    corners: wgpu::Buffer,
}

impl Background {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("curva background"),
            source: wgpu::ShaderSource::Wgsl(BACKGROUND_SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva background"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("curva background"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: BACKGROUND_VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
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
            // Behind everything, and hiding nothing.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let corners = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curva background corners"),
            size: 6 * BACKGROUND_VERTEX_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, corners }
    }

    /// Sets the colours at the top and the bottom.
    fn set(&self, queue: &wgpu::Queue, top: epaint::Color32, bottom: epaint::Color32) {
        let colour = |c: epaint::Color32| [c.r(), c.g(), c.b()].map(|v| f32::from(v) / 255.0);
        let (high, low) = (colour(top), colour(bottom));
        let corner = |x: f32, y: f32| {
            let c = if y > 0.0 { high } else { low };
            [x, y, c[0], c[1], c[2]]
        };
        let corners: Vec<f32> = [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ]
        .concat();
        let bytes: Vec<u8> = corners.iter().flat_map(|v| v.to_le_bytes()).collect();
        queue.write_buffer(&self.corners, 0, &bytes);
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.corners.slice(..));
        pass.draw(0..6, 0..1);
    }
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
    /// Whether the buffer is read once per instance (a segment) rather than per vertex.
    per_instance: bool,
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
                step_mode: if parts.per_instance {
                    wgpu::VertexStepMode::Instance
                } else {
                    wgpu::VertexStepMode::Vertex
                },
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
        let slots = (0..VIEW_SLOTS + DETAIL_SLOTS)
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
                    zebra: false,
                    draft: false,
                }
            })
            .collect();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("curva drawing"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let grid = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva grid",
                shader: GRID_SHADER,
                stride: LINE_VERTEX_SIZE,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                topology: wgpu::PrimitiveTopology::LineList,
                bias: wgpu::DepthBiasState::default(),
                // Writes no depth, so model lines lying on it always draw over it. Blending at full
                // strength changes nothing else.
                see_through: true,
                per_instance: false,
            },
        );
        let lines = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva lines",
                shader: LINE_SHADER,
                stride: SEGMENT_SIZE,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, 1 => Float32x3, 2 => Float32,
                    3 => Float32x3, 4 => Float32x3, 5 => Float32
                ],
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Pulled a little toward the camera, so a line wins over a face or grid at its own depth.
                bias: LINE_DEPTH_BIAS,
                see_through: false,
                per_instance: true,
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
                per_instance: false,
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
                per_instance: false,
            },
        );
        let zebra_faces = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva zebra faces",
                shader: ZEBRA_FACE_SHADER,
                stride: FACE_VERTEX_SIZE,
                attributes: &face_attributes,
                topology: wgpu::PrimitiveTopology::TriangleList,
                bias: FACE_DEPTH_BIAS,
                see_through: false,
                per_instance: false,
            },
        );
        let draft_faces = pipeline(
            device,
            format,
            &pipeline_layout,
            &PipelineParts {
                label: "curva draft faces",
                shader: DRAFT_FACE_SHADER,
                stride: FACE_VERTEX_SIZE,
                attributes: &face_attributes,
                topology: wgpu::PrimitiveTopology::TriangleList,
                bias: FACE_DEPTH_BIAS,
                see_through: false,
                per_instance: false,
            },
        );
        let background = Background::new(device, format);
        Self {
            grid,
            lines,
            faces,
            ghost_faces,
            zebra_faces,
            draft_faces,
            background,
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
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawing: &Drawing,
        pixel: [f32; 3],
    ) {
        let Some(slot) = self.slots.get_mut(drawing.slot) else {
            return;
        };
        queue.write_buffer(
            &slot.uniforms,
            0,
            &uniform_bytes(&drawing.matrix, drawing.light, pixel),
        );
        slot.shaded = drawing.shaded;
        slot.ghosted = drawing.ghosted;
        slot.zebra = drawing.zebra;
        slot.draft = drawing.draft;
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
            draw_faces(
                pass,
                if slot.zebra {
                    &self.zebra_faces
                } else if slot.draft {
                    &self.draft_faces
                } else {
                    &self.faces
                },
                faces,
            );
        }
        // The grid stays a pixel wide; the model's lines have their own widths.
        if let Some(grid) = slot.grid.as_ref().filter(|lines| lines.count > 0) {
            pass.set_pipeline(&self.grid);
            pass.set_vertex_buffer(0, grid.buffer.slice(..));
            pass.draw(0..grid.count, 0..1);
        }
        if let Some(scene) = slot.scene.as_ref() {
            self.draw_segments(pass, scene);
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
        if let Some(overlay) = self.overlay.as_ref() {
            self.draw_segments(pass, overlay);
        }
    }

    /// Draws lines as quads, one instance a segment.
    fn draw_segments(&self, pass: &mut wgpu::RenderPass<'_>, lines: &Uploaded<Vertex>) {
        let segments = lines.count / 2;
        if segments > 0 {
            pass.set_pipeline(&self.lines);
            pass.set_vertex_buffer(0, lines.buffer.slice(..));
            pass.draw(0..SEGMENT_CORNERS, 0..segments);
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
    /// Whether shaded faces show zebra stripes.
    pub zebra: bool,
    /// Whether shaded faces show their draft angles.
    pub draft: bool,
    /// Where the view is in the window, in points.
    pub area: epaint::Rect,
    /// The window's frame number, so the first view drawn in a frame clears the scene texture.
    pub frame: u64,
    /// Behind the model.
    pub background: epaint::Color32,
    /// The 3D background's colour at the bottom, which it shades to from `background` at the
    /// top.
    pub background_low: epaint::Color32,
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
        // The size of a pixel in the view's clip space, and of a millimetre in pixels, for widths.
        let pixels = self.area.size() * screen.pixels_per_point;
        let pixel = [
            2.0 / pixels.x.max(1.0),
            2.0 / pixels.y.max(1.0),
            PIXELS_PER_MM * screen.pixels_per_point,
        ];
        renderer.update(device, queue, self, pixel);
        renderer
            .background
            .set(queue, self.background, self.background_low);
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
        renderer.background.draw(&mut pass);
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
    // The grid is drawn as in a window: thin and under the model's lines.
    let (grid, lines): (Vec<Vertex>, Vec<Vertex>) = picture
        .lines
        .into_iter()
        .partition(|vertex| vertex.width == crate::GRID_WIDTH);
    let drawing = Drawing {
        slot: 0,
        grid: Arc::new(grid),
        scene: Arc::new(lines),
        faces: Arc::new(picture.faces),
        overlay: Arc::clone(&empty),
        preview_faces: Arc::new(Vec::new()),
        matrix: picture.matrix,
        light: picture.light,
        shaded,
        ghosted: picture.ghosted,
        zebra: false,
        draft: false,
        area: epaint::Rect::NOTHING,
        frame: 0,
        background: epaint::Color32::BLACK,
        background_low: epaint::Color32::BLACK,
    };
    renderer.update(
        &device,
        &queue,
        &drawing,
        [2.0 / width as f32, 2.0 / height as f32, PIXELS_PER_MM],
    );
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
                        width: -1.0,
                    },
                    Vertex {
                        position: [0.5, y, 0.8],
                        colour: [1.0; 3],
                        width: -1.0,
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

    /// How many rows of the centre column a horizontal line of `width` lights up, or `None`
    /// without a graphics adapter.
    fn rows_lit(width: f32) -> Option<usize> {
        let end = |x: f32| Vertex {
            position: [x, 0.0, 0.5],
            colour: [1.0; 3],
            width,
        };
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let picture = Picture {
            lines: vec![end(-0.8), end(0.8)],
            faces: Vec::new(),
            matrix: identity,
            light: [0.0, 0.0, 1.0],
            ghosted: false,
        };
        let path = std::env::temp_dir().join(format!(
            "curva-width-test-{}-{}.png",
            width.to_bits(),
            std::process::id()
        ));
        if save_picture(picture, SIZE, &path).is_err() {
            return None;
        }
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).ok()?));
        let mut reader = decoder.read_info().ok()?;
        let mut pixels = vec![0; reader.output_buffer_size()?];
        reader.next_frame(&mut pixels).ok()?;
        let _ = std::fs::remove_file(&path);
        let w = SIZE[0] as usize;
        Some(
            (0..SIZE[1] as usize)
                .filter(|row| pixels[(row * w + w / 2) * 4] > 200)
                .count(),
        )
    }

    #[test]
    fn lines_draw_at_their_own_width() {
        // The default line is 2 pixels; 1 mm is about 3.8 pixels at 96 dpi; 2 mm about 7.6.
        let (Some(default), Some(one), Some(two)) = (rows_lit(-2.0), rows_lit(1.0), rows_lit(2.0))
        else {
            return;
        };
        assert_eq!(default, 2);
        assert!((3..=5).contains(&one), "1 mm lit {one} rows");
        assert!((7..=9).contains(&two), "2 mm lit {two} rows");
    }
}
