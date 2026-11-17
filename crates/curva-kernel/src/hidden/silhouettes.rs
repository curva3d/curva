//! Silhouettes: exact for cylinders and spheres, trimmed to the face, and from the tessellation
//! for other curved faces.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The exact silhouettes of a cylinder or sphere face from `view`, before trimming to the face:
/// lines along the cylinder `reach` either way from its frame, or the sphere's outline circle.
/// `None` for other surfaces.
pub(super) fn analytic_silhouettes(
    surface: &crate::record::Surface,
    view: &View,
    reach: f64,
) -> Option<Vec<Curve>> {
    use crate::record::Surface;
    match surface {
        Surface::Cylinder { frame, radius } => {
            let (o, x, y) = (frame.origin, frame.x_axis, frame.y_axis);
            let z = cross(x, y);
            // The angles round the axis where the surface turns edge-on.
            let angles: Vec<f64> = if view.parallel {
                let Some(across) = unit(cross(z, view.forward)) else {
                    // Seen along its axis: its outline is its end circles, which are edges.
                    return Some(Vec::new());
                };
                let u = dot(across, y).atan2(dot(across, x));
                vec![u, u + std::f64::consts::PI]
            } else {
                let rel = sub(o, view.eye);
                let (a, b) = (dot(x, rel), dot(y, rel));
                let r = a.hypot(b);
                if r <= *radius {
                    return Some(Vec::new());
                }
                let phi = b.atan2(a);
                let spread = (-radius / r).acos();
                vec![phi + spread, phi - spread]
            };
            Some(
                angles
                    .into_iter()
                    .filter_map(|u| {
                        let foot = plus(plus(o, x, radius * u.cos()), y, radius * u.sin());
                        Curve::line(plus(foot, z, -reach), plus(foot, z, reach)).ok()
                    })
                    .collect(),
            )
        }
        Surface::Sphere { frame, radius } => {
            let o = frame.origin;
            let (centre, size, normal) = if view.parallel {
                (o, *radius, view.forward)
            } else {
                let to_eye = sub(view.eye, o);
                let d = length(to_eye);
                if d <= *radius {
                    return Some(Vec::new());
                }
                let n = to_eye.map(|c| c / d);
                let share = radius * radius / d;
                (
                    plus(o, n, share),
                    radius * (1.0 - (radius / d).powi(2)).sqrt(),
                    n,
                )
            };
            let x = unit(cross(normal, view.up)).or_else(|| unit(cross(normal, view.right)))?;
            let y = cross(normal, x);
            Some(vec![Curve::Circle {
                centre,
                radius: size,
                x,
                y,
            }])
        }
        _ => None,
    }
}

/// How many points along a silhouette candidate the engine classifies to trim it to its face.
pub(super) const TRIM_SAMPLES: usize = 96;

/// The parts of `candidate` lying on `body`'s surface, by the engine's point classification,
/// their ends moved onto the nearest edge of the body so they meet its outline exactly.
pub(super) fn trimmed(
    candidate: &Curve,
    body: &Body,
    edges: &[Curve],
) -> Result<Vec<Curve>, KernelError> {
    let points: Vec<Point> = if let Some((a, b)) = ends(candidate) {
        (0..=TRIM_SAMPLES)
            .map(|k| plus(a, sub(b, a), k as f64 / TRIM_SAMPLES as f64))
            .collect()
    } else {
        candidate.divided(TRIM_SAMPLES)?
    };
    let on = body.on_surface(&points)?;
    if on.iter().all(|v| *v) {
        return Ok(vec![candidate.clone()]);
    }
    let spacing = points
        .windows(2)
        .map(|w| length(sub(w[1], w[0])))
        .fold(0.0, f64::max);
    // The nearest point on an edge within a step and a half, so the silhouette meets the
    // outline exactly.
    // Edges running along the candidate (a seam it happens to lie on) are not where it ends.
    let tolerance = spacing * 1e-6;
    let across: Vec<&Curve> = edges
        .iter()
        .filter(|e| !lies_on(e, candidate, tolerance))
        .collect();
    let snap = |p: Point| {
        across
            .iter()
            .filter_map(|e| e.closest_point(p).ok())
            .map(|q| (length(sub(q, p)), q))
            .filter(|(d, _)| *d <= spacing * 1.5)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(p, |(_, q)| q)
    };
    let mut out = Vec::new();
    let mut run: Option<usize> = None;
    for k in 0..=points.len() {
        let inside = k < points.len() && on[k];
        match (inside, run) {
            (true, None) => run = Some(k),
            (false, Some(start)) => {
                run = None;
                if k - start < 2 {
                    continue;
                }
                let (a, b) = (snap(points[start]), snap(points[k - 1]));
                if ends(candidate).is_some() {
                    if let Ok(line) = Curve::line(a, b) {
                        out.push(line);
                    }
                } else {
                    // The piece between the two cuts: the one whose middle is on the surface.
                    for piece in candidate.cut_at(&[a, b])? {
                        if let Some(m) = middle(&piece)
                            && body.on_surface(&[m])?.first().copied().unwrap_or(false)
                        {
                            out.push(piece);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// A mesh edge by its corners' rounded positions, lowest first.
pub(super) type MeshEdge = ([i64; 3], [i64; 3]);

/// Silhouette segments of a mesh's faces `faces`: edges between a triangle that faces the eye
/// and one that faces away, chained into polylines and straightened where they run straight.
pub(super) fn mesh_silhouettes(
    mesh: &crate::BodyMesh,
    faces: &[usize],
    view: &View,
    tolerance: f64,
) -> Vec<Curve> {
    let key = |p: Point| p.map(|c| (c / tolerance).round() as i64);
    let facing = |t: [Point; 3]| -> Option<bool> {
        let normal = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let towards = view.sight(t[0]).map(|c| -c);
        let scale = length(normal) * length(towards);
        let along = dot(normal, towards);
        (along.abs() > 1e-9 * scale).then_some(along > 0.0)
    };
    // Ordered, so the same model always chains the same way.
    let mut sides: std::collections::BTreeMap<MeshEdge, (Point, Point, Vec<Option<bool>>)> =
        std::collections::BTreeMap::new();
    for (index, t) in mesh.triangles.iter().enumerate() {
        if !faces.contains(
            &mesh
                .triangle_faces
                .get(index)
                .copied()
                .unwrap_or(usize::MAX),
        ) {
            continue;
        }
        let corners = t.map(|i| mesh.positions[i as usize]);
        let side = facing(corners);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (ka, kb) = (key(corners[i]), key(corners[j]));
            let edge = if ka < kb { (ka, kb) } else { (kb, ka) };
            sides
                .entry(edge)
                .or_insert((corners[i], corners[j], Vec::new()))
                .2
                .push(side);
        }
    }
    let segments: Vec<(Point, Point)> = sides
        .values()
        .filter(|(_, _, f)| matches!(f[..], [Some(a), Some(b)] if a != b))
        .map(|(a, b, _)| (*a, *b))
        .collect();
    chain(&segments, tolerance)
}

/// Segments joined end to end into polylines, straightened where they run straight.
pub(super) fn chain(segments: &[(Point, Point)], tolerance: f64) -> Vec<Curve> {
    let mut left: Vec<(Point, Point)> = segments.to_vec();
    let near = |a: Point, b: Point| length(sub(a, b)) <= tolerance * 10.0;
    let mut out = Vec::new();
    while let Some((a, b)) = left.pop() {
        let mut run = vec![a, b];
        loop {
            let tail = run[run.len() - 1];
            let head = run[0];
            if let Some(k) = left
                .iter()
                .position(|(p, q)| near(*p, tail) || near(*q, tail))
            {
                let (p, q) = left.swap_remove(k);
                run.push(if near(p, tail) { q } else { p });
            } else if let Some(k) = left
                .iter()
                .position(|(p, q)| near(*p, head) || near(*q, head))
            {
                let (p, q) = left.swap_remove(k);
                run.insert(0, if near(p, head) { q } else { p });
            } else {
                break;
            }
        }
        out.extend(straightened(&run, tolerance));
    }
    out
}

/// A run of points with the corners that turn by nothing taken out; a line when that leaves
/// two.
pub(super) fn straightened(run: &[Point], tolerance: f64) -> Option<Curve> {
    let mut kept: Vec<Point> = Vec::new();
    for p in run {
        if let [.., a, b] = kept[..] {
            // Drop `b` when it lies on the way from `a` to `p`.
            let ab = sub(b, a);
            let ap = sub(*p, a);
            let off = length(cross(ab, ap)) / length(ap).max(1e-12);
            if off <= tolerance * 10.0 && dot(ab, ap) > 0.0 {
                kept.pop();
            }
        }
        kept.push(*p);
    }
    match kept[..] {
        [a, b] => Curve::line(a, b).ok(),
        _ if kept.len() > 2 => Curve::polyline(kept, false).ok(),
        _ => None,
    }
}
