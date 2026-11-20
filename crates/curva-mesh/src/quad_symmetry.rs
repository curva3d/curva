//! QuadRemesh's symmetry, as Rhino 8's `Symmetry`: the quads come out as mirror images across
//! planes through the middle of the mesh's bounding box. Measured in Rhino 8: its `X` mirrors y,
//! `Y` mirrors x and `Z` mirrors z, always about the bounding box's centre, not the world origin.
//!
//! The mesh is cut along each plane, the part on the far side of each is remeshed alone, its
//! border on the plane is put exactly on it, and the part is mirrored back and welded at the seam.

use crate::{Mesh, Point, QuadRemeshError, QuadRemeshOptions, QuadTarget};
use std::collections::{BTreeMap, BTreeSet};

impl Mesh {
    /// As [`Mesh::quad_remeshed`], mirrored across the planes through the bounding box's centre
    /// that flip the coordinates marked in `mirror` (x, y, z).
    ///
    /// # Errors
    /// When the mesh has no faces, or quadrs cannot remesh it.
    pub fn quad_remeshed_mirrored(
        &self,
        options: &QuadRemeshOptions,
        mirror: [bool; 3],
    ) -> Result<Self, QuadRemeshError> {
        let Some((low, high)) = self.bounds().filter(|_| mirror.contains(&true)) else {
            return self.quad_remeshed(options);
        };
        let centre: Point = std::array::from_fn(|i| (low[i] + high[i]) / 2.0);
        let axes: Vec<usize> = (0..3).filter(|&i| mirror[i]).collect();
        let mut part = self.clone();
        for &axis in &axes {
            part = half(&part, axis, centre[axis])?;
        }
        let pieces = 1 << axes.len();
        let target = match options.target {
            QuadTarget::Count(n) => QuadTarget::Count((n / pieces).max(1)),
            length => length,
        };
        let quads = part.quad_remeshed(&QuadRemeshOptions { target, ..*options })?;
        let side = mean_edge(&quads);
        let mut whole = on_the_planes(&quads, &axes, centre, 2.0 * side)?;
        for &axis in &axes {
            whole = with_mirror_image(&whole, axis, centre[axis])?;
        }
        Ok(whole.welded(side * 1e-6).0)
    }
}

fn failed(error: impl std::fmt::Display) -> QuadRemeshError {
    QuadRemeshError(error.to_string())
}

/// The part of the mesh where coordinate `axis` is at least `at`, its triangles that cross the
/// plane cut along it, with the cut points shared between neighbouring triangles.
fn half(mesh: &Mesh, axis: usize, at: f64) -> Result<Mesh, QuadRemeshError> {
    // Points this close to the plane count as on it, so no sliver is cut off beside them.
    let near = mesh
        .bounds()
        .map_or(0.0, |(lo, hi)| (hi[axis] - lo[axis]).abs() * 1e-9);
    let mut points = mesh.vertices().to_vec();
    let side = |p: &Point| {
        let d = p[axis] - at;
        if d.abs() <= near { 0.0 } else { d }
    };
    let mut cuts: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    for triangle in mesh.triangles().triangles {
        let mut kept = Vec::new();
        for k in 0..3 {
            let (a, b) = (triangle[k], triangle[(k + 1) % 3]);
            let (sa, sb) = (side(&points[a as usize]), side(&points[b as usize]));
            if sa >= 0.0 {
                kept.push(a);
            }
            if sa * sb < 0.0 {
                let key = (a.min(b), a.max(b));
                let cut = *cuts.entry(key).or_insert_with(|| {
                    let (p, q) = (points[key.0 as usize], points[key.1 as usize]);
                    let t = (at - p[axis]) / (q[axis] - p[axis]);
                    let mut point: Point = std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t);
                    point[axis] = at;
                    points.push(point);
                    u32::try_from(points.len() - 1).unwrap_or(u32::MAX)
                });
                kept.push(cut);
            }
        }
        if kept.len() >= 3 {
            faces.push(kept);
        }
    }
    compact(&points, &faces)
}

/// A mesh of only the points the faces use, as quadrs takes every point it is given.
fn compact(points: &[Point], faces: &[Vec<u32>]) -> Result<Mesh, QuadRemeshError> {
    let mut new_index: BTreeMap<u32, u32> = BTreeMap::new();
    let mut kept = Vec::new();
    let faces: Vec<Vec<u32>> = faces
        .iter()
        .map(|face| {
            face.iter()
                .map(|&v| {
                    *new_index.entry(v).or_insert_with(|| {
                        kept.push(points[v as usize]);
                        u32::try_from(kept.len() - 1).unwrap_or(u32::MAX)
                    })
                })
                .collect()
        })
        .collect();
    Mesh::new(kept, &faces).map_err(failed)
}

/// The quads' average side, to size the snapping and the weld.
fn mean_edge(mesh: &Mesh) -> f64 {
    let points = mesh.vertices();
    let (mut total, mut count) = (0.0, 0.0);
    for face in mesh.faces() {
        for k in 0..face.len() {
            let (p, q) = (
                points[face[k] as usize],
                points[face[(k + 1) % face.len()] as usize],
            );
            total += (0..3).map(|i| (p[i] - q[i]).powi(2)).sum::<f64>().sqrt();
            count += 1.0;
        }
    }
    if count > 0.0 { total / count } else { 1.0 }
}

/// The quads with each border point within `near` of a mirror plane put exactly on it, so the
/// mirrored halves meet.
fn on_the_planes(
    mesh: &Mesh,
    axes: &[usize],
    centre: Point,
    near: f64,
) -> Result<Mesh, QuadRemeshError> {
    let mut uses: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for face in mesh.faces() {
        for k in 0..face.len() {
            let (a, b) = (face[k], face[(k + 1) % face.len()]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let border: BTreeSet<u32> = uses
        .iter()
        .filter(|&(_, &n)| n == 1)
        .flat_map(|(&(a, b), _)| [a, b])
        .collect();
    let mut points = mesh.vertices().to_vec();
    // Each point goes onto its nearest plane, and onto another as well only when it is a quarter
    // as near to that one, where two planes meet.
    for &v in &border {
        let p = &mut points[v as usize];
        let away = |axis: usize| (p[axis] - centre[axis]).abs();
        let Some(&nearest) = axes.iter().min_by(|&&a, &&b| away(a).total_cmp(&away(b))) else {
            continue;
        };
        let snap: Vec<usize> = axes
            .iter()
            .copied()
            .filter(|&axis| {
                let limit = if axis == nearest { near } else { near / 4.0 };
                away(axis) <= limit
            })
            .collect();
        for axis in snap {
            p[axis] = centre[axis];
        }
    }
    let faces: Vec<&[u32]> = mesh.faces().collect();
    Mesh::new(points, &faces).map_err(failed)
}

/// The mesh together with its mirror image across the plane where coordinate `axis` is `at`, the
/// image's faces turned so they still face outwards.
fn with_mirror_image(mesh: &Mesh, axis: usize, at: f64) -> Result<Mesh, QuadRemeshError> {
    let count = u32::try_from(mesh.vertex_count()).unwrap_or(u32::MAX);
    let mut points = mesh.vertices().to_vec();
    points.extend(mesh.vertices().iter().map(|p| {
        let mut image = *p;
        image[axis] = 2.0 * at - p[axis];
        image
    }));
    let mut faces: Vec<Vec<u32>> = mesh.faces().map(<[u32]>::to_vec).collect();
    faces.extend(
        mesh.faces()
            .map(|face| face.iter().rev().map(|&v| v + count).collect()),
    );
    Mesh::new(points, &faces).map_err(failed)
}

#[cfg(test)]
mod tests {
    use crate::{Frame, Mesh, QuadRemeshOptions, SphereStyle};

    fn sphere_at(centre: [f64; 3]) -> Mesh {
        let mut frame = Frame::world();
        frame.origin = centre;
        Mesh::mesh_sphere(
            &frame,
            5.0,
            SphereStyle::Uv {
                vertical: 40,
                around: 40,
            },
        )
    }

    /// The farthest any point's mirror image is from the nearest point.
    fn asymmetry(mesh: &Mesh, axis: usize, at: f64) -> f64 {
        let points = mesh.vertices();
        points
            .iter()
            .map(|p| {
                let mut image = *p;
                image[axis] = 2.0 * at - p[axis];
                points
                    .iter()
                    .map(|q| {
                        (0..3)
                            .map(|i| (q[i] - image[i]).powi(2))
                            .sum::<f64>()
                            .sqrt()
                    })
                    .fold(f64::MAX, f64::min)
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn the_quads_mirror_about_the_middle_of_the_box_as_in_rhino() {
        // Rhino 8: a sphere centred at (10, 3, 0) remeshed with symmetry is a mirror image about
        // the planes through (10, 3, 0), not the world's.
        let sphere = sphere_at([10.0, 3.0, 0.0]);
        let quads = sphere
            .quad_remeshed_mirrored(&QuadRemeshOptions::count(500), [true, false, false])
            .unwrap();
        assert!(asymmetry(&quads, 0, 10.0) < 1e-9);
        assert!(quads.is_closed(), "the halves are welded at the seam");
        let count = quads.face_count();
        // Rhino 8 makes 412 quads here.
        assert!((350..=600).contains(&count), "{count}");
    }

    #[test]
    fn two_planes_make_four_matching_quarters() {
        let sphere = sphere_at([0.0, 0.0, 2.0]);
        let quads = sphere
            .quad_remeshed_mirrored(&QuadRemeshOptions::count(400), [true, true, false])
            .unwrap();
        assert!(asymmetry(&quads, 0, 0.0) < 1e-9);
        assert!(asymmetry(&quads, 1, 0.0) < 1e-9);
        assert!(quads.is_closed());
    }
}
