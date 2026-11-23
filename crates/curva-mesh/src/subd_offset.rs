//! Rhino 8's OffsetSubD: a SubD moved off itself along its smooth surface's normals. Measured
//! (23 November 2026): each cage point moves the distance along the limit surface's normal at
//! it, so a 4 m box's corner points move along (1, 1, 1)/√3, its edge points along (1, 1, 0)/√2
//! and its side centres straight out; the cage keeps its layout. `Solid` joins the two with side
//! quads round the border, the rims creased (an open strip of 2 faces: 10 quads on 12 points, 12
//! creases); `BothSides` offsets both ways.

use std::collections::BTreeMap;

use crate::{LiveSurface, Mesh, MeshError, Point, SmoothError, SubD};

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

impl SubD {
    /// Each cage point's normal on the smooth surface, facing the way the surface does.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn limit_normals(&self) -> Result<Vec<Point>, SmoothError> {
        let live = LiveSurface::new(self, 1)?;
        let (fine, normals) = live.positions(self.cage().vertices());
        // The library numbers each level's points from the level before's last.
        let first = fine.len() - self.cage().vertex_count();
        Ok(normals[first..].to_vec())
    }

    /// The cage points moved `distance` along their normals on the smooth surface, as Rhino's
    /// OffsetSubD (a negative distance goes the other way).
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn offset(&self, distance: f64) -> Result<Self, SmoothError> {
        let normals = self.limit_normals()?;
        let moves: Vec<(u32, Point)> = self
            .cage()
            .vertices()
            .iter()
            .zip(&normals)
            .enumerate()
            .map(|(v, (p, n))| (index(v), std::array::from_fn(|i| p[i] + distance * n[i])))
            .collect();
        Ok(self.with_cage_points_moved(&moves))
    }

    /// One closed SubD between the offsets at `inner` and `outer` (`outer` > `inner`), as
    /// Rhino's OffsetSubD with `Solid`: the inner shell turned round, the outer as it is, and a
    /// quad joining them along each border edge, the rims creased. Each shell keeps the creases
    /// of the SubD it comes from.
    ///
    /// # Errors
    /// When the library refuses the cage, or the solid cage is not a valid mesh.
    pub fn offset_solid(&self, inner: f64, outer: f64) -> Result<Self, MeshError> {
        let normals = self
            .limit_normals()
            .map_err(|_| MeshError::FaceTooSmall { face: 0 })?;
        let cage = self.cage();
        let n = cage.vertex_count();
        let shell = |d: f64| -> Vec<Point> {
            cage.vertices()
                .iter()
                .zip(&normals)
                .map(|(p, m)| std::array::from_fn(|i| p[i] + d * m[i]))
                .collect()
        };
        let mut points = shell(inner);
        points.extend(shell(outer));
        let up = |v: u32| v + index(n);
        let mut rings: Vec<Vec<u32>> = cage
            .faces()
            .map(|f| f.iter().rev().copied().collect())
            .collect();
        rings.extend(
            cage.faces()
                .map(|f| f.iter().map(|&v| up(v)).collect::<Vec<_>>()),
        );
        // Each border edge, the way its one face runs.
        let mut count: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for face in cage.faces() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut rims: Vec<[u32; 2]> = Vec::new();
        for face in cage.faces() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                if count[&(a.min(b), a.max(b))] == 1 {
                    rings.push(vec![up(b), up(a), a, b]);
                    rims.push([a, b]);
                    rims.push([up(a), up(b)]);
                }
            }
        }
        // The shells' own creases, other than their open borders, come along.
        for [a, b] in self.creases() {
            if count.get(&(a.min(b), a.max(b))) == Some(&2) {
                rims.push([a, b]);
                rims.push([up(a), up(b)]);
            }
        }
        Ok(SubD::from_cage(Mesh::new(points, &rings)?).with_creases(&rims))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Frame, Mesh, SubD};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_box_offsets_along_its_limit_normals_as_in_rhino() {
        // Rhino 8: Distance 1 on a 4 m box of 2 × 2 a side puts its cage x at 199, 199.2929,
        // 199.4226, 202, 204.5774, 204.7071 and 205, and keeps its 24 faces on 26 points.
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let out = subd.offset(1.0).unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (24, 26)
        );
        let mut xs: Vec<f64> = out
            .cage()
            .vertices()
            .iter()
            .map(|p| (p[0] * 1e4).round() / 1e4)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let rhino = [199.0, 199.2929, 199.4226, 202.0, 204.5774, 204.7071, 205.0];
        assert_eq!(xs.len(), rhino.len(), "{xs:?}");
        for (x, r) in xs.iter().zip(rhino) {
            assert!(close(*x, r), "{xs:?}");
        }
    }

    fn strip() -> SubD {
        let points = vec![
            [300.0, 0.0, 0.0],
            [302.0, 0.0, 0.0],
            [304.0, 0.0, 0.0],
            [300.0, 2.0, 0.0],
            [302.0, 2.0, 0.0],
            [304.0, 2.0, 0.0],
        ];
        SubD::new(Mesh::new(points, &[vec![0, 1, 4, 3], vec![1, 2, 5, 4]]).unwrap())
    }

    #[test]
    fn an_open_strip_offsets_up_and_makes_a_solid_as_in_rhino() {
        // Rhino 8: the strip offsets to z = 1; with Solid it becomes 10 quads on 12 points with
        // 12 creased edges; with BothSides too, from z = -1 to 1.
        let up = strip().offset(1.0).unwrap();
        assert!(up.cage().vertices().iter().all(|p| close(p[2], 1.0)));
        let solid = strip().offset_solid(0.0, 1.0).unwrap();
        assert_eq!(
            (solid.cage().face_count(), solid.cage().vertex_count()),
            (10, 12)
        );
        assert_eq!(solid.creases().count(), 12);
        assert!(solid.cage().is_closed());
        let both = strip().offset_solid(-1.0, 1.0).unwrap();
        let mut zs: Vec<f64> = both.cage().vertices().iter().map(|p| p[2]).collect();
        zs.sort_by(f64::total_cmp);
        assert!(close(zs[0], -1.0) && close(zs[11], 1.0));
        assert!((both.cage().volume().unwrap() - 16.0).abs() < 1e-9);
    }
}
