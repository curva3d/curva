//! ToSubD for surfaces and polysurfaces, as Rhino 8's `-ToSubD` fits a SubD to a NURBS surface
//! (its "SubD friendly fit"). Measured with RhinoCommon:
//! - each surface gives one SubD point per control point, joined as the control net is;
//! - on a curved face (degree 2 or more) the points are placed so the smooth surface passes
//!   through the surface at each point's Greville parameters;
//! - flat, straight-sided faces keep their corners as they are;
//! - open borders are creased, with no corners.
//!
//! Rhino takes extrusions; Curva has none, so a polysurface stands in for one. Flat faces become
//! one SubD face each, bounded by the points along their edges (the caps of an extruded circle
//! come out as polygons, as Rhino's do). A prism's cap rims are creased and its sides join smoothly,
//! as on Rhino's extrusions; any other polysurface keeps its sharp edges creased.

use crate::{Mesh, Point, SmoothError, SubD};
use curva_kernel::record::{Brep, Loop, Surface};
use curva_kernel::{Body, NurbsSurface};
use std::collections::{HashMap, HashSet};

fn failed(why: impl Into<String>) -> SmoothError {
    SmoothError(why.into())
}

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| a[k] - b[k])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|k| a[k] * b[k]).sum()
}

fn gap(a: Point, b: Point) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

/// Newell's normal of a polygon (not made unit).
fn newell(points: &[Point]) -> Point {
    let mut n = [0.0; 3];
    for (k, a) in points.iter().enumerate() {
        let b = points[(k + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// The cage's points: where the smooth surface must pass, one per place, and whether each stays
/// put.
struct Points {
    targets: Vec<Point>,
    keep: Vec<bool>,
    at: HashMap<[i64; 3], u32>,
    step: f64,
}

impl Points {
    fn id(&mut self, target: Point, keep: bool) -> u32 {
        let key = target.map(|c| {
            #[allow(clippy::cast_possible_truncation)]
            let k = (c / self.step).round() as i64;
            k
        });
        if let Some(&id) = self.at.get(&key) {
            self.keep[id as usize] &= keep;
            return id;
        }
        let id = u32::try_from(self.targets.len()).unwrap_or(u32::MAX);
        self.targets.push(target);
        self.keep.push(keep);
        self.at.insert(key, id);
        id
    }
}

/// One face of the cage: its points in order, the body face it comes from, and which way it
/// should face.
struct CageFace {
    points: Vec<u32>,
    from: usize,
    facing: Point,
}

/// The Greville parameters: each control point's average of the knots it spans.
fn greville(knots: &[f64], degree: usize, count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            if degree == 0 {
                knots[i]
            } else {
                #[allow(clippy::cast_precision_loss)]
                let d = degree as f64;
                knots[i + 1..=i + degree].iter().sum::<f64>() / d
            }
        })
        .collect()
}

fn is_flat(surface: &Surface) -> bool {
    matches!(
        surface,
        Surface::Plane { .. } | Surface::PlaneThrough { .. }
    )
}

/// Which way each face faces in its shell (true when along its surface's natural normal).
fn forward_faces(record: &Brep) -> Vec<bool> {
    let mut forward = vec![true; record.faces.len()];
    for shell in &record.shells {
        if let curva_kernel::record::Shell::Faces { faces, .. } = shell {
            for f in faces {
                if let Some(slot) = forward.get_mut(f.face as usize) {
                    *slot = f.forward;
                }
            }
        }
    }
    forward
}

impl SubD {
    /// A SubD fitted to a surface or polysurface, as Rhino's ToSubD fits one to a surface or an
    /// extrusion (see the module notes). Curved faces must be untrimmed, and flat faces have no
    /// holes.
    ///
    /// # Errors
    /// When a face is trimmed or holed, or the cage cannot be fitted.
    pub fn from_body(body: &Body) -> Result<Self, SmoothError> {
        let record = body.portable_record().map_err(|e| failed(e.0))?;
        let (lo, hi) = body.bounds().map_err(|e| failed(e.0))?;
        let size = gap(lo, hi).max(1e-9);
        let mut points = Points {
            targets: Vec::new(),
            keep: Vec::new(),
            at: HashMap::new(),
            step: size * 1e-7,
        };
        let forward = forward_faces(&record);
        let mut faces = Vec::new();
        let mut borders: Vec<Vec<u32>> = Vec::new();
        for (f, face) in record.faces.iter().enumerate() {
            if !is_flat(&face.surface) {
                curved_face(
                    &record,
                    f,
                    forward[f],
                    &mut points,
                    &mut faces,
                    &mut borders,
                )?;
            }
        }
        for (f, face) in record.faces.iter().enumerate() {
            if is_flat(&face.surface) {
                faces.push(flat_face(
                    &record,
                    f,
                    forward[f],
                    &mut points,
                    &borders,
                    size,
                )?);
            }
        }
        for face in &mut faces {
            settle(face, &points.targets);
        }
        faces.retain(|f| f.points.len() >= 3);
        let creased = creased_edges(&record, body, &faces, &points.targets);
        let cage_faces: Vec<Vec<u32>> = faces.iter().map(|c| c.points.clone()).collect();
        let cage =
            Mesh::new(points.targets.clone(), &cage_faces).map_err(|e| failed(e.to_string()))?;
        SubD::from_cage(cage)
            .with_creases(&creased)
            .fitted_keeping(&points.targets, &points.keep)
    }
}

/// Drops repeated points (a sphere's poles meet in one) and turns the face to face its way.
fn settle(face: &mut CageFace, targets: &[Point]) {
    face.points.dedup();
    while face.points.len() > 1 && face.points[0] == face.points[face.points.len() - 1] {
        face.points.pop();
    }
    let at: Vec<Point> = face.points.iter().map(|&p| targets[p as usize]).collect();
    if dot(newell(&at), face.facing) < 0.0 {
        face.points.reverse();
    }
}

/// A curved face as its control net's quads, with its four borders kept for the flat faces.
fn curved_face(
    record: &Brep,
    f: usize,
    forward: bool,
    points: &mut Points,
    faces: &mut Vec<CageFace>,
    borders: &mut Vec<Vec<u32>>,
) -> Result<(), SmoothError> {
    let surface = NurbsSurface::of_face(record, f).map_err(|e| failed(e.0))?;
    let [du, dv] = surface.degree;
    let (nu, nv) = (surface.points.len(), surface.points[0].len());
    let domain = surface.domain();
    let face = &record.faces[f];
    let trimmed = face.loops.len() != 1
        || (0..2).any(|k| {
            (0..2).any(|e| {
                (face.domain[k][e] - domain[k][e]).abs() > 1e-9 * (1.0 + domain[k][e].abs())
            })
        });
    if trimmed {
        return Err(failed(
            "ToSubD takes untrimmed surfaces and flat faces without holes",
        ));
    }
    let gu = greville(&surface.u_knots, du, nu);
    let gv = greville(&surface.v_knots, dv, nv);
    let keep = du < 2 && dv < 2;
    let grid: Vec<Vec<u32>> = gu
        .iter()
        .map(|&u| {
            gv.iter()
                .map(|&v| points.id(surface.point(u, v), keep))
                .collect()
        })
        .collect();
    let sign = if forward { 1.0 } else { -1.0 };
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let n = surface.normal(
                f64::midpoint(gu[i], gu[i + 1]),
                f64::midpoint(gv[j], gv[j + 1]),
            );
            faces.push(CageFace {
                points: vec![
                    grid[i][j],
                    grid[i + 1][j],
                    grid[i + 1][j + 1],
                    grid[i][j + 1],
                ],
                from: f,
                facing: n.map(|c| c * sign),
            });
        }
    }
    borders.push(grid[0].clone());
    borders.push(grid[nu - 1].clone());
    borders.push(grid.iter().map(|row| row[0]).collect());
    borders.push(grid.iter().map(|row| row[nv - 1]).collect());
    Ok(())
}

/// A flat face as one polygon through the points along its edges: a curved neighbour's border
/// where one runs along the edge, else the edge's own ends.
fn flat_face(
    record: &Brep,
    f: usize,
    forward: bool,
    points: &mut Points,
    borders: &[Vec<u32>],
    size: f64,
) -> Result<CageFace, SmoothError> {
    let face = &record.faces[f];
    let [Loop::Edges { edges }] = &face.loops[..] else {
        return Err(failed(
            "ToSubD takes untrimmed surfaces and flat faces without holes",
        ));
    };
    let surface = NurbsSurface::of_face(record, f).map_err(|e| failed(e.0))?;
    let [[u0, u1], [v0, v1]] = surface.domain();
    let normal = surface.normal(f64::midpoint(u0, u1), f64::midpoint(v0, v1));
    let origin = surface.point(u0, v0);
    let tolerance = size * 1e-6;
    let on_plane = |p: Point| dot(sub(p, origin), normal).abs() <= tolerance;
    let mut ring: Vec<u32> = Vec::new();
    for used in edges {
        let edge = &record.edges[used.edge as usize];
        let [a, b] = edge.vertices.map(|v| record.vertices[v as usize].point);
        let (start, end) = if used.forward { (a, b) } else { (b, a) };
        let found = borders.iter().find_map(|row| {
            let first = points.targets[row[0] as usize];
            let last = points.targets[row[row.len() - 1] as usize];
            let ends = |s: Point, e: Point| gap(first, s) <= tolerance && gap(last, e) <= tolerance;
            let flat = row.iter().all(|&p| on_plane(points.targets[p as usize]));
            let along = row.len() > 2 || gap(start, end) > tolerance;
            if !(flat && along) {
                return None;
            }
            if ends(start, end) {
                Some(row.clone())
            } else if ends(end, start) {
                Some(row.iter().rev().copied().collect())
            } else {
                None
            }
        });
        match found {
            Some(row) => ring.extend(&row[..row.len() - 1]),
            None => ring.push(points.id(start, true)),
        }
    }
    let sign = if forward { 1.0 } else { -1.0 };
    Ok(CageFace {
        points: ring,
        from: f,
        facing: normal.map(|c| c * sign),
    })
}

/// The cage edges to crease: a prism's cap rims, else every sharp edge between body faces.
fn creased_edges(
    record: &Brep,
    body: &Body,
    faces: &[CageFace],
    targets: &[Point],
) -> Vec<[u32; 2]> {
    // Each cage edge's body faces.
    let mut sides: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for face in faces {
        let n = face.points.len();
        for k in 0..n {
            let (a, b) = (face.points[k], face.points[(k + 1) % n]);
            sides
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(face.from);
        }
    }
    let caps = prism_caps(record);
    sides
        .into_iter()
        .filter(|((a, b), from)| {
            let [x, y] = from[..] else { return false };
            if x == y {
                return false;
            }
            if let Some([c, d]) = caps {
                return [c, d].iter().any(|cap| *cap == x || *cap == y);
            }
            let mid: Point = std::array::from_fn(|k| {
                f64::midpoint(targets[*a as usize][k], targets[*b as usize][k])
            });
            match (body.face_normal_near(x, mid), body.face_normal_near(y, mid)) {
                (Ok(n), Ok(m)) => dot(n, m) < (1.0_f64).to_radians().cos(),
                _ => true,
            }
        })
        .map(|((a, b), _)| [a, b])
        .collect()
}

/// The two end caps when the body is a prism: exactly one pair of parallel flat faces that every
/// other face meets.
fn prism_caps(record: &Brep) -> Option<[usize; 2]> {
    let mut meets: HashSet<(usize, usize)> = HashSet::new();
    for edge in &record.edges {
        for a in &edge.uses {
            for b in &edge.uses {
                meets.insert((a.face as usize, b.face as usize));
            }
        }
    }
    let normals: Vec<Option<Point>> = (0..record.faces.len())
        .map(|f| {
            if !is_flat(&record.faces[f].surface) {
                return None;
            }
            let s = NurbsSurface::of_face(record, f).ok()?;
            let [[u0, u1], [v0, v1]] = s.domain();
            Some(s.normal(f64::midpoint(u0, u1), f64::midpoint(v0, v1)))
        })
        .collect();
    let mut found = Vec::new();
    for a in 0..normals.len() {
        for b in a + 1..normals.len() {
            let (Some(n), Some(m)) = (normals[a], normals[b]) else {
                continue;
            };
            let parallel = dot(n, m).abs() > 1.0 - 1e-9;
            let ends = (0..normals.len())
                .filter(|&c| c != a && c != b)
                .all(|c| meets.contains(&(a, c)) && meets.contains(&(b, c)));
            if parallel && ends && !meets.contains(&(a, b)) {
                found.push([a, b]);
            }
        }
    }
    (found.len() == 1).then(|| found[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    /// Faces by side count, points, and creased edges.
    fn shape(subd: &SubD) -> (Vec<(usize, usize)>, usize, usize) {
        let mut sides: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for face in subd.cage().faces() {
            *sides.entry(face.len()).or_default() += 1;
        }
        let creases = subd
            .edge_uses()
            .keys()
            .filter(|&&(a, b)| subd.sharpness(a, b).is_crease())
            .count();
        (
            sides.into_iter().collect(),
            subd.cage().vertex_count(),
            creases,
        )
    }

    /// The volume of a solid, the area of an open SubD, through ToNURBS.
    fn size(subd: &SubD) -> f64 {
        let (body, _) = subd.to_nurbs(2, true).unwrap();
        if body.is_solid() {
            body.volume().unwrap()
        } else {
            body.area().unwrap()
        }
    }

    fn circle() -> Curve {
        Curve::circle([0.0; 3], 2.0).unwrap()
    }

    #[test]
    fn an_extruded_circle_comes_out_as_rhinos_extrusion_does() {
        // Rhino 8's ToSubD on a capped extrusion of a 2 m circle, 4 m tall: 8 sides and two
        // eight-sided caps, 16 points, the 16 rim edges creased, 50.2041 m³.
        let body = Body::extruded(&[circle()], [0.0, 0.0, 1.0], 4.0, true).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 8), (8, 2)], 16, 16));
        let volume = size(&subd);
        assert!((volume / 50.2041 - 1.0).abs() < 2e-3, "{volume}");
    }

    #[test]
    fn an_open_tube_comes_out_as_rhinos_does() {
        // Rhino 8: 8 faces, 16 points, its two borders creased (16 edges), 50.2351 m².
        let body = Body::extruded(&[circle()], [0.0, 0.0, 1.0], 4.0, false).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 8)], 16, 16));
        let area = size(&subd);
        assert!((area / 50.2351 - 1.0).abs() < 2e-3, "{area}");
    }

    #[test]
    fn a_flat_surface_keeps_its_corners_as_in_rhino() {
        // Rhino 8 on a 4 m square surface: one face on its four corners, its border creased,
        // 10.8444 m² of smooth surface.
        let net = vec![
            vec![[0.0, 0.0, 0.0], [0.0, 4.0, 0.0]],
            vec![[4.0, 0.0, 0.0], [4.0, 4.0, 0.0]],
        ];
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let body = NurbsSurface::new([1, 1], knots.clone(), knots, &net, None)
            .unwrap()
            .to_body()
            .unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 1)], 4, 4));
        let mut corners = subd.cage().vertices().to_vec();
        corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(corners[0], [0.0, 0.0, 0.0]);
        let area = size(&subd);
        assert!((area / 10.8444 - 1.0).abs() < 2e-3, "{area}");
    }

    #[test]
    fn a_sphere_closes_up_at_its_poles() {
        // Rhino 8 gives 48 faces (32 quads, 16 triangles); the control net here gives 32 (16 quads,
        // 16 triangles meeting at the poles), still closed.
        let subd = SubD::from_body(&Body::sphere([0.0; 3], 2.0).unwrap()).unwrap();
        assert_eq!(shape(&subd).0, vec![(3, 16), (4, 16)]);
        assert!(subd.to_nurbs(1, true).unwrap().0.is_solid());
    }

    #[test]
    fn a_box_keeps_its_corners_and_its_edges() {
        // Rhino's extrusion box creases its rims only (8 edges); a box here has no one pair of
        // ends, so all 12 sharp edges are creased. Its points stay on the corners, as Rhino's do.
        let body = Body::cuboid([0.0; 3], [4.0; 3]).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 6)], 8, 12));
        assert!(
            subd.cage()
                .vertices()
                .iter()
                .all(|p| p.iter().all(|&c| c == 0.0 || c == 4.0))
        );
    }

    #[test]
    fn a_curved_surface_is_fitted_through_its_greville_points() {
        // Rhino 8's fit puts the smooth surface through the surface at each control point's
        // Greville parameters, exactly.
        let net: Vec<Vec<Point>> = (0..5)
            .map(|i| {
                (0..4)
                    .map(|j| {
                        [
                            f64::from(i),
                            f64::from(j),
                            0.3 * f64::from((i * 7 + j * 3) % 5),
                        ]
                    })
                    .collect()
            })
            .collect();
        let knots = |n: usize| {
            let mut k = vec![0.0; 4];
            for s in 1..n - 3 {
                #[allow(clippy::cast_precision_loss)]
                k.push(s as f64);
            }
            #[allow(clippy::cast_precision_loss)]
            k.extend([(n - 3) as f64; 4]);
            k
        };
        let surface = NurbsSurface::new([3, 3], knots(5), knots(4), &net, None).unwrap();
        let subd = SubD::from_body(&surface.to_body().unwrap()).unwrap();
        assert_eq!(shape(&subd).1, 20);
        let gu = greville(&surface.u_knots, 3, 5);
        let gv = greville(&surface.v_knots, 3, 4);
        let limit = subd.surface(1).unwrap();
        for &u in &gu {
            for &v in &gv {
                let p = surface.point(u, v);
                let near = limit
                    .vertices()
                    .iter()
                    .map(|&q| gap(p, q))
                    .fold(f64::MAX, f64::min);
                assert!(near < 1e-6, "{near} at {u}, {v}");
            }
        }
    }
}
