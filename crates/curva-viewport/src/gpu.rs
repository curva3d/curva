//! Drawing lines and shaded surfaces on the graphics card, on screen and into a picture file.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

mod drawing;
mod picture;
mod shaders;
mod upload;

pub use drawing::*;
pub use picture::*;
use shaders::{
    DRAFT_FACE_SHADER, FACE_SHADER, GHOST_FACE_SHADER, GRID_SHADER, LINE_SHADER, PICK_ROW,
    ZEBRA_FACE_SHADER, table_wgsl,
};
pub use upload::*;

/// What each view keeps for itself on the graphics card.
struct ViewSlot {
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    grid: Option<Uploaded<Vertex>>,
    /// Shared between views drawing the same lines: shaded and wireframe views draw edges in
    /// different colours, so there are at most two different ones.
    scene: Option<Arc<Uploaded<Vertex>>>,
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
    /// The graphics card's timer for the benchmark, once asked for and if the card has one.
    timer: Option<crate::timer::GpuTimer>,
    /// Draws outlines of curved shapes from the depth picture (#182).
    outliner: crate::outline::Outliner,
    /// The bindings every view uses, kept to bind again when the selection table grows.
    layout: wgpu::BindGroupLayout,
    /// The selection table.
    picks: Picks,
    /// The largest block one upload is split into ([`CHUNK_BYTES`]; smaller in tests).
    chunk_bytes: u64,
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
    shader: &'a str,
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
        let layout = Self::camera_layout(device);
        let picks = Picks::new(device, 1);
        let slots = (0..VIEW_SLOTS + DETAIL_SLOTS)
            .map(|_| {
                let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("curva camera"),
                    size: UNIFORM_SIZE,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bind_group = bind_view(device, &layout, &uniforms, &picks);
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
        let (grid, lines) = Self::line_pipelines(device, format, &pipeline_layout);
        let face = |label, shader, see_through| {
            Self::face_pipeline(device, format, &pipeline_layout, label, shader, see_through)
        };
        let faces = face("curva faces", FACE_SHADER, false);
        let ghost_faces = face("curva ghosted faces", GHOST_FACE_SHADER, true);
        let zebra_faces = face("curva zebra faces", ZEBRA_FACE_SHADER, false);
        let draft_faces = face("curva draft faces", DRAFT_FACE_SHADER, false);
        let background = Background::new(device, format);
        Self {
            grid,
            lines,
            faces,
            ghost_faces,
            zebra_faces,
            draft_faces,
            background,
            timer: None,
            chunk_bytes: CHUNK_BYTES,
            outliner: crate::outline::Outliner::new(device, format),
            layout,
            picks,
            slots,
            surfaces: None,
            overlay: None,
            preview_faces: None,
            compositor: crate::glass::Compositor::new(device, format),
        }
    }

    /// What every drawing reads: the camera and the selection table.
    fn camera_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curva camera"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // The selection table (`table_wgsl`).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        })
    }

    /// The grid's pipeline and the model lines' pipeline.
    fn line_pipelines(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        pipeline_layout: &wgpu::PipelineLayout,
    ) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
        let grid = pipeline(
            device,
            format,
            pipeline_layout,
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
            pipeline_layout,
            &PipelineParts {
                label: "curva lines",
                shader: &format!("{}{LINE_SHADER}", table_wgsl()),
                stride: SEGMENT_SIZE,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, 1 => Float32x3, 2 => Float32, 6 => Uint32,
                    3 => Float32x3, 4 => Float32x3, 5 => Float32, 7 => Uint32
                ],
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Pulled a little toward the camera, so a line wins over a face or grid at its own depth.
                bias: LINE_DEPTH_BIAS,
                see_through: false,
                per_instance: true,
            },
        );
        (grid, lines)
    }

    /// A pipeline shading faces with `shader`, which reads the selection table.
    fn face_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        pipeline_layout: &wgpu::PipelineLayout,
        label: &'static str,
        shader: &str,
        see_through: bool,
    ) -> wgpu::RenderPipeline {
        let face_attributes = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Uint32
        ];
        pipeline(
            device,
            format,
            pipeline_layout,
            &PipelineParts {
                label,
                shader: &format!("{}{shader}", table_wgsl()),
                stride: FACE_VERTEX_SIZE,
                attributes: &face_attributes,
                topology: wgpu::PrimitiveTopology::TriangleList,
                bias: FACE_DEPTH_BIAS,
                see_through,
                per_instance: false,
            },
        )
    }

    /// Turns the glass blur on or off; off where the graphics card is a software one, so glass
    /// panels show their tint alone.
    pub fn set_blur(&mut self, on: bool) {
        self.compositor.blur = on;
    }

    /// Starts or stops timing the viewports on the graphics card, for the benchmark. Returns
    /// whether the card has a timer; without one nothing is measured.
    pub fn time_gpu(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, on: bool) -> bool {
        if self.timer.is_none() {
            self.timer = crate::timer::GpuTimer::new(device, queue);
        }
        match &mut self.timer {
            Some(timer) => {
                timer.switch(on);
                true
            }
            None => false,
        }
    }

    /// The graphics card's average time per frame for drawing the viewports, in milliseconds,
    /// since [`Self::time_gpu`] switched timing on; `None` before any reading has come back.
    #[must_use]
    pub fn gpu_frame_ms(&self) -> Option<f64> {
        self.timer.as_ref()?.per_frame_ms()
    }

    /// Writes the selection table again when the numbered objects or the selection changed: one
    /// byte per object, so even a large scene's table is a small, quick copy. The views of a
    /// frame share the lists, so only the first of them writes.
    fn update_picks(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        objects: &Arc<Vec<u64>>,
        selection: &Arc<Vec<u64>>,
    ) {
        if Arc::ptr_eq(&self.picks.objects, objects)
            && Arc::ptr_eq(&self.picks.selection, selection)
        {
            return;
        }
        let rows = (objects.len() as u32).div_ceil(PICK_ROW).max(1);
        if rows > self.picks.rows {
            // Grown with room to spare, so a model growing object by object rarely binds again.
            self.picks = Picks::new(device, rows.next_power_of_two());
            for slot in &mut self.slots {
                slot.bind_group = bind_view(device, &self.layout, &slot.uniforms, &self.picks);
            }
        }
        let bytes = table_bytes(self.picks.rows, objects, selection);
        self.picks.write(queue, &bytes);
        self.picks.objects = Arc::clone(objects);
        self.picks.selection = Arc::clone(selection);
    }

    /// Sends one view's camera and grid, and anything that changed, to the graphics card.
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawing: &Drawing,
        pixel: [f32; 3],
    ) {
        // Before the view's own bindings are used: growing the table binds every view again.
        self.update_picks(device, queue, &drawing.objects, &drawing.selection);
        let Some(slot) = self.slots.get_mut(drawing.slot) else {
            return;
        };
        queue.write_buffer(
            &slot.uniforms,
            0,
            &uniform_bytes(&drawing.matrix, drawing.light, pixel, &drawing.clips),
        );
        slot.shaded = drawing.shaded;
        slot.ghosted = drawing.ghosted;
        slot.zebra = drawing.zebra;
        slot.draft = drawing.draft;
        let chunk = self.chunk_bytes;
        if stale(slot.grid.as_ref(), &drawing.grid) {
            slot.grid = Some(upload(
                device,
                queue,
                &drawing.grid,
                line_bytes,
                LINE_VERTEX_SIZE,
                2,
                chunk,
            ));
        }
        if stale(slot.scene.as_deref(), &drawing.scene) {
            self.update_scene(device, queue, drawing);
        }
        self.update_shared(device, queue, drawing);
    }

    /// Sends the view's model lines, or shares the copy another view showing the same lines has
    /// already sent.
    fn update_scene(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, drawing: &Drawing) {
        let chunk = self.chunk_bytes;
        let shared = self
            .slots
            .iter()
            .filter_map(|other| other.scene.as_ref())
            .find(|held| Arc::ptr_eq(&held.source, &drawing.scene))
            .map(Arc::clone);
        let scene = shared.unwrap_or_else(|| {
            Arc::new(upload(
                device,
                queue,
                &drawing.scene,
                line_bytes,
                LINE_VERTEX_SIZE,
                2,
                chunk,
            ))
        });
        if let Some(slot) = self.slots.get_mut(drawing.slot) {
            slot.scene = Some(scene);
        }
    }

    /// Sends the faces, overlay and preview faces, if they changed. The views of one frame share
    /// these, so only the first of them uploads.
    fn update_shared(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, drawing: &Drawing) {
        let chunk = self.chunk_bytes;
        if stale(self.surfaces.as_ref(), &drawing.faces) {
            self.surfaces = Some(upload(
                device,
                queue,
                &drawing.faces,
                face_bytes,
                FACE_VERTEX_SIZE,
                3,
                chunk,
            ));
        }
        if stale(self.overlay.as_ref(), &drawing.overlay) {
            self.overlay = Some(upload(
                device,
                queue,
                &drawing.overlay,
                line_bytes,
                LINE_VERTEX_SIZE,
                2,
                chunk,
            ));
        }
        if stale(self.preview_faces.as_ref(), &drawing.preview_faces) {
            self.preview_faces = Some(upload(
                device,
                queue,
                &drawing.preview_faces,
                face_bytes,
                FACE_VERTEX_SIZE,
                3,
                chunk,
            ));
        }
    }

    /// Whether this view's depth picture holds surfaces to find outlines in (#182): shaded, not
    /// ghosted (ghosted faces write no depth), with faces to show.
    fn outlines_wanted(&self, slot: usize) -> bool {
        self.slots.get(slot).is_some_and(|slot| {
            slot.shaded
                && !slot.ghosted
                && self.surfaces.as_ref().is_some_and(|faces| faces.count > 0)
        })
    }

    /// Draws faces with `pipeline`, block by block.
    fn draw_faces(
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        faces: &Uploaded<MeshVertex>,
    ) {
        pass.set_pipeline(pipeline);
        for (buffer, count) in &faces.chunks {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }

    /// The first part of a view: the grid and solid surfaces. The outline pass reads the depth
    /// they leave before lines are drawn over them, so lines never count as outlines.
    fn draw_under(&self, pass: &mut wgpu::RenderPass<'_>, slot: usize) {
        let Some(slot) = self.slots.get(slot) else {
            return;
        };
        pass.set_bind_group(0, &slot.bind_group, &[]);
        let faces = self.surfaces.as_ref().filter(|faces| faces.count > 0);
        // The grid goes first, writing no depth, so whatever lies on it or near it draws over
        // it, as in Rhino; drawn after faces, it showed through faces lying exactly on it. It
        // stays a pixel wide; the model's lines have their own widths.
        if let Some(grid) = slot.grid.as_ref().filter(|lines| lines.count > 0) {
            pass.set_pipeline(&self.grid);
            for (buffer, count) in &grid.chunks {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        // Solid faces go next and hide the lines behind them.
        if slot.shaded
            && !slot.ghosted
            && let Some(faces) = faces
        {
            Self::draw_faces(
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
    }

    /// The second part of a view, after the outline pass: lines, ghosted surfaces, previews.
    fn draw_over(&self, pass: &mut wgpu::RenderPass<'_>, slot: usize) {
        let Some(slot) = self.slots.get(slot) else {
            return;
        };
        pass.set_bind_group(0, &slot.bind_group, &[]);
        let faces = self.surfaces.as_ref().filter(|faces| faces.count > 0);
        if let Some(scene) = slot.scene.as_deref() {
            self.draw_segments(pass, scene);
        }
        // Ghosted faces go over the lines, so lines behind a face show through it, dimmed,
        // while lines on its near side stay in front.
        if slot.shaded
            && slot.ghosted
            && let Some(faces) = faces
        {
            Self::draw_faces(pass, &self.ghost_faces, faces);
        }
        // A command's solid preview is ghosted in every display mode, over the model.
        if let Some(preview) = self.preview_faces.as_ref().filter(|faces| faces.count > 0) {
            Self::draw_faces(pass, &self.ghost_faces, preview);
        }
        if let Some(overlay) = self.overlay.as_ref() {
            self.draw_segments(pass, overlay);
        }
    }

    /// Draws lines as quads, one instance a segment, block by block.
    fn draw_segments(&self, pass: &mut wgpu::RenderPass<'_>, lines: &Uploaded<Vertex>) {
        if lines.count < 2 {
            return;
        }
        pass.set_pipeline(&self.lines);
        for (buffer, count) in &lines.chunks {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..SEGMENT_CORNERS, 0..count / 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PICK_ROW, Picture, Scene, chunk_ranges, draw_picture, draw_picture_in_chunks, save_picture,
        table_bytes,
    };
    use crate::{MeshVertex, NO_OBJECT, Vertex};
    use std::sync::Arc;

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
            object: NO_OBJECT,
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
                        object: NO_OBJECT,
                    },
                    Vertex {
                        position: [0.5, y, 0.8],
                        colour: [1.0; 3],
                        width: -1.0,
                        object: NO_OBJECT,
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
            object: NO_OBJECT,
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

    /// A grey face crossed by lines of three widths, drawn straight in clip space.
    fn mixed_picture() -> Picture {
        let corner = |x: f32, y: f32| MeshVertex {
            position: [x, y, 0.5],
            normal: [0.0, 0.0, 1.0],
            colour: [0.4, 0.4, 0.4],
            object: NO_OBJECT,
        };
        let line = |y: f32, width: f32| {
            [-0.8, 0.8].map(|x| Vertex {
                position: [x, y, 0.3],
                colour: [0.9, 0.5, 0.2],
                width,
                object: NO_OBJECT,
            })
        };
        Picture {
            lines: [line(-0.4, -1.0), line(0.0, -3.0), line(0.4, 0.5)]
                .into_iter()
                .flatten()
                .collect(),
            // Two triangles, so faces too cross a block boundary when blocks are small.
            faces: vec![
                corner(-0.9, -0.9),
                corner(0.9, -0.9),
                corner(0.0, 0.9),
                corner(0.5, 0.2),
                corner(0.95, 0.95),
                corner(0.2, 0.95),
            ],
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            light: [0.0, 0.0, 1.0],
            ghosted: false,
        }
    }

    /// Spec `display-pipeline.md` §5: optional extras never change the picture, so a card
    /// without them shows the same.
    #[test]
    fn every_optional_extra_off_draws_the_same_picture() {
        let (Ok(bare), Ok(full)) = (
            draw_picture(mixed_picture(), SIZE, false),
            draw_picture(mixed_picture(), SIZE, true),
        ) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        // Something was drawn: not every pixel is the background.
        assert!(bare.chunks(4).any(|pixel| pixel != &bare[0..4]));
        assert_eq!(bare, full);
    }

    #[test]
    fn blocks_never_split_a_segment_or_a_triangle() {
        assert!(chunk_ranges(0, 2, 10).is_empty());
        assert_eq!(chunk_ranges(7, 3, 100), vec![0..7]);
        // 10 vertices to a block holds 3 triangles (9 vertices), never a part of a fourth.
        assert_eq!(chunk_ranges(20, 3, 10), vec![0..9, 9..18, 18..20]);
        assert_eq!(chunk_ranges(10, 2, 5), vec![0..4, 4..8, 8..10]);
        // A block smaller than one group still holds a whole group.
        assert_eq!(chunk_ranges(4, 2, 1), vec![0..2, 2..4]);
        for (len, group, most) in [(1000, 2, 7), (999, 3, 64), (12, 3, 3)] {
            let ranges = chunk_ranges(len, group, most);
            assert_eq!(ranges.first().map(|r| r.start), Some(0));
            assert_eq!(ranges.last().map(|r| r.end), Some(len));
            for pair in ranges.windows(2) {
                assert_eq!(pair[0].end, pair[1].start);
                assert_eq!(pair[0].end % group, 0);
            }
        }
    }

    /// Spec `display-pipeline.md` step 4: a scene too big for one block of graphics memory is
    /// drawn in several, and looks exactly the same.
    #[test]
    fn a_scene_drawn_in_many_blocks_looks_the_same() {
        // Blocks of 100 bytes: one segment or one triangle each.
        let (Ok(whole), Ok(split)) = (
            draw_picture(mixed_picture(), SIZE, true),
            draw_picture_in_chunks(mixed_picture(), SIZE, true, 100, &Scene::default()),
        ) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        assert!(whole.chunks(4).any(|pixel| pixel != &whole[0..4]));
        assert_eq!(whole, split);
    }

    /// A small model, seen whole: two circles and a box, so lines, edges on faces and faces all
    /// show. Returns its outlines, the camera's matrix, the light and the ids made.
    fn small_model() -> (crate::Outlines, [[f32; 4]; 4], [f32; 3], Vec<u64>) {
        let mut doc = curva_document::Document::default();
        let ids = vec![
            doc.add(curva_kernel::Curve::circle([0.0, 0.0, 0.0], 5.0).unwrap()),
            doc.add(curva_kernel::Curve::circle([12.0, 0.0, 0.0], 5.0).unwrap()),
            doc.add_body(curva_kernel::Body::cuboid([-4.0, 8.0, 0.0], [8.0, 8.0, 8.0]).unwrap()),
        ];
        let outlines = crate::Outlines::of(doc.state());
        let (low, high) = outlines.bounds().unwrap();
        let mut camera = crate::Camera::default();
        camera.frame(low, high);
        let light = camera.forward().map(|value| value as f32);
        (outlines, camera.matrix(1.0), light, ids)
    }

    /// Spec `display-pipeline.md` step 4: the selection drawn through the graphics card's table
    /// looks exactly as when it was drawn into the vertices' colours, lines a pixel wider
    /// included, in a shaded and in a wireframe view.
    #[test]
    fn the_selection_table_draws_what_coloured_vertices_drew() {
        let (outlines, matrix, light, ids) = small_model();
        // One circle and the box selected; the other circle not.
        let selected = vec![ids[0], ids[2]];
        let parts = crate::SceneParts::of(&outlines);
        for shaded in [true, false] {
            let picture = |lines: Vec<Vertex>, faces: Vec<MeshVertex>| Picture {
                lines,
                faces: if shaded { faces } else { Vec::new() },
                matrix,
                light,
                ghosted: false,
            };
            let coloured = picture(
                if shaded {
                    crate::shaded_object_lines(&outlines, &selected)
                } else {
                    crate::object_lines(&outlines, &selected)
                },
                crate::face_vertices(&outlines, &selected),
            );
            let numbered = picture(
                if shaded {
                    parts.shaded_lines.clone()
                } else {
                    parts.lines.clone()
                },
                parts.faces.clone(),
            );
            let scene = Scene {
                objects: Arc::new(parts.objects.clone()),
                selection: Arc::new(selected.clone()),
                clips: Vec::new(),
            };
            let size = [96, 96];
            let (Ok(before), Ok(after)) = (
                draw_picture(coloured, size, true),
                draw_picture_in_chunks(numbered, size, true, super::CHUNK_BYTES, &scene),
            ) else {
                eprintln!("no graphics adapter to draw with, so not checked");
                return;
            };
            // The selection colour shows, so the comparison is about something.
            let orange = before
                .chunks(4)
                .filter(|p| p[0] > 180 && p[2] < 120)
                .count();
            assert!(orange > 20, "only {orange} selected pixels");
            assert_eq!(before, after, "shaded: {shaded}");
        }
    }

    #[test]
    fn the_selection_table_marks_only_the_selected_objects() {
        let objects = [10, 20, 30, 40];
        let table = table_bytes(1, &objects, &[20, 40, 99]);
        assert_eq!(table.len(), PICK_ROW as usize);
        assert_eq!(&table[..5], &[0, 1, 0, 1, 0]);
        assert_eq!(table.iter().map(|&b| usize::from(b)).sum::<usize>(), 2);
        // Nothing selected: all zeros; more rows: still one byte per object, in order.
        assert!(table_bytes(1, &objects, &[]).iter().all(|&b| b == 0));
        let big: Vec<u64> = (0..5000).collect();
        let two_rows = table_bytes(2, &big, &[4999]);
        assert_eq!(two_rows.len(), 2 * PICK_ROW as usize);
        assert_eq!(two_rows[4999], 1);
    }

    /// #182: a sphere, which has no edges to draw, gets an outline at its rim from the depth
    /// picture, and nothing inside it: the outline test never mistakes the curved surface itself
    /// for a step back.
    #[test]
    fn a_sphere_is_outlined_at_its_rim_and_clean_inside() {
        let mut doc = curva_document::Document::default();
        doc.add_body(curva_kernel::Body::sphere([0.0; 3], 10.0).unwrap());
        let outlines = crate::Outlines::of(doc.state());
        // Seen straight down, so the rim on screen is exactly the sphere's radius.
        let mut camera = crate::StandardView::Top.camera();
        camera.frame([-12.0, -12.0, -10.0], [12.0, 12.0, 10.0]);
        let size = [128u32, 128];
        let picture = Picture {
            lines: crate::shaded_object_lines(&outlines, &[]),
            faces: crate::face_vertices(&outlines, &[]),
            matrix: camera.matrix(1.0),
            light: camera.forward().map(|v| v as f32),
            ghosted: false,
        };
        let Ok(pixels) = draw_picture(picture, size, true) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        let red = |x: usize, y: usize| pixels[(y * 128 + x) * 4];
        let screen = |p: [f64; 3]| camera.project(p, [128.0, 128.0]).unwrap();
        let [cx, cy] = screen([0.0; 3]);
        let rim = screen([10.0, 0.0, 0.0])[0] - cx;
        // Within the last few pixels inside the rim, on every side: the outline's ink. (The
        // meshed sphere lies a fraction of a pixel inside the true one. Pictures store colours
        // converted for display, so the near-black ink reads about 80, the lit surface 180 up.)
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let darkest = (1..=4)
                .map(|inside| {
                    let at = rim - f64::from(inside);
                    red((cx + dx * at) as usize, (cy + dy * at) as usize)
                })
                .min()
                .unwrap_or(255);
            assert!(darkest <= 90, "rim towards {dx},{dy}: {darkest}");
        }
        // Inside, well clear of the rim, no ink at all: the curved surface is never taken for a
        // step back.
        for y in 0..128 {
            for x in 0..128 {
                let from = (x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy);
                if from < rim - 5.0 {
                    assert!(red(x, y) > 120, "ink inside at {x},{y}: {}", red(x, y));
                }
            }
        }
    }
}
