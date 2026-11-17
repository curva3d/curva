//! Splitting faces into triangles, for display and for the libraries that take only triangles.

use crate::{Mesh, Point};

/// A mesh's faces as triangles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Triangles {
    /// Each triangle's three vertices, counter-clockwise seen from outside, as its face.
    pub triangles: Vec<[u32; 3]>,
    /// For each triangle, the face it came from.
    pub faces: Vec<u32>,
    /// For each triangle, the three corners (positions in [`Mesh::corners`]) it uses, so
    /// per-corner normals, texture coordinates and colours follow it.
    pub corners: Vec<[u32; 3]>,
}

impl Mesh {
    /// Every face split into triangles. Triangles stay as they are, quads split along their
    /// shorter diagonal, and larger faces by ear clipping in their own plane, so a face shaped
    /// like an L or a U splits without triangles outside it.
    #[must_use]
    pub fn triangles(&self) -> Triangles {
        let mut out = Triangles::default();
        for f in 0..self.face_count() {
            let range = self.face_range(f);
            let ring: Vec<usize> = range.collect();
            for [a, b, c] in self.split_face(&ring) {
                out.triangles
                    .push([self.corners()[a], self.corners()[b], self.corners()[c]]);
                out.corners
                    .push([a, b, c].map(|i| u32::try_from(i).unwrap_or(u32::MAX)));
                out.faces.push(u32::try_from(f).unwrap_or(u32::MAX));
            }
        }
        out
    }

    /// One face, given by its corner positions, as triangles of corner positions.
    fn split_face(&self, ring: &[usize]) -> Vec<[usize; 3]> {
        let at = |corner: usize| self.vertices()[self.corners()[corner] as usize];
        match ring {
            [a, b, c] => vec![[*a, *b, *c]],
            [a, b, c, d] => {
                // The shorter diagonal gives the better-shaped pair, as most modellers split quads.
                if distance2(at(*a), at(*c)) <= distance2(at(*b), at(*d)) {
                    vec![[*a, *b, *c], [*a, *c, *d]]
                } else {
                    vec![[*a, *b, *d], [*b, *c, *d]]
                }
            }
            _ => {
                let points: Vec<Point> = ring.iter().map(|&c| at(c)).collect();
                ear_clip(&points)
                    .into_iter()
                    .map(|t| t.map(|i| ring[i]))
                    .collect()
            }
        }
    }
}

/// A polygon in space as triangles of its point numbers, by ear clipping (Meisters, "Polygons have
/// ears", American Mathematical Monthly 82, 1975; as set out in Eberly, "Triangulation by Ear
/// Clipping", Geometric Tools, 2002). The polygon is laid flat on the plane its Newell normal faces
/// (Sutherland, Sproull and Schumacker, "A characterization of ten hidden-surface algorithms",
/// 1974, p. 15). A polygon too twisted to have an ear left is finished as a fan, so this never
/// fails; the fan may then overlap, as Rhino's own display of such faces does.
pub(crate) fn ear_clip(points: &[Point]) -> Vec<[usize; 3]> {
    let normal = newell_normal(points);
    // Drop the axis the normal leans on most; flip one axis if it points backwards, so the polygon
    // stays counter-clockwise on the page.
    let axis = (0..3)
        .max_by(|&i, &j| normal[i].abs().total_cmp(&normal[j].abs()))
        .unwrap_or(2);
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let sign = if normal[axis] < 0.0 { -1.0 } else { 1.0 };
    let flat: Vec<[f64; 2]> = points.iter().map(|p| [p[u], p[v] * sign]).collect();

    let mut left: Vec<usize> = (0..points.len()).collect();
    let mut out = Vec::with_capacity(points.len().saturating_sub(2));
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&k| {
            let (a, b, c) = (left[(k + n - 1) % n], left[k], left[(k + 1) % n]);
            cross(flat[a], flat[b], flat[c]) > 0.0
                && left.iter().all(|&p| {
                    p == a || p == b || p == c || !inside(flat[p], flat[a], flat[b], flat[c])
                })
        });
        let Some(k) = ear else { break };
        out.push([left[(k + n - 1) % n], left[k], left[(k + 1) % n]]);
        left.remove(k);
    }
    for k in 1..left.len() - 1 {
        out.push([left[0], left[k], left[k + 1]]);
    }
    out
}

/// The direction a polygon faces, with length twice its area, robust for polygons that are not
/// quite flat or not convex.
pub(crate) fn newell_normal(points: &[Point]) -> Point {
    let mut n = [0.0; 3];
    for (k, p) in points.iter().enumerate() {
        let q = points[(k + 1) % points.len()];
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    n
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Whether `p` is inside triangle `a b c` or on its edge (counter-clockwise triangle).
fn inside(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
}

fn distance2(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    #[test]
    fn a_cube_splits_into_twelve_triangles_that_keep_their_face() {
        let t = cube().triangles();
        assert_eq!(t.triangles.len(), 12);
        assert_eq!(t.faces, vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
        assert_eq!(t.corners[2], [4, 5, 6]);
    }

    #[test]
    fn an_l_shaped_face_splits_without_triangles_outside_it() {
        // An L: the corner at (1, 1) points inwards.
        let v = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        let mesh = Mesh::new(v.clone(), &[[0u32, 1, 2, 3, 4, 5]]).unwrap();
        let t = mesh.triangles();
        assert_eq!(t.triangles.len(), 4);
        let area: f64 = t
            .triangles
            .iter()
            .map(|&[a, b, c]| cross(xy(v[a as usize]), xy(v[b as usize]), xy(v[c as usize])) / 2.0)
            .sum();
        // Every triangle counter-clockwise and together exactly the L's area (3), so none overlap
        // or stick out.
        assert!(t.triangles.iter().all(|&[a, b, c]| cross(
            xy(v[a as usize]),
            xy(v[b as usize]),
            xy(v[c as usize])
        ) > 0.0));
        assert!((area - 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_face_seen_from_below_splits_the_same_way_round() {
        // The same square n-gon wound the other way: its triangles must face down too.
        let v = vec![
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 1.5, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ];
        let t = Mesh::new(v.clone(), &[[0u32, 1, 2, 3, 4]])
            .unwrap()
            .triangles();
        for &[a, b, c] in &t.triangles {
            let n = newell_normal(&[v[a as usize], v[b as usize], v[c as usize]]);
            assert!(n[2] < 0.0);
        }
    }

    fn xy(p: Point) -> [f64; 2] {
        [p[0], p[1]]
    }
}
