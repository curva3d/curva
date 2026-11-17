//! Drawing into a picture file instead of the window.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

use super::{CHUNK_BYTES, DEPTH_FORMAT, Drawing, LineRenderer, PICTURE_BACKGROUND, PIXELS_PER_MM};

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
    let pixels = draw_picture(picture, size, true)?;
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

/// Draws a picture without a window and returns its pixels, four bytes each, row by row. The
/// device is asked for as the window asks ([`crate::device_descriptor`]); with `extras` false,
/// without any optional extra, as on a card that has none.
pub(crate) fn draw_picture(
    picture: Picture,
    size: [u32; 2],
    extras: bool,
) -> Result<Vec<u8>, String> {
    draw_picture_in_chunks(picture, size, extras, CHUNK_BYTES, &Scene::default())
}

/// [`draw_picture`], cut by clipping planes, each `[nx, ny, nz, d]` cutting away where
/// `n·p + d > 0` (a plan cut, a section).
pub(crate) fn draw_picture_cut(
    picture: Picture,
    size: [u32; 2],
    clips: Vec<[f32; 4]>,
) -> Result<Vec<u8>, String> {
    let scene = Scene {
        clips,
        ..Scene::default()
    };
    draw_picture_in_chunks(picture, size, true, CHUNK_BYTES, &scene)
}

/// The object list and selection a picture's numbered lines and faces are drawn with; empty
/// for a picture whose selection is drawn into its colours.
#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) objects: Arc<Vec<u64>>,
    pub(crate) selection: Arc<Vec<u64>>,
    /// Clipping planes cutting the picture, as for [`Drawing::clips`].
    pub(crate) clips: Vec<[f32; 4]>,
}

/// [`draw_picture`], with uploads split into blocks of at most `chunk_bytes`.
pub(crate) fn draw_picture_in_chunks(
    picture: Picture,
    size: [u32; 2],
    extras: bool,
    chunk_bytes: u64,
    scene: &Scene,
) -> Result<Vec<u8>, String> {
    let [width, height] = size;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|error| error.to_string())?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, extras)))
            .map_err(|error| error.to_string())?;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let (texture, depth) = picture_targets(&device, extent, format);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = LineRenderer::new(&device, format);
    renderer.chunk_bytes = chunk_bytes;
    let drawing = picture_drawing(picture, scene);
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
    draw_picture_passes(
        &mut renderer,
        &device,
        &queue,
        &mut encoder,
        [&view, &depth_view],
        size,
    );
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
    read_pixels(&device, &readback, row, size)
}

/// The picture's colour texture and its depth, both `extent` in size.
fn picture_targets(
    device: &wgpu::Device,
    extent: wgpu::Extent3d,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::Texture) {
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
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("curva picture depth"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    (texture, depth)
}

/// The picture as one frame of a view.
fn picture_drawing(picture: Picture, scene: &Scene) -> Drawing {
    let empty = Arc::new(Vec::new());
    let shaded = !picture.faces.is_empty();
    // The grid is drawn as in a window: thin and under the model's lines.
    let (grid, lines): (Vec<Vertex>, Vec<Vertex>) = picture
        .lines
        .into_iter()
        .partition(|vertex| vertex.width == crate::GRID_WIDTH);
    Drawing {
        clips: scene.clips.clone(),
        slot: 0,
        grid: Arc::new(grid),
        scene: Arc::new(lines),
        faces: Arc::new(picture.faces),
        shapes: Arc::default(),
        overlay: Arc::clone(&empty),
        preview_faces: Arc::new(Vec::new()),
        objects: Arc::clone(&scene.objects),
        selection: Arc::clone(&scene.selection),
        matrix: picture.matrix,
        light: picture.light,
        shaded,
        ghosted: picture.ghosted,
        area: epaint::Rect::NOTHING,
        frame: 0,
        background: epaint::Color32::BLACK,
        background_low: epaint::Color32::BLACK,
        camera: crate::Camera::default(),
    }
}

/// As in a window: surfaces, outlines from the surfaces' depth, then lines over them, into the
/// picture's colour and depth `targets`.
fn draw_picture_passes(
    renderer: &mut LineRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    [view, depth_view]: [&wgpu::TextureView; 2],
    [width, height]: [u32; 2],
) {
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("curva picture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(PICTURE_BACKGROUND),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.draw_under(&mut pass, 0);
    }
    if renderer.outlines_wanted(0) {
        renderer.outliner.draw(
            device,
            queue,
            encoder,
            view,
            depth_view,
            [0.0, 0.0, width as f32, height as f32],
            crate::EDGE_ON_FACES_COLOUR,
        );
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("curva picture lines"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    renderer.draw_over(&mut pass, 0);
}

/// Reads a four-byte-per-pixel texture of `size` back from the card.
pub(crate) fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
) -> Vec<u8> {
    let row = (size[0] * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("curva texture readback"),
        size: u64::from(row) * u64::from(size[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    read_pixels(device, &readback, row, size).expect("pixels")
}

/// Waits for the picture and reads its pixels back, row by row, leaving out each row's padding.
fn read_pixels(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    row: u32,
    [width, height]: [u32; 2],
) -> Result<Vec<u8>, String> {
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
    Ok(pixels)
}
