//! The Bevy pilot (decision 0028, `docs/specs/bevy-pilot.md`): a view set to Bevy has Bevy draw
//! its picture, on Curva's own graphics device, and the picture is copied into the scene texture
//! where the view sits. Step 1 draws the background only; faces come in step 2, Curva's lines on
//! top in step 3.
// CUSTOM BEVY PART: bevy-host

use std::collections::HashMap;
use std::sync::Mutex;

mod clip;
mod scene;
pub use scene::{Lighting, Look};
use scene::{SharedMeshes, ViewScene};

use crate::Drawing;
use crate::camera::Lens;
use std::sync::Arc;

use egui_wgpu::wgpu;

use bevy::{
    app::{AppLabel, PanicHandlerPlugin, SubApps, TerminalCtrlCHandlerPlugin},
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, ScalingMode, visibility::RenderLayers},
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
        sync_world::RenderEntity,
        texture::GpuImage,
        view::ViewDepthStencilTexture,
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

/// One Bevy view: its camera, the picture it draws into, and what it shows.
struct PilotView {
    camera: Entity,
    target: Handle<Image>,
    size: [u32; 2],
    scene: ViewScene,
    /// What its camera was last given: the view and size, and the look's lighting; Bevy redoes
    /// work for each change, so only changes are given.
    placed: Option<(Lens, [u32; 2])>,
    lit: Option<Look>,
}

/// Bevy, running on Curva's device, with a camera and a copy of the scene for each view drawn by
/// it.
pub(crate) struct Pilot {
    apps: Mutex<SubApps>,
    views: HashMap<usize, PilotView>,
    meshes: SharedMeshes,
    /// How every Bevy view's surfaces look, until styles set it per view.
    pub(crate) look: Look,
    /// What surfaces reflect all around them, for each lighting, made the first time a view draws.
    skies: Option<[EnvironmentMapLight; 2]>,
    /// This frame's views so far, with their sizes and places, until the last of them comes.
    pending: Vec<(u64, Drawing, [u32; 2], [u32; 2])>,
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
        )
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 0.0,
            ..default()
        });
        clip::install(&mut app);
        app.finish();
        app.cleanup();
        Self {
            apps: Mutex::new(std::mem::take(app.sub_apps_mut())),
            views: HashMap::new(),
            meshes: SharedMeshes::default(),
            look: Look::default(),
            skies: None,
            pending: Vec::new(),
            format: format.add_srgb_suffix(),
        }
    }

    /// Has Bevy draw `drawing`'s view, `size` pixels, and returns the picture; `None` while
    /// Bevy has not made it yet.
    #[cfg(test)]
    pub(crate) fn draw(&mut self, drawing: &Drawing, size: [u32; 2]) -> Option<wgpu::Texture> {
        self.draw_all(&[(drawing, size)])
            .pop()
            .flatten()
            .map(|(colour, _)| colour)
    }

    /// Takes one view of this frame. Bevy draws all of a frame's views together, in one update,
    /// when the last of them comes ([`Drawing::bevy_last`]): then it returns every view's
    /// picture, with its drawing, its place and its depth; before that, nothing.
    pub(crate) fn turn(
        &mut self,
        drawing: &Drawing,
        size: [u32; 2],
        low: [u32; 2],
    ) -> Vec<BevyPicture> {
        self.pending.retain(|(frame, ..)| *frame == drawing.frame);
        self.pending
            .push((drawing.frame, drawing.clone(), size, low));
        if !drawing.bevy_last {
            return Vec::new();
        }
        let pending = std::mem::take(&mut self.pending);
        let views: Vec<(&Drawing, [u32; 2])> = pending
            .iter()
            .map(|(_, drawing, size, _)| (drawing, *size))
            .collect();
        let pictures = self.draw_all(&views);
        pending
            .into_iter()
            .zip(pictures)
            .filter_map(|((_, drawing, _, low), picture)| {
                picture.map(|(colour, depth)| BevyPicture {
                    drawing,
                    low,
                    colour,
                    depth,
                })
            })
            .collect()
    }

    /// Has Bevy draw each of `views`, at its size, in one update, and returns their pictures and
    /// depths in order; `None` for one Bevy has not made yet.
    fn draw_all(
        &mut self,
        views: &[(&Drawing, [u32; 2])],
    ) -> Vec<Option<(wgpu::Texture, Option<wgpu::Texture>)>> {
        let Ok(mut apps) = self.apps.lock() else {
            return vec![None; views.len()];
        };
        let world = apps.main.world_mut();
        let skies = self
            .skies
            .get_or_insert_with(|| scene::skies(&mut world.resource_mut::<Assets<Image>>()));
        let sky = skies[usize::from(self.look.lighting == Lighting::Sunlit)].clone();
        let mut fresh = false;
        for (drawing, size) in views {
            let size = size.map(|side| side.max(1));
            let slot = drawing.slot;
            fresh |= !self.views.contains_key(&slot);
            let view = self.views.entry(slot).or_insert_with(|| {
                // Render layer 0 is Bevy's default; each view gets its own above it.
                let scene = ViewScene::new(world, slot + 1);
                let (camera, target) = make_camera(world, size, self.format, scene.layer());
                PilotView {
                    camera,
                    target,
                    size,
                    scene,
                    placed: None,
                    lit: None,
                }
            });
            if view.size != size {
                fresh = true;
                world.resource_mut::<Assets<Image>>().remove(&view.target);
                world.despawn(view.camera);
                (view.camera, view.target) =
                    make_camera(world, size, self.format, view.scene.layer());
                view.size = size;
                view.placed = None;
                view.lit = None;
            }
            view.scene
                .update(world, &mut self.meshes, drawing, self.look);
            let lens = drawing.camera.lens();
            if view.placed != Some((lens, size)) {
                view.placed = Some((lens, size));
                world.entity_mut(view.camera).insert(look(&lens, size));
            }
            if view.lit != Some(self.look) {
                view.lit = Some(self.look);
                world.entity_mut(view.camera).insert((
                    AmbientLight {
                        brightness: scene::ambient(self.look),
                        ..default()
                    },
                    sky.clone(),
                ));
            }
            let background = drawing.background;
            let colour = Color::srgb_u8(background.r(), background.g(), background.b());
            if let Some(mut camera) = world.get_mut::<Camera>(view.camera)
                && (!matches!(camera.clear_color, ClearColorConfig::Custom(c) if c == colour)
                    || !camera.is_active)
            {
                camera.clear_color = ClearColorConfig::Custom(colour);
                camera.is_active = true;
            }
        }
        // Only this frame's views are drawn.
        for (slot, view) in &self.views {
            let drawn = views.iter().any(|(drawing, _)| drawing.slot == *slot);
            if let Some(mut camera) = world.get_mut::<Camera>(view.camera)
                && camera.is_active != drawn
            {
                camera.is_active = drawn;
            }
        }
        apps.update();
        // A new picture reaches the card during the first update and is drawn in the next.
        if fresh {
            apps.update();
        }
        let main = apps.main.world();
        let render = apps
            .sub_apps
            .get(&RenderApp.intern())
            .map(bevy::app::SubApp::world);
        let images = render.and_then(|render| render.get_resource::<RenderAssets<GpuImage>>());
        views
            .iter()
            .map(|(drawing, _)| {
                let view = self.views.get(&drawing.slot)?;
                let colour = images?.get(&view.target)?.texture.clone();
                // The depth Bevy drew this view with, in its render world.
                let depth = main
                    .get::<RenderEntity>(view.camera)
                    .and_then(|entity| render?.get::<ViewDepthStencilTexture>(entity.id()))
                    .map(|depth| (**depth.texture()).clone());
                Some(((*colour).clone(), depth))
            })
            .collect()
    }
}

/// A view Bevy drew this frame: its drawing, its top left corner on the scene texture, its
/// picture, and its depth (four samples per pixel, reversed: near is 1).
pub(crate) struct BevyPicture {
    pub(crate) drawing: Drawing,
    pub(crate) low: [u32; 2],
    pub(crate) colour: wgpu::Texture,
    pub(crate) depth: Option<wgpu::Texture>,
}

/// A camera on render layer `layer` drawing into a new picture of `size`.
fn make_camera(
    world: &mut World,
    size: [u32; 2],
    format: TextureFormat,
    layer: RenderLayers,
) -> (Entity, Handle<Image>) {
    let mut image = Image::new_target_texture(size[0], size[1], format, None);
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    // Copied into the scene texture after Bevy draws it.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = world.resource_mut::<Assets<Image>>().add(image);
    // Colours as Curva writes them: no tone mapping or dithering changing the stored values.
    let camera = world
        .spawn((
            Camera3d {
                // Curva reads Bevy's depth, to hide its lines behind Bevy's surfaces.
                depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING)
                    .into(),
                ..default()
            },
            // Four samples per pixel, which the depth handover reads (`gpu/depth.rs`).
            Msaa::Sample4,
            RenderTarget::Image(target.clone().into()),
            Tonemapping::None,
            DebandDither::Disabled,
            layer,
        ))
        .id();
    (camera, target)
}

/// Curva's camera as Bevy's: the same eye, direction, lens and depth range.
fn look(lens: &Lens, size: [u32; 2]) -> (Transform, Projection) {
    let v = |p: [f64; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
    let transform =
        Transform::from_translation(v(lens.eye)).looking_to(v(lens.forward), v(lens.up));
    let aspect = size[0] as f32 / size[1] as f32;
    let projection = if lens.parallel {
        let height = 2.0 * lens.half_height as f32;
        Projection::Orthographic(OrthographicProjection {
            near: lens.near as f32,
            far: lens.far as f32,
            scaling_mode: ScalingMode::Fixed {
                width: height * aspect,
                height,
            },
            ..OrthographicProjection::default_3d()
        })
    } else {
        Projection::Perspective(PerspectiveProjection {
            fov: lens.fov as f32,
            aspect_ratio: aspect,
            near: lens.near as f32,
            far: lens.far as f32,
            ..default()
        })
    };
    (transform, projection)
}

/// How many redraws [`save_bevy_picture`] times.
pub const TIMED_FRAMES: usize = 120;

/// Draws `picture` as a view Bevy draws, through the window's own path: Bevy's surfaces in
/// `look` on `background`, with Curva's lines, outlines and previews on them, seen by `camera`
/// (`picture.matrix` must be made from it), its faces numbered by `objects` as
/// [`crate::SceneParts`] numbers them. Saves it as a PNG file at `path`, `size` pixels, and
/// returns the median time to draw one frame, in milliseconds, over [`TIMED_FRAMES`] redraws.
pub fn save_bevy_picture(
    picture: crate::Picture,
    objects: Vec<u64>,
    camera: crate::Camera,
    look: Look,
    background: epaint::Color32,
    size: [u32; 2],
    path: &std::path::Path,
) -> Result<f64, String> {
    use egui_wgpu::CallbackTrait;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|error| error.to_string())?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, true)))
            .map_err(|error| error.to_string())?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let seed = PilotSeed { instance, adapter };
    let mut pilot = Pilot::new(&seed, &device, &queue, format);
    pilot.look = look;
    let mut resources = egui_wgpu::CallbackResources::default();
    resources.insert(crate::LineRenderer::new(&device, format));
    resources.insert(pilot);
    let mut drawing = Drawing {
        slot: 0,
        grid: Arc::default(),
        scene: Arc::new(picture.lines),
        faces: Arc::new(picture.faces),
        objects: Arc::new(objects),
        selection: Arc::default(),
        overlay: Arc::default(),
        preview_faces: Arc::default(),
        matrix: picture.matrix,
        light: picture.light,
        clips: Vec::new(),
        shaded: true,
        ghosted: picture.ghosted,
        zebra: false,
        draft: false,
        area: epaint::Rect::from_min_size(
            epaint::Pos2::ZERO,
            epaint::vec2(size[0] as f32, size[1] as f32),
        ),
        frame: 0,
        background,
        background_low: background,
        engine: Engine::BevyPilot,
        camera,
        bevy_last: true,
    };
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: size,
        pixels_per_point: 1.0,
    };
    let frame = |drawing: &Drawing, resources: &mut egui_wgpu::CallbackResources| {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let extra = drawing.prepare(&device, &queue, &screen, &mut encoder, resources);
        queue.submit(extra.into_iter().chain([encoder.finish()]));
    };
    // Bevy's first picture of a new view comes ready on its second update.
    for _ in 0..2 {
        frame(&drawing, &mut resources);
        drawing.frame += 1;
    }
    let renderer = resources
        .get_mut::<crate::LineRenderer>()
        .ok_or("no line renderer")?;
    let target = renderer.compositor.target(&device, size);
    let pixels = crate::gpu::read_texture(&device, &queue, target.colour.texture(), size);
    let mut times = Vec::with_capacity(TIMED_FRAMES);
    for _ in 0..TIMED_FRAMES {
        let started = std::time::Instant::now();
        frame(&drawing, &mut resources);
        drawing.frame += 1;
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| error.to_string())?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())?;
    Ok(times.get(TIMED_FRAMES / 2).copied().unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{MeshVertex, NO_OBJECT, StandardView};

    const BACKGROUND: epaint::Color32 = epaint::Color32::from_rgb(200, 120, 40);
    const SIZE: [u32; 2] = [64, 48];

    /// Curva runs one Bevy at a time; tests that start Bevy take turns too, as two at once crash
    /// some graphics (Windows' software graphics, on which the checks run).
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        TURN.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Bevy on a device made as Curva makes its own; `None` on a machine with no graphics.
    fn pilot() -> Option<(Pilot, wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, true)))
                .ok()?;
        let seed = PilotSeed { instance, adapter };
        let pilot = Pilot::new(&seed, &device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        Some((pilot, device, queue))
    }

    /// A square much larger than the view, through the origin, facing along `normal`, as object
    /// number 0.
    fn square(normal: [f32; 3], colour: [f32; 3]) -> Vec<MeshVertex> {
        let n = Vec3::from_array(normal);
        let a = n.any_orthonormal_vector() * 1.0e5;
        let b = n.cross(a);
        let corner = |p: Vec3| MeshVertex {
            position: p.to_array(),
            normal,
            colour,
            object: 0,
        };
        [-a - b, a - b, a + b, -a - b, a + b, -a + b]
            .into_iter()
            .map(corner)
            .collect()
    }

    /// A view of `faces` from `view`, as the window would hand it over.
    fn drawing(view: StandardView, faces: Vec<MeshVertex>) -> Drawing {
        let camera = view.camera();
        Drawing {
            slot: 0,
            grid: Arc::default(),
            scene: Arc::default(),
            faces: Arc::new(faces),
            objects: Arc::new(vec![7]),
            selection: Arc::default(),
            overlay: Arc::default(),
            preview_faces: Arc::default(),
            matrix: camera.matrix(f64::from(SIZE[0]) / f64::from(SIZE[1])),
            light: camera.forward().map(|value| value as f32),
            clips: Vec::new(),
            shaded: true,
            ghosted: false,
            zebra: false,
            draft: false,
            area: epaint::Rect::from_min_size(
                epaint::Pos2::ZERO,
                epaint::vec2(SIZE[0] as f32, SIZE[1] as f32),
            ),
            frame: 0,
            background: BACKGROUND,
            background_low: BACKGROUND,
            engine: Engine::BevyPilot,
            camera,
            bevy_last: true,
        }
    }

    /// The middle pixel of Bevy's picture of `drawing`.
    fn middle(
        pilot: &mut Pilot,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawing: &Drawing,
    ) -> [u8; 4] {
        let texture = pilot.draw(drawing, SIZE).expect("Bevy's picture");
        assert_eq!([texture.width(), texture.height()], SIZE);
        let pixels = crate::gpu::read_texture(device, queue, &texture, SIZE);
        let at = ((SIZE[1] / 2 * SIZE[0] + SIZE[0] / 2) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
    }

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

    /// With nothing to show, a view's picture is its background colour, made by Bevy on a device
    /// Curva made; a new size makes a new picture.
    #[test]
    fn pilot_draws_the_background_on_curvas_device() {
        let _turn = one_at_a_time();
        let Some((mut pilot, device, queue)) = pilot() else {
            return;
        };
        let empty = drawing(StandardView::Top, Vec::new());
        let texture = pilot.draw(&empty, SIZE).expect("Bevy's picture");
        let pixels = crate::gpu::read_texture(&device, &queue, &texture, SIZE);
        for pixel in pixels.chunks(4) {
            assert_eq!(pixel, [200, 120, 40, 255]);
        }
        let texture = pilot.draw(&empty, [32, 32]).expect("resized");
        assert_eq!([texture.width(), texture.height()], [32, 32]);
    }

    /// Behind the scene is Curva's own background: its top colour at the top, its bottom colour
    /// at the bottom, blended between as Curva blends it.
    #[test]
    fn views_keep_curvas_background() {
        let _turn = one_at_a_time();
        let Some((mut pilot, device, queue)) = pilot() else {
            return;
        };
        let mut view = drawing(StandardView::Perspective, Vec::new());
        view.background = epaint::Color32::from_rgb(200, 210, 220);
        view.background_low = epaint::Color32::from_rgb(40, 50, 60);
        let texture = pilot.draw(&view, SIZE).expect("Bevy's picture");
        let pixels = crate::gpu::read_texture(&device, &queue, &texture, SIZE);
        let at = |row: u32| {
            let i = ((row * SIZE[0] + SIZE[0] / 2) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        let near = |a: [u8; 3], b: [u8; 3], by: u8| (0..3).all(|c| a[c].abs_diff(b[c]) <= by);
        assert!(near(at(0), [200, 210, 220], 6), "top {:?}", at(0));
        assert!(
            near(at(SIZE[1] - 1), [40, 50, 60], 6),
            "bottom {:?}",
            at(SIZE[1] - 1)
        );
        assert!(
            near(at(SIZE[1] / 2), [120, 130, 140], 6),
            "middle {:?}",
            at(SIZE[1] / 2)
        );
    }

    /// Curva's lines on a view Bevy draws are hidden behind Bevy's surfaces exactly as behind
    /// Curva's own: seen from above, a red line just above a grey face shows, a blue line just
    /// below it does not. Drawn through the window's own path (`Drawing::prepare`).
    #[test]
    fn lines_on_bevy_views_hide_behind_bevy_surfaces() {
        use egui_wgpu::CallbackTrait;
        let _turn = one_at_a_time();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter, true)))
                .expect("device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut resources = egui_wgpu::CallbackResources::default();
        resources.insert(crate::LineRenderer::new(&device, format));
        resources.insert(PilotSeed { instance, adapter });
        let mut lines = Vec::new();
        let (red, blue) = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        crate::lines::segment_with(&mut lines, [-1.0e5, 0.0, 10.0], [0.0, 0.0, 10.0], red, 4.0);
        crate::lines::segment_with(
            &mut lines,
            [0.0, 0.0, -10.0],
            [1.0e5, 0.0, -10.0],
            blue,
            4.0,
        );
        let mut view = drawing(StandardView::Top, square([0.0, 0.0, 1.0], [0.6; 3]));
        view.scene = Arc::new(lines);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: SIZE,
            pixels_per_point: 1.0,
        };
        // Twice: Bevy's first picture of a new view comes ready on its second update.
        for frame in 0..2 {
            view.frame = frame;
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            let extra = view.prepare(&device, &queue, &screen, &mut encoder, &mut resources);
            queue.submit(extra.into_iter().chain([encoder.finish()]));
        }
        let renderer = resources.get_mut::<crate::LineRenderer>().unwrap();
        let target = renderer.compositor.target(&device, SIZE);
        let pixels = crate::gpu::read_texture(&device, &queue, target.colour.texture(), SIZE);
        let row = SIZE[1] / 2;
        let along = |x: u32| {
            let i = ((row * SIZE[0] + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        let left = (0..SIZE[0] / 2)
            .map(along)
            .filter(|p| p[0] > 200 && p[2] < 80)
            .count();
        let right = (SIZE[0] / 2..SIZE[0])
            .map(along)
            .filter(|p| p[2] > 200 && p[0] < 80)
            .count();
        assert!(left > 10, "the red line above the face should show");
        assert_eq!(right, 0, "the blue line below the face should be hidden");
        // Raised above the face, the blue line shows: it was hidden by the face, not missing.
        let mut lines = Vec::new();
        crate::lines::segment_with(&mut lines, [0.0, 0.0, 10.0], [1.0e5, 0.0, 10.0], blue, 4.0);
        view.scene = Arc::new(lines);
        view.frame = 2;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let extra = view.prepare(&device, &queue, &screen, &mut encoder, &mut resources);
        queue.submit(extra.into_iter().chain([encoder.finish()]));
        let renderer = resources.get_mut::<crate::LineRenderer>().unwrap();
        let target = renderer.compositor.target(&device, SIZE);
        let pixels = crate::gpu::read_texture(&device, &queue, target.colour.texture(), SIZE);
        let shown = (SIZE[0] / 2..SIZE[0])
            .map(|x| {
                let i = ((row * SIZE[0] + x) * 4) as usize;
                [pixels[i], pixels[i + 1], pixels[i + 2]]
            })
            .filter(|p| p[2] > 200 && p[0] < 80)
            .count();
        assert!(shown > 10, "the blue line above the face should show");
    }

    /// A clipping plane cuts Bevy's surfaces as it cuts Curva's: a grey face seen from above,
    /// cut by a plane through the middle, shows the background where it is cut away.
    #[test]
    fn clipping_planes_cut_bevy_surfaces() {
        let Some((mut pilot, device, queue)) = pilot() else {
            return;
        };
        let mut view = drawing(StandardView::Top, square([0.0, 0.0, 1.0], [0.6; 3]));
        // Cuts away where x > 0: the right half of the picture seen from above.
        view.clips = vec![[1.0, 0.0, 0.0, 0.0]];
        let texture = pilot.draw(&view, SIZE).expect("Bevy's picture");
        let pixels = crate::gpu::read_texture(&device, &queue, &texture, SIZE);
        let at = |x: u32| {
            let i = (((SIZE[1] / 2) * SIZE[0] + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        let (kept, cut) = (at(SIZE[0] / 4), at(SIZE[0] * 3 / 4));
        assert_eq!(kept[0], kept[2], "the kept half is the grey face: {kept:?}");
        assert_eq!(cut, [200, 120, 40], "the cut half shows the background");
    }

    /// In the Studio look a white face is light grey, as in Rhino's Shaded, but not burnt out:
    /// lighter seen from above (lit from the viewer, from above and by the sky) than from the
    /// front, and grey, not tinted.
    #[test]
    fn faces_are_lit_brightly_but_not_burnt_out() {
        let _turn = one_at_a_time();
        let Some((mut pilot, device, queue)) = pilot() else {
            return;
        };
        let top = middle(
            &mut pilot,
            &device,
            &queue,
            &drawing(StandardView::Top, square([0.0, 0.0, 1.0], [1.0; 3])),
        );
        let front = middle(
            &mut pilot,
            &device,
            &queue,
            &drawing(StandardView::Front, square([0.0, -1.0, 0.0], [1.0; 3])),
        );
        println!("top {top:?}, front {front:?}");
        for (pixel, range) in [(top, 210..=250), (front, 175..=230)] {
            assert!(range.contains(&pixel[0]), "{pixel:?} outside {range:?}");
            assert!(
                pixel[0] == pixel[1] && pixel[1] == pixel[2],
                "tinted: {pixel:?}"
            );
        }
        assert!(
            top[0] > front[0],
            "top {top:?} not lighter than front {front:?}"
        );
    }

    /// Selecting turns a face the selection colour; Wireframe hides it; Ghosted lets the
    /// background show through.
    #[test]
    fn faces_follow_selection_and_display_mode() {
        let _turn = one_at_a_time();
        let Some((mut pilot, device, queue)) = pilot() else {
            return;
        };
        let grey = [0.6; 3];
        let mut view = drawing(StandardView::Top, square([0.0, 0.0, 1.0], grey));
        let plain = middle(&mut pilot, &device, &queue, &view);
        assert_eq!(plain[0], plain[2], "grey: {plain:?}");
        view.selection = Arc::new(vec![7]);
        let picked = middle(&mut pilot, &device, &queue, &view);
        assert!(picked[0] > picked[2] + 60, "selection colour: {picked:?}");
        view.selection = Arc::default();
        view.ghosted = true;
        let ghosted = middle(&mut pilot, &device, &queue, &view);
        assert!(
            ghosted[0] > plain[0] && ghosted[2] < plain[2],
            "background through: {ghosted:?}"
        );
        view.shaded = false;
        assert_eq!(
            middle(&mut pilot, &device, &queue, &view),
            [200, 120, 40, 255]
        );
        // Faces with no object number still draw.
        let mut loose = square([0.0, 0.0, 1.0], grey);
        for corner in &mut loose {
            corner.object = NO_OBJECT;
        }
        let shown = middle(
            &mut pilot,
            &device,
            &queue,
            &drawing(StandardView::Top, loose),
        );
        assert_eq!(shown, plain);
    }
}
