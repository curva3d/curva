//! Rhino's polysurfaces and extrusions as Curva's bodies.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A B-rep as a Curva body: the file's own topology turned into the engine's record, which the
/// engine checks and heals.
pub(super) fn brep_body(b: &Brep) -> Result<Body, KernelError> {
    Body::from_foreign_record(&brep_record(b)?)
}

/// Polysurfaces with more faces than this are read quickly first and healed later (#476): the
/// healer's time grows faster than the faces, from milliseconds for small ones to 40 s for a
/// 2 400-face polysurface, while small ones heal quickly enough to wait for, and the few that
/// read differently without it are small.
pub(super) const QUICK_FROM_FACES: usize = 200;

/// A large B-rep read quickly, without the engine's healer, and the work that heals it, to run
/// later; a small one healed now, with nothing to come.
pub(super) fn brep_body_quickly(
    b: &Brep,
) -> Result<(Body, Option<curva_document::Repair>), KernelError> {
    let record = brep_record(b)?;
    if b.faces.len() > QUICK_FROM_FACES
        && let Ok(quick) = Body::from_foreign_record_unhealed(&record)
    {
        let heal: curva_document::Repair =
            Box::new(move || Body::from_foreign_record(&record).ok());
        return Ok((quick, Some(heal)));
    }
    Body::from_foreign_record(&record).map(|body| (body, None))
}

/// The file's B-rep as the engine's record.
fn brep_record(b: &Brep) -> Result<Record, KernelError> {
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
    Ok(Record {
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
    })
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
        let domain = if matches!(surface, RecordSurface::Plane { .. }) {
            around_trims(natural, &loops)
        } else {
            natural
        };
        faces.push(Face {
            surface,
            nurbs: None,
            domain,
            loops,
        });
    }
    Ok(faces)
}

/// A plane face's domain: a plane has no edges of its own, so its domain is only a box around the
/// face's trims. Rhino's can be a sliver no wider than the tolerance (a face Rhino keeps, about
/// 1e-11 wide in one of the owner's files), or fall a rounding short of a trim; the engine refuses
/// both (status 1007). The box covers the trims' control points, which hold each trim, grown by
/// the tolerance the B-rep is read at.
fn around_trims(domain: [[f64; 2]; 2], loops: &[Loop]) -> [[f64; 2]; 2] {
    let mut box_ = domain;
    let mut take = |u: f64, v: f64| {
        for (axis, value) in [u, v].into_iter().enumerate() {
            if value.is_finite() {
                box_[axis] = [box_[axis][0].min(value), box_[axis][1].max(value)];
            }
        }
    };
    for l in loops {
        let Loop::Edges { edges } = l else { continue };
        for used in edges {
            match &used.pcurve {
                Curve2::Spline { points, .. } => points.iter().for_each(|p| take(p[0], p[1])),
                Curve2::RationalSpline { homogeneous, .. } => {
                    homogeneous
                        .iter()
                        .for_each(|h| take(h[0] / h[2], h[1] / h[2]));
                }
            }
        }
    }
    box_.map(|[low, high]| [low - BREP_TOLERANCE, high + BREP_TOLERANCE])
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
    // Closed as the engine's record counts it: each edge used at least twice, so a shell where
    // three or four faces meet at an edge is closed too.
    let closed: Vec<bool> = shells
        .iter()
        .map(|faces_in| {
            let inside: Vec<usize> = faces_in.iter().map(|f| f.face as usize).collect();
            users
                .iter()
                .filter(|list| list.iter().any(|f| inside.contains(f)))
                .all(|list| list.len() >= 2)
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
    // Far from the origin the sweep can fail to close (an elliptical tube 11 km out, in one of
    // the owner's files): made in the profile plane at the origin, then moved into place whole,
    // which keeps its faces as made.
    if let Some(body) = extruded_at_origin(e, length, &placement) {
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

/// The extrusion made in its profile plane, along z from the origin, then placed.
fn extruded_at_origin(
    e: &model::Extrusion,
    length: f64,
    placement: &[[f64; 4]; 4],
) -> Option<Body> {
    let profiles = e
        .profiles
        .iter()
        .map(curve)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let cap = e.caps[0] && e.caps[1] && profiles.iter().all(|p| p.is_closed().unwrap_or(false));
    Body::extruded(&profiles, [0.0, 0.0, 1.0], length, cap)
        .ok()?
        .transformed(*placement)
        .ok()
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
mod extrusion_tests {
    use super::*;

    /// An extrusion as one of the owner's files has it: a capped elliptical tube 15 units tall,
    /// its profile a rational NURBS ellipse whose control points carry their weights already.
    fn elliptical_tube() -> model::Extrusion {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let (a, b) = (7.567_406_862_789_359, 6.479_482_245_657_572_5);
        let (ca, cb) = (5.350_964_708_675_974, 4.581_685_834_482_309);
        let q = std::f64::consts::FRAC_PI_8 * 2.0;
        model::Extrusion {
            path: [
                [
                    10_993.066_429_534_24,
                    24_028.907_386_955_64,
                    575.482_593_137_209_3,
                ],
                [
                    10_993.066_429_534_24,
                    24_028.907_386_955_64,
                    590.617_406_862_788_4,
                ],
            ],
            up: [0.986_292_529_026_102_1, -0.165_006_203_481_248_53, 0.0],
            profiles: vec![model::Curve::Nurbs(model::NurbsCurve {
                dimension: 2,
                order: 3,
                knots: vec![
                    0.0,
                    0.0,
                    q,
                    q,
                    2.0 * q,
                    2.0 * q,
                    3.0 * q,
                    3.0 * q,
                    4.0 * q,
                    4.0 * q,
                ],
                points: vec![
                    [a, 0.0, 0.0, 1.0],
                    [ca, cb, 0.0, w],
                    [0.0, b, 0.0, 1.0],
                    [-ca, cb, 0.0, w],
                    [-a, 0.0, 0.0, 1.0],
                    [-ca, -cb, 0.0, w],
                    [0.0, -b, 0.0, 1.0],
                    [ca, -cb, 0.0, w],
                    [a, 0.0, 0.0, 1.0],
                ],
                rational: true,
            })],
            caps: [true, true],
        }
    }

    #[test]
    fn a_capped_elliptical_tube_comes_in_as_a_solid() {
        let body = extrusion_body(&elliptical_tube()).unwrap();
        assert!(body.is_solid());
        assert_eq!(body.face_count().unwrap(), 3);
    }
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

    /// A plane face no wider than a rounding, as Rhino keeps them, gets a domain the engine
    /// takes: its trims inside, at least twice the tolerance across.
    #[test]
    fn a_sliver_plane_face_gets_a_domain_around_its_trims() {
        let side = |from: [f64; 2], to: [f64; 2]| OrientedEdge {
            edge: 0,
            forward: true,
            pcurve: Curve2::Spline {
                knots: vec![0.0, 0.0, 1.0, 1.0],
                points: vec![from, to],
            },
        };
        let (u, wide) = (4.100_000_000_041_291, 1.4e-11);
        let loops = vec![Loop::Edges {
            edges: vec![
                side([u, -0.245], [u + wide, -0.245]),
                side([u + wide, -0.245], [u + wide, 0.245]),
                side([u + wide, 0.245], [u - 3e-12, 0.245]),
                side([u - 3e-12, 0.245], [u, -0.245]),
            ],
        }];
        let domain = around_trims([[u, u + wide], [-0.245, 0.245]], &loops);
        assert_eq!(
            domain[0],
            [u - 3e-12 - BREP_TOLERANCE, u + wide + BREP_TOLERANCE]
        );
        assert_eq!(domain[1], [-0.245 - BREP_TOLERANCE, 0.245 + BREP_TOLERANCE]);
        assert!(domain[0][1] - domain[0][0] >= 2.0 * BREP_TOLERANCE);
    }

    /// Three faces meeting along both their edges, as where three walls of a Rhino solid share an
    /// edge: the engine counts the shell closed, so it bounds a region of its own. It used to be
    /// left out ("a closed shell bounds two regions and encloses one").
    #[test]
    fn a_shell_whose_edges_three_faces_share_is_closed() {
        let face = || Face {
            surface: RecordSurface::Plane {
                origin: [0.0; 3],
                u_axis: [1.0, 0.0, 0.0],
                v_axis: [0.0, 1.0, 0.0],
            },
            nurbs: None,
            domain: [[0.0, 1.0]; 2],
            loops: vec![Loop::Edges {
                edges: [0, 1]
                    .map(|edge| OrientedEdge {
                        edge,
                        forward: true,
                        pcurve: Curve2::Spline {
                            knots: vec![0.0, 0.0, 1.0, 1.0],
                            points: vec![[0.0, 0.0], [1.0, 0.0]],
                        },
                    })
                    .to_vec(),
            }],
        };
        let brep = Brep {
            faces: vec![
                model::BrepFace {
                    loops: Vec::new(),
                    surface: 0,
                    reversed: false,
                };
                3
            ],
            ..Brep::default()
        };
        let (shells, regions) = shells_in(&brep, &[face(), face(), face()], 2).unwrap();
        assert_eq!(shells.len(), 1);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[1].shells, vec![0]);
    }
}
