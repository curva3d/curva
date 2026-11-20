//! Welding by angle, as Rhino 8's `Weld` command and `Mesh.Weld(angle)`, and SketchUp import's
//! Weld angle: points at the same place become one where the faces on either side of an edge meet
//! at no more than the angle, and stay apart along sharper edges, so a smooth surface shades
//! smoothly while its creases stay crisp.
//!
//! Measured in Rhino 8 on a hexagonal prism with every face apart (60 points): it keeps 38 points
//! up to 59° (each cap's flat triangles join; the 60° folds between the sides stay apart), 26 from
//! 61° (the sides join) and 14 from 91° (the 90° edges round the caps join). Flat neighbours join
//! even at 0°.

use crate::triangulate::newell_normal;
use crate::{Mesh, MeshError, Point};
use std::collections::BTreeMap;

impl Mesh {
    /// The mesh with points at the same place joined where the faces meeting along an edge there
    /// turn by no more than `angle` degrees, and the number of points taken away. Normals,
    /// texture coordinates and colours are not carried.
    ///
    /// # Errors
    /// When the welded faces do not make a mesh.
    pub fn welded_by_angle(&self, angle: f64) -> Result<(Self, usize), MeshError> {
        let place = self.places();
        let points = self.vertices();
        let faces: Vec<&[u32]> = self.faces().collect();
        let normals: Vec<Point> = faces
            .iter()
            .map(|f| {
                unit(newell_normal(
                    &f.iter().map(|&v| points[v as usize]).collect::<Vec<_>>(),
                ))
            })
            .collect();
        // Each edge, by the places at its ends, with the faces along it and the points each uses.
        let mut edges: BTreeMap<(u32, u32), Vec<Side>> = BTreeMap::new();
        for (f, face) in faces.iter().enumerate() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                let (pa, pb) = (place[a as usize], place[b as usize]);
                let (key, ends) = if pa <= pb {
                    ((pa, pb), (a, b))
                } else {
                    ((pb, pa), (b, a))
                };
                edges.entry(key).or_default().push((f, ends.0, ends.1));
            }
        }
        let limit = angle.to_radians().cos() - 1e-12;
        let mut parent: Vec<u32> = (0..u32::try_from(points.len()).unwrap_or(u32::MAX)).collect();
        for along in edges.values() {
            for (i, &(f, a, b)) in along.iter().enumerate() {
                for &(g, c, d) in &along[i + 1..] {
                    if dot(normals[f], normals[g]) >= limit {
                        join(&mut parent, a, c);
                        join(&mut parent, b, d);
                    }
                }
            }
        }
        let mut new_index: BTreeMap<u32, u32> = BTreeMap::new();
        let mut kept: Vec<Point> = Vec::new();
        let welded: Vec<Vec<u32>> = faces
            .iter()
            .map(|face| {
                face.iter()
                    .map(|&v| {
                        let r = root(&mut parent, v);
                        *new_index.entry(r).or_insert_with(|| {
                            kept.push(points[r as usize]);
                            u32::try_from(kept.len() - 1).unwrap_or(u32::MAX)
                        })
                    })
                    .collect()
            })
            .collect();
        let removed = points.len() - kept.len();
        Ok((Self::new(kept, &welded)?, removed))
    }
}

/// A face along an edge, and the points it uses at the edge's two ends.
type Side = (usize, u32, u32);

fn root(parent: &mut [u32], mut v: u32) -> u32 {
    while parent[v as usize] != v {
        parent[v as usize] = parent[parent[v as usize] as usize];
        v = parent[v as usize];
    }
    v
}

fn join(parent: &mut [u32], a: u32, b: u32) {
    let (ra, rb) = (root(parent, a), root(parent, b));
    if ra != rb {
        parent[ra.max(rb) as usize] = ra.min(rb);
    }
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(p: Point) -> Point {
    let length = dot(p, p).sqrt();
    if length > 0.0 {
        p.map(|c| c / length)
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use crate::{Mesh, Point};

    /// Rhino's measuring prism: a hexagon of radius 1, 2 high, its sides quads and its caps fans
    /// of triangles about their centres, every face with points of its own (60 points).
    fn prism() -> Mesh {
        let ring: Vec<(f64, f64)> = (0..6)
            .map(|k| {
                let a = f64::from(k) * std::f64::consts::PI / 3.0;
                (a.cos(), a.sin())
            })
            .collect();
        let mut points: Vec<Point> = Vec::new();
        let mut faces: Vec<Vec<u32>> = Vec::new();
        let mut add = |face: &[Point]| {
            let start = u32::try_from(points.len()).unwrap();
            points.extend_from_slice(face);
            faces.push((start..start + u32::try_from(face.len()).unwrap()).collect());
        };
        for k in 0..6 {
            let (a, b) = (ring[k], ring[(k + 1) % 6]);
            add(&[
                [a.0, a.1, 0.0],
                [b.0, b.1, 0.0],
                [b.0, b.1, 2.0],
                [a.0, a.1, 2.0],
            ]);
            add(&[[0.0, 0.0, 2.0], [a.0, a.1, 2.0], [b.0, b.1, 2.0]]);
            add(&[[0.0, 0.0, 0.0], [b.0, b.1, 0.0], [a.0, a.1, 0.0]]);
        }
        Mesh::new(points, &faces).unwrap()
    }

    #[test]
    fn the_prism_welds_as_in_rhino_8() {
        for (angle, points) in [
            (0.0, 38),
            (22.5, 38),
            (59.0, 38),
            (61.0, 26),
            (89.0, 26),
            (91.0, 14),
            (180.0, 14),
        ] {
            let (welded, removed) = prism().welded_by_angle(angle).unwrap();
            assert_eq!(welded.vertex_count(), points, "at {angle}°");
            assert_eq!(removed, 60 - points);
        }
    }
}
