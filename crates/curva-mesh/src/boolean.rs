//! Mesh booleans (union, difference, intersection and split), and the two tests `Check` and
//! MeshRepair borrow (faces crossing, and which way closed shells wind), on manifold-rust
//! (decision 0032, spec `docs/specs/mesh-and-subd.md` §3.1). This file is the only one that calls it.
//!
//! Meshes go in with double-precision points, through manifold-rust's robust import, and its
//! automatic engine choice: the exact engine for clean closed meshes, and its robust engine
//! (Barki, Guennebaud and Foufou, "Exact, robust, and efficient regularized Booleans on general
//! 3D meshes", Computers & Mathematics with Applications 70, 2015) for closed meshes that cross
//! themselves or share edges between more than two faces. Only closed meshes have an inside,
//! so an open one is refused (Rhino 8 cuts with an open mesh too, leaving an open result).
//!
//! Results are triangles. Normals, texture coordinates and colours do not come through yet.

use crate::{Mesh, Point};
use manifold_rust::linalg::Vec3;
use manifold_rust::manifold::Manifold;
use manifold_rust::robust::repair::plan_repair;
use manifold_rust::types::{BooleanEngine, Error, MeshGLP, OpType};
use std::fmt;

/// Why a boolean could not be made. The text is for logs; the command bar says it in its own
/// words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BooleanError {
    /// A mesh is not closed, so it has no inside. `first` says which set it is in, and `index`
    /// which mesh in that set.
    Open {
        /// Whether it is in the first set (else the second).
        first: bool,
        /// Its place in its set.
        index: usize,
    },
    /// A set that needs meshes has none.
    Nothing,
    /// The library stopped with this complaint.
    Failed(String),
}

impl fmt::Display for BooleanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { first, index } => write!(
                f,
                "mesh {index} of the {} set is not closed",
                if *first { "first" } else { "second" }
            ),
            Self::Nothing => f.write_str("no meshes to work on"),
            Self::Failed(why) => write!(f, "the mesh boolean failed: {why}"),
        }
    }
}

impl std::error::Error for BooleanError {}

/// The meshes joined into one, with what lies inside them removed (MeshBooleanUnion).
///
/// # Errors
/// When there are none or one is open.
pub fn union(meshes: &[Mesh]) -> Result<Mesh, BooleanError> {
    let shift = centre(meshes.iter());
    let solids = solids(meshes, true, shift)?;
    let mut all = solids.into_iter();
    let first = all.next().ok_or(BooleanError::Nothing)?;
    let joined = all.try_fold(first, |a, b| {
        checked(a.boolean_with_engine(&b, OpType::Add, ENGINE))
    })?;
    Ok(mesh(&joined, shift))
}

/// Each mesh of the first set with the second set taken away (MeshBooleanDifference). A mesh
/// taken away entirely leaves nothing, and is left out of the result.
///
/// # Errors
/// When either set is empty or a mesh is open.
pub fn difference(first: &[Mesh], second: &[Mesh]) -> Result<Vec<Mesh>, BooleanError> {
    each(first, second, |a, b| {
        checked(a.boolean_with_engine(b, OpType::Subtract, ENGINE)).map(|m| vec![m])
    })
}

/// Each mesh of the first set cut down to where it overlaps the second set
/// (MeshBooleanIntersection). A mesh that overlaps nothing is left out of the result.
///
/// # Errors
/// When either set is empty or a mesh is open.
pub fn intersection(first: &[Mesh], second: &[Mesh]) -> Result<Vec<Mesh>, BooleanError> {
    each(first, second, |a, b| {
        checked(a.boolean_with_engine(b, OpType::Intersect, ENGINE)).map(|m| vec![m])
    })
}

/// Each mesh of the first set cut in two by the second set: the part inside it, then the part
/// outside, for each (MeshBooleanSplit). Empty parts are left out.
///
/// # Errors
/// When either set is empty or a mesh is open.
pub fn split(first: &[Mesh], second: &[Mesh]) -> Result<Vec<Mesh>, BooleanError> {
    each(first, second, |a, b| {
        let inside = checked(a.boolean_with_engine(b, OpType::Intersect, ENGINE))?;
        let outside = checked(a.boolean_with_engine(b, OpType::Subtract, ENGINE))?;
        Ok(vec![inside, outside])
    })
}

/// Exact for clean meshes, robust for messy closed ones; chosen by the library from the input.
const ENGINE: BooleanEngine = BooleanEngine::Auto;

/// Each mesh of `first` against the union of `second`, keeping the non-empty results in order.
fn each(
    first: &[Mesh],
    second: &[Mesh],
    work: impl Fn(&Manifold, &Manifold) -> Result<Vec<Manifold>, BooleanError>,
) -> Result<Vec<Mesh>, BooleanError> {
    if first.is_empty() || second.is_empty() {
        return Err(BooleanError::Nothing);
    }
    let shift = centre(first.iter().chain(second));
    let cutters = solids(second, false, shift)?;
    let mut cutters = cutters.into_iter();
    let cutter = cutters.next().ok_or(BooleanError::Nothing)?;
    let cutter = cutters.try_fold(cutter, |a, b| {
        checked(a.boolean_with_engine(&b, OpType::Add, ENGINE))
    })?;
    let mut out = Vec::new();
    for solid in solids(first, true, shift)? {
        for result in work(&solid, &cutter)? {
            if !result.is_empty() {
                out.push(mesh(&result, shift));
            }
        }
    }
    Ok(out)
}

/// The middle of everything worked on. Points are measured from it while the library works, so
/// meshes far from the origin keep their digits.
fn centre<'a>(meshes: impl Iterator<Item = &'a Mesh>) -> Point {
    let bounds = meshes.filter_map(Mesh::bounds).reduce(|(lo, hi), (l, h)| {
        (
            std::array::from_fn(|i| lo[i].min(l[i])),
            std::array::from_fn(|i| hi[i].max(h[i])),
        )
    });
    bounds.map_or([0.0; 3], |(lo, hi)| {
        std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)
    })
}

fn solids(meshes: &[Mesh], first: bool, shift: Point) -> Result<Vec<Manifold>, BooleanError> {
    meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| {
            let solid = solid(mesh, shift);
            match solid.status() {
                Error::NoError => Ok(solid),
                Error::NotClosed | Error::NotManifold => Err(BooleanError::Open { first, index }),
                other => Err(BooleanError::Failed(format!("{other:?}"))),
            }
        })
        .collect()
}

/// A mesh as the library's solid, its points measured from `shift`.
fn solid(mesh: &Mesh, shift: Point) -> Manifold {
    let gl = MeshGLP::<f64, u32> {
        num_prop: 3,
        vert_properties: mesh
            .vertices()
            .iter()
            .flat_map(|p| [p[0] - shift[0], p[1] - shift[1], p[2] - shift[2]])
            .collect(),
        tri_verts: mesh.triangles().triangles.into_iter().flatten().collect(),
        ..MeshGLP::default()
    };
    Manifold::from_mesh_glp_robust(&gl)
}

/// Whether a mesh's faces cross through each other, by the library's exact test (faces lying on
/// top of each other count); `None` when the library will not take the mesh.
pub(crate) fn crosses_itself(mesh: &Mesh) -> Option<bool> {
    if mesh.face_count() == 0 {
        return Some(false);
    }
    let shift = centre(std::iter::once(mesh));
    let solid = solid(mesh, shift);
    (solid.status() == Error::NoError).then(|| solid.has_self_intersections())
}

/// Which of these triangles to turn round so closed shells point out, and hollows inside them
/// point in, by the library's winding repair (exact winding numbers at points just off each
/// shell). Shells are found by the library from shared corners.
pub(crate) fn inside_out(triangles: &[[Point; 3]]) -> Vec<bool> {
    let shift = centre_of(triangles.iter().flatten());
    let tris: Vec<[Vec3; 3]> = triangles
        .iter()
        .map(|t| t.map(|p| Vec3::new(p[0] - shift[0], p[1] - shift[1], p[2] - shift[2])))
        .collect();
    plan_repair(&tris).flip
}

fn centre_of<'a>(points: impl Iterator<Item = &'a Point>) -> Point {
    let bounds = points.fold(None, |b: Option<(Point, Point)>, p| {
        Some(b.map_or((*p, *p), |(lo, hi)| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        }))
    });
    bounds.map_or([0.0; 3], |(lo, hi)| {
        std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)
    })
}

fn checked(result: Manifold) -> Result<Manifold, BooleanError> {
    match result.status() {
        Error::NoError => Ok(result),
        other => Err(BooleanError::Failed(format!("{other:?}"))),
    }
}

/// The library's solid as a mesh of triangles, moved back from `shift`.
fn mesh(solid: &Manifold, shift: Point) -> Mesh {
    let gl = solid.get_mesh_glp::<f64, u32>(-1);
    let stride = gl.num_prop as usize;
    let vertices: Vec<Point> = gl
        .vert_properties
        .chunks(stride.max(3))
        .map(|p| [p[0] + shift[0], p[1] + shift[1], p[2] + shift[2]])
        .collect();
    let faces: &[[u32; 3]] = gl.tri_verts.as_chunks::<3>().0;
    // The library's output is closed, finite and refers only to its own points.
    Mesh::new(vertices, faces).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The unit cube moved half a unit along x: it overlaps the first cube by half.
    fn shifted() -> Mesh {
        cube().translated([0.5, 0.0, 0.0])
    }

    #[test]
    fn two_overlapping_cubes_unite_into_one_and_a_half() {
        let joined = union(&[cube(), shifted()]).unwrap();
        assert!(joined.is_closed());
        assert!(close(joined.volume().unwrap(), 1.5));
        assert_eq!(joined.bounds(), Some(([0.0; 3], [1.5, 1.0, 1.0])));
    }

    #[test]
    fn difference_intersection_and_split_have_the_volumes_worked_out_by_hand() {
        let left = difference(&[cube()], &[shifted()]).unwrap();
        assert_eq!(left.len(), 1);
        assert!(close(left[0].volume().unwrap(), 0.5));
        let both = intersection(&[cube()], &[shifted()]).unwrap();
        assert!(close(both[0].volume().unwrap(), 0.5));
        assert_eq!(both[0].bounds(), Some(([0.5, 0.0, 0.0], [1.0, 1.0, 1.0])));
        let parts = split(&[cube()], &[shifted()]).unwrap();
        let volumes: Vec<f64> = parts.iter().map(|m| m.volume().unwrap()).collect();
        assert_eq!(volumes.len(), 2);
        assert!(volumes.iter().all(|&v| close(v, 0.5)));
    }

    #[test]
    fn a_hole_through_a_box_leaves_a_closed_ring() {
        // A 3 × 3 × 1 slab with a 1 × 1 bar through it: 9 - 1 = 8 left, with a hole.
        let slab = cube().mapped(&curva_kernel::Similarity::scaling([0.0; 3], 3.0));
        let slab = Mesh::new(
            slab.vertices()
                .iter()
                .map(|p| [p[0], p[1], p[2] / 3.0])
                .collect(),
            &slab.faces().collect::<Vec<_>>(),
        )
        .unwrap();
        let bar = Mesh::new(
            cube()
                .vertices()
                .iter()
                .map(|p| [p[0] + 1.0, p[1] + 1.0, p[2] * 3.0 - 1.0])
                .collect(),
            &cube().faces().collect::<Vec<_>>(),
        )
        .unwrap();
        let ring = difference(&[slab], &[bar]).unwrap();
        assert!(ring[0].is_closed());
        assert!(close(ring[0].volume().unwrap(), 8.0));
        // A ring's surface: Euler characteristic 0, so V - E + F = 0.
        let (v, e, f) = (
            ring[0].vertex_count(),
            ring[0].edge_count(),
            ring[0].face_count(),
        );
        assert_eq!(v + f, e);
    }

    #[test]
    fn far_from_the_origin_the_answer_keeps_its_digits() {
        let far = [1e9, -2e9, 3e8];
        let a = cube().translated(far);
        let b = shifted().translated(far);
        let joined = union(&[a, b]).unwrap();
        assert!((joined.volume().unwrap() - 1.5).abs() < 1e-6);
        assert_eq!(joined.bounds().unwrap().0, far);
    }

    #[test]
    fn cutting_away_everything_leaves_nothing() {
        let big = cube().mapped(&curva_kernel::Similarity::scaling([0.5; 3], 3.0));
        assert!(difference(&[cube()], &[big]).unwrap().is_empty());
        let apart = cube().translated([5.0, 0.0, 0.0]);
        assert!(intersection(&[cube()], &[apart]).unwrap().is_empty());
    }

    #[test]
    fn an_open_mesh_is_refused_and_named() {
        let open: Vec<Vec<u32>> = cube().faces().skip(1).map(<[u32]>::to_vec).collect();
        let open = Mesh::new(cube().vertices().to_vec(), &open).unwrap();
        assert_eq!(
            difference(&[cube()], &[open]),
            Err(BooleanError::Open {
                first: false,
                index: 0
            })
        );
        assert_eq!(union(&[]), Err(BooleanError::Nothing));
    }

    #[test]
    fn an_octant_cut_from_a_faceted_sphere_leaves_seven_eighths() {
        // The cube's faces lie exactly along the sphere's own edges, the hardest case for a
        // boolean; by symmetry exactly 7/8 of the sphere's volume must stay.
        let sphere = crate::measure::tests::uv_sphere(32);
        let whole = sphere.volume().unwrap();
        let cut = difference(
            &[sphere],
            &[cube().mapped(&curva_kernel::Similarity::scaling([0.0; 3], 2.0))],
        )
        .unwrap();
        assert_eq!(cut.len(), 1);
        assert!(cut[0].is_closed());
        assert!((cut[0].volume().unwrap() - whole * 7.0 / 8.0).abs() < 1e-12);
    }

    #[test]
    fn a_mesh_that_crosses_itself_counts_its_overlap_once() {
        // Two overlapping cubes given as one mesh, as scans and careless exports often are.
        let (a, b) = (cube(), shifted());
        let mut vertices = a.vertices().to_vec();
        vertices.extend_from_slice(b.vertices());
        let mut faces: Vec<Vec<u32>> = a.faces().map(<[u32]>::to_vec).collect();
        faces.extend(
            b.faces()
                .map(|f| f.iter().map(|v| v + 8).collect::<Vec<_>>()),
        );
        let crossing = Mesh::new(vertices, &faces).unwrap();
        let apart = cube().translated([5.0, 0.0, 0.0]);
        let joined = union(&[crossing, apart]).unwrap();
        assert!(close(joined.volume().unwrap(), 2.5));
        assert!(joined.non_manifold_edges().is_empty() && joined.is_closed());
    }

    #[test]
    fn a_closed_mesh_with_an_edge_on_four_faces_still_unites() {
        // Two cubes touching along one edge, given as one mesh: closed, but the shared edge has
        // four faces, which only the robust engine accepts.
        let a = cube();
        let b = cube().translated([1.0, 1.0, 0.0]);
        let mut vertices = a.vertices().to_vec();
        vertices.extend_from_slice(b.vertices());
        let mut faces: Vec<Vec<u32>> = a.faces().map(<[u32]>::to_vec).collect();
        faces.extend(
            b.faces()
                .map(|f| f.iter().map(|v| v + 8).collect::<Vec<_>>()),
        );
        let pair = Mesh::new(vertices, &faces).unwrap();
        let joined = union(&[pair, cube().translated([0.0, 0.0, 0.5])]).unwrap();
        // 2 for the pair, plus the half of the third cube above the first.
        assert!(close(joined.volume().unwrap(), 2.5));
    }
}
