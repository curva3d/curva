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

/// One shape held for Bevy: the kernel's mesh, kept alive; Bevy's mesh of it, measured from
/// `centre` (#225); and how many copies use it.
struct HeldShape {
    _mesh: Arc<BodyMesh>,
    handle: Handle<Mesh>,
    centre: [f64; 3],
    users: usize,
}

/// One Bevy mesh per shared shape, used by every view.
#[derive(Default)]
pub(in crate::pilot) struct ShapeMeshes(HashMap<ShapeKey, HeldShape>);

impl ShapeMeshes {
    /// Bevy's mesh of `mesh`, made the first time, and the point it is measured from.
    fn take(
        &mut self,
        world: &mut World,
        mesh: &Arc<BodyMesh>,
    ) -> (ShapeKey, Handle<Mesh>, [f64; 3]) {
        let key = Arc::as_ptr(mesh) as ShapeKey;
        if let Some(held) = self.0.get_mut(&key) {
            held.users += 1;
            return (key, held.handle.clone(), held.centre);
        }
        let centre = middle_of(mesh);
        let handle = world
            .resource_mut::<Assets<Mesh>>()
            .add(bevy_mesh(mesh, centre));
        let held = HeldShape {
            _mesh: Arc::clone(mesh),
            handle: handle.clone(),
            centre,
            users: 1,
        };
        self.0.insert(key, held);
        (key, handle, centre)
    }

    /// The point the mesh of shape `key` is measured from; the origin for one not held.
    fn centre(&self, key: ShapeKey) -> [f64; 3] {
        self.0.get(&key).map_or([0.0; 3], |held| held.centre)
    }

    fn give_back(&mut self, world: &mut World, key: ShapeKey) {
        if let Some(held) = self.0.get_mut(&key) {
            held.users -= 1;
            if held.users == 0 {
                world.resource_mut::<Assets<Mesh>>().remove(&held.handle);
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
                        let centre = meshes.centre(part.shape);
                        world
                            .entity_mut(part.entity)
                            .insert(placed(&shape.placement, centre));
                    }
                    part.colour = colour;
                }
                Some(part) => {
                    meshes.give_back(world, part.shape);
                    let (shape_key, handle, centre) = meshes.take(world, &shape.mesh);
                    world
                        .entity_mut(part.entity)
                        .insert((Mesh3d(handle), placed(&shape.placement, centre)));
                    part.shape = shape_key;
                    part.placement = shape.placement;
                    part.colour = colour;
                }
                None => {
                    let (shape_key, handle, centre) = meshes.take(world, &shape.mesh);
                    let entity = world
                        .spawn((
                            Mesh3d(handle),
                            placed(&shape.placement, centre),
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
/// each row of the placement is a column of Bevy's matrix. For a mesh measured from `centre`
/// ([`bevy_mesh`]), the placement first moves by `centre`, worked out in double precision, so a
/// shape made far from the origin is placed exactly (#225).
fn placed(placement: &[[f64; 4]; 4], centre: [f64; 3]) -> Transform {
    let mut moved = *placement;
    for k in 0..3 {
        moved[3][k] += (0..3).map(|i| centre[i] * placement[i][k]).sum::<f64>();
    }
    let columns = moved.map(|row| row.map(|value| value as f32));
    Transform::from_matrix(Mat4::from_cols_array_2d(&columns))
}

/// The middle of the box around `mesh`'s corners; the origin for a mesh with none.
fn middle_of(mesh: &BodyMesh) -> [f64; 3] {
    let Some(&first) = mesh.positions.first() else {
        return [0.0; 3];
    };
    let (low, high) = mesh
        .positions
        .iter()
        .fold((first, first), |(mut low, mut high), p| {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
            (low, high)
        });
    std::array::from_fn(|k| (low[k] + high[k]) / 2.0)
}

/// A shape's triangles as a Bevy mesh, its corners measured from `centre` so they keep their
/// precision wherever the shape was made (#225), shared between triangles as the kernel gives
/// them, kept only on the graphics card.
fn bevy_mesh(mesh: &BodyMesh, centre: [f64; 3]) -> Mesh {
    let narrow = |p: &[f64; 3]| p.map(|value| value as f32);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        mesh.positions
            .iter()
            .map(|p| crate::narrow_from(*p, centre))
            .collect::<Vec<_>>(),
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
        let moved = placed(&placement, [0.0; 3]).transform_point(Vec3::X);
        assert!(
            moved.distance(Vec3::new(10.0, 21.0, 30.0)) < 1e-5,
            "{moved}"
        );
    }

    /// A mesh measured from its centre (5, 0, 0) is placed where the kernel places it: its corner
    /// (1, 0, 0), which is (6, 0, 0), turned a quarter about Z and moved by (10, 20, 30), lands at
    /// (10, 26, 30).
    #[test]
    fn a_mesh_measured_from_its_centre_is_placed_as_made() {
        let placement = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [10.0, 20.0, 30.0, 1.0],
        ];
        let moved = placed(&placement, [5.0, 0.0, 0.0]).transform_point(Vec3::X);
        assert!(
            moved.distance(Vec3::new(10.0, 26.0, 30.0)) < 1e-5,
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
        let moved = placed(&placement, [0.0; 3]).transform_point(Vec3::new(1.0, 2.0, 3.0));
        assert!(moved.distance(Vec3::new(-1.0, 2.0, 3.0)) < 1e-5, "{moved}");
    }
}
