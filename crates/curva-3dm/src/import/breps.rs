//! Rhino's polysurfaces and extrusions as Curva's bodies.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A B-rep as a Curva body: the file's own topology turned into the engine's record, which the
/// engine checks and heals.
pub(super) fn brep_body(b: &Brep) -> Result<Body, KernelError> {
    let mut surfaces = Vec::with_capacity(b.surfaces.len());
    for s in &b.surfaces {
        surfaces.push(match s {
            Some(s) => Some(record_surface(s)?),
            None => None,
        });
    }
    let vertices = vertices_in(b);
    let edges = edges_in(b)?;
    let faces = faces_in(b, &surfaces, &edges)?;
    let (shells, regions) = shells_in(b, &faces, edges.len())?;
    let record = Record {
        version: curva_kernel::record::VERSION,
        tolerance: BREP_TOLERANCE,
        regions,
        shells: shells
            .into_iter()
            .map(|faces| Shell::Faces {
                faces,
                wires: Vec::new(),
            })
            .collect(),
        faces,
        edges,
        vertices,
    };
    Body::from_foreign_record(&record)
}

/// The least tolerance a B-rep's vertices and edges are taken at.
const BREP_TOLERANCE: f64 = 1e-5;

/// The error for a B-rep that cannot be read, saying why.
fn bad_brep(why: &str) -> KernelError {
    KernelError(format!("a B-rep Curva cannot read: {why}"))
}

/// An index in the file's lists, if it is one.
fn index(i: i32) -> Option<usize> {
    usize::try_from(i).ok()
}

/// The largest tolerance the engine takes for an edge or a point (its `MAX_TOLERANCE`, 1).
/// Rhino keeps looser ones where surfaces were joined at a loose tolerance; they are taken at
/// just under it, and the engine's repair closes the joins (#317's owner files).
const LOOSEST: f64 = 0.999;

/// A tolerance from the file, from [`BREP_TOLERANCE`] to [`LOOSEST`]; the least where it gives
/// none.
fn at_least(tolerance: f64) -> f64 {
    if tolerance.is_finite() && tolerance > 0.0 {
        tolerance.clamp(BREP_TOLERANCE, LOOSEST)
    } else {
        BREP_TOLERANCE
    }
}

/// The B-rep's vertices for the engine.
fn vertices_in(b: &Brep) -> Vec<Vertex> {
    b.vertices
        .iter()
        .map(|v| Vertex {
            point: v.point,
            tolerance: at_least(v.tolerance),
        })
        .collect()
}

/// The B-rep's edges for the engine: each its part of its 3D curve, its way round.
fn edges_in(b: &Brep) -> Result<Vec<Edge>, KernelError> {
    let mut edges = Vec::with_capacity(b.edges.len());
    for e in &b.edges {
        let c3 = index(e.curve)
            .and_then(|i| b.curves3.get(i))
            .and_then(Option::as_ref)
            .ok_or_else(|| bad_brep("an edge without its curve"))?;
        let mut spline = portion(c3, e.proxy_domain[0], e.proxy_domain[1])?;
        if e.reversed {
            spline = spline.reversed();
        }
        let interval = {
            let (a, b) = spline.domain();
            [a, b]
        };
        edges.push(Edge {
            curve: record_curve(&spline),
            nurbs: None,
            interval,
            vertices: [
                u32::try_from(e.vertices[0]).map_err(|_| bad_brep("an edge without its start"))?,
                u32::try_from(e.vertices[1]).map_err(|_| bad_brep("an edge without its end"))?,
            ],
            tolerance: at_least(e.tolerance),
            uses: Vec::new(),
        });
    }
    Ok(edges)
}

/// The B-rep's faces for the engine: each on its surface, with its loops of trims.
fn faces_in(
    b: &Brep,
    surfaces: &[Option<Placed>],
    edges: &[Edge],
) -> Result<Vec<Face>, KernelError> {
    let mut faces = Vec::with_capacity(b.faces.len());
    for f in &b.faces {
        let (surface, maps, domain) = index(f.surface)
            .and_then(|i| surfaces.get(i))
            .and_then(Option::as_ref)
            .ok_or_else(|| bad_brep("a face without its surface"))?
            .clone();
        let mut loops = Vec::new();
        for l in f
            .loops
            .iter()
            .filter_map(|l| index(*l))
            .filter_map(|l| b.loops.get(l))
        {
            let mut uses = Vec::new();
            for t in l
                .trims
                .iter()
                .filter_map(|t| index(*t))
                .filter_map(|t| b.trims.get(t))
            {
                if let Some(used) = trim_in(b, t, &maps, edges)? {
                    uses.push(used);
                }
            }
            if !uses.is_empty() {
                loops.push(Loop::Edges { edges: uses });
            }
        }
        // The domain is already in the record surface's parameters.
        let natural = [sorted(domain[0]), sorted(domain[1])];
        faces.push(Face {
            surface,
            nurbs: None,
            domain: natural,
            loops,
        });
    }
    Ok(faces)
}

/// A trim as its edge's use by the face, with its curve on the record surface's parameters;
/// `None` for a trim at a pole, which has no edge: the engine closes the loop there itself.
fn trim_in(
    b: &Brep,
    t: &model::BrepTrim,
    maps: &[Warp; 2],
    edges: &[Edge],
) -> Result<Option<OrientedEdge>, KernelError> {
    let Some(edge_index) = index(t.edge) else {
        return Ok(None);
    };
    let edge = edges
        .get(edge_index)
        .ok_or_else(|| bad_brep("a trim without its edge"))?;
    let c2 = index(t.curve)
        .and_then(|i| b.curves2.get(i))
        .and_then(Option::as_ref)
        .ok_or_else(|| bad_brep("a trim without its curve"))?;
    let mut pcurve = portion(c2, t.proxy_domain[0], t.proxy_domain[1])?;
    if t.proxy_reversed {
        pcurve = pcurve.reversed();
    }
    // Onto the record surface's parameters.
    pcurve = warped(pcurve, maps);
    let forward = !t.reversed3d;
    if !forward {
        pcurve = pcurve.reversed();
    }
    let pcurve = pcurve.reparameterized(edge.interval[0], edge.interval[1]);
    Ok(Some(OrientedEdge {
        edge: u32::try_from(edge_index).map_err(|_| bad_brep("too many edges"))?,
        forward,
        pcurve: record_curve2(&pcurve),
    }))
}

/// Shells: faces joined through shared edges; a shell whose every edge has two uses is closed
/// and bounds a solid. The shells, and the regions: one of every shell, one for each closed.
fn shells_in(
    b: &Brep,
    faces: &[Face],
    edge_count: usize,
) -> Result<(Vec<Vec<OrientedFace>>, Vec<Region>), KernelError> {
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); edge_count];
    for (fi, face) in faces.iter().enumerate() {
        for l in &face.loops {
            if let Loop::Edges { edges: uses } = l {
                for u in uses {
                    users[u.edge as usize].push(fi);
                }
            }
        }
    }
    for list in &users {
        for pair in list.windows(2) {
            let (a, b) = (root(&mut parent, pair[0]), root(&mut parent, pair[1]));
            parent[a] = b;
        }
    }
    let mut shell_of: BTreeMap<usize, usize> = BTreeMap::new();
    let mut shells: Vec<Vec<OrientedFace>> = Vec::new();
    for (fi, f) in b.faces.iter().enumerate() {
        let r = root(&mut parent, fi);
        let s = *shell_of.entry(r).or_insert_with(|| {
            shells.push(Vec::new());
            shells.len() - 1
        });
        shells[s].push(OrientedFace {
            face: u32::try_from(fi).map_err(|_| bad_brep("too many faces"))?,
            forward: !f.reversed,
        });
    }
    let closed: Vec<bool> = shells
        .iter()
        .map(|faces_in| {
            let inside: Vec<usize> = faces_in.iter().map(|f| f.face as usize).collect();
            users
                .iter()
                .filter(|list| list.iter().any(|f| inside.contains(f)))
                .all(|list| list.len() == 2)
        })
        .collect();
    let shell_ids: Vec<u32> = (0..shells.len()).map(|s| s as u32).collect();
    let mut regions = vec![Region {
        shells: shell_ids,
        void: false,
    }];
    for (s, is_closed) in closed.iter().enumerate() {
        if *is_closed {
            regions.push(Region {
                shells: vec![s as u32],
                void: false,
            });
        }
    }
    Ok((shells, regions))
}

fn sorted(d: [f64; 2]) -> [f64; 2] {
    if d[0] <= d[1] { d } else { [d[1], d[0]] }
}

/// A file curve moved by a rigid row placement.
fn placed(c: &model::Curve, m: &[[f64; 4]; 4]) -> model::Curve {
    let point = |p: P3| -> P3 {
        std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
    };
    let vector =
        |p: P3| -> P3 { std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j]) };
    match c {
        model::Curve::Line { ends, domain } => model::Curve::Line {
            ends: ends.map(point),
            domain: *domain,
        },
        model::Curve::Polyline { points, params } => model::Curve::Polyline {
            points: points.iter().map(|p| point(*p)).collect(),
            params: params.clone(),
        },
        model::Curve::Arc {
            plane,
            radius,
            angles,
            domain,
        } => model::Curve::Arc {
            plane: model::Plane {
                origin: point(plane.origin),
                x: vector(plane.x),
                y: vector(plane.y),
                z: vector(plane.z),
            },
            radius: *radius,
            angles: *angles,
            domain: *domain,
        },
        model::Curve::Nurbs(n) => {
            let mut n = n.clone();
            for h in &mut n.points {
                let w = h[3];
                let q = point([h[0] / w, h[1] / w, h[2] / w]);
                *h = [q[0] * w, q[1] * w, q[2] * w, w];
            }
            n.dimension = 3;
            model::Curve::Nurbs(n)
        }
        model::Curve::Poly { segments, params } => model::Curve::Poly {
            segments: segments.iter().map(|s| placed(s, m)).collect(),
            params: params.clone(),
        },
    }
}

/// An extrusion object as a Curva body: its profiles pulled along its path, capped as the file
/// says.
pub(super) fn extrusion_body(e: &model::Extrusion) -> Result<Body, KernelError> {
    let along = sub(e.path[1], e.path[0]);
    let length = (along[0] * along[0] + along[1] * along[1] + along[2] * along[2]).sqrt();
    let z = curva_kernel::unit(along)
        .ok_or_else(|| KernelError("an extrusion of no length".to_owned()))?;
    let y = curva_kernel::unit(e.up).unwrap_or([0.0, 1.0, 0.0]);
    let x = curva_kernel::cross(y, z);
    let o = e.path[0];
    let placement = [
        [x[0], x[1], x[2], 0.0],
        [y[0], y[1], y[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [o[0], o[1], o[2], 1.0],
    ];
    // Placed while still file curves, so circles stay circles.
    let profiles = e
        .profiles
        .iter()
        .map(|p| curve(&placed(p, &placement)))
        .collect::<Result<Vec<_>, _>>()?;
    let cap = e.caps[0] && e.caps[1] && profiles.iter().all(|p| p.is_closed().unwrap_or(false));
    if let Ok(body) = Body::extruded(&profiles, z, length, cap) {
        return Ok(body);
    }
    // Profiles the sweep does not take (weighted ones): each side a surface of its own, the
    // caps flat faces, sewn together.
    let by: P3 = z.map(|v| v * length);
    let mut parts = Vec::new();
    for profile in &profiles {
        let spline = Spline::of(profile)?;
        let points = spline
            .points
            .iter()
            .map(|h| {
                vec![
                    *h,
                    [
                        h[0] + by[0] * h[3],
                        h[1] + by[1] * h[3],
                        h[2] + by[2] * h[3],
                        h[3],
                    ],
                ]
            })
            .collect();
        parts.push(
            NurbsSurface {
                degree: [spline.degree, 1],
                u_knots: spline.knots.clone(),
                v_knots: vec![0.0, 0.0, 1.0, 1.0],
                points,
            }
            .to_body()?,
        );
    }
    if cap {
        let bottom = Body::planar(&profiles)?;
        parts.push(bottom.translated(by));
        parts.push(bottom);
    }
    Body::assembled(&parts)
}

/// Curva's field of view, by which a view's distance gives how much of the model it shows.
pub(crate) const HALF_FIELD: f64 = 0.35;

pub(super) fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross3(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(super) fn unit3(a: P3) -> Option<P3> {
    let length = dot3(a, a).sqrt();
    (length > 1e-12 && length.is_finite()).then(|| a.map(|v| v / length))
}

#[cfg(test)]
mod tolerance_tests {
    use super::*;

    #[test]
    fn a_files_tolerances_are_kept_within_what_the_engine_takes() {
        assert_eq!(at_least(0.01), 0.01);
        assert_eq!(at_least(1e-9), BREP_TOLERANCE);
        assert_eq!(at_least(0.0), BREP_TOLERANCE);
        assert_eq!(at_least(f64::NAN), BREP_TOLERANCE);
        // A loose join from Rhino comes in just under the engine's limit of 1.
        assert_eq!(at_least(2.5), LOOSEST);
        const { assert!(LOOSEST < 1.0) };
    }
}
