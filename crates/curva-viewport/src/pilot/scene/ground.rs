//! The Clay display mode's ground (#551): a grey plane at the model's height 0, reaching far
//! past the model to the horizon, lit by the view's sun so the model's shadows fall on it, as
//! SketchUp's ground looks. It casts no shadow and is not part of the model: nothing picks or
//! snaps to it.

use bevy::{asset::RenderAssetUsages, light::NotShadowCaster, mesh::PrimitiveTopology, prelude::*};

use super::ViewScene;
use crate::Drawing;

/// The ground's colour: a light, slightly warm grey.
pub(crate) const GROUND_COLOUR: [f32; 3] = [0.72, 0.72, 0.70];

/// How far the ground reaches each way, as a share of the camera's distance: far enough that
/// its edge meets the sky at the horizon in any ordinary view.
const REACH: f32 = 40.0;

impl ViewScene {
    /// Shows the ground under the model in a Clay view, centred under what the view looks at
    /// and sized for how far away that is; hides it in every other view.
    pub(super) fn place_ground(&mut self, world: &mut World, drawing: &Drawing) {
        if !drawing.sketch {
            if let Some(ground) = self.ground {
                world.entity_mut(ground).insert(Visibility::Hidden);
            }
            return;
        }
        let ground = *self.ground.get_or_insert_with(|| {
            let mesh = world.resource_mut::<Assets<Mesh>>().add(unit_square());
            let [r, g, b] = GROUND_COLOUR;
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    perceptual_roughness: 1.0,
                    reflectance: 0.1,
                    ..default()
                });
            world
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    self.layer.clone(),
                    NotShadowCaster,
                ))
                .id()
        });
        let target = drawing.camera.target;
        let reach = drawing.camera.distance as f32 * REACH;
        world.entity_mut(ground).insert((
            Transform::from_xyz(target[0] as f32, target[1] as f32, drawing.ground)
                .with_scale(Vec3::new(reach, reach, 1.0)),
            Visibility::Visible,
        ));
    }
}

/// A square two units across in the XY plane, facing up (Curva's Z), centred on the origin.
fn unit_square() -> Mesh {
    let corners = [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let positions: Vec<[f32; 3]> = [0, 1, 2, 0, 2, 3].map(|i| corners[i]).to_vec();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 6])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ground faces up and spans its square, so it lies flat under the model.
    #[test]
    fn the_ground_is_a_flat_square_facing_up() {
        let square = unit_square();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
            square.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        assert!(normals.iter().all(|n| *n == [0.0, 0.0, 1.0]));
        assert_eq!(square.count_vertices(), 6);
    }
}
