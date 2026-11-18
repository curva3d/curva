use crate::field::BoundaryConstraint;
use crate::hierarchy::HierarchyLevel;
use crate::meshio::Vec3;
use crate::rotational_symmetry::rotate_vector_into_plane;
use crate::topology::{DirectedEdges, TriMesh, INVALID};

pub fn build_boundary_constraints(
    mesh: &TriMesh,
    dedges: &DirectedEdges,
    normals: &[Vec3],
) -> Vec<Option<BoundaryConstraint>> {
    let mut constraints = vec![None; mesh.vertices.len()];
    for edge in 0..dedges.e2e.len() {
        if dedges.e2e[edge] != INVALID {
            continue;
        }
        let face = mesh.faces[edge / 3];
        let i0 = face[edge % 3];
        let i1 = face[(edge + 1) % 3];
        let direction = mesh.vertices[i1] - mesh.vertices[i0];
        if direction.length_squared() <= 1e-12 {
            continue;
        }
        let tangent0 = (direction - normals[i0] * normals[i0].dot(direction)).normalize();
        let tangent1 = (direction - normals[i1] * normals[i1].dot(direction)).normalize();
        constraints[i0] = Some(BoundaryConstraint {
            origin: mesh.vertices[i0],
            tangent: tangent0,
            weight: 1.0,
        });
        constraints[i1] = Some(BoundaryConstraint {
            origin: mesh.vertices[i1],
            tangent: tangent1,
            weight: 1.0,
        });
    }
    constraints
}

pub fn build_boundary_hierarchy(
    levels: &[HierarchyLevel],
    fine_boundary: Vec<Option<BoundaryConstraint>>,
) -> Vec<Vec<Option<BoundaryConstraint>>> {
    let mut hierarchy = Vec::with_capacity(levels.len());
    hierarchy.push(fine_boundary);
    for level_idx in 0..levels.len().saturating_sub(1) {
        let fine = &levels[level_idx];
        let coarse = &levels[level_idx + 1];
        let to_coarser = fine.to_coarser.as_ref().unwrap();
        let mut origins = vec![Vec3::ZERO; coarse.positions.len()];
        let mut tangents = vec![Vec3::ZERO; coarse.positions.len()];
        let mut weights = vec![0.0; coarse.positions.len()];
        for (i, constraint) in hierarchy[level_idx].iter().enumerate() {
            let Some(constraint) = constraint else {
                continue;
            };
            let parent = to_coarser[i];
            let weight = fine.areas[i].max(1e-12) * constraint.weight.max(1e-12);
            origins[parent] += constraint.origin * weight;
            tangents[parent] += rotate_vector_into_plane(
                constraint.tangent,
                fine.normals[i],
                coarse.normals[parent],
            ) * weight;
            weights[parent] += weight;
        }

        let mut coarse_boundary = vec![None; coarse.positions.len()];
        for i in 0..coarse.positions.len() {
            if weights[i] == 0.0 {
                continue;
            }
            let normal = coarse.normals[i];
            let position = coarse.positions[i];
            let mut origin = origins[i] / weights[i];
            origin -= normal * normal.dot(origin - position);
            let mut tangent = tangents[i];
            tangent -= normal * normal.dot(tangent);
            if tangent.length_squared() <= 1e-12 {
                continue;
            }
            coarse_boundary[i] = Some(BoundaryConstraint {
                origin,
                tangent: tangent.normalize(),
                weight: 1.0,
            });
        }
        hierarchy.push(coarse_boundary);
    }
    hierarchy
}

/// Holds inside edges whose two faces meet at more than `angle_deg` degrees like borders: the
/// points on them get the edge's direction and stay on it, so the quads follow hard edges.
pub fn add_crease_constraints(
    mesh: &TriMesh,
    dedges: &DirectedEdges,
    normals: &[Vec3],
    angle_deg: f64,
    constraints: &mut [Option<BoundaryConstraint>],
) {
    let face_normal = |f: usize| {
        let [a, b, c] = mesh.faces[f];
        (mesh.vertices[b] - mesh.vertices[a])
            .cross(mesh.vertices[c] - mesh.vertices[a])
            .normalize_or_zero()
    };
    let limit = libm::cos(angle_deg.to_radians());
    for edge in 0..dedges.e2e.len() {
        let other = dedges.e2e[edge];
        if other == INVALID || other < edge {
            continue;
        }
        if face_normal(edge / 3).dot(face_normal(other / 3)) >= limit {
            continue;
        }
        let face = mesh.faces[edge / 3];
        let i0 = face[edge % 3];
        let i1 = face[(edge + 1) % 3];
        let direction = mesh.vertices[i1] - mesh.vertices[i0];
        if direction.length_squared() <= 1e-12 {
            continue;
        }
        for i in [i0, i1] {
            let tangent = (direction - normals[i] * normals[i].dot(direction)).normalize_or_zero();
            if tangent != Vec3::ZERO {
                constraints[i] = Some(BoundaryConstraint {
                    origin: mesh.vertices[i],
                    tangent,
                    weight: 1.0,
                });
            }
        }
    }
}
