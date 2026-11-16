//! Turning lines, faces and the selection into bytes for the graphics card, sent in blocks.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

use super::shaders::PICK_ROW;

/// The depth buffer's format, which the window must be opened with.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
/// Bits of depth to ask the window for, matching [`DEPTH_FORMAT`].
pub const DEPTH_BITS: u8 = 24;

/// A line vertex: position, colour, width and object number.
pub(crate) const LINE_VERTEX_SIZE: u64 = 32;
/// A segment, both its ends, read as one instance by [`LINE_SHADER`].
pub(crate) const SEGMENT_SIZE: u64 = 2 * LINE_VERTEX_SIZE;
/// The corners of the two triangles of a segment's quad.
pub(crate) const SEGMENT_CORNERS: u32 = 6;
/// A face vertex: position, normal, colour and object number.
pub(crate) const FACE_VERTEX_SIZE: u64 = 40;
pub(crate) const UNIFORM_SIZE: u64 = 160;

/// The most clipping planes one view is cut by.
pub const MOST_CLIPS: usize = 4;
/// Pixels per millimetre on a 96 dpi screen at 100 %.
pub(crate) const PIXELS_PER_MM: f32 = 96.0 / 25.4;
/// Lines are pulled slightly forward, so they win ties with what lies at their depth.
pub(crate) const LINE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: -2,
    slope_scale: -1.0,
    clamp: 0.0,
};
/// Faces are pushed slightly back, so edges on them are drawn in front.
pub(crate) const FACE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 16,
    slope_scale: 6.0,
    clamp: 0.0,
};
pub(crate) const PICTURE_BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.11,
    g: 0.12,
    b: 0.14,
    a: 1.0,
};

// Written out by hand so the crate needs no unsafe casting of memory.
pub(crate) fn line_bytes(lines: &[Vertex]) -> Vec<u8> {
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
        bytes.extend(vertex.object.to_le_bytes());
    }
    bytes
}

pub(crate) fn face_bytes(faces: &[MeshVertex]) -> Vec<u8> {
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
        bytes.extend(vertex.object.to_le_bytes());
    }
    bytes
}

/// `pixel`: a pixel's width and height in clip space, and the screen's pixels per millimetre.
/// `clips`: up to [`MOST_CLIPS`] clipping planes, each `[nx, ny, nz, d]`, cutting away where
/// `n·p + d > 0`.
pub(crate) fn uniform_bytes(
    matrix: &[[f32; 4]; 4],
    light: [f32; 3],
    pixel: [f32; 3],
    clips: &[[f32; 4]],
) -> Vec<u8> {
    let clips = &clips[..clips.len().min(MOST_CLIPS)];
    #[allow(clippy::cast_precision_loss)]
    let count = clips.len() as f32;
    let mut planes = [[0.0_f32; 4]; MOST_CLIPS];
    planes[..clips.len()].copy_from_slice(clips);
    matrix
        .iter()
        .flatten()
        .chain(&light)
        .chain(&[0.0])
        .chain(&pixel)
        .chain(&[count])
        .chain(planes.iter().flatten())
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// The largest block of graphics memory one upload is split into. The baseline cards refuse a
/// single block over 256 MB (`wgpu::Limits::max_buffer_size`); a quarter of that leaves room and
/// keeps each copy to the card short.
pub(crate) const CHUNK_BYTES: u64 = 64 * 1024 * 1024;

/// Vertices on the graphics card, in blocks of at most [`CHUNK_BYTES`], and what they were made
/// from, so unchanged ones are kept.
pub(crate) struct Uploaded<T> {
    /// Each block and how many vertices it holds.
    pub(crate) chunks: Vec<(wgpu::Buffer, u32)>,
    pub(crate) count: u32,
    pub(crate) source: Arc<Vec<T>>,
}

/// Where to split `len` vertices into blocks of at most `most` vertices, never splitting a
/// group of `group` (2 for a line segment, 3 for a triangle).
pub(crate) fn chunk_ranges(len: usize, group: usize, most: usize) -> Vec<std::ops::Range<usize>> {
    let step = (most / group).max(1) * group;
    (0..len)
        .step_by(step)
        .map(|start| start..(start + step).min(len))
        .collect()
}

/// Sends `source` to the graphics card in blocks of at most `chunk_bytes`, each turned to bytes
/// on its own so the whole scene is never copied at once.
pub(crate) fn upload<T>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &Arc<Vec<T>>,
    bytes: impl Fn(&[T]) -> Vec<u8>,
    vertex_size: u64,
    group: usize,
    chunk_bytes: u64,
) -> Uploaded<T> {
    let most = (chunk_bytes / vertex_size).max(1) as usize;
    let chunks = chunk_ranges(source.len(), group, most)
        .into_iter()
        .map(|range| {
            let count = range.len() as u32;
            let bytes = bytes(&source[range]);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("curva vertices"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &bytes);
            (buffer, count)
        })
        .collect();
    Uploaded {
        chunks,
        count: source.len() as u32,
        source: Arc::clone(source),
    }
}

pub(crate) fn stale<T>(held: Option<&Uploaded<T>>, wanted: &Arc<Vec<T>>) -> bool {
    held.is_none_or(|uploaded| !Arc::ptr_eq(&uploaded.source, wanted))
}

/// How many views can be drawn at once.
pub const VIEW_SLOTS: usize = 4;

/// Slots after the viewports' for the detail views on a sheet (spec `layouts.md` §2). A slot
/// holds buffers only once it draws something.
pub const DETAIL_SLOTS: usize = 12;

/// The selection table on the graphics card (`table_wgsl`): one byte per numbered object,
/// [`PICK_ROW`] to a row, 1 where the object is selected, and the object list and selection it
/// was last written for.
pub(crate) struct Picks {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) rows: u32,
    pub(crate) objects: Arc<Vec<u64>>,
    pub(crate) selection: Arc<Vec<u64>>,
}

impl Picks {
    /// An empty table of `rows` rows.
    pub(crate) fn new(device: &wgpu::Device, rows: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("curva selection table"),
            size: wgpu::Extent3d {
                width: PICK_ROW,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            rows,
            // Never the same as a drawing's lists, so the first drawing writes the table.
            objects: Arc::default(),
            selection: Arc::default(),
        }
    }

    pub(crate) fn write(&self, queue: &wgpu::Queue, bytes: &[u8]) {
        queue.write_texture(
            self.texture.as_image_copy(),
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(PICK_ROW),
                rows_per_image: Some(self.rows),
            },
            wgpu::Extent3d {
                width: PICK_ROW,
                height: self.rows,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// What the selection table holds, row by row, for `rows` rows of objects numbered as in
/// `objects` with `selection` chosen: 1 for a selected object, 0 otherwise.
pub(crate) fn table_bytes(rows: u32, objects: &[u64], selection: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0; (PICK_ROW * rows) as usize];
    if !selection.is_empty() {
        let chosen: std::collections::HashSet<&u64> = selection.iter().collect();
        for (byte, id) in bytes.iter_mut().zip(objects) {
            *byte = u8::from(chosen.contains(id));
        }
    }
    bytes
}

/// A view's bindings: its camera and the selection table.
pub(crate) fn bind_view(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    picks: &Picks,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("curva camera"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&picks.view),
            },
        ],
    })
}
