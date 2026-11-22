//! What each Bevy view shows: Curva's faces as Bevy entities, one copy per view on that view's
//! render layer, so each view has its own look (decision 0030: per-view looks with plain Bevy).
//! Copies share their meshes. Materials and lights are Bevy's own.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::PrimitiveTopology,
    prelude::*,
};

mod ground;
mod shapes;

use super::clip::{Clipping, Clips, CurvaMaterial};
use crate::{Drawing, MeshVertex};

/// How surfaces are lit (`views-and-styles.md` §2.1, the Surfaces layer's lighting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lighting {
    /// A soft light from the viewer and a little from above, whichever way the view turns.
    #[default]
    Studio,
    /// A sun from a fixed direction, casting shadows, and light from the sky.
    Sunlit,
}

/// How a view's surfaces look under Bevy: the lighting, and how much they shine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Which lights shine on the surfaces.
    pub lighting: Lighting,
    /// 0 is matte, 1 is polished.
    pub shine: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            lighting: Lighting::Studio,
            shine: 0.6,
        }
    }
}

/// How bright Bevy's lights are, in Bevy's units (lux, and its own for ambient light), set so a
/// white face is light but not burnt out (the test `faces_are_lit_brightly_but_not_burnt_out`).
const HEADLIGHT_LUX: f32 = 1_200.0;
const LIGHT_FROM_ABOVE_LUX: f32 = 800.0;
const STUDIO_AMBIENT: f32 = 0.0;
/// How far above and to the left of the viewer the headlight sits, as parts of the view's
/// direction.
const HEADLIGHT_RAISE: f32 = 0.35;
const HEADLIGHT_SHIFT: f32 = 0.25;
const SUN_LUX: f32 = 3_500.0;
const SKY_AMBIENT: f32 = 60.0;
/// The sun of the Sunlit look: from the south-west, 40 degrees above the horizon.
const SUN_FROM: [f32; 3] = [-0.54, -0.54, 0.64];
/// What surfaces reflect all around them, as Rhino's Shaded does: a light sky, a bright horizon
/// and a darker ground, which makes round surfaces read as round. Bevy's own gradient
/// environment (`EnvironmentMapLight::hemispherical_gradient`), turned so Z is up.
const STUDIO_SKY: [[f32; 3]; 3] = [[0.72, 0.72, 0.72], [1.0, 1.0, 1.0], [0.18, 0.18, 0.18]];
const OPEN_SKY: [[f32; 3]; 3] = [[0.62, 0.72, 0.86], [0.92, 0.93, 0.94], [0.40, 0.38, 0.35]];
const STUDIO_SKY_BRIGHTNESS: f32 = 150.0;
const OPEN_SKY_BRIGHTNESS: f32 = 120.0;
/// How much of what is behind a ghosted face shows through, as in Curva's Ghosted mode.
const GHOSTED_ALPHA: f32 = 0.5;
/// How much a command's preview covers what is behind it, as Curva's ghosted faces did.
const PREVIEW_ALPHA: f32 = 0.5;

/// A mesh's contents, as a fingerprint and a length: equal contents share one Bevy mesh.
type MeshKey = (u64, usize);
/// One object's faces of one colour: its number in the scene and the colour's bits.
type PartKey = (u32, [u32; 3]);
/// Corners and their normals, three per triangle.
type Triangles = (Vec<[f32; 3]>, Vec<[f32; 3]>);

/// Meshes shared by every view's copies, each with how many copies use it: loose triangles by
/// their contents, then the kernel's shared shapes.
#[derive(Default)]
pub(super) struct SharedMeshes(
    HashMap<MeshKey, (Handle<Mesh>, usize)>,
    pub(super) shapes::ShapeMeshes,
);

impl SharedMeshes {
    fn take(
        &mut self,
        world: &mut World,
        key: MeshKey,
        make: impl FnOnce() -> Mesh,
    ) -> Handle<Mesh> {
        if let Some((handle, users)) = self.0.get_mut(&key) {
            *users += 1;
            return handle.clone();
        }
        let handle = world.resource_mut::<Assets<Mesh>>().add(make());
        self.0.insert(key, (handle.clone(), 1));
        handle
    }

    fn give_back(&mut self, world: &mut World, key: MeshKey) {
        if let Some((handle, users)) = self.0.get_mut(&key) {
            *users -= 1;
            if *users == 0 {
                world.resource_mut::<Assets<Mesh>>().remove(&*handle);
                self.0.remove(&key);
            }
        }
    }
}

/// One object's faces of one colour, as shown in one view.
struct Part {
    mesh: MeshKey,
    entity: Entity,
}

/// One view's copy of the scene, its lights and its materials.
pub(super) struct ViewScene {
    layer: RenderLayers,
    root: Entity,
    headlight: Entity,
    from_above: Entity,
    sun: Entity,
    /// Curva's background behind the scene, and the two colours it was made with.
    backdrop: Entity,
    /// A command's live preview of the surfaces it makes, see-through, and the faces it shows.
    preview: Entity,
    preview_faces: Option<Arc<Vec<MeshVertex>>>,
    backdrop_colours: Option<(epaint::Color32, epaint::Color32)>,
    /// What was last handed to Bevy: whether faces show, and the view (its lens and shape).
    shaded: Option<bool>,
    view: Option<(crate::camera::Lens, u32)>,
    look: Option<Look>,
    faces: Option<Arc<Vec<MeshVertex>>>,
    selection: Option<Arc<Vec<u64>>>,
    ghosted: Option<bool>,
    /// How faces are coloured: their own colours, flat paper white (Hidden, #164) or white and lit
    /// (Clay, #551).
    hidden: Option<FaceLook>,
    /// The Clay display mode's ground, once a Clay view has shown it (#551).
    ground: Option<Entity>,
    parts: HashMap<PartKey, Part>,
    /// This view's material for each colour, so a style change touches only this view.
    materials: HashMap<[u32; 3], Handle<CurvaMaterial>>,
    /// The colour each part and copy is shown in now, so a new selection hands Bevy only the
    /// parts whose colour changes: handing it every part made it redo them all (step 5).
    shown: HashMap<Entity, [u32; 3]>,
    /// Each object's parts and copies, by its id (`None` for parts of no object), each with its
    /// own colour.
    by_object: HashMap<Option<u64>, Vec<(Entity, [u32; 3])>>,
    /// The objects shown selected.
    picked: HashSet<u64>,
    /// The planes this view's surfaces were last cut by.
    clips: Option<Clips>,
    /// The shared shapes last handed to Bevy, and each copy shown, by its object and its order
    /// within it.
    shapes: Option<Arc<Vec<crate::PlacedShape>>>,
    shape_parts: HashMap<(u32, u32), shapes::ShapePart>,
}

impl ViewScene {
    /// An empty scene on render layer `layer`, with this view's lights; [`ViewScene::update`]
    /// switches them for its look.
    pub(super) fn new(world: &mut World, layer: usize) -> Self {
        let layer = RenderLayers::layer(layer);
        let root = world.spawn((Transform::IDENTITY, Visibility::Visible)).id();
        let mut light = |transform: Transform| {
            world
                .spawn((DirectionalLight::default(), transform, layer.clone()))
                .id()
        };
        let headlight = light(Transform::IDENTITY);
        // A little light from above as well, so tops and sides of a box differ.
        let from_above = light(Transform::IDENTITY.looking_to(Vec3::NEG_Z, Vec3::Y));
        let sun = light(Transform::IDENTITY.looking_to(-Vec3::from_array(SUN_FROM), Vec3::Z));
        let flat = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                ..default()
            });
        // Previews are drawn in the preview colour, half see-through, as Curva's ghosted faces.
        let ghost = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::WHITE.with_alpha(PREVIEW_ALPHA),
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
        let preview = world
            .spawn((
                MeshMaterial3d(ghost),
                Transform::IDENTITY,
                Visibility::Hidden,
                layer.clone(),
                NotShadowCaster,
            ))
            .id();
        let backdrop = world
            .spawn((
                MeshMaterial3d(flat),
                Transform::IDENTITY,
                layer.clone(),
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        Self {
            layer,
            root,
            headlight,
            from_above,
            sun,
            backdrop,
            preview,
            preview_faces: None,
            backdrop_colours: None,
            shaded: None,
            view: None,
            look: None,
            faces: None,
            selection: None,
            ghosted: None,
            hidden: None,
            ground: None,
            parts: HashMap::new(),
            shapes: None,
            shape_parts: HashMap::new(),
            materials: HashMap::new(),
            shown: HashMap::new(),
            by_object: HashMap::new(),
            picked: HashSet::new(),
            clips: None,
        }
    }

    /// The render layer this view's camera sees.
    pub(super) fn layer(&self) -> RenderLayers {
        self.layer.clone()
    }

    /// Brings this view's scene up to `drawing`: only what changed since the last frame.
    pub(super) fn update(
        &mut self,
        world: &mut World,
        meshes: &mut SharedMeshes,
        drawing: &Drawing,
        look: Look,
    ) {
        // Only what changed is handed to Bevy: each change makes Bevy redo work for it.
        if self.shaded != Some(drawing.shaded) {
            self.shaded = Some(drawing.shaded);
            // Wireframe views show no faces.
            let shown = if drawing.shaded {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            world.entity_mut(self.root).insert(shown);
        }
        let lens = drawing.camera.lens();
        let aspect = drawing.area.width() / drawing.area.height().max(1.0);
        let new_view = self.view != Some((lens, aspect.to_bits()));
        if new_view || self.backdrop_colours != Some((drawing.background, drawing.background_low)) {
            self.place_backdrop(world, drawing);
        }
        if new_view {
            self.view = Some((lens, aspect.to_bits()));
            // The light from the viewer comes from a little above and to the left, as in
            // Rhino's Shaded, so highlights sit off-centre and flat faces shade across.
            let v = |p: [f64; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
            let (forward, up) = (v(lens.forward), v(lens.up));
            let right = forward.cross(up);
            let towards = (forward - HEADLIGHT_RAISE * up + HEADLIGHT_SHIFT * right).normalize();
            world
                .entity_mut(self.headlight)
                .insert(Transform::IDENTITY.looking_to(towards, up));
        }
        let new_look = self.look != Some(look);
        if new_look {
            self.look = Some(look);
            self.light(world, look);
            let materials = self.materials.values().cloned().collect::<Vec<_>>();
            let mut assets = world.resource_mut::<Assets<CurvaMaterial>>();
            for handle in materials {
                if let Some(mut material) = assets.get_mut(&handle) {
                    set_shine(&mut material.base, look.shine);
                }
            }
        }
        if look.lighting == Lighting::Sunlit && (new_view || new_look) {
            // Shadows reach from just in front of the viewer to well past what it looks at, in
            // millimetres (Bevy's defaults are for metres), set again whenever the camera moves
            // or the look turns Sunlit.
            let distance = drawing.camera.distance as f32;
            world.entity_mut(self.sun).insert(
                bevy::light::CascadeShadowConfigBuilder {
                    minimum_distance: distance * 0.01,
                    first_cascade_far_bound: distance * 0.6,
                    maximum_distance: distance * 4.0,
                    ..default()
                }
                .build(),
            );
        }
        self.place_preview(world, &drawing.preview_faces);
        self.place_ground(world, drawing);
        let new_faces = !self
            .faces
            .as_ref()
            .is_some_and(|faces| Arc::ptr_eq(faces, &drawing.faces));
        if new_faces {
            self.mirror(world, meshes, &drawing.faces);
            self.faces = Some(Arc::clone(&drawing.faces));
        }
        let new_shapes = !self
            .shapes
            .as_ref()
            .is_some_and(|shapes| Arc::ptr_eq(shapes, &drawing.shapes));
        if new_shapes {
            self.mirror_shapes(world, &mut meshes.1, &drawing.shapes);
            self.shapes = Some(Arc::clone(&drawing.shapes));
        }
        self.restyle(world, drawing.ghosted, FaceLook::of(drawing));
        let clips = Clips::of(&drawing.clips);
        if self.clips != Some(clips) {
            self.clips = Some(clips);
            let materials = self.materials.values().cloned().collect::<Vec<_>>();
            let mut assets = world.resource_mut::<Assets<CurvaMaterial>>();
            for handle in materials {
                if let Some(mut material) = assets.get_mut(&handle) {
                    material.extension.clips = clips;
                }
            }
        }
        let new_selection = !self
            .selection
            .as_ref()
            .is_some_and(|selection| Arc::ptr_eq(selection, &drawing.selection));
        if new_faces || new_shapes {
            self.colour(world, &drawing.objects, &drawing.selection);
        } else if new_selection {
            self.reselect(world, &drawing.selection);
        }
        if new_selection {
            self.selection = Some(Arc::clone(&drawing.selection));
        }
    }

    /// Puts Curva's background behind everything this view shows: a flat, unlit surface just
    /// inside the farthest distance drawn, filling the view, shaded from `drawing.background` at
    /// the top to `drawing.background_low` at the bottom, as Curva's own background is.
    /// Shows a command's preview of the surfaces it makes, or hides it when there is none.
    fn place_preview(&mut self, world: &mut World, faces: &Arc<Vec<MeshVertex>>) {
        if self
            .preview_faces
            .as_ref()
            .is_some_and(|shown| Arc::ptr_eq(shown, faces))
        {
            return;
        }
        self.preview_faces = Some(Arc::clone(faces));
        if faces.is_empty() {
            world.entity_mut(self.preview).insert(Visibility::Hidden);
            return;
        }
        let positions: Vec<[f32; 3]> = faces.iter().map(|corner| corner.position).collect();
        let normals: Vec<[f32; 3]> = faces.iter().map(|corner| corner.normal).collect();
        let colours: Vec<[f32; 4]> = faces
            .iter()
            .map(|corner| {
                let [r, g, b] = corner.colour;
                Color::srgb(r, g, b).to_linear().to_f32_array()
            })
            .collect();
        let mesh =
            bevy_mesh(positions, normals).with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world
            .entity_mut(self.preview)
            .insert((Mesh3d(mesh), Visibility::Visible));
    }

    fn place_backdrop(&mut self, world: &mut World, drawing: &Drawing) {
        let colours = (drawing.background, drawing.background_low);
        if self.backdrop_colours != Some(colours) {
            self.backdrop_colours = Some(colours);
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(backdrop_mesh(colours.0, colours.1));
            world.entity_mut(self.backdrop).insert(Mesh3d(mesh));
        }
        let lens = drawing.camera.lens();
        let v = |p: [f64; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
        let (eye, forward, up) = (v(lens.eye), v(lens.forward), v(lens.up));
        let distance = (lens.near + crate::camera::BACKDROP_DEPTH * (lens.far - lens.near)) as f32;
        let height = if lens.parallel {
            2.0 * lens.half_height as f32
        } else {
            2.0 * distance * (lens.fov as f32 / 2.0).tan()
        };
        let aspect = drawing.area.width() / drawing.area.height().max(1.0);
        // A little larger than the view, so its edges never show.
        let scale = Vec3::new(height * aspect * 1.05, height * 1.05, 1.0);
        world.entity_mut(self.backdrop).insert(
            Transform::from_translation(eye + forward * distance)
                .looking_to(forward, up)
                .with_scale(scale),
        );
    }

    /// Switches this view's lights for `look`.
    fn light(&self, world: &mut World, look: Look) {
        let studio = look.lighting == Lighting::Studio;
        for (entity, lux, shadows) in [
            (
                self.headlight,
                if studio { HEADLIGHT_LUX } else { 0.0 },
                false,
            ),
            (
                self.from_above,
                if studio { LIGHT_FROM_ABOVE_LUX } else { 0.0 },
                false,
            ),
            (self.sun, if studio { 0.0 } else { SUN_LUX }, !studio),
        ] {
            if let Some(mut light) = world.get_mut::<DirectionalLight>(entity) {
                light.illuminance = lux;
                light.shadow_maps_enabled = shadows;
            }
        }
    }

    /// Makes this view's entities match `faces`: unchanged parts stay, changed parts get their
    /// new mesh, parts gone are removed.
    fn mirror(&mut self, world: &mut World, meshes: &mut SharedMeshes, faces: &[MeshVertex]) {
        let mut groups: HashMap<PartKey, Triangles> = HashMap::new();
        for triangle in faces.as_chunks::<3>().0 {
            let key = (triangle[0].object, triangle[0].colour.map(f32::to_bits));
            let (positions, normals) = groups.entry(key).or_default();
            for corner in triangle {
                positions.push(corner.position);
                normals.push(corner.normal);
            }
        }
        let mut kept = HashSet::new();
        for (key, (positions, normals)) in groups {
            kept.insert(key);
            let mesh = (fingerprint(&positions, &normals), positions.len());
            match self.parts.get_mut(&key) {
                Some(part) if part.mesh == mesh => {}
                Some(part) => {
                    meshes.give_back(world, part.mesh);
                    let handle = meshes.take(world, mesh, || bevy_mesh(positions, normals));
                    world.entity_mut(part.entity).insert(Mesh3d(handle));
                    part.mesh = mesh;
                }
                None => {
                    let handle = meshes.take(world, mesh, || bevy_mesh(positions, normals));
                    let entity = world
                        .spawn((
                            Mesh3d(handle),
                            Transform::IDENTITY,
                            self.layer.clone(),
                            ChildOf(self.root),
                        ))
                        .id();
                    self.parts.insert(key, Part { mesh, entity });
                }
            }
        }
        let gone: Vec<PartKey> = self
            .parts
            .keys()
            .filter(|key| !kept.contains(*key))
            .copied()
            .collect();
        for key in gone {
            if let Some(part) = self.parts.remove(&key) {
                world.despawn(part.entity);
                meshes.give_back(world, part.mesh);
            }
        }
    }

    /// Gives each part its colour's material, or the selection colour's when selected, and keeps
    /// which parts each object has, so a later change of selection recolours only those
    /// ([`Self::reselect`]).
    fn colour(&mut self, world: &mut World, objects: &[u64], selection: &[u64]) {
        let copies = self
            .shape_parts
            .values()
            .map(|part| (part.object, part.colour, part.entity));
        self.by_object.clear();
        for (number, colour, entity) in self
            .parts
            .iter()
            .map(|(&(number, colour), part)| (number, colour, part.entity))
            .chain(copies)
        {
            // Parts of no object (numbered past the list) are drawn too, never selected.
            let id = objects.get(number as usize).copied();
            self.by_object.entry(id).or_default().push((entity, colour));
        }
        // Parts gone are forgotten, so the list does not grow.
        let live: HashSet<Entity> = self
            .by_object
            .values()
            .flatten()
            .map(|(entity, _)| *entity)
            .collect();
        self.shown.retain(|entity, _| live.contains(entity));
        self.picked = selection.iter().copied().collect();
        let all: Vec<(Option<u64>, Entity, [u32; 3])> = self
            .by_object
            .iter()
            .flat_map(|(&id, parts)| {
                parts
                    .iter()
                    .map(move |&(entity, colour)| (id, entity, colour))
            })
            .collect();
        for (id, entity, colour) in all {
            let picked = id.is_some_and(|id| self.picked.contains(&id));
            self.show(world, entity, colour, picked);
        }
    }

    /// Recolours the parts of only the objects whose selection changed (step 5: a new selection
    /// on *Large* went through all 12,000 parts in every view).
    fn reselect(&mut self, world: &mut World, selection: &[u64]) {
        let now: HashSet<u64> = selection.iter().copied().collect();
        let changed: Vec<u64> = now.symmetric_difference(&self.picked).copied().collect();
        for id in changed {
            let parts = self.by_object.get(&Some(id)).cloned().unwrap_or_default();
            for (entity, colour) in parts {
                self.show(world, entity, colour, now.contains(&id));
            }
        }
        self.picked = now;
    }

    /// Turns this view's materials see-through for the Ghosted display mode, or flat paper white
    /// for the Hidden one (#164), when either changed since the last frame.
    fn restyle(&mut self, world: &mut World, ghosted: bool, hidden: FaceLook) {
        if self.ghosted == Some(ghosted) && self.hidden == Some(hidden) {
            return;
        }
        (self.ghosted, self.hidden) = (Some(ghosted), Some(hidden));
        let materials = self
            .materials
            .iter()
            .map(|(colour, handle)| (*colour, handle.clone()))
            .collect::<Vec<_>>();
        let mut assets = world.resource_mut::<Assets<CurvaMaterial>>();
        for (colour, handle) in materials {
            if let Some(mut material) = assets.get_mut(&handle) {
                set_paper(&mut material.base, hidden, colour);
                set_ghosted(&mut material.base, ghosted);
            }
        }
    }

    /// Shows `entity` in `colour`, or the selection colour when `picked`, handing Bevy a material
    /// only when that changes: each one handed makes Bevy redo the part.
    fn show(&mut self, world: &mut World, entity: Entity, colour: [u32; 3], picked: bool) {
        let colour = if picked {
            crate::SELECTED_COLOUR.map(f32::to_bits)
        } else {
            colour
        };
        if self.shown.insert(entity, colour) == Some(colour) {
            return;
        }
        let (ghosted, paper, shine) = (
            self.ghosted.unwrap_or(false),
            self.hidden.unwrap_or_default(),
            self.look.unwrap_or_default().shine,
        );
        let clips = self.clips.unwrap_or_default();
        let material = self
            .materials
            .entry(colour)
            .or_insert_with(|| {
                let mut material = StandardMaterial {
                    base_color: Color::srgb(
                        f32::from_bits(colour[0]),
                        f32::from_bits(colour[1]),
                        f32::from_bits(colour[2]),
                    ),
                    // Open surfaces are seen from both sides.
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                };
                set_paper(&mut material, paper, colour);
                set_ghosted(&mut material, ghosted);
                set_shine(&mut material, shine);
                world
                    .resource_mut::<Assets<CurvaMaterial>>()
                    .add(CurvaMaterial {
                        base: material,
                        extension: Clipping { clips },
                    })
            })
            .clone();
        world.entity_mut(entity).insert(MeshMaterial3d(material));
    }
}

/// From matte (0) to polished (1): rougher surfaces spread their highlights wide and faint.
fn set_shine(material: &mut StandardMaterial, shine: f32) {
    material.perceptual_roughness = 0.9 - 0.8 * shine;
    material.reflectance = 0.2 + 0.6 * shine;
}

/// How a view colours its faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum FaceLook {
    /// Each in its own colour, lit.
    #[default]
    Own,
    /// Flat paper white and unlit, for the Hidden and Technical display modes (#164), so a face
    /// hides what is behind it and shows only its edges.
    Paper,
    /// White and lit, for the Clay display mode (#551), so the sun shades it and its shadows
    /// fall on it.
    White,
}

impl FaceLook {
    /// How `drawing` asks for its faces to be coloured.
    pub(super) fn of(drawing: &Drawing) -> Self {
        if drawing.hidden {
            Self::Paper
        } else if drawing.sketch {
            Self::White
        } else {
            Self::Own
        }
    }
}

/// Colours `material` as `look` says; `colour` is its own colour, as the material's key holds it.
fn set_paper(material: &mut StandardMaterial, look: FaceLook, colour: [u32; 3]) {
    let alpha = material.base_color.alpha();
    let [r, g, b] = if look == FaceLook::Own {
        colour.map(f32::from_bits)
    } else {
        crate::PAPER_COLOUR
    };
    material.base_color = Color::srgba(r, g, b, alpha);
    material.unlit = look == FaceLook::Paper;
}

fn set_ghosted(material: &mut StandardMaterial, ghosted: bool) {
    material
        .base_color
        .set_alpha(if ghosted { GHOSTED_ALPHA } else { 1.0 });
    material.alpha_mode = if ghosted {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    };
}

/// Triangles as a Bevy mesh, kept only on the graphics card.
fn bevy_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// A fingerprint of a mesh's contents (FNV-1a over the numbers' bits).
fn fingerprint(positions: &[[f32; 3]], normals: &[[f32; 3]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in positions.iter().chain(normals).flatten() {
        hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The light from all around for `look`, in Bevy's units for ambient light.
pub(super) fn ambient(look: Look) -> f32 {
    match look.lighting {
        Lighting::Studio => STUDIO_AMBIENT,
        Lighting::Sunlit => SKY_AMBIENT,
    }
}

/// What each lighting's surfaces reflect all around them: Studio's, then Sunlit's.
pub(super) fn skies(images: &mut Assets<Image>) -> [EnvironmentMapLight; 2] {
    let colour = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    let mut sky = |colours: [[f32; 3]; 3], brightness: f32| EnvironmentMapLight {
        // Bevy's sky is above Y; Curva's above Z.
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ..EnvironmentMapLight::hemispherical_gradient(
            images,
            colour(colours[0]),
            colour(colours[1]),
            colour(colours[2]),
        )
        .with_intensity(brightness)
    };
    [
        sky(STUDIO_SKY, STUDIO_SKY_BRIGHTNESS),
        sky(OPEN_SKY, OPEN_SKY_BRIGHTNESS),
    ]
}

/// Bands the backdrop's gradient is made of; Curva's background blends the colours as stored,
/// so each band's colour is worked out that way, and Bevy only blends within a thin band.
const BACKDROP_BANDS: usize = 32;

/// A square of side 1 facing +Z, shading from `top` to `bottom` in horizontal bands.
fn backdrop_mesh(top: epaint::Color32, bottom: epaint::Color32) -> Mesh {
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    for row in 0..=BACKDROP_BANDS {
        let t = row as f32 / BACKDROP_BANDS as f32;
        let stored = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) / 255.0;
        let colour = Color::srgb(
            stored(top.r(), bottom.r()),
            stored(top.g(), bottom.g()),
            stored(top.b(), bottom.b()),
        )
        .to_linear()
        .to_f32_array();
        for x in [-0.5, 0.5] {
            positions.push([x, 0.5 - t, 0.0]);
            colours.push(colour);
        }
    }
    let mut indices = Vec::new();
    for row in 0..BACKDROP_BANDS as u32 {
        let (a, b, c, d) = (2 * row, 2 * row + 1, 2 * row + 2, 2 * row + 3);
        indices.extend([a, c, b, b, c, d]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 0.0, 1.0]; 2 * (BACKDROP_BANDS + 1)],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}
