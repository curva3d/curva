//! A SubD's smooth surface, on subdiv-kernels (decision 0032, spec `docs/specs/mesh-and-subd.md`
//! §3.2). This file is the only one that calls it.
//!
//! The library works out Catmull–Clark's weights (Rhino's SubD rule, matching Pixar's OpenSubdiv)
//! for the cage's shape, with its creases and corners, and Curva applies them to its own
//! double-precision points, measured from the cage's centre so a SubD far from the origin keeps
//! its digits. Each level's weights are applied in turn, not multiplied together first, so the
//! library's single-precision weights round as little as they can (see the precision test).

use crate::{Attribute, Mesh, Point, SubD, VertexTag};
use std::fmt;
use std::num::NonZeroU8;
use subdiv_kernels::topology::Mesh as Cage;
use subdiv_kernels::{Refiner, Scheme, SchemeOptions, UniformRefine};

/// Sharpness the library takes as infinite (OpenSubdiv's `SHARPNESS_INFINITE`).
const INFINITE: f32 = 10.0;

/// The most subdivision levels asked for: 6 turns each face into 4096 quads, finer than any
/// display needs.
pub const MAX_LEVELS: u8 = 6;

/// Why a SubD's surface could not be worked out. The text is for logs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmoothError(pub String);

impl fmt::Display for SmoothError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SmoothError {}

impl SubD {
    /// The cage as the library sees it: its faces, its edges with their sharpness, and its
    /// corners.
    fn library_cage(&self) -> Cage {
        let cage = self.cage();
        let edges: Vec<(u32, u32)> = self.edge_uses().into_keys().collect();
        Cage {
            vertex_count: u32::try_from(cage.vertex_count()).unwrap_or(u32::MAX),
            face_vertex_counts: cage
                .faces()
                .map(|f| u32::try_from(f.len()).unwrap_or(u32::MAX))
                .collect(),
            face_vertex_indices: cage.corners().to_vec(),
            edge_vertices: edges.iter().map(|&(a, b)| [a, b]).collect(),
            edge_creases: edges
                .iter()
                .map(|&(a, b)| {
                    let s = self.sharpness(a, b).value();
                    #[allow(clippy::cast_possible_truncation)]
                    let s = s.min(f64::from(INFINITE)) as f32;
                    s
                })
                .collect(),
            vertex_corners: (0..cage.vertex_count())
                .map(|v| {
                    let v = u32::try_from(v).unwrap_or(u32::MAX);
                    if self.vertex_tag(v) == VertexTag::Corner {
                        INFINITE
                    } else {
                        0.0
                    }
                })
                .collect(),
        }
    }

    /// The cage's centre and its points measured from it.
    fn centred_points(&self) -> (Point, Vec<Point>) {
        let centre = self.bounds().map_or([0.0; 3], |(lo, hi)| {
            std::array::from_fn(|i| f64::midpoint(lo[i], hi[i]))
        });
        let points = self
            .cage()
            .vertices()
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] - centre[i]))
            .collect();
        (centre, points)
    }

    /// The cage subdivided `levels` times (1 to [`MAX_LEVELS`]) by Catmull–Clark, as its quads,
    /// and the refinement it came from. Each refined face remembers its cage face.
    fn refine(
        &self,
        levels: u8,
    ) -> Result<(subdiv_kernels::RefinementResult, Point, Vec<Point>), SmoothError> {
        let levels = NonZeroU8::new(levels.clamp(1, MAX_LEVELS)).unwrap_or(NonZeroU8::MIN);
        let refiner = Refiner::new(
            self.library_cage(),
            Scheme::CatmullClark,
            SchemeOptions::default(),
        )
        .map_err(|e| SmoothError(e.to_string()))?;
        let result = refiner
            .refine_uniform(&UniformRefine::from(levels))
            .map_err(|e| SmoothError(e.to_string()))?;
        let (centre, points) = self.centred_points();
        let refined = result.interpolate(&points);
        Ok((result, centre, refined))
    }

    /// The same SubD with its cage points moved so that each one's point on the smooth surface
    /// lands on `targets` (one per cage point), as Rhino places its SubD spheres and tori: the
    /// surface then passes through the shape's own points.
    ///
    /// Each cage point's limit point is a fixed blend of it and its neighbours; the cage is found
    /// by moving every point by what its limit point still misses, again and again (a Jacobi
    /// iteration, which settles because each limit point leans mostly on its own cage point),
    /// until the largest miss is a ten-billionth of the shape's size.
    ///
    /// # Errors
    /// When the library refuses the cage, or the targets are not one per cage point.
    // CUSTOM MESH OPERATION: subd-limit-fit
    pub fn fitted(self, targets: &[Point]) -> Result<Self, SmoothError> {
        let cage = self.cage();
        if targets.len() != cage.vertex_count() {
            return Err(SmoothError(
                "one target is needed for each cage point".to_owned(),
            ));
        }
        let refiner = Refiner::new(
            self.library_cage(),
            Scheme::CatmullClark,
            SchemeOptions::default(),
        )
        .map_err(|e| SmoothError(e.to_string()))?;
        let result = refiner
            .refine_uniform(&UniformRefine::from(NonZeroU8::MIN))
            .map_err(|e| SmoothError(e.to_string()))?;
        let limit = result
            .limit_stencils()
            .map_err(|e| SmoothError(e.to_string()))?;
        // The refined point that stands for each cage point.
        let mut row = vec![usize::MAX; cage.vertex_count()];
        for (r, origin) in result.lineage.vertex_origin.iter().enumerate() {
            if let subdiv_kernels::VertexOrigin::Vertex(v) = *origin {
                row[v as usize] = r;
            }
        }
        if row.contains(&usize::MAX) {
            return Err(SmoothError("a cage point has no limit point".to_owned()));
        }
        let (lo, hi) = bounds_of(targets);
        let centre: Point = std::array::from_fn(|i| f64::midpoint(lo[i], hi[i]));
        let size = (0..3)
            .map(|i| hi[i] - lo[i])
            .fold(0.0, f64::max)
            .max(f64::MIN_POSITIVE);
        let goal: Vec<Point> = targets
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] - centre[i]))
            .collect();
        let mut points = goal.clone();
        for _ in 0..500 {
            let on_surface = limit.position.interpolate(&result.interpolate(&points));
            let mut worst: f64 = 0.0;
            for (v, p) in points.iter_mut().enumerate() {
                let miss: Point = std::array::from_fn(|i| goal[v][i] - on_surface[row[v]][i]);
                worst = miss.iter().fold(worst, |w, m| w.max(m.abs()));
                for i in 0..3 {
                    p[i] += miss[i];
                }
            }
            if worst <= size * 1e-10 {
                break;
            }
        }
        let placed: Vec<Point> = points
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] + centre[i]))
            .collect();
        Ok(self.with_cage_points(placed))
    }

    /// The cage subdivided `levels` times (1 to [`MAX_LEVELS`]): the control net one level
    /// finer, all quads, not yet on the limit surface.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn refined(&self, levels: u8) -> Result<Mesh, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        quads_mesh(&result.topology, centre, &refined)
    }

    /// The smooth surface itself, as quads `levels` subdivisions fine (1 to [`MAX_LEVELS`]),
    /// every point moved onto the limit surface and given its normal there: what a smooth SubD
    /// looks like, for display and for meshing.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn surface(&self, levels: u8) -> Result<Mesh, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let limit = result
            .limit_stencils()
            .map_err(|e| SmoothError(e.to_string()))?;
        let on_surface = limit.position.interpolate(&refined);
        let t1 = limit.tangent1.interpolate(&refined);
        let t2 = limit.tangent2.interpolate(&refined);
        let normals: Vec<Point> = t1
            .iter()
            .zip(&t2)
            .map(|(a, b)| unit(cross(*a, *b)))
            .collect();
        let mesh = quads_mesh(&result.topology, centre, &on_surface)?;
        let mesh = mesh
            .clone()
            .with_normals(Attribute::PerVertex(normals))
            .unwrap_or(mesh);
        // The library's tangents follow its own face order; turn the normals to face the way the
        // faces do.
        Ok(facing(mesh))
    }

    /// The creases and open borders on the smooth surface, `levels` subdivisions fine, each as a
    /// run of points (a closed one repeats its first point at the end): the lines the viewport
    /// draws on a smooth SubD.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn crease_curves(&self, levels: u8) -> Result<Vec<Vec<Point>>, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let limit = result
            .limit_stencils()
            .map_err(|e| SmoothError(e.to_string()))?;
        let on_surface: Vec<Point> = limit
            .position
            .interpolate(&refined)
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] + centre[i]))
            .collect();
        let topology = &result.topology;
        // How many refined faces use each refined edge, to find the open border.
        let mut uses: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        let mut at = 0;
        for &n in &topology.face_vertex_counts {
            let face = &topology.face_vertex_indices[at..at + n as usize];
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                *uses.entry((a.min(b), a.max(b))).or_default() += 1;
            }
            at += n as usize;
        }
        // A crease stays infinitely sharp at every level, so the refined edges still marked so
        // are exactly the cage's creases, subdivided.
        let lines: Vec<[u32; 2]> = topology
            .edge_vertices
            .iter()
            .zip(&topology.edge_creases)
            .filter(|&(&[a, b], &crease)| {
                crease >= INFINITE || uses.get(&(a.min(b), a.max(b))).copied() == Some(1)
            })
            .map(|(&e, _)| e)
            .collect();
        Ok(chains(&lines)
            .into_iter()
            .map(|run| run.iter().map(|&v| on_surface[v as usize]).collect())
            .collect())
    }
}

/// Edges joined end to end into runs: each run goes from a point where the lines do not simply
/// continue (an end, or where three or more meet) to the next such point; lines left over form
/// closed loops, which repeat their first point.
fn chains(lines: &[[u32; 2]]) -> Vec<Vec<u32>> {
    let mut next: std::collections::BTreeMap<u32, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, &[a, b]) in lines.iter().enumerate() {
        next.entry(a).or_default().push(i);
        next.entry(b).or_default().push(i);
    }
    let mut used = vec![false; lines.len()];
    let mut runs = Vec::new();
    let walk = |start: u32, first: usize, used: &mut Vec<bool>| {
        let mut run = vec![start];
        let (mut here, mut line) = (start, first);
        loop {
            used[line] = true;
            let [a, b] = lines[line];
            here = if a == here { b } else { a };
            run.push(here);
            let onward = &next[&here];
            if onward.len() != 2 {
                break;
            }
            match onward.iter().find(|&&l| !used[l]) {
                Some(&l) => line = l,
                None => break,
            }
        }
        run
    };
    let ends: Vec<u32> = next
        .iter()
        .filter(|(_, l)| l.len() != 2)
        .map(|(&v, _)| v)
        .collect();
    for v in ends {
        for &l in &next[&v].clone() {
            if !used[l] {
                runs.push(walk(v, l, &mut used));
            }
        }
    }
    for l in 0..lines.len() {
        if !used[l] {
            runs.push(walk(lines[l][0], l, &mut used));
        }
    }
    runs
}

/// A SubD's smooth surface ready to be worked out again for new cage positions, the cage's shape
/// unchanged: what dragging cage points needs (spec §2.2). The subdivision's layout and weights
/// are found once; each new set of positions only reapplies the weights, and the faces stay the
/// same, so the viewport can update its triangles in place.
pub struct LiveSurface {
    result: subdiv_kernels::RefinementResult,
    limit: subdiv_kernels::LimitStencils,
    faces: Mesh,
}

impl LiveSurface {
    /// The surface of `subd`, `levels` subdivisions fine (1 to [`MAX_LEVELS`]).
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn new(subd: &SubD, levels: u8) -> Result<Self, SmoothError> {
        let faces = subd.surface(levels)?;
        let (result, _, _) = subd.refine(levels)?;
        let limit = result
            .limit_stencils()
            .map_err(|e| SmoothError(e.to_string()))?;
        Ok(Self {
            result,
            limit,
            faces,
        })
    }

    /// The surface's faces, the same for every set of positions.
    #[must_use]
    pub fn mesh(&self) -> &Mesh {
        &self.faces
    }

    /// The surface's points and normals for the cage points `cage` (one per cage point, in the
    /// cage's order), in the order of [`LiveSurface::mesh`]'s points.
    #[must_use]
    pub fn positions(&self, cage: &[Point]) -> (Vec<Point>, Vec<Point>) {
        let (lo, hi) = cage.iter().fold(
            ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
            |(lo, hi), p| {
                (
                    std::array::from_fn(|i| lo[i].min(p[i])),
                    std::array::from_fn(|i| hi[i].max(p[i])),
                )
            },
        );
        let centre: Point = std::array::from_fn(|i| f64::midpoint(lo[i], hi[i]));
        let centred: Vec<Point> = cage
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] - centre[i]))
            .collect();
        let refined = self.result.interpolate(&centred);
        let points = self
            .limit
            .position
            .interpolate(&refined)
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] + centre[i]))
            .collect();
        let t1 = self.limit.tangent1.interpolate(&refined);
        let t2 = self.limit.tangent2.interpolate(&refined);
        let mut normals: Vec<Point> = t1
            .iter()
            .zip(&t2)
            .map(|(a, b)| unit(cross(*a, *b)))
            .collect();
        // Turned the way the surface's own normals were, so faces keep facing out.
        if let Attribute::PerVertex(first) = self.faces.normals() {
            let agree: f64 = first
                .iter()
                .zip(&normals)
                .map(|(a, b)| a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
                .sum();
            if agree < 0.0 {
                normals = normals.iter().map(|n| n.map(|c| -c)).collect();
            }
        }
        (points, normals)
    }
}

/// A refined level's quads as a mesh, its points moved back from `centre`.
fn quads_mesh(topology: &Cage, centre: Point, points: &[Point]) -> Result<Mesh, SmoothError> {
    let vertices = points
        .iter()
        .map(|p| std::array::from_fn(|i| p[i] + centre[i]))
        .collect();
    let mut faces = Vec::with_capacity(topology.face_vertex_counts.len());
    let mut at = 0;
    for &n in &topology.face_vertex_counts {
        let n = n as usize;
        faces.push(topology.face_vertex_indices[at..at + n].to_vec());
        at += n;
    }
    Mesh::new(vertices, &faces).map_err(|e| SmoothError(e.to_string()))
}

/// The mesh with its normals turned, all together, to agree with its faces.
fn facing(mesh: Mesh) -> Mesh {
    let Attribute::PerVertex(normals) = mesh.normals().clone() else {
        return mesh;
    };
    let agree: f64 = (0..mesh.face_count())
        .map(|f| {
            let face = mesh.face(f);
            let p: Vec<Point> = face.iter().map(|&v| mesh.vertices()[v as usize]).collect();
            let n = crate::triangulate::newell_normal(&p);
            let m = normals[face[0] as usize];
            n[0] * m[0] + n[1] * m[1] + n[2] * m[2]
        })
        .sum();
    if agree >= 0.0 {
        return mesh;
    }
    let turned = normals.iter().map(|n| n.map(|c| -c)).collect();
    mesh.clone()
        .with_normals(Attribute::PerVertex(turned))
        .unwrap_or(mesh)
}

/// The lowest and highest of some points in each direction.
fn bounds_of(points: &[Point]) -> (Point, Point) {
    points.iter().fold(
        ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
        |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        },
    )
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: Point) -> Point {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 { v.map(|c| c / l) } else { v }
}

#[cfg(test)]
#[path = "smooth_tests.rs"]
mod tests;
