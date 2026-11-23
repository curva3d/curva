//! Visualisation styles (spec `views-and-styles.md`). First prototype (§5, step 1): the Diagram
//! look in pictures made without a window, so the owner can judge its character before the live
//! viewport and the style editor are built.
//!
//! Diagram: light surfaces, thin black edges, and thick black outlines wherever one object ends
//! against another or against the background. Outlines are found from a hidden picture of object
//! numbers: a pixel is on an outline when an object number within the outline's reach differs
//! from its own. The picture is drawn at twice the size and shrunk, so the outlines come out
//! smooth. In this prototype the outline pass runs on the main processor; the live viewport will
//! do the same on the graphics card.

use crate::gpu::{DEPTH_FORMAT, Picture, draw_picture, draw_picture_cut};
use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;

/// A look a picture can be drawn in. Only the prototype's looks so far; style files come later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureStyle {
    /// White surfaces with soft shading and occlusion, thin grey edges, bold black outlines.
    Diagram,
    /// A drawing cut open: solid black where solids are cut, lines thinning and lightening the
    /// further they are from the viewer, white paper.
    Architectural,
}

impl PictureStyle {
    /// The style a name stands for (`--style diagram`), any case.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "diagram" => Some(Self::Diagram),
            "architectural" => Some(Self::Architectural),
            _ => None,
        }
    }
}

/// The Architectural look (owner, 15 November 2026: thick black cut lines; lines lighter grey
/// and thinner the further from the camera). Widths at the final size, in pixels, and greys from
/// 0 (black) to 1 (white), for the nearest and the furthest lines.
const NEAR_LINE: (f32, f32) = (2.2, 0.05);
const FAR_LINE: (f32, f32) = (0.6, 0.75);
/// The fill where a solid is cut.
const POCHE: [u8; 3] = [22, 22, 22];

/// How many times larger the picture is drawn before being shrunk, for smooth outlines.
const SUPERSAMPLE: u32 = 2;
/// The Diagram look, at the final size, in pixels (owner, 15 November 2026: bold outer lines,
/// thin grey inner lines, white surfaces).
const OUTLINE_PIXELS: f32 = 4.0;
const EDGE_PIXELS: f32 = 1.0;
/// Bold outlines in near-black; inner edges in mid grey; surfaces and paper pure white.
const INK: [f32; 3] = [0.08, 0.08, 0.08];
const INNER: [u8; 3] = [140, 140, 140];
const PAPER: [u8; 3] = [255, 255, 255];
/// The colour inner edges are first drawn in, which no lit surface can take (surfaces are grey),
/// so they can be told apart and recoloured once drawing has hidden the ones behind surfaces.
const MARKER: [f32; 3] = [1.0, 0.0, 1.0];
/// How much of a surface's shading is kept: 1 is as lit, 0 is flat white.
const SHADING: f32 = 0.55;
/// How dark full occlusion makes a surface, and how far occlusion reaches, in millimetres.
const OCCLUSION_STRENGTH: f32 = 0.7;
const OCCLUSION_REACH: f32 = 1200.0;

/// Draws `picture` in `style` and saves it as a PNG file. The picture's faces must carry object
/// numbers ([`crate::SceneParts`]), which is how outlines find where one object ends. `cut`, if
/// given, is a cutting plane `[nx, ny, nz, d]` cutting away where `n·p + d > 0`, as a plan
/// cuts at a height.
pub fn save_styled_picture(
    picture: Picture,
    style: PictureStyle,
    cut: Option<[f32; 4]>,
    size: [u32; 2],
    path: &std::path::Path,
) -> Result<(), String> {
    let pixels = match style {
        PictureStyle::Diagram => draw_diagram(picture, size)?,
        PictureStyle::Architectural => draw_architectural(picture, cut.unwrap_or(NO_CUT), size)?,
    };
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

/// The Diagram look's pixels, four bytes each, row by row.
pub(crate) fn draw_diagram(picture: Picture, size: [u32; 2]) -> Result<Vec<u8>, String> {
    let big = [size[0] * SUPERSAMPLE, size[1] * SUPERSAMPLE];
    let scale = SUPERSAMPLE as f32;
    let ids = draw_object_numbers(&picture.faces, picture.matrix, big, NUMBERS_SHADER, NO_CUT)?;
    let depths: Vec<f32> =
        draw_object_numbers(&picture.faces, picture.matrix, big, DEPTH_SHADER, NO_CUT)?
            .into_iter()
            .map(f32::from_bits)
            .collect();
    let recoloured = Picture {
        lines: picture
            .lines
            .iter()
            .map(|vertex| Vertex {
                colour: MARKER,
                width: -EDGE_PIXELS * scale,
                kind: crate::LineKind::Given,
                ..*vertex
            })
            .collect(),
        faces: picture
            .faces
            .iter()
            .map(|vertex| MeshVertex {
                colour: [1.0; 3],
                ..*vertex
            })
            .collect(),
        ..picture
    };
    let mut base = draw_picture(recoloured, big, true)?;
    let occlusion = ambient_occlusion(&ids, &depths, big, picture.matrix[1][1]);
    // Edges that survived being hidden behind surfaces become grey; surfaces keep a soft
    // version of their shading, darkened where occluded (owner: shading and occlusion); the
    // background is paper.
    let marker = MARKER.map(|c| (c * 255.0) as u8);
    for ((pixel, id), occluded) in base.chunks_mut(4).zip(&ids).zip(&occlusion) {
        let colour = if pixel[..3] == marker {
            INNER
        } else if *id == 0 {
            PAPER
        } else {
            let darken = 1.0 - OCCLUSION_STRENGTH * occluded;
            [0, 1, 2].map(|channel| {
                let lit = f32::from(pixel[channel]);
                let soft = 255.0 - (255.0 - lit) * SHADING;
                (soft * darken).round().clamp(0.0, 255.0) as u8
            })
        };
        pixel[..3].copy_from_slice(&colour);
    }
    outline(&mut base, &ids, big, OUTLINE_PIXELS * scale);
    Ok(shrink(&base, big, SUPERSAMPLE))
}

/// The Architectural look's pixels, four bytes each, row by row. Each line's width and grey are
/// set from how far it lies from the viewer along the view, between the nearest and furthest
/// lines left after the cut: the edges at the cut thick and black, those far beyond thin and
/// light. Where the cut has opened a solid, the inside of its faces shows; those pixels are
/// filled solid. Surfaces are white.
pub(crate) fn draw_architectural(
    picture: Picture,
    cut: [f32; 4],
    size: [u32; 2],
) -> Result<Vec<u8>, String> {
    let big = [size[0] * SUPERSAMPLE, size[1] * SUPERSAMPLE];
    let scale = SUPERSAMPLE as f32;
    let ids = draw_object_numbers(&picture.faces, picture.matrix, big, CUT_NUMBERS_SHADER, cut)?;
    // How far along the view each line end lies; `light` is the direction the camera looks.
    let forward = picture.light;
    let along = |p: [f32; 3]| p[0] * forward[0] + p[1] * forward[1] + p[2] * forward[2];
    let kept = |p: [f32; 3]| cut[0] * p[0] + cut[1] * p[1] + cut[2] * p[2] + cut[3] <= 0.0;
    let (near, far) = picture
        .lines
        .iter()
        .filter(|vertex| kept(vertex.position))
        .map(|vertex| along(vertex.position))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| {
            (lo.min(d), hi.max(d))
        });
    let span = (far - near).max(1e-3);
    let lines = picture
        .lines
        .iter()
        .map(|vertex| {
            let t = ((along(vertex.position) - near) / span).clamp(0.0, 1.0);
            let width = NEAR_LINE.0 + (FAR_LINE.0 - NEAR_LINE.0) * t;
            let grey = NEAR_LINE.1 + (FAR_LINE.1 - NEAR_LINE.1) * t;
            Vertex {
                colour: [grey; 3],
                width: -width * scale,
                kind: crate::LineKind::Given,
                ..*vertex
            }
        })
        .collect();
    let faces: Vec<MeshVertex> = picture
        .faces
        .iter()
        .map(|vertex| MeshVertex {
            colour: [1.0; 3],
            ..*vertex
        })
        .collect();
    // Drawn twice, with and without the lines: where the two differ is a line that surfaces did
    // not hide, whatever its grey; everywhere else is a surface or the paper.
    let surfaces_only = draw_picture_cut(
        Picture {
            lines: Vec::new(),
            faces: faces.clone(),
            ..picture
        },
        big,
        vec![cut],
    )?;
    let mut base = draw_picture_cut(
        Picture {
            lines,
            faces,
            ..picture
        },
        big,
        vec![cut],
    )?;
    // Cut solids are filled; lines keep their greys; surfaces and paper are white.
    for ((pixel, plain), id) in base.chunks_mut(4).zip(surfaces_only.chunks(4)).zip(&ids) {
        let colour = if id & CUT_FLAG != 0 {
            POCHE
        } else if pixel[..3] != plain[..3] {
            [pixel[0], pixel[1], pixel[2]]
        } else {
            PAPER
        };
        pixel[..3].copy_from_slice(&colour);
    }
    Ok(shrink(&base, big, SUPERSAMPLE))
}

/// Darkens `pixels` to the ink colour within `width` pixels of every place the object number
/// changes, the stroke straddling the change, with a soft edge one pixel wide.
fn outline(pixels: &mut [u8], ids: &[u32], size: [u32; 2], width: f32) {
    let [w, h] = size.map(|v| v as usize);
    // Where the number changes from a pixel to its right neighbour, and to its lower one.
    let rightward: Vec<bool> = (0..w * h)
        .map(|at| at % w + 1 < w && ids[at] != ids[at + 1])
        .collect();
    let downward: Vec<bool> = (0..w * h)
        .map(|at| at / w + 1 < h && ids[at] != ids[at + w])
        .collect();
    let half = width / 2.0;
    let reach = half.ceil() as isize + 1;
    let ink = INK.map(|c| c * 255.0);
    for y in 0..h {
        for x in 0..w {
            let mut nearest = f32::INFINITY;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    // A change lies between a pixel and its neighbour: half a pixel on, along
                    // the direction of that neighbour only.
                    let at = ny as usize * w + nx as usize;
                    let (dx, dy) = (dx as f32, dy as f32);
                    if rightward[at] {
                        nearest = nearest.min((dx + 0.5).hypot(dy));
                    }
                    if downward[at] {
                        nearest = nearest.min(dx.hypot(dy + 0.5));
                    }
                }
            }
            let cover = (half + 0.5 - nearest).clamp(0.0, 1.0);
            if cover > 0.0 {
                let at = (y * w + x) * 4;
                for channel in 0..3 {
                    let value = f32::from(pixels[at + channel]);
                    pixels[at + channel] = (value + (ink[channel] - value) * cover).round() as u8;
                }
            }
        }
    }
}

/// How occluded each pixel is, 0 to 1, from the distances alone (screen-space ambient occlusion,
/// after Mittring, "Finding next gen: CryEngine 2", SIGGRAPH 2007 course notes): pairs
/// of points opposite each other around a pixel are compared with it, and where the surface there
/// lies deeper than the average of the pair, as in an inside corner or where one object stands on
/// another, it is occluded. Comparing pairs keeps a flat surface seen at an angle from darkening
/// itself. Points much nearer the camera than the reach (an object far in front) do not count.
/// `focal` is the camera matrix's vertical scale, which sets how many pixels the reach covers.
fn ambient_occlusion(ids: &[u32], depths: &[f32], size: [u32; 2], focal: f32) -> Vec<f32> {
    let [w, h] = size.map(|v| v as usize);
    let pixels_per_mm_at_unit = focal * h as f32 / 2.0;
    // Eight directions over half a turn, each at two distances: 16 pairs.
    let steps: Vec<(f32, f32)> = (0..8)
        .flat_map(|k| {
            let angle = std::f32::consts::PI * (k as f32 + 0.5) / 8.0;
            [0.45, 1.0].map(|part| (angle.cos() * part, angle.sin() * part))
        })
        .collect();
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let at = y * w + x;
            let depth = depths[at];
            if ids[at] == 0 || depth <= 0.0 {
                continue;
            }
            let reach_pixels = (OCCLUSION_REACH * pixels_per_mm_at_unit / depth).clamp(2.0, 80.0);
            let sample = |dx: f32, dy: f32| {
                let (sx, sy) = (x as f32 + dx, y as f32 + dy);
                if sx < 0.0 || sy < 0.0 || sx >= w as f32 || sy >= h as f32 {
                    return None;
                }
                let s = sy as usize * w + sx as usize;
                (ids[s] != 0).then_some(depths[s])
            };
            let mut total = 0.0;
            for &(cx, cy) in &steps {
                let (dx, dy) = (cx * reach_pixels, cy * reach_pixels);
                let (Some(a), Some(b)) = (sample(dx, dy), sample(-dx, -dy)) else {
                    continue;
                };
                let deeper = depth - (a + b) / 2.0;
                let nearest = depth - a.min(b);
                if nearest > 2.0 * OCCLUSION_REACH {
                    continue;
                }
                total += (deeper / OCCLUSION_REACH * 6.0).clamp(0.0, 1.0);
            }
            out[at] = total / steps.len() as f32;
        }
    }
    out
}
/// Shrinks a picture by `factor`, averaging each square of pixels.
fn shrink(pixels: &[u8], size: [u32; 2], factor: u32) -> Vec<u8> {
    let [w, h] = size.map(|v| v as usize);
    let f = factor as usize;
    let (sw, sh) = (w / f, h / f);
    let mut out = vec![0; sw * sh * 4];
    for y in 0..sh {
        for x in 0..sw {
            for channel in 0..4 {
                let mut sum = 0u32;
                for dy in 0..f {
                    for dx in 0..f {
                        sum += u32::from(pixels[((y * f + dy) * w + x * f + dx) * 4 + channel]);
                    }
                }
                out[(y * sw + x) * 4 + channel] = (sum / (f * f) as u32) as u8;
            }
        }
    }
    out
}

/// Draws each face's object number plus one (0 is the background) into a hidden picture, nearest
/// face winning, and reads it back row by row.
fn draw_object_numbers(
    faces: &[MeshVertex],
    matrix: [[f32; 4]; 4],
    size: [u32; 2],
    shader: &str,
    cut: [f32; 4],
) -> Result<Vec<u32>, String> {
    let [width, height] = size;
    let (device, queue) = numbers_device()?;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let (numbers, depth) = numbers_targets(&device, extent);
    let (uniform_buffer, vertex_buffer) = numbers_buffers(&device, &queue, faces, matrix, cut);
    let (layout, bind_group) = numbers_bind_group(&device, &uniform_buffer);
    let pipeline = numbers_pipeline(&device, &layout, shader);
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva object numbers readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let numbers_view = numbers.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva object numbers"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &numbers_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        if !faces.is_empty() {
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..faces.len() as u32, 0..1);
        }
    }
    encoder.copy_texture_to_buffer(
        numbers.as_image_copy(),
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
    read_numbers(&device, &readback, row, size)
}

/// A device of its own for the hidden picture, asked for as the window asks for its device.
fn numbers_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|error| error.to_string())?;
    pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, false)))
        .map_err(|error| error.to_string())
}

/// The hidden picture's object numbers and its depth, both `extent` in size.
fn numbers_targets(
    device: &wgpu::Device,
    extent: wgpu::Extent3d,
) -> (wgpu::Texture, wgpu::Texture) {
    let target = |format, usage, label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let numbers = target(
        wgpu::TextureFormat::R32Uint,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        "curva object numbers",
    );
    let depth = target(
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        "curva object numbers depth",
    );
    (numbers, depth)
}

/// The camera and cutting plane, and each face corner's position with its object number plus
/// one, sent to the graphics card.
fn numbers_buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    faces: &[MeshVertex],
    matrix: [[f32; 4]; 4],
    cut: [f32; 4],
) -> (wgpu::Buffer, wgpu::Buffer) {
    let uniforms: Vec<u8> = matrix
        .iter()
        .flatten()
        .chain(&cut)
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva object numbers camera"),
        size: uniforms.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &uniforms);
    let mut vertices = Vec::with_capacity(faces.len() * 16);
    for vertex in faces {
        for value in vertex.position {
            vertices.extend(value.to_le_bytes());
        }
        vertices.extend(vertex.object.wrapping_add(1).to_le_bytes());
    }
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva object numbers faces"),
        size: (vertices.len() as u64).max(16),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, &vertices);
    (uniform_buffer, vertex_buffer)
}

/// The shaders' view of the camera and cutting plane.
fn numbers_bind_group(
    device: &wgpu::Device,
    uniform_buffer: &wgpu::Buffer,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("curva object numbers"),
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("curva object numbers"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    (layout, bind_group)
}

/// The pipeline that writes object numbers, nearest face winning.
fn numbers_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    shader: &str,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("curva object numbers"),
        source: wgpu::ShaderSource::Wgsl(format!("{HIDDEN_COMMON}{shader}").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("curva object numbers"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("curva object numbers"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 16,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R32Uint,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Waits for the hidden picture and reads its numbers back, row by row, leaving out each row's
/// padding.
fn read_numbers(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    row: u32,
    [width, height]: [u32; 2],
) -> Result<Vec<u32>, String> {
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| error.to_string())?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    let mut ids = Vec::with_capacity((width * height) as usize);
    for line in mapped.chunks(row as usize) {
        ids.extend(
            line[..(width * 4) as usize]
                .chunks(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        );
    }
    Ok(ids)
}

/// The camera and one cutting plane, shared by the hidden-picture shaders: a point where
/// `dot(cut.xyz, p) + cut.w > 0` is cut away (a plane that never cuts: `(0, 0, 0, -1)`).
const HIDDEN_COMMON: &str = "
struct Camera { matrix: mat4x4<f32>, cut: vec4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) number: u32,
    @location(1) distance: f32,
    @location(2) world: vec3<f32>,
};
@vertex fn vertex_main(@location(0) p: vec3<f32>, @location(1) number: u32) -> Out {
    let at = camera.matrix * vec4<f32>(p, 1.0);
    return Out(at, number, at.w, p);
}
fn cut_away(p: vec3<f32>) -> bool {
    return dot(camera.cut.xyz, p) + camera.cut.w > 0.0;
}
";

/// Each face's number, flat across the triangle, nearest face winning by depth.
const NUMBERS_SHADER: &str = "
@fragment fn fragment_main(input: Out) -> @location(0) u32 {
    if (cut_away(input.world)) { discard; }
    return input.number;
}";

/// Each pixel's distance from the camera along its view (the clip-space w of a perspective
/// view), as the bits of a float, nearest face winning; 0 where nothing is drawn.
const DEPTH_SHADER: &str = "
@fragment fn fragment_main(input: Out) -> @location(0) u32 {
    if (cut_away(input.world)) { discard; }
    return bitcast<u32>(input.distance);
}";

/// As [`NUMBERS_SHADER`], with [`CUT_FLAG`] added where the face is seen from behind: looking
/// into a solid the cut has opened, which is where its cut fill goes.
const CUT_NUMBERS_SHADER: &str = "
@fragment fn fragment_main(input: Out, @builtin(front_facing) front: bool) -> @location(0) u32 {
    if (cut_away(input.world)) { discard; }
    return select(input.number | 0x80000000u, input.number, front);
}";

/// Added to an object number where a solid is seen cut open.
const CUT_FLAG: u32 = 0x8000_0000;

/// A cutting plane that never cuts.
const NO_CUT: [f32; 4] = [0.0, 0.0, 0.0, -1.0];
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outline_follows_the_border_between_two_numbers() {
        // Left half number 1, right half number 2, on white.
        let size = [20u32, 4];
        let ids: Vec<u32> = (0..80).map(|at| if at % 20 < 10 { 1 } else { 2 }).collect();
        let mut pixels = vec![255u8; 80 * 4];
        outline(&mut pixels, &ids, size, 2.0);
        let red = |x: usize| pixels[x * 4];
        // On the border it is ink; far from it, untouched.
        assert!(red(9) < 60 && red(10) < 60, "{} {}", red(9), red(10));
        assert_eq!(red(0), 255);
        assert_eq!(red(19), 255);
    }

    #[test]
    fn shrinking_averages_each_square() {
        let pixels: Vec<u8> = [0u8, 0, 0, 255, 100, 100, 100, 255]
            .repeat(2)
            .into_iter()
            .collect();
        // 2 by 2 pixels: black and grey on both rows.
        assert_eq!(shrink(&pixels, [2, 2], 2), vec![50, 50, 50, 255]);
    }

    #[test]
    fn styles_are_found_by_name() {
        assert_eq!(PictureStyle::named("Diagram"), Some(PictureStyle::Diagram));
        assert_eq!(
            PictureStyle::named("ARCHITECTURAL"),
            Some(PictureStyle::Architectural)
        );
        assert_eq!(PictureStyle::named("watercolour"), None);
    }

    /// A box seen from above and cut through its middle: the cut fills it solid, and around it
    /// is paper.
    #[test]
    fn a_cut_solid_is_filled_and_the_rest_is_paper() {
        let body = curva_kernel::Body::cuboid([-50.0, -50.0, 0.0], [100.0, 100.0, 100.0]).unwrap();
        let mesh = body.mesh().unwrap();
        let narrow = |p: [f64; 3]| p.map(|v| v as f32);
        let faces = mesh
            .triangles
            .iter()
            .flatten()
            .map(|&corner| MeshVertex {
                position: narrow(mesh.positions[corner as usize]),
                normal: narrow(mesh.normals[corner as usize]),
                colour: [1.0; 3],
                object: 0,
                kind: crate::FaceKind::Given,
            })
            .collect();
        let mut camera = crate::StandardView::Top.camera();
        camera.frame([-200.0, -200.0, 0.0], [200.0, 200.0, 100.0]);
        let picture = Picture {
            lines: Vec::new(),
            faces,
            matrix: camera.matrix(1.0),
            light: camera.forward().map(|v| v as f32),
            style: crate::Style::shaded(),
        };
        let size = [64, 64];
        let Ok(pixels) = draw_architectural(picture, [0.0, 0.0, 1.0, -50.0], size) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        let at = |x: usize, y: usize| pixels[(y * 64 + x) * 4];
        assert!(at(32, 32) < 60, "centre {}", at(32, 32));
        assert_eq!(at(2, 2), 255);
    }
}
