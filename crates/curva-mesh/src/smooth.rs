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
pub(crate) use subdiv_kernels::topology::Mesh as Cage;
use subdiv_kernels::{Refiner, Scheme, SchemeOptions, UniformRefine};

/// Patches, the subdivided quads they lie on, and whether each quad comes from a four-sided cage
/// face.
pub(crate) type PatchesOnQuads = (Vec<Patch>, Cage, Vec<bool>);

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
    pub(crate) fn refine(
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
        let keep = vec![false; targets.len()];
        self.fitted_keeping(targets, &keep)
    }

    /// [`SubD::fitted`], with the points marked in `keep` left where they are (Rhino's ToSubD leaves
    /// the points of flat, straight-sided faces on their corners).
    pub(crate) fn fitted_keeping(
        self,
        targets: &[Point],
        keep: &[bool],
    ) -> Result<Self, SmoothError> {
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
                if keep.get(v) == Some(&true) {
                    continue;
                }
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

    /// The smooth surface as bicubic Bézier patches, one per quad of the cage subdivided `levels`
    /// times (1 to [`MAX_LEVELS`]), each as its 4 × 4 control points (`net[i][j]`, `i` along the
    /// quad's first edge), and for each whether it is exact.
    ///
    /// Where the quad is regular (its corners meet four faces, no crease or border near), the
    /// library's bicubic B-spline patch is the smooth surface exactly, and is turned into Bézier
    /// form by the usual change of basis (1/6 [1 4 1 0; 0 4 2 0; 0 2 4 0; 0 1 4 1] each way).
    /// Elsewhere the patch passes through the true surface at the quad's 4 × 4 grid of thirds,
    /// found exactly by the library's limit evaluator. A patch's edge depends only on the four
    /// points along it, so neighbouring patches share their edges and meet without gaps.
    ///
    /// # Errors
    /// When the library refuses the cage.
    // CUSTOM MESH OPERATION: subd-feature-patches
    pub fn bezier_patches(&self, levels: u8) -> Result<Vec<Patch>, SmoothError> {
        Ok(self.patches_and_quads(levels)?.0)
    }

    /// [`SubD::bezier_patches`], the subdivided quads they lie on (one patch per quad, in order),
    /// and for each quad whether it comes from a four-sided cage face.
    pub(crate) fn patches_and_quads(&self, levels: u8) -> Result<PatchesOnQuads, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let table = result
            .patch_table()
            .map_err(|e| SmoothError(e.to_string()))?;
        #[allow(clippy::cast_possible_truncation)]
        let single: Vec<[f32; 3]> = refined.iter().map(|p| p.map(|c| c as f32)).collect();
        let evaluator = result
            .limit_evaluator(&single)
            .map_err(|e| SmoothError(e.to_string()))?;
        let to_world = |p: Point| -> Point { std::array::from_fn(|i| p[i] + centre[i]) };
        let faces = result.topology.face_vertex_counts.len();
        let mut out = Vec::with_capacity(faces);
        for f in 0..faces {
            let f32_index = u32::try_from(f).unwrap_or(u32::MAX);
            if let Some(patch) = table.face_patch(f32_index) {
                // Library order: entry 4 j + i at (i along u, j along v).
                let control = table.control_points[patch as usize];
                let b: [[Point; 4]; 4] = std::array::from_fn(|i| {
                    std::array::from_fn(|j| refined[control[4 * j + i] as usize])
                });
                out.push(Patch {
                    net: map_net(&b, &B_SPLINE_TO_BEZIER).map(|row| row.map(to_world)),
                    off: 0.0,
                });
            } else {
                let mut samples = [[[0.0; 3]; 4]; 4];
                for (i, row) in samples.iter_mut().enumerate() {
                    for (j, s) in row.iter_mut().enumerate() {
                        #[allow(clippy::cast_precision_loss)]
                        let uv = [i as f32 / 3.0, j as f32 / 3.0];
                        let weights = evaluator
                            .weights_at(f32_index, uv)
                            .map_err(|e| SmoothError(e.to_string()))?;
                        *s = weights.iter().fold([0.0; 3], |acc, &(v, w)| {
                            let p = refined[v as usize];
                            std::array::from_fn(|k| acc[k] + f64::from(w) * p[k])
                        });
                    }
                }
                let net = map_net(&samples, &THIRDS_TO_BEZIER);
                // How far it strays: checked midway between the points it passes through.
                let mut off: f64 = 0.0;
                for u in [1.0 / 6.0, 0.5, 5.0 / 6.0] {
                    for v in [1.0 / 6.0, 0.5, 5.0 / 6.0] {
                        #[allow(clippy::cast_possible_truncation)]
                        let weights = evaluator
                            .weights_at(f32_index, [u as f32, v as f32])
                            .map_err(|e| SmoothError(e.to_string()))?;
                        let truth = weights.iter().fold([0.0; 3], |acc, &(w_at, w)| {
                            let p = refined[w_at as usize];
                            std::array::from_fn(|k| acc[k] + f64::from(w) * p[k])
                        });
                        let ours = bezier_at(&net, u, v);
                        off = off.max(
                            (0..3)
                                .map(|k| (ours[k] - truth[k]).powi(2))
                                .sum::<f64>()
                                .sqrt(),
                        );
                    }
                }
                out.push(Patch {
                    net: net.map(|row| row.map(to_world)),
                    off,
                });
            }
        }
        let sides: Vec<usize> = self.cage().faces().map(<[u32]>::len).collect();
        let from_quads = result
            .face_root
            .iter()
            .map(|&root| sides.get(root as usize) == Some(&4))
            .collect();
        Ok((out, result.topology.clone(), from_quads))
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
        Ok(self.surface_and_roots(levels)?.0)
    }

    /// [`SubD::surface`] with each quad given its cage face's colour at every corner
    /// ([`Attribute::PerCorner`]), when the SubD has face colours; a face with none takes
    /// `fallback`. Without face colours the surface comes back uncoloured.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn surface_coloured(&self, levels: u8, fallback: [f32; 3]) -> Result<Mesh, SmoothError> {
        let (mesh, roots) = self.surface_and_roots(levels)?;
        let Some(colours) = self.face_colours() else {
            return Ok(mesh);
        };
        let per_corner = (0..mesh.face_count())
            .flat_map(|f| {
                let colour = roots
                    .get(f)
                    .and_then(|&root| colours.get(root as usize).copied().flatten())
                    .unwrap_or(fallback);
                std::iter::repeat_n(colour, mesh.face(f).len())
            })
            .collect();
        Ok(mesh
            .clone()
            .with_colours(Attribute::PerCorner(per_corner))
            .unwrap_or(mesh))
    }

    /// The smooth surface, and for each of its quads the cage face it lies in.
    pub(crate) fn surface_and_roots(&self, levels: u8) -> Result<(Mesh, Vec<u32>), SmoothError> {
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
        let roots = result.face_root.clone();
        Ok((facing(mesh), roots))
    }

    /// Every cage edge as it lies on the smooth surface, `levels` subdivisions fine, as runs of
    /// points joined end to end between cage points where other than two edges meet (a closed
    /// loop repeats its first point): how Rhino draws a smooth SubD in wireframe. Creases and
    /// borders are among them.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn edge_curves(&self, levels: u8) -> Result<Vec<Vec<Point>>, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let on_surface = on_surface(&result, centre, &refined)?;
        Ok(chains(&cage_edge_lines(&result))
            .into_iter()
            .map(|run| run.iter().map(|&v| on_surface[v as usize]).collect())
            .collect())
    }

    /// The creases and open borders on the smooth surface, `levels` subdivisions fine, each as a
    /// run of points (a closed one repeats its first point at the end): the lines the viewport
    /// draws on a smooth SubD.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn crease_curves(&self, levels: u8) -> Result<Vec<Vec<Point>>, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let on_surface = on_surface(&result, centre, &refined)?;
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

/// The refined points moved onto the limit surface, in world coordinates.
pub(crate) fn on_surface(
    result: &subdiv_kernels::RefinementResult,
    centre: Point,
    refined: &[Point],
) -> Result<Vec<Point>, SmoothError> {
    let limit = result
        .limit_stencils()
        .map_err(|e| SmoothError(e.to_string()))?;
    Ok(limit
        .position
        .interpolate(refined)
        .iter()
        .map(|p| std::array::from_fn(|i| p[i] + centre[i]))
        .collect())
}

/// The refined edges lying along cage edges: those between faces that come from different cage
/// faces, and those on the open border (one face). Every other refined edge lies inside one cage
/// face.
pub(crate) fn cage_edge_lines(result: &subdiv_kernels::RefinementResult) -> Vec<[u32; 2]> {
    let topology = &result.topology;
    let mut roots: std::collections::BTreeMap<(u32, u32), Vec<u32>> =
        std::collections::BTreeMap::new();
    let mut at = 0;
    for (face, &n) in topology.face_vertex_counts.iter().enumerate() {
        let corners = &topology.face_vertex_indices[at..at + n as usize];
        for k in 0..corners.len() {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            roots
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(result.face_root[face]);
        }
        at += n as usize;
    }
    roots
        .into_iter()
        .filter(|(_, faces)| faces.len() == 1 || faces.iter().any(|&f| f != faces[0]))
        .map(|((a, b), _)| [a, b])
        .collect()
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

/// A uniform cubic B-spline span's four control points as its Bézier points (rows: Bézier
/// points; columns: B-spline points).
const B_SPLINE_TO_BEZIER: [[f64; 4]; 4] = [
    [1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0, 0.0],
    [0.0, 4.0 / 6.0, 2.0 / 6.0, 0.0],
    [0.0, 2.0 / 6.0, 4.0 / 6.0, 0.0],
    [0.0, 1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0],
];

/// The Bézier points of the cubic through four values at 0, 1/3, 2/3 and 1: the inverse of the
/// Bernstein basis there, (1 − t)³, 3t(1 − t)², 3t²(1 − t), t³.
const THIRDS_TO_BEZIER: [[f64; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [-5.0 / 6.0, 3.0, -3.0 / 2.0, 1.0 / 3.0],
    [1.0 / 3.0, -3.0 / 2.0, 3.0, -5.0 / 6.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `m` applied along both directions of a 4 × 4 net: `m · net · mᵀ`.
fn map_net(net: &[[Point; 4]; 4], m: &[[f64; 4]; 4]) -> [[Point; 4]; 4] {
    std::array::from_fn(|a| {
        std::array::from_fn(|b| {
            let mut p = [0.0; 3];
            for (i, row) in net.iter().enumerate() {
                for (j, q) in row.iter().enumerate() {
                    let w = m[a][i] * m[b][j];
                    for k in 0..3 {
                        p[k] += w * q[k];
                    }
                }
            }
            p
        })
    })
}

/// A bicubic Bézier patch of a SubD's smooth surface ([`SubD::bezier_patches`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    /// Its 4 × 4 control points, `net[i][j]` with `i` along the quad's first edge.
    pub net: [[Point; 4]; 4],
    /// How far it strays from the true surface: 0 for an exact patch on a regular quad, else the
    /// largest distance found midway between the points it passes through.
    pub off: f64,
}

/// A bicubic Bézier patch at `(u, v)`.
fn bezier_at(net: &[[Point; 4]; 4], u: f64, v: f64) -> Point {
    let basis = |t: f64| {
        let s = 1.0 - t;
        [s * s * s, 3.0 * t * s * s, 3.0 * t * t * s, t * t * t]
    };
    let (bu, bv) = (basis(u), basis(v));
    let mut p = [0.0; 3];
    for (i, row) in net.iter().enumerate() {
        for (j, q) in row.iter().enumerate() {
            for k in 0..3 {
                p[k] += bu[i] * bv[j] * q[k];
            }
        }
    }
    p
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
