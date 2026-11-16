//! Surfaces and solids drawn from the mesh they share with their copies (`display-on-bevy.md`
//! step 4, #153): each shape is one Bevy mesh, held once for every view, and each copy one
//! entity with that mesh at its placement. Bevy draws entities with the same mesh and material
//! together, so a thousand trees cost about what one does on the graphics card.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};
use curva_kernel::BodyMesh;

use super::ViewScene;
use crate::PlacedShape;

/// Which shape a mesh is: the address of the kernel's shared mesh, kept alive by
/// [`ShapeMeshes`] while any view uses it, so no other mesh can take its address meanwhile.
type ShapeKey = usize;

/// One Bevy mesh per shared shape, used by every view, each with how many copies use it.
#[derive(Default)]
pub(in crate::pilot) struct ShapeMeshes(HashMap<ShapeKey, (Arc<BodyMesh>, Handle<Mesh>, usize)>);

impl ShapeMeshes {
    fn take(&mut self, world: &mut World, mesh: &Arc<BodyMesh>) -> (ShapeKey, Handle<Mesh>) {
        let key = Arc::as_ptr(mesh) as ShapeKey;
        if let Some((_, handle, users)) = self.0.get_mut(&key) {
            *users += 1;
            return (key, handle.clone());
        }
        let handle = world.resource_mut::<Assets<Mesh>>().add(bevy_mesh(mesh));
        self.0.insert(key, (Arc::clone(mesh), handle.clone(), 1));
        (key, handle)
    }

    fn give_back(&mut self, world: &mut World, key: ShapeKey) {
        if let Some((_, handle, users)) = self.0.get_mut(&key) {
            *users -= 1;
            if *users == 0 {
                world.resource_mut::<Assets<Mesh>>().remove(&*handle);
                self.0.remove(&key);
            }
        }
    }

    /// How many shapes are held, for tests.
    #[cfg(test)]
    pub(in crate::pilot) fn len(&self) -> usize {
        self.0.len()
    }
}

/// One copy of a shape, as shown in one view.
pub(super) struct ShapePart {
    /// The object's number, for the selection.
    pub(super) object: u32,
    /// Its colour's bits, for its material.
    pub(super) colour: [u32; 3],
    shape: ShapeKey,
    placement: [[f64; 4]; 4],
    pub(super) entity: Entity,
}

impl ViewScene {
    /// Makes this view's copies match `shapes`: a copy whose shape and placement are unchanged
    /// stays as it is, a moved one only gets its new place, gone ones are removed. Copies are
    /// matched by their object and their order within it (an instance has one per part).
    pub(super) fn mirror_shapes(
        &mut self,
        world: &mut World,
        meshes: &mut ShapeMeshes,
        shapes: &[PlacedShape],
    ) {
        let mut kept = HashSet::new();
        let mut count: HashMap<u32, u32> = HashMap::new();
        for shape in shapes {
            let nth = count.entry(shape.object).or_default();
            let key = (shape.object, *nth);
            *nth += 1;
            kept.insert(key);
            let colour = shape.colour.map(f32::to_bits);
            let same_shape = Arc::as_ptr(&shape.mesh) as ShapeKey;
            match self.shape_parts.get_mut(&key) {
                Some(part) if part.shape == same_shape => {
                    if part.placement != shape.placement {
                        part.placement = shape.placement;
                        world
                            .entity_mut(part.entity)
                            .insert(placed(&shape.placement));
                    }
                    part.colour = colour;
                }
                Some(part) => {
                    meshes.give_back(world, part.shape);
                    let (shape_key, handle) = meshes.take(world, &shape.mesh);
                    world
                        .entity_mut(part.entity)
                        .insert((Mesh3d(handle), placed(&shape.placement)));
                    part.shape = shape_key;
                    part.placement = shape.placement;
                    part.colour = colour;
                }
                None => {
                    let (shape_key, handle) = meshes.take(world, &shape.mesh);
                    let entity = world
                        .spawn((
                            Mesh3d(handle),
                            placed(&shape.placement),
                            self.layer.clone(),
                            ChildOf(self.root),
                        ))
                        .id();
                    self.shape_parts.insert(
                        key,
                        ShapePart {
                            object: shape.object,
                            colour,
                            shape: shape_key,
                            placement: shape.placement,
                            entity,
                        },
                    );
                }
            }
        }
        let gone: Vec<(u32, u32)> = self
            .shape_parts
            .keys()
            .filter(|key| !kept.contains(*key))
            .copied()
            .collect();
        for key in gone {
            if let Some(part) = self.shape_parts.remove(&key) {
                world.despawn(part.entity);
                meshes.give_back(world, part.shape);
            }
        }
    }
}

/// A placement for points written as rows (`p' = p·A + t`, [`curva_kernel::Body::placement`])
/// as Bevy's transform, whose matrices take points as columns: the same numbers transposed, so
/// each row of the placement is a column of Bevy's matrix.
fn placed(placement: &[[f64; 4]; 4]) -> Transform {
    let columns = placement.map(|row| row.map(|value| value as f32));
    Transform::from_matrix(Mat4::from_cols_array_2d(&columns))
}

/// A shape's triangles as a Bevy mesh, corners shared between triangles as the kernel gives
/// them, kept only on the graphics card.
fn bevy_mesh(mesh: &BodyMesh) -> Mesh {
    let narrow = |p: &[f64; 3]| p.map(|value| value as f32);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        mesh.positions.iter().map(narrow).collect::<Vec<_>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        mesh.normals.iter().map(narrow).collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(
        mesh.triangles.iter().flatten().copied().collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::placed;
    use bevy::prelude::*;

    /// A placement turning a quarter about Z and moving by (10, 20, 30) puts (1, 0, 0) at
    /// (10, 21, 30) in Bevy as in the kernel.
    #[test]
    fn a_placement_is_the_same_transform_in_bevy() {
        let placement = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [10.0, 20.0, 30.0, 1.0],
        ];
        let moved = placed(&placement).transform_point(Vec3::X);
        assert!(
            moved.distance(Vec3::new(10.0, 21.0, 30.0)) < 1e-5,
            "{moved}"
        );
    }

    /// A mirror is kept: (1, 2, 3) mirrored in the YZ plane goes to (-1, 2, 3).
    #[test]
    fn a_mirrored_placement_is_kept() {
        let placement = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let moved = placed(&placement).transform_point(Vec3::new(1.0, 2.0, 3.0));
        assert!(moved.distance(Vec3::new(-1.0, 2.0, 3.0)) < 1e-5, "{moved}");
    }
}
