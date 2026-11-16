//! One frame of the viewport, drawn into the window by egui.

use crate::{MeshVertex, Vertex};
use egui_wgpu::wgpu;
use std::sync::Arc;

use super::{LineRenderer, PIXELS_PER_MM};

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
    /// Which object each number in `scene` and `faces` stands for ([`crate::SceneParts`]);
    /// empty when their vertices carry no numbers.
    pub objects: Arc<Vec<u64>>,
    /// The selected objects, drawn in the selection colour through the selection table.
    pub selection: Arc<Vec<u64>>,
    /// The live preview.
    pub overlay: Arc<Vec<Vertex>>,
    /// The live preview's surfaces and solids, ghosted ([`crate::ghost_faces`]).
    pub preview_faces: Arc<Vec<MeshVertex>>,
    /// The camera.
    pub matrix: [[f32; 4]; 4],
    /// The direction the camera looks, of length 1, which lights the faces.
    pub light: [f32; 3],
    /// Clipping planes cutting this view, each `[nx, ny, nz, d]`, cutting away where
    /// `n·p + d > 0`; at most [`MOST_CLIPS`] are used.
    pub clips: Vec<[f32; 4]>,
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
        // A view is drawn in up to three passes: surfaces, outlines found from their depth, then
        // lines and previews. The card's clock times them as one.
        let timing = renderer
            .timer
            .as_mut()
            .and_then(|timer| timer.begin_view(self.frame));
        let clock = |end: bool| {
            timing
                .as_ref()
                .map(|(queries, first)| wgpu::RenderPassTimestampWrites {
                    query_set: queries,
                    beginning_of_pass_write_index: (!end).then_some(*first),
                    end_of_pass_write_index: end.then_some(*first + 1),
                })
        };
        // The first viewport of a frame clears the whole texture, gaps included.
        let clear = first.then_some(self.background);
        let mut pass = surfaces_pass(encoder, &colour, &depth, clear, clock(false));
        let area = [low.x, low.y, high.x - low.x, high.y - low.y];
        fit(&mut pass, area);
        renderer.background.draw(&mut pass);
        renderer.draw_under(&mut pass, self.slot);
        drop(pass);
        if renderer.outlines_wanted(self.slot) {
            renderer.outliner.draw(
                device,
                queue,
                encoder,
                &colour,
                &depth,
                area,
                crate::EDGE_ON_FACES_COLOUR,
            );
        }
        let mut pass = lines_pass(encoder, &colour, &depth, clock(true));
        fit(&mut pass, area);
        renderer.draw_over(&mut pass, self.slot);
        drop(pass);
        if let Some(timer) = &mut renderer.timer {
            timer.finish_pass(encoder);
        }
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

/// The pass drawing a view's background and surfaces, clearing the texture to `clear` first if
/// given, and keeping its depth for the outline pass and the lines drawn after it.
fn surfaces_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    colour: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    clear: Option<epaint::Color32>,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'encoder> {
    // Written as stored, like every colour Curva draws: egui gives the window a texture that
    // is not converted to linear light.
    let channel = |value: u8| f64::from(value) / 255.0;
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("curva viewport"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: colour,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: clear.map_or(wgpu::LoadOp::Load, |background| {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: channel(background.r()),
                        g: channel(background.g()),
                        b: channel(background.b()),
                        a: 1.0,
                    })
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                // Kept for the outline pass and for the lines drawn after it.
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// The pass drawing a view's lines and previews over its surfaces and outlines.
fn lines_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    colour: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'encoder> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("curva viewport lines"),
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
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Keeps a pass to the view's `area` on the texture: left, top, width and height in pixels.
fn fit(pass: &mut wgpu::RenderPass<'_>, area: [f32; 4]) {
    pass.set_viewport(area[0], area[1], area[2], area[3], 0.0, 1.0);
    pass.set_scissor_rect(
        area[0] as u32,
        area[1] as u32,
        area[2] as u32,
        area[3] as u32,
    );
}
