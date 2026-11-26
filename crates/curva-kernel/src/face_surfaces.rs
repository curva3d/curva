//! A face's own surface when the face is the whole of it (#749), for the mesh stream to grid
//! untrimmed faces in quads as Rhino's Mesh does (#733): one loop, each edge of which runs along
//! a side of the face's parameter box. Seams and collapsed poles run along sides too.

use crate::KernelError;
use crate::body::Body;
use crate::surface::NurbsSurface;
use crate::surface_edit::curve2_points;
use solidcore_record::brep::{Brep as Record, Loop};

/// Whether every edge of record face `face`'s only loop runs along a side of its box.
fn untrimmed(record: &Record, face: usize) -> bool {
    let Some(f) = record.faces.get(face) else {
        return false;
    };
    let [Loop::Edges { edges }] = f.loops.as_slice() else {
        return false;
    };
    let [[u0, u1], [v0, v1]] = f.domain;
    let size = (u1 - u0).abs().max((v1 - v0).abs()).max(1.0);
    let near = |x: f64, y: f64| (x - y).abs() < 1e-7 * size;
    edges.iter().all(|used| {
        let points = curve2_points(&used.pcurve);
        let (Some(a), Some(b)) = (points.first(), points.last()) else {
            return false;
        };
        // Both ends on the same side, and the pcurve straight along it.
        let on = |k: usize, at: f64| points.iter().all(|p| near(p[k], at));
        (near(a[0], u0) && near(b[0], u0) && on(0, u0))
            || (near(a[0], u1) && near(b[0], u1) && on(0, u1))
            || (near(a[1], v0) && near(b[1], v0) && on(1, v0))
            || (near(a[1], v1) && near(b[1], v1) && on(1, v1))
    })
}

impl Body {
    /// Face `face`'s surface, cut to the face's box, when the face is untrimmed: one loop, every
    /// edge of which runs along a side of that box (seams and collapsed poles count as sides).
    /// `None` for a trimmed face, one with holes, or one whose surface has no NURBS form.
    pub fn untrimmed_face_surface(&self, face: usize) -> Result<Option<NurbsSurface>, KernelError> {
        let record = self.record()?;
        Ok(untrimmed(&record, face)
            .then(|| NurbsSurface::of_face(&record, face).ok())
            .flatten()
            .map(|s| s.cut_to(record.faces[face].domain)))
    }

    /// [`Body::untrimmed_face_surface`] for every face, reading the record once.
    pub fn untrimmed_face_surfaces(&self) -> Result<Vec<Option<NurbsSurface>>, KernelError> {
        let record = self.record()?;
        Ok((0..record.faces.len())
            .map(|face| {
                untrimmed(&record, face)
                    .then(|| NurbsSurface::of_face(&record, face).ok())
                    .flatten()
                    .map(|s| s.cut_to(record.faces[face].domain))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    #[test]
    fn a_sphere_and_a_boxs_faces_are_untrimmed() {
        let sphere = Body::sphere([0.0; 3], 5.0).unwrap();
        assert!(sphere.untrimmed_face_surface(0).unwrap().is_some());
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let faces = cube.untrimmed_face_surfaces().unwrap();
        assert_eq!(faces.len(), 6);
        assert!(faces.iter().all(Option::is_some));
    }

    #[test]
    fn a_trimmed_face_and_a_split_piece_are_not() {
        let disc = Body::planar(&[Curve::circle([0.0; 3], 3.0).unwrap()]).unwrap();
        assert!(disc.untrimmed_face_surface(0).unwrap().is_none());
        let square = Body::ruled(
            &Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]).unwrap(),
            false,
        )
        .unwrap();
        let cut = square
            .split_by_curves(0, &[Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap()])
            .unwrap();
        let surfaces = cut.untrimmed_face_surfaces().unwrap();
        assert!(surfaces.iter().all(Option::is_none), "{}", surfaces.len());
    }
}
