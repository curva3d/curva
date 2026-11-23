//! How a view's faces look: lit or flat, and each kind of object's faces shown or not, in one
//! colour or their own, and how opaque, as the view's style says.

use bevy::prelude::*;

use super::{CurvaMaterial, ViewScene, set_shine};
use crate::FaceKind;
use crate::pilot::clip::Clipping;

impl ViewScene {
    /// Brings this view's faces to `style` when it changed since the last frame: lit or flat
    /// (Hidden, #164), and each kind of object's faces shown or not, in one colour or their own,
    /// and how opaque.
    pub(super) fn restyle(&mut self, world: &mut World, style: &crate::Style) {
        let unlit = style.light() == Some(crate::Light::Flat);
        if self.unlit != Some(unlit) {
            self.unlit = Some(unlit);
            let materials = self.materials.values().cloned().collect::<Vec<_>>();
            let mut assets = world.resource_mut::<Assets<CurvaMaterial>>();
            for handle in materials {
                if let Some(mut material) = assets.get_mut(&handle) {
                    material.base.unlit = unlit;
                }
            }
        }
        if self.style_faces != Some(style.faces) {
            self.style_faces = Some(style.faces);
            self.face_style = *style;
            // Each part is shown again in its kind's new look; unchanged ones are skipped.
            let all: Vec<(Option<u64>, Entity, [u32; 3], FaceKind)> = self
                .by_object
                .iter()
                .flat_map(|(&id, parts)| {
                    parts
                        .iter()
                        .map(move |&(entity, colour, kind)| (id, entity, colour, kind))
                })
                .collect();
            for (id, entity, colour, kind) in all {
                let picked = id.is_some_and(|id| self.picked.contains(&id));
                self.show(world, entity, colour, kind, picked);
            }
        }
    }

    /// Shows `entity`, of `kind`, as its view's style gives that kind: in its own `colour`, or
    /// the selection colour when `picked`, unless the style gives the kind one colour (selected
    /// objects then show by their lines, as Hidden's white faces stay white, #164); as opaque as
    /// the style says; hidden if the style hides the kind. Bevy is handed a material only when
    /// that changes: each one handed makes Bevy redo the part.
    pub(super) fn show(
        &mut self,
        world: &mut World,
        entity: Entity,
        colour: [u32; 3],
        kind: FaceKind,
        picked: bool,
    ) {
        let look = self.face_style.faces_of(kind);
        let colour = look.colour.map_or(
            if picked {
                crate::SELECTED_COLOUR.map(f32::to_bits)
            } else {
                colour
            },
            |one| one.map(f32::to_bits),
        );
        let seen = Seen {
            colour,
            opacity: look.opacity.to_bits(),
            shown: look.shown,
        };
        let before = self.shown.insert(entity, seen);
        if before == Some(seen) {
            return;
        }
        if before.is_none_or(|before| before.shown != seen.shown) {
            let visibility = if seen.shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            world.entity_mut(entity).insert(visibility);
        }
        let (unlit, shine) = (
            self.unlit.unwrap_or(false),
            self.look.unwrap_or_default().shine,
        );
        let clips = self.clips.unwrap_or_default();
        let material = self
            .materials
            .entry((colour, seen.opacity))
            .or_insert_with(|| {
                let [r, g, b] = colour.map(f32::from_bits);
                let opacity = f32::from_bits(seen.opacity);
                let mut material = StandardMaterial {
                    base_color: Color::srgba(r, g, b, opacity),
                    alpha_mode: if opacity < 1.0 {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    unlit,
                    // Open surfaces are seen from both sides.
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                };
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

/// How a part is shown now: its colour's bits, its opacity's bits and whether it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Seen {
    pub(super) colour: [u32; 3],
    pub(super) opacity: u32,
    pub(super) shown: bool,
}
