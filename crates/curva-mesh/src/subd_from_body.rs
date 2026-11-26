//! ToSubD for surfaces and polysurfaces, as Rhino 8's `-ToSubD` fits a SubD to a NURBS surface
//! (its "SubD friendly fit"). Measured with RhinoCommon, 21 November 2026:
//! - Rhino rebuilds each surface as a uniform cubic through the surface at each control point's
//!   Greville parameters (periodic round a closed direction, with no bending at an open end;
//!   straight directions stay straight), and that rebuilt surface's control net is the cage;
//! - at an open border the row a clamped cubic adds next to its end is left out, so the border
//!   is the cage's creased edge, with no corners;
//! - at a pole (a sphere's) that row stays as a ring of points round one pole point;
//! - trims and holes are ignored: a trimmed surface gives its whole surface.
//!
//! Rhino takes extrusions; Curva has none, so a polysurface stands in for one. Flat faces become
//! one SubD face each, bounded by the points along their outer edges (the caps of an extruded
//! circle come out as polygons, as Rhino's do). A prism's cap rims are creased and its sides join
//! smoothly, as on Rhino's extrusions (a box's top and bottom, as drawn in the Top view); any
//! other polysurface keeps its sharp edges creased.

use crate::{Mesh, Point, SmoothError, SubD};
use curva_kernel::record::{Brep, Loop, OrientedEdge, Surface};
use curva_kernel::{Body, NurbsSurface};
use std::collections::{HashMap, HashSet};

mod pole_kinks;

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

/// The cage's points, one per place: a point made twice where faces meet is joined.
struct Points {
    targets: Vec<Point>,
    at: HashMap<[i64; 3], u32>,
    step: f64,
}

impl Points {
    fn id(&mut self, target: Point) -> u32 {
        let key = target.map(|c| {
            #[allow(clippy::cast_possible_truncation)]
            let k = (c / self.step).round() as i64;
            k
        });
        if let Some(&id) = self.at.get(&key) {
            return id;
        }
        let id = u32::try_from(self.targets.len()).unwrap_or(u32::MAX);
        self.targets.push(target);
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
    /// extrusion (see the module notes). Trims and holes are ignored, as Rhino ignores them.
    ///
    /// # Errors
    /// When the body's faces cannot be read or make no cage.
    pub fn from_body(body: &Body) -> Result<Self, SmoothError> {
        let record = body.portable_record().map_err(|e| failed(e.0))?;
        let (lo, hi) = body.bounds().map_err(|e| failed(e.0))?;
        let size = gap(lo, hi).max(1e-9);
        let mut points = Points {
            targets: Vec::new(),
            at: HashMap::new(),
            step: size * 1e-7,
        };
        let forward = forward_faces(&record);
        let mut faces = Vec::new();
        let mut borders: Vec<Border> = Vec::new();
        let mut kinked: Vec<[u32; 2]> = Vec::new();
        // A lone face is taken as its whole surface, trims and all, as Rhino takes one.
        let lone = record.faces.len() == 1;
        for (f, face) in record.faces.iter().enumerate() {
            if lone || !is_flat(&face.surface) {
                curved_face(
                    &record,
                    f,
                    forward[f],
                    &mut points,
                    &mut faces,
                    &mut borders,
                    &mut kinked,
                )?;
            }
        }
        for (f, face) in record.faces.iter().enumerate() {
            if !lone && is_flat(&face.surface) {
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
        let mut creased = creased_edges(&record, body, &faces, &points.targets);
        creased.extend(kinked);
        let cage_faces: Vec<Vec<u32>> = faces.iter().map(|c| c.points.clone()).collect();
        let cage =
            Mesh::new(points.targets.clone(), &cage_faces).map_err(|e| failed(e.to_string()))?;
        Ok(SubD::from_cage(cage).with_creases(&creased))
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

/// The control points of a uniform cubic through `q`, one per point: periodic when `closed` (its
/// last point repeats its first), else with no bending at its ends, as a SubD's creased border
/// runs. A SubD's points along a row are these, so its smooth surface passes through `q`.
fn through(q: &[Point], closed: bool) -> Vec<Point> {
    let n = if closed { q.len() - 1 } else { q.len() };
    if n < 3 {
        return q.to_vec();
    }
    let scale = q
        .iter()
        .flat_map(|p| p.iter())
        .fold(1.0_f64, |m, c| m.max(c.abs()));
    let mut c: Vec<Point> = q[..n].to_vec();
    // Each point's own equation is ruled by its middle term (4 against 1 + 1), so sweeping
    // through them settles quickly.
    for _ in 0..500 {
        let mut moved = 0.0_f64;
        for i in 0..n {
            if !closed && (i == 0 || i == n - 1) {
                continue;
            }
            let (before, after) = (c[(i + n - 1) % n], c[(i + 1) % n]);
            let next: Point = std::array::from_fn(|k| (6.0 * q[i][k] - before[k] - after[k]) / 4.0);
            moved = moved.max(gap(next, c[i]));
            c[i] = next;
        }
        if moved <= scale * 1e-15 {
            break;
        }
    }
    if closed {
        c.push(c[0]);
    }
    c
}

/// Whether every point of a row is one point (a sphere's pole).
fn one_point(row: &[Point], tolerance: f64) -> bool {
    row.iter().all(|&p| gap(p, row[0]) <= tolerance)
}

/// A curved face's border: the surface's own points along it, and the cage points that run
/// along it.
struct Border {
    along: Vec<Point>,
    ids: Vec<u32>,
}

/// Rhino's SubD friendly net of a surface: the surface's own points at the Greville parameters,
/// the cage points made from them, and the parameters each cage row and column stands for.
struct Friendly {
    on: Vec<Vec<Point>>,
    net: Vec<Vec<Point>>,
    gu: Vec<f64>,
    gv: Vec<f64>,
}

fn column(grid: &[Vec<Point>], j: usize) -> Vec<Point> {
    grid.iter().map(|row| row[j]).collect()
}

/// A uniform cubic through the surface at each control point's Greville parameters, both ways;
/// straight directions stay as they are.
fn friendly(surface: &NurbsSurface, tolerance: f64) -> (Friendly, [bool; 2]) {
    let [du, dv] = surface.degree;
    let (nu, nv) = (surface.points.len(), surface.points[0].len());
    let gu = greville(&surface.u_knots, du, nu);
    let gv = greville(&surface.v_knots, dv, nv);
    let on: Vec<Vec<Point>> = gu
        .iter()
        .map(|&u| gv.iter().map(|&v| surface.point(u, v)).collect())
        .collect();
    let closed_u = (0..nv).all(|j| gap(on[0][j], on[nu - 1][j]) <= tolerance);
    let closed_v = (0..nu).all(|i| gap(on[i][0], on[i][nv - 1]) <= tolerance);
    let mut net = on.clone();
    if dv >= 2 {
        for row in &mut net {
            *row = through(row, closed_v);
        }
    }
    if du >= 2 {
        for j in 0..nv {
            for (i, p) in through(&column(&net, j), closed_u).into_iter().enumerate() {
                net[i][j] = p;
            }
        }
    }
    (Friendly { on, net, gu, gv }, [closed_u, closed_v])
}

fn ring(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| (2.0 * a[k] + b[k]) / 3.0)
}

fn third(a: f64, b: f64) -> f64 {
    (2.0 * a + b) / 3.0
}

/// Adds the ring a clamped cubic has next to each pole, a third of the way to the next row, as
/// Rhino keeps it.
fn add_poles(fr: &mut Friendly, curved: [bool; 2], tolerance: f64) {
    let Friendly { on, net, gu, gv } = fr;
    let (nu, nv) = (on.len(), on[0].len());
    if curved[1] {
        if one_point(&column(on, nv - 1), tolerance) {
            for row in net.iter_mut() {
                let p = ring(row[nv - 1], row[nv - 2]);
                row.insert(nv - 1, p);
            }
            gv.insert(nv - 1, third(gv[nv - 1], gv[nv - 2]));
        }
        if one_point(&column(on, 0), tolerance) {
            for row in net.iter_mut() {
                let p = ring(row[0], row[1]);
                row.insert(1, p);
            }
            gv.insert(1, third(gv[0], gv[1]));
        }
    }
    if curved[0] {
        if one_point(&on[nu - 1], tolerance) {
            let p = (0..net[0].len())
                .map(|j| ring(net[nu - 1][j], net[nu - 2][j]))
                .collect();
            net.insert(nu - 1, p);
            gu.insert(nu - 1, third(gu[nu - 1], gu[nu - 2]));
        }
        if one_point(&on[0], tolerance) {
            let p = (0..net[0].len())
                .map(|j| ring(net[0][j], net[1][j]))
                .collect();
            net.insert(1, p);
            gu.insert(1, third(gu[0], gu[1]));
        }
    }
}

/// A curved face as Rhino's SubD friendly surface's control net (see `friendly`). A pole keeps
/// the extra ring a clamped cubic has next to its end; elsewhere that ring is left out, which
/// leaves the SubD's own creased border. Trims are ignored, as Rhino ignores them.
fn curved_face(
    record: &Brep,
    f: usize,
    forward: bool,
    points: &mut Points,
    faces: &mut Vec<CageFace>,
    borders: &mut Vec<Border>,
    creases: &mut Vec<[u32; 2]>,
) -> Result<(), SmoothError> {
    let surface = NurbsSurface::of_face(record, f).map_err(|e| failed(e.0))?;
    let tolerance = points.step * 10.0;
    let (mut fr, closed) = friendly(&surface, tolerance);
    let [du, dv] = surface.degree;
    let kinks = pole_kinks::pole_kinks(&surface, &fr, closed, tolerance);
    if let Some(pk) = &kinks {
        pole_kinks::cut_at_kinks(&mut fr, pk, closed);
    }
    let pole_side = |d: usize| kinks.as_ref().is_some_and(|pk| pk.around != d);
    let curved = [du >= 2 || pole_side(0), dv >= 2 || pole_side(1)];
    add_poles(
        &mut fr,
        [curved[0] && !closed[0], curved[1] && !closed[1]],
        tolerance,
    );
    let grid: Vec<Vec<u32>> = fr
        .net
        .iter()
        .map(|row| row.iter().map(|&p| points.id(p)).collect())
        .collect();
    let (nu, nv) = (grid.len(), grid[0].len());
    if let Some(pk) = &kinks {
        creases.extend(pole_kinks::kink_creases(&grid, pk));
    }
    let on = &fr.on;
    let last = on[0].len() - 1;
    borders.extend([
        Border {
            along: on[0].clone(),
            ids: grid[0].clone(),
        },
        Border {
            along: on[on.len() - 1].clone(),
            ids: grid[nu - 1].clone(),
        },
        Border {
            along: column(on, 0),
            ids: grid.iter().map(|row| row[0]).collect(),
        },
        Border {
            along: column(on, last),
            ids: grid.iter().map(|row| row[nv - 1]).collect(),
        },
    ]);
    let (gu, gv) = (&fr.gu, &fr.gv);
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
    Ok(())
}

/// Each edge of a loop as its start and end, in the loop's order.
fn loop_ends(record: &Brep, edges: &[OrientedEdge]) -> Vec<(Point, Point)> {
    edges
        .iter()
        .map(|used| {
            let edge = &record.edges[used.edge as usize];
            let [a, b] = edge.vertices.map(|v| record.vertices[v as usize].point);
            if used.forward { (a, b) } else { (b, a) }
        })
        .collect()
}

/// A flat face as one polygon through the points along its outer edges (holes are ignored, as
/// Rhino ignores trims): a curved neighbour's border where one runs along the edge, else the
/// edge's own ends.
fn flat_face(
    record: &Brep,
    f: usize,
    forward: bool,
    points: &mut Points,
    borders: &[Border],
    size: f64,
) -> Result<CageFace, SmoothError> {
    let surface = NurbsSurface::of_face(record, f).map_err(|e| failed(e.0))?;
    let [[u0, u1], [v0, v1]] = surface.domain();
    let normal = surface.normal(f64::midpoint(u0, u1), f64::midpoint(v0, v1));
    // The outer loop is the one around the most area.
    let outer = record.faces[f]
        .loops
        .iter()
        .filter_map(|l| match l {
            Loop::Edges { edges } => Some(loop_ends(record, edges)),
            #[allow(unreachable_patterns)]
            _ => None,
        })
        .max_by(|a, b| {
            let area = |l: &[(Point, Point)]| {
                dot(newell(&l.iter().map(|e| e.0).collect::<Vec<_>>()), normal).abs()
            };
            area(a).total_cmp(&area(b))
        })
        .ok_or_else(|| failed("a flat face has no edges"))?;
    let origin = surface.point(u0, v0);
    let tolerance = size * 1e-6;
    let on_plane = |p: Point| dot(sub(p, origin), normal).abs() <= tolerance;
    let mut ring: Vec<u32> = Vec::new();
    for (start, end) in outer {
        let found = borders.iter().find_map(|row| {
            let (first, last) = (row.along[0], row.along[row.along.len() - 1]);
            let ends = |s: Point, e: Point| gap(first, s) <= tolerance && gap(last, e) <= tolerance;
            let flat = row.along.iter().all(|&p| on_plane(p));
            let along = row.ids.len() > 2 || gap(start, end) > tolerance;
            if !(flat && along) {
                return None;
            }
            if ends(start, end) {
                Some(row.ids.clone())
            } else if ends(end, start) {
                Some(row.ids.iter().rev().copied().collect())
            } else {
                None
            }
        });
        match found {
            Some(row) => ring.extend(&row[..row.len() - 1]),
            None => ring.push(points.id(start)),
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

/// The two end caps when the body is a prism: a pair of parallel flat faces that every other
/// face meets, and that do not meet each other.
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
    // A box has three such pairs. Rhino builds a box as an extrusion up from the plane it is
    // drawn on; a body does not say which plane that was, so the pair facing most nearly up
    // (world Z, the Top view's plane) is taken.
    let upward = |[a, _]: [usize; 2]| normals[a].map_or(0.0, |n| n[2].abs());
    found
        .into_iter()
        .max_by(|&p, &q| upward(p).total_cmp(&upward(q)))
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
    fn a_trimmed_surface_is_taken_whole_as_in_rhino() {
        // Rhino 8 ignores trims: a 6 m square with a round hole gives its untrimmed surface's one
        // face on four points, its border creased (RhinoCommon, 21 November 2026).
        let square = Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [6.0, 0.0, 0.0],
                [6.0, 6.0, 0.0],
                [0.0, 6.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let hole = Curve::circle([3.0, 3.0, 0.0], 1.0).unwrap();
        let body = Body::planar(&[square, hole]).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 1)], 4, 4));
        let xs: Vec<f64> = subd.cage().vertices().iter().map(|p| p[0]).collect();
        let wide =
            xs.iter().fold(f64::MIN, |m, &x| m.max(x)) - xs.iter().fold(f64::MAX, |m, &x| m.min(x));
        assert!(wide >= 6.0 - 1e-9, "{wide}");
    }

    #[test]
    fn a_sphere_closes_up_at_its_poles() {
        // Rhino 8 on a 2 m sphere (RhinoCommon, 21 November 2026): 32 quads and 16 triangles on
        // 42 points, nothing creased. Its rings stand at these heights and radii; its poles stay
        // on the sphere.
        let subd = SubD::from_body(&Body::sphere([0.0; 3], 2.0).unwrap()).unwrap();
        assert_eq!(shape(&subd), (vec![(3, 16), (4, 32)], 42, 0));
        let mut rings: Vec<(f64, f64)> = subd
            .cage()
            .vertices()
            .iter()
            .map(|p| (p[2].abs(), p[0].hypot(p[1])))
            .collect();
        rings.sort_by(|a, b| a.partial_cmp(b).unwrap());
        rings.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
        let rhino = [
            (0.0, 2.45619),
            (1.62132, 1.73679),
            (1.87377, 0.57893),
            (2.0, 0.0),
        ];
        assert_eq!(rings.len(), rhino.len(), "{rings:?}");
        for ((z, r), (want_z, want_r)) in rings.iter().zip(rhino) {
            assert!(
                (z - want_z).abs() < 1e-4 && (r - want_r).abs() < 1e-4,
                "{rings:?}"
            );
        }
        assert!(subd.to_nurbs(1, true).unwrap().0.is_solid());
    }

    #[test]
    fn a_box_keeps_its_corners_and_its_edges() {
        // Rhino 8's box is an extrusion up from the plane it is drawn on, and ToSubD creases its
        // bottom and top rims only (8 edges). Its points stay on the corners, as Rhino's do.
        let body = Body::cuboid([0.0; 3], [4.0; 3]).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        assert_eq!(shape(&subd), (vec![(4, 6)], 8, 8));
        assert!(
            subd.cage()
                .vertices()
                .iter()
                .all(|p| p.iter().all(|&c| c == 0.0 || c == 4.0))
        );
        // The creased edges are the bottom's and top's: none runs up.
        let body = Body::cuboid([0.0; 3], [4.0, 3.0, 2.0]).unwrap();
        let subd = SubD::from_body(&body).unwrap();
        let at = subd.cage().vertices();
        let creased: Vec<(u32, u32)> = subd
            .edge_uses()
            .into_keys()
            .filter(|&(a, b)| subd.sharpness(a, b).is_crease())
            .collect();
        assert_eq!(creased.len(), 8);
        for (a, b) in creased {
            let along = sub(at[b as usize], at[a as usize]);
            assert!(along[2].abs() < 1e-9, "{along:?} runs up");
        }
    }

    #[test]
    fn a_curved_surface_gets_rhinos_net() {
        // Rhino 8's ToSubD on this degree-3 surface (5 × 4 points, measured 21 November 2026):
        // 20 points, its border creased; these are some of them, to Rhino's 4 decimals.
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
        assert_eq!(shape(&subd), (vec![(4, 12)], 20, 14));
        let rhino = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.6222],
            [0.0, 2.0, 0.5778],
            [0.0, 3.0, 1.2],
            [0.7778, 0.0, 0.4935],
            [0.7778, 1.0, 0.3362],
            [0.7778, 2.0, 0.6155],
            [0.7778, 3.0, 0.506],
        ];
        for p in rhino {
            let near = subd
                .cage()
                .vertices()
                .iter()
                .map(|&q| gap(p, q))
                .fold(f64::MAX, f64::min);
            assert!(near < 1e-4, "{near} from {p:?}");
        }
    }
}
