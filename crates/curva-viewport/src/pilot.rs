//! The Bevy pilot (decision 0028, `docs/specs/bevy-pilot.md`): a view set to Bevy has Bevy draw
//! its picture, on Curva's own graphics device, and the picture is copied into the scene texture
//! where the view sits. Step 1 draws the background only; faces come in step 2, Curva's lines on
//! top in step 3.
// CUSTOM BEVY PART: bevy-host

use std::collections::HashMap;
use std::sync::Mutex;

use egui_wgpu::wgpu;

use bevy::{
    app::{AppLabel, PanicHandlerPlugin, SubApps, TerminalCtrlCHandlerPlugin},
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::Image,
    prelude::*,
    render::{
        RenderApp, RenderPlugin,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{TextureFormat, TextureUsages},
        renderer::{RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue},
        settings::RenderCreation,
        texture::GpuImage,
    },
    window::ExitCondition,
};

/// Which engine draws a view's faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    /// Curva's own drawing, as before the pilot.
    #[default]
    Curva,
    /// Bevy draws the view's picture (the pilot, decision 0028).
    BevyPilot,
}

/// What Bevy needs from the window's graphics set-up besides the device and queue, kept at
/// start-up so Bevy can be started the first time a view is set to it.
#[derive(Clone)]
pub struct PilotSeed {
    /// The window's graphics instance.
    pub instance: wgpu::Instance,
    /// The graphics card the window's device was made on.
    pub adapter: wgpu::Adapter,
}

/// One pilot view's camera and the picture it draws into.
struct PilotView {
    camera: Entity,
    target: Handle<Image>,
    size: [u32; 2],
}

/// Bevy, running on Curva's device, with a camera for each view set to it.
pub(crate) struct Pilot {
    apps: Mutex<SubApps>,
    views: HashMap<usize, PilotView>,
    /// The format of the pictures Bevy draws: the scene texture's, in its sRGB form, so a copy
    /// moves the stored bytes unchanged.
    format: TextureFormat,
}

impl Pilot {
    /// Starts Bevy on `device` and `queue`, drawing pictures that copy into a texture of
    /// `format`.
    pub(crate) fn new(
        seed: &PilotSeed,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let render = RenderPlugin {
            render_creation: RenderCreation::manual(
                RenderDevice::new(device.clone()),
                RenderQueue::new(queue.clone()),
                RenderAdapterInfo::new(seed.adapter.get_info()),
                RenderAdapter::new(seed.adapter.clone()),
                RenderInstance::new(seed.instance.clone()),
            ),
            synchronous_pipeline_compilation: true,
            ..default()
        };
        let window = WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        };
        let mut app = App::new();
        // Curva owns the window, the panic hook and Ctrl-C; Bevy draws when a view asks, on
        // this thread, so its picture is ready before the view is shown.
        app.add_plugins(
            DefaultPlugins
                .set(window)
                .set(render)
                .disable::<PipelinedRenderingPlugin>()
                .disable::<TerminalCtrlCHandlerPlugin>()
                .disable::<PanicHandlerPlugin>(),
        );
        app.finish();
        app.cleanup();
        Self {
            apps: Mutex::new(std::mem::take(app.sub_apps_mut())),
            views: HashMap::new(),
            format: format.add_srgb_suffix(),
        }
    }

    /// Has Bevy draw view `slot`'s picture, `size` pixels, and returns it; `None` while Bevy
    /// has not made it yet.
    pub(crate) fn draw(
        &mut self,
        slot: usize,
        size: [u32; 2],
        background: epaint::Color32,
    ) -> Option<wgpu::Texture> {
        let size = size.map(|side| side.max(1));
        let mut apps = self.apps.lock().ok()?;
        let fresh = self.views.get(&slot).is_none_or(|view| view.size != size);
        if fresh {
            let view = make_view(&mut apps, size, self.format, self.views.remove(&slot));
            self.views.insert(slot, view);
        }
        let world = apps.main.world_mut();
        for (other, view) in &self.views {
            if let Some(mut camera) = world.get_mut::<Camera>(view.camera) {
                camera.is_active = *other == slot;
                if *other == slot {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb_u8(
                        background.r(),
                        background.g(),
                        background.b(),
                    ));
                }
            }
        }
        apps.update();
        // A new picture reaches the card during the first update and is drawn in the next.
        if fresh {
            apps.update();
        }
        let target = &self.views.get(&slot)?.target;
        let render = apps.sub_apps.get(&RenderApp.intern())?;
        let images = render.world().get_resource::<RenderAssets<GpuImage>>()?;
        images.get(target).map(|image| (*image.texture).clone())
    }
}

/// A camera drawing into a new picture of `size`, replacing `old` if given.
fn make_view(
    apps: &mut SubApps,
    size: [u32; 2],
    format: TextureFormat,
    old: Option<PilotView>,
) -> PilotView {
    let world = apps.main.world_mut();
    let mut image = Image::new_target_texture(size[0], size[1], format, None);
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    // Copied into the scene texture after Bevy draws it.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = world.resource_mut::<Assets<Image>>().add(image);
    if let Some(old) = old {
        world.resource_mut::<Assets<Image>>().remove(&old.target);
        world.despawn(old.camera);
    }
    // Colours as Curva writes them: no tone mapping or dithering changing the stored values.
    let camera = world
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(target.clone().into()),
            Tonemapping::None,
            DebandDither::Disabled,
            Transform::from_xyz(0.0, -10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        ))
        .id();
    PilotView {
        camera,
        target,
        size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `// CUSTOM BEVY PART:` mark has a row in `docs/bevy/custom-parts.md`, and every row a
    /// mark (decision 0030), as the kernel's custom operations are kept.
    #[test]
    fn custom_bevy_parts_match_their_list() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let list = std::fs::read_to_string(root.join("docs/bevy/custom-parts.md")).unwrap();
        let rows: std::collections::BTreeSet<String> = list
            .lines()
            .skip_while(|line| !line.starts_with("| Part | Code"))
            .skip(2)
            .take_while(|line| line.starts_with("| "))
            .map(|line| line.split('|').nth(1).unwrap().trim().to_owned())
            .collect();
        let mut marks = std::collections::BTreeSet::new();
        let mut stack = vec![root.join("crates")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for line in text.lines() {
                        if let Some(name) = line.trim().strip_prefix("// CUSTOM BEVY PART: ") {
                            marks.insert(name.trim().to_owned());
                        }
                    }
                }
            }
        }
        assert_eq!(marks, rows, "marks in the code and rows in the list differ");
    }

    /// A pilot view's picture is its background colour, made by Bevy on a device Curva made.
    #[test]
    fn pilot_draws_the_background_on_curvas_device() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, true)))
                .expect("device");
        let seed = PilotSeed {
            instance: instance.clone(),
            adapter: adapter.clone(),
        };
        let mut pilot = Pilot::new(&seed, &device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        let background = epaint::Color32::from_rgb(200, 120, 40);
        let size = [64, 48];
        let texture = pilot.draw(0, size, background).expect("Bevy's picture");
        assert_eq!([texture.width(), texture.height()], size);
        let pixels = crate::gpu::read_texture(&device, &queue, &texture, size);
        for pixel in pixels.chunks(4) {
            assert_eq!(pixel, [200, 120, 40, 255]);
        }
        // A new size makes a new picture.
        let texture = pilot.draw(0, [32, 32], background).expect("resized");
        assert_eq!([texture.width(), texture.height()], [32, 32]);
    }
}
