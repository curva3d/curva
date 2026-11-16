//! Spike for #198: Bevy's renderer (bevy_render, bevy_core_pipeline, bevy_pbr) inside Curva's 3D
//! views. Curva makes the graphics device and hands it to Bevy (`RenderCreation::manual`); Bevy
//! draws into a texture, as a view in Curva's window would show; Curva owns the loop and calls
//! Bevy's update once per frame; Curva's camera drives Bevy's. Not part of Curva's build.
//!
//! Run: `cargo run --release -- <output folder> pictures` or `... orbit <scene> <mode>`, mode
//! `plain` (four samples per pixel), `full` (SSAO, TAA, occlusion culling) or `fallback`
//! (the card's optional extras off, decision 0026).

#[path = "../../common/scenes.rs"]
mod scenes;

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    app::SubApps,
    asset::RenderAssetUsages,
    camera::RenderTarget,
    core_pipeline::prepass::DepthPrepass,
    diagnostic::DiagnosticsStore,
    gizmos::{GizmoAsset, config::GizmoLineConfig, config::GizmoLineStyle, retained::Gizmo},
    image::Image,
    mesh::{Indices, PrimitiveTopology},
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
    render::{
        RenderPlugin,
        diagnostic::RenderDiagnosticsPlugin,
        occlusion_culling::OcclusionCulling,
        render_resource::{PollType, TextureFormat},
        renderer::{RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue},
        settings::RenderCreation,
        view::screenshot::{Screenshot, save_to_disk},
    },
    window::ExitCondition,
};
use scenes::{Point, Scene};
use std::time::Instant;

/// The orbit picture: 1920 by 1080, or with SMALL=1 the 1280 by 800 Curva's window has here.
fn size() -> [u32; 2] {
    if std::env::var("SMALL").is_ok_and(|v| v == "1") {
        [1280, 800]
    } else {
        [1920, 1080]
    }
}
const PICTURE: [u32; 2] = [1200, 800];
const FRAMES: usize = 240;
const FOV: f32 = 0.7;
/// Lines per kept line asset.
const PIECE: usize = 4_000;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Plain,
    Full,
    Fallback,
}

/// Where positions are measured from on the card. Bevy positions are single precision; Curva
/// subtracts this point in double precision first. `None`: the world origin, as Bevy takes them.
#[derive(Clone, Copy)]
struct Origin(Option<Point>);

impl Origin {
    fn rel(self, p: Point) -> Vec3 {
        let o = self.0.unwrap_or([0.0; 3]);
        Vec3::new(
            (p[0] - o[0]) as f32,
            (p[1] - o[1]) as f32,
            (p[2] - o[2]) as f32,
        )
    }
}

struct Viewer {
    apps: SubApps,
    target: Handle<Image>,
    camera: Entity,
    origin: Origin,
    size: [u32; 2],
    first_solid: Option<Entity>,
}

/// The graphics device: Curva's own (`rich` false), with or without its optional extras; or with
/// everything the card offers, as Bevy asks by default and as Curva could on recommended cards
/// (decision 0026).
fn device(extras: bool, rich: bool) -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    let mut descriptor = curva_viewport::device_descriptor(&adapter, extras);
    if rich {
        descriptor.required_features =
            adapter.features()
                - wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
                - wgpu::Features::all_experimental_mask();
        descriptor.required_limits = adapter.limits();
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).unwrap();
    (instance, adapter, device, queue)
}

impl Viewer {
    fn new(scene: &Scene, mode: Mode, size: [u32; 2], origin: Origin) -> Self {
        let (instance, adapter, device, queue) = device(mode != Mode::Fallback, mode == Mode::Full);
        let info = adapter.get_info();
        let render = RenderPlugin {
            render_creation: RenderCreation::manual(
                RenderDevice::new(device),
                RenderQueue::new(queue),
                RenderAdapterInfo::new(info),
                RenderAdapter::new(adapter),
                RenderInstance::new(instance),
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
        app.add_plugins(DefaultPlugins.set(window).set(render))
            .add_plugins(RenderDiagnosticsPlugin)
            .insert_resource(ClearColor(Color::WHITE))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 600.0,
                ..default()
            });
        app.finish();
        app.cleanup();
        let mut apps = std::mem::take(app.sub_apps_mut());
        let world = apps.main.world_mut();

        let mut image =
            Image::new_target_texture(size[0], size[1], TextureFormat::Rgba8UnormSrgb, None);
        image.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let target = world.resource_mut::<Assets<Image>>().add(image);

        let first_solid = mirror(world, scene, origin);
        let (low, high) = scene.bounds();
        let size_of_scene = (0..3).map(|a| (high[a] - low[a]).powi(2)).sum::<f64>().sqrt();

        let mut camera = world.spawn((
            Camera3d::default(),
            RenderTarget::Image(target.clone().into()),
            Projection::Perspective(PerspectiveProjection::default()),
            Transform::IDENTITY,
        ));
        match mode {
            Mode::Plain | Mode::Fallback => {
                camera.insert(Msaa::Sample4);
            }
            Mode::Full => {
                // SSAO and TAA both need one sample per pixel; TAA smooths the edges instead.
                camera.insert((
                    Msaa::Off,
                    ScreenSpaceAmbientOcclusion {
                        // Bevy's default suits metres; Curva works in millimetres.
                        radius: (size_of_scene / 40.0) as f32,
                        constant_object_thickness: (size_of_scene / 200.0) as f32,
                        ..default()
                    },
                    TemporalAntiAliasing::default(),
                    DepthPrepass,
                    OcclusionCulling,
                ));
            }
        }
        let camera = camera.id();
        world.spawn((
            DirectionalLight {
                illuminance: 6000.0,
                shadow_maps_enabled: mode == Mode::Full,
                ..default()
            },
            Transform::from_xyz(-0.4, -0.7, 1.0).looking_at(Vec3::ZERO, Vec3::Z),
            bevy::light::CascadeShadowConfigBuilder {
                maximum_distance: (size_of_scene * 3.0) as f32,
                first_cascade_far_bound: (size_of_scene * 0.5) as f32,
                minimum_distance: (size_of_scene * 0.001) as f32,
                ..default()
            }
            .build(),
        ));
        Viewer {
            apps,
            target,
            camera,
            origin,
            size,
            first_solid,
        }
    }

    /// Curva's camera as Bevy's: the same eye, target, field of view and depth range.
    fn look(&mut self, c: &curva_viewport::Camera) {
        let (cp, sp) = (c.pitch.cos(), c.pitch.sin());
        let eye = [
            c.target[0] + c.distance * cp * c.yaw.cos(),
            c.target[1] + c.distance * cp * c.yaw.sin(),
            c.target[2] + c.distance * sp,
        ];
        let transform = Transform::from_translation(self.origin.rel(eye))
            .looking_at(self.origin.rel(c.target), Vec3::Z);
        let projection = Projection::Perspective(PerspectiveProjection {
            fov: FOV,
            near: (c.distance * 0.01) as f32,
            far: (c.distance * 100.0) as f32,
            aspect_ratio: self.size[0] as f32 / self.size[1] as f32,
            ..default()
        });
        let world = self.apps.main.world_mut();
        world
            .entity_mut(self.camera)
            .insert((transform, projection));
    }

    fn update(&mut self) {
        self.apps.update();
        self.apps
            .main
            .world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .poll(PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
    }

    fn picture(&mut self, path: &std::path::Path) {
        let shot = Screenshot::image(self.target.clone());
        let path = path.to_owned();
        self.apps
            .main
            .world_mut()
            .spawn(shot)
            .observe(save_to_disk(path));
        // A few updates: the copy back from the card arrives later.
        for _ in 0..4 {
            self.update();
        }
    }

    fn gpu_bytes(&self) -> Option<u64> {
        self.apps
            .main
            .world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .generate_allocator_report()
            .map(|r| r.total_allocated_bytes)
    }

    /// The card's time for the last frame: the sum of Bevy's top-level timed passes.
    fn gpu_ms(&self) -> Option<f64> {
        let store = self.apps.main.world().resource::<DiagnosticsStore>();
        let mut sum = 0.0;
        let mut any = false;
        for d in store.iter() {
            let path = d.path().as_str();
            if path.ends_with("/elapsed_gpu")
                && path.matches('/').count() == 2
                && let Some(v) = d.value()
            {
                sum += v;
                any = true;
            }
        }
        any.then_some(sum)
    }

    fn gpu_passes(&self) -> Vec<String> {
        let store = self.apps.main.world().resource::<DiagnosticsStore>();
        store
            .iter()
            .filter(|d| d.path().as_str().ends_with("elapsed_gpu"))
            .map(|d| format!("{} {:.2}", d.path().as_str(), d.value().unwrap_or(f64::NAN)))
            .collect()
    }
}

/// The translation layer: Curva's shapes, placements and lines as Bevy assets and entities.
/// Returns the first solid, for the selection test.
fn mirror(world: &mut World, scene: &Scene, origin: Origin) -> Option<Entity> {
    let meshes: Vec<Handle<Mesh>> = scene
        .shapes
        .iter()
        .map(|m| {
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                m.positions
                    .iter()
                    .map(|p| p.map(|v| v as f32))
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                m.normals
                    .iter()
                    .map(|p| p.map(|v| v as f32))
                    .collect::<Vec<_>>(),
            )
            .with_inserted_indices(Indices::U32(
                m.triangles.iter().flatten().copied().collect(),
            ));
            world.resource_mut::<Assets<Mesh>>().add(mesh)
        })
        .collect();
    let (white, green) = {
        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        let look = |c: Color| StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.9,
            reflectance: 0.1,
            ..default()
        };
        (
            mats.add(look(Color::srgb(0.92, 0.92, 0.92))),
            mats.add(look(Color::srgb(0.55, 0.7, 0.5))),
        )
    };
    let first_tree = scene.placed.len() - scene.tree_parts;
    let mut first = None;
    for (i, (shape, at)) in scene.placed.iter().enumerate() {
        let material = if i >= first_tree {
            green.clone()
        } else {
            white.clone()
        };
        let e = world
            .spawn((
                Mesh3d(meshes[*shape].clone()),
                MeshMaterial3d(material),
                Transform::from_translation(origin.rel(*at)),
            ))
            .id();
        first.get_or_insert(e);
    }
    let mut strips = scene.curves.clone();
    strips.extend(scene.edges());
    for piece in strips.chunks(PIECE) {
        let mut asset = GizmoAsset::new();
        for s in piece {
            asset.linestrip(s.iter().map(|p| origin.rel(*p)), Color::srgb(0.1, 0.1, 0.1));
        }
        let handle = world.resource_mut::<Assets<GizmoAsset>>().add(asset);
        world.spawn(Gizmo {
            handle,
            line_config: GizmoLineConfig {
                width: 2.0,
                ..default()
            },
            // A little nearer than the faces, so edges lying on faces show.
            depth_bias: -0.0002,
        });
    }
    first
}

fn framed(scene: &Scene, zoom: f64) -> curva_viewport::Camera {
    let (low, high) = scene.bounds();
    let mut c = curva_viewport::Camera::default();
    c.frame(low, high);
    c.zoom(zoom);
    c
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    match args.get(2).map_or("pictures", String::as_str) {
        "pictures" => pictures(&out),
        "orbit" => {
            let name = args.get(3).map_or("medium", String::as_str);
            let mode = match args.get(4).map_or("plain", String::as_str) {
                "full" => Mode::Full,
                "fallback" => Mode::Fallback,
                _ => Mode::Plain,
            };
            orbit(name, mode, &out);
        }
        other => panic!("unknown {other}"),
    }
}

fn pictures(out: &std::path::Path) {
    for (name, mode) in [
        ("building", Mode::Plain),
        ("building", Mode::Full),
        ("house", Mode::Full),
    ] {
        let scene = scenes::named(name);
        let mut v = Viewer::new(&scene, mode, PICTURE, Origin(None));
        v.look(&framed(&scene, 0.74));
        for _ in 0..8 {
            v.update();
        }
        v.picture(&out.join(format!("bevy-{name}-{mode:?}.png").to_lowercase()));
        let mut near = curva_viewport::Camera::default();
        near.frame([0.0, 0.0, 0.0], [12000.0, 8000.0, 3000.0]);
        near.zoom(0.7);
        v.look(&near);
        // TAA settles over several frames.
        for _ in 0..8 {
            v.update();
        }
        v.picture(&out.join(format!("bevy-{name}-{mode:?}-near.png").to_lowercase()));
    }
    // Lines up close, and a circle zoomed far in.
    let scene = scenes::named("medium");
    let mut v = Viewer::new(&scene, Mode::Plain, PICTURE, Origin(None));
    {
        // A wide red line and a dashed blue one, as viewport-lib's test drew.
        let world = v.apps.main.world_mut();
        for (curve, width, style, colour) in [
            (1, 6.0, GizmoLineStyle::Solid, Color::srgb(0.8, 0.1, 0.1)),
            (
                2,
                2.0,
                GizmoLineStyle::Dashed {
                    gap_scale: 3.0,
                    line_scale: 5.0,
                },
                Color::srgb(0.1, 0.2, 0.8),
            ),
        ] {
            let mut asset = GizmoAsset::new();
            asset.linestrip(
                scene.curves[curve].iter().map(|p| Origin(None).rel(*p)),
                colour,
            );
            let handle = world.resource_mut::<Assets<GizmoAsset>>().add(asset);
            world.spawn(Gizmo {
                handle,
                line_config: GizmoLineConfig {
                    width,
                    style,
                    ..default()
                },
                depth_bias: -0.001,
            });
        }
    }
    let mut close = curva_viewport::Camera::default();
    close.frame([0.0, 0.0, 0.0], [30.0, 30.0, 5.0]);
    v.look(&close);
    v.update();
    v.picture(&out.join("bevy-lines-close.png"));
    let mut zoomed = curva_viewport::Camera::default();
    zoomed.frame([4.0, -1.0, 0.0], [6.0, 1.0, 0.0]);
    v.look(&zoomed);
    v.update();
    v.picture(&out.join("bevy-arc-zoomed.png"));
    // Far from the origin: as Bevy takes positions, and with Curva subtracting a near point first.
    for (name, origin, tag) in [
        ("large", Origin(None), ""),
        ("far", Origin(None), ""),
        ("far", Origin(Some([5.0e8, 5.0e8, 0.0])), "-rebased"),
    ] {
        let scene = scenes::named(name);
        let shift = if name == "far" { 5.0e8 } else { 0.0 };
        let mut v = Viewer::new(&scene, Mode::Plain, PICTURE, origin);
        let mut close = curva_viewport::Camera::default();
        close.frame([shift, shift, 0.0], [shift + 40.0, shift + 40.0, 10.0]);
        v.look(&close);
        v.update();
        v.picture(&out.join(format!("bevy-{name}{tag}-close.png")));
    }
}

fn orbit(name: &str, mode: Mode, out: &std::path::Path) {
    let started = Instant::now();
    let scene = scenes::named(name);
    println!(
        "scene {name}: {} solids ({} shapes), {} triangles, built in {:.1} s; mode {mode:?}",
        scene.placed.len(),
        scene.shapes.len(),
        scene.triangles(),
        started.elapsed().as_secs_f64()
    );
    let started = Instant::now();
    let origin = if name == "far" {
        Origin(Some([5.0e8, 5.0e8, 0.0]))
    } else {
        Origin(None)
    };
    let mut v = Viewer::new(&scene, mode, size(), origin);
    let before = v.gpu_bytes();
    let mut cam = framed(&scene, 1.0);
    v.look(&cam);
    v.update();
    println!("first frame: {:.2} s", started.elapsed().as_secs_f64());
    for _ in 0..3 {
        v.update();
    }
    if let Some(b) = v.gpu_bytes() {
        println!(
            "graphics memory: {:.0} MB after the scene arrived ({:.0} MB before)",
            b as f64 / 1e6,
            before.unwrap_or(0) as f64 / 1e6
        );
    }
    let (mut times, mut gpu, mut select_times) = (Vec::new(), Vec::new(), Vec::new());
    let mut selected = false;
    let (white, orange) = {
        let world = v.apps.main.world_mut();
        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        (
            mats.add(Color::srgb(0.92, 0.92, 0.92)),
            mats.add(Color::srgb(1.0, 0.6, 0.1)),
        )
    };
    for k in 0..FRAMES {
        cam.orbit(4.0, 0.0);
        let changing = k % 10 == 0 && k > 0;
        let t = Instant::now();
        v.look(&cam);
        if changing {
            // Selection as a change of colour: Bevy has no selection outline of its own.
            selected = !selected;
            let first = v.first_solid.unwrap();
            let m = if selected {
                orange.clone()
            } else {
                white.clone()
            };
            v.apps
                .main
                .world_mut()
                .entity_mut(first)
                .insert(MeshMaterial3d(m));
        }
        v.update();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if changing {
            select_times.push(ms);
        } else if k >= 20 {
            times.push(ms);
        }
        if k >= 20
            && let Some(g) = v.gpu_ms()
        {
            gpu.push(g);
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(f64::NAN)
    };
    println!(
        "orbit: {:.2} ms per frame (median), card {:.2} ms; frames changing the selection {:.2} ms",
        median(&mut times),
        median(&mut gpu),
        median(&mut select_times)
    );
    if std::env::var("PASSES").is_ok() {
        for p in v.gpu_passes() {
            println!("  {p}");
        }
    }
    v.picture(&out.join(format!("bevy-orbit-{name}-{mode:?}.png").to_lowercase()));
}
