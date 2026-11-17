//! Drawing lines and shaded surfaces on the graphics card, on screen and into a picture file.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

mod circles;
mod depth;
mod drawing;
mod picture;
mod shaders;
mod upload;

pub use drawing::*;
pub use picture::*;
use shaders::{FACE_SHADER, GHOST_FACE_SHADER, GRID_SHADER, LINE_SHADER, PICK_ROW, table_wgsl};
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
}

/// The graphics card's side of the viewport.
pub struct LineRenderer {
    /// Hands Bevy's depth to Curva's lines, for views Bevy draws; made when first needed.
    pub(crate) handover: Option<depth::Handover>,
    grid: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    /// Draws whole circles from their centre and axes (#153).
    circle_lines: wgpu::RenderPipeline,
    faces: wgpu::RenderPipeline,
    ghost_faces: wgpu::RenderPipeline,
    slots: Vec<ViewSlot>,
    surfaces: Option<Uploaded<MeshVertex>>,
    /// The whole circles, shared by every view drawing the model's lines.
    circles: Option<Uploaded<crate::PlacedCircle>>,
    overlay: Option<Uploaded<Vertex>>,
    /// The ghosted faces of a command's preview.
    preview_faces: Option<Uploaded<MeshVertex>>,
    /// Draws the viewports through a texture, which glass panels blur.
    pub(crate) compositor: crate::glass::Compositor,
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
        Self {
            grid,
            lines,
            circle_lines: circles::circle_pipeline(device, format, &pipeline_layout),
            faces,
            ghost_faces,
            timer: None,
            chunk_bytes: CHUNK_BYTES,
            outliner: crate::outline::Outliner::new(device, format),
            handover: None,
            layout,
            picks,
            slots,
            surfaces: None,
            circles: None,
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
        if stale(self.circles.as_ref(), &drawing.circles) {
            self.circles = Some(upload(
                device,
                queue,
                &drawing.circles,
                circles::circle_bytes,
                circles::CIRCLE_SIZE,
                1,
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
            Self::draw_faces(pass, &self.faces, faces);
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
        if let Some(circles) = self.circles.as_ref() {
            circles::draw_circles(pass, &self.circle_lines, circles);
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
mod tests;
