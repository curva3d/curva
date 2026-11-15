//! The graphics card's own time for drawing the viewports, for the benchmark (spec
//! `display-pipeline.md` §7 step 1), and what the viewport asks of the card.
//!
//! The card's timer is an optional extra (`wgpu::Features::TIMESTAMP_QUERY`): without it nothing
//! is measured and the benchmark says so. Each viewport pass writes the card's clock at its start
//! and end; the two readings are copied to a small buffer read back a frame or two later, so the
//! measurement never makes a frame wait.

use egui_wgpu::wgpu;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// What the viewport asks of the graphics card: the baseline limits every supported card has
/// (spec `display-pipeline.md` §5), plus each optional extra the card offers. Nothing beyond the
/// baseline is ever required: with `extras` false the extras are left out, as on a card without
/// them.
#[must_use]
pub fn device_descriptor(adapter: &wgpu::Adapter, extras: bool) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        label: Some("curva device"),
        required_features: if extras {
            adapter.features() & OPTIONAL_FEATURES
        } else {
            wgpu::Features::empty()
        },
        // The OpenGL fallback (machines without Vulkan, DirectX 12 or Metal) has lower limits;
        // egui asked for these before, so nothing changes there.
        required_limits: if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..wgpu::Limits::downlevel_webgl2_defaults()
            }
        } else {
            baseline_limits()
        },
        ..wgpu::DeviceDescriptor::default()
    }
}

/// The limits the viewport keeps to on every card: wgpu's defaults, which 2020 Intel integrated
/// graphics meet, with textures large enough for a 4K or larger window's depth buffer.
#[must_use]
pub fn baseline_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 8192,
        ..wgpu::Limits::default()
    }
}

/// The optional extras the viewport uses when a card has them.
pub const OPTIONAL_FEATURES: wgpu::Features = wgpu::Features::TIMESTAMP_QUERY;

/// Most viewport passes timed in one frame: the four viewports and the detail views of a sheet.
const MOST_PASSES: u32 = (crate::VIEW_SLOTS + crate::DETAIL_SLOTS) as u32;

/// How many readings may be on their way back at once before new passes go untimed.
const READBACKS: usize = 64;

/// One reading's buffer: free, waiting for the card, or ready to read.
struct Readback {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// The frame its readings are from.
    frame: u64,
}

const FREE: u8 = 0;
const WAITING: u8 = 1;
const READY: u8 = 2;

/// Times the viewport passes on the graphics card while switched on.
pub(crate) struct GpuTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readbacks: Vec<Readback>,
    /// Nanoseconds per tick of the card's clock.
    period: f64,
    /// The next pair of queries to use, within this frame.
    next: u32,
    /// The frame the queries are being used in.
    frame: u64,
    total_ms: f64,
    /// The frames whose readings have come back.
    frames: std::collections::HashSet<u64>,

    on: bool,
}

impl GpuTimer {
    /// A timer, if the device has the card's timer.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        // Each pass's two readings are resolved to their own aligned place.
        let bytes = u64::from(MOST_PASSES) * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        let readbacks = (0..READBACKS)
            .map(|_| Readback {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("curva timer readback"),
                    size: 16,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(FREE)),
                frame: 0,
            })
            .collect();
        Some(Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("curva timer"),
                ty: wgpu::QueryType::Timestamp,
                count: MOST_PASSES * 2,
            }),
            resolved: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("curva timer resolved"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readbacks,
            period: f64::from(queue.get_timestamp_period()),
            next: 0,
            frame: u64::MAX,
            total_ms: 0.0,
            frames: std::collections::HashSet::new(),

            on: false,
        })
    }

    /// Starts or stops timing, and clears what was measured.
    pub(crate) fn switch(&mut self, on: bool) {
        self.on = on;
        self.total_ms = 0.0;
        self.frames.clear();
    }

    /// The clock writes for a pass in `frame`, or `None` when off or out of room.
    pub(crate) fn writes(&mut self, frame: u64) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if !self.on {
            return None;
        }
        if frame != self.frame {
            self.frame = frame;
            self.next = 0;
        }
        if self.next >= MOST_PASSES {
            return None;
        }
        let pair = self.next * 2;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(pair),
            end_of_pass_write_index: Some(pair + 1),
        })
    }

    /// After the pass that used [`Self::writes`]: copies its two readings to a free buffer and
    /// asks for it back. Readings that have come back are added up first.
    pub(crate) fn finish_pass(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.collect();
        if !self.on || self.next >= MOST_PASSES {
            return;
        }
        let pair = self.next * 2;
        self.next += 1;
        let frame = self.frame;
        let Some(readback) = self
            .readbacks
            .iter_mut()
            .find(|r| r.state.load(Ordering::Acquire) == FREE)
        else {
            return;
        };
        let offset = u64::from(pair / 2) * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        encoder.resolve_query_set(&self.queries, pair..pair + 2, &self.resolved, offset);
        encoder.copy_buffer_to_buffer(&self.resolved, offset, &readback.buffer, 0, 16);
        readback.frame = frame;
        readback.state.store(WAITING, Ordering::Release);
        let state = Arc::clone(&readback.state);
        // Asked for now, answered once the card has run this frame's commands.
        encoder.map_buffer_on_submit(&readback.buffer, wgpu::MapMode::Read, .., move |result| {
            state.store(if result.is_ok() { READY } else { FREE }, Ordering::Release);
        });
    }

    /// Adds up the readings that have come back and frees their buffers.
    fn collect(&mut self) {
        for readback in &self.readbacks {
            if readback.state.load(Ordering::Acquire) != READY {
                continue;
            }
            if let Ok(bytes) = readback.buffer.slice(..).get_mapped_range() {
                let tick = |at: usize| {
                    bytes
                        .get(at..at + 8)
                        .and_then(|b| b.try_into().ok())
                        .map_or(0, u64::from_le_bytes)
                };
                let (start, end) = (tick(0), tick(8));
                // A clock that wrapped or was reset gives a reading to leave out; equal readings
                // are a pass shorter than one tick of the clock, which counts as no time.
                if end >= start && self.on {
                    self.total_ms += (end - start) as f64 * self.period / 1.0e6;
                    self.frames.insert(readback.frame);
                }
            }
            readback.buffer.unmap();
            readback.state.store(FREE, Ordering::Release);
        }
    }

    /// The card's average time per frame for the viewport passes since timing was switched on,
    /// in milliseconds, over the frames whose readings have come back. The last frame or two may
    /// be counted with only some of their passes, which over a benchmark's 200 frames is noise.
    pub(crate) fn per_frame_ms(&self) -> Option<f64> {
        (!self.frames.is_empty()).then(|| self.total_ms / self.frames.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The machine's graphics adapter, or `None` on a machine without one.
    fn adapter() -> Option<wgpu::Adapter> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()
    }

    #[test]
    fn the_viewport_asks_only_for_the_baseline_and_extras_the_card_has() {
        let Some(adapter) = adapter() else {
            eprintln!("no graphics adapter, so not checked");
            return;
        };
        let bare = device_descriptor(&adapter, false);
        assert!(bare.required_features.is_empty());
        let full = device_descriptor(&adapter, true);
        assert!(OPTIONAL_FEATURES.contains(full.required_features));
        assert!(adapter.features().contains(full.required_features));
        // Both ways the card gives a device: nothing beyond the baseline is required.
        for descriptor in [bare, full] {
            assert!(pollster::block_on(adapter.request_device(&descriptor)).is_ok());
        }
    }

    #[test]
    fn without_the_cards_timer_there_is_nothing_to_time() {
        let Some(adapter) = adapter() else {
            eprintln!("no graphics adapter, so not checked");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&device_descriptor(&adapter, false)))
                .unwrap();
        assert!(GpuTimer::new(&device, &queue).is_none());
    }

    #[test]
    fn the_timer_reads_back_the_time_of_a_pass() {
        let Some(adapter) = adapter() else {
            eprintln!("no graphics adapter, so not checked");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&device_descriptor(&adapter, true))).unwrap();
        let Some(mut timer) = GpuTimer::new(&device, &queue) else {
            eprintln!("this card has no timer, so not checked");
            return;
        };
        assert!(timer.per_frame_ms().is_none());
        timer.switch(true);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        // Two frames of two passes each, like two viewports.
        for frame in 0..2 {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            for _ in 0..2 {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: timer.writes(frame),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                drop(pass);
                timer.finish_pass(&mut encoder);
            }
            queue.submit([encoder.finish()]);
        }
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        timer.collect();
        let ms = timer.per_frame_ms().expect("both frames read back");
        assert!((0.0..1000.0).contains(&ms), "{ms} ms for two empty passes");
        // Switching again starts afresh.
        timer.switch(true);
        assert!(timer.per_frame_ms().is_none());
    }
}
