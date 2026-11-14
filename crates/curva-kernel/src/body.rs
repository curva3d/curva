//! Surfaces and solids: bodies made of faces, kept in the engine's own native body format so
//! that files keep them exactly.
//!
//! The native format (SMLib's `.smb`, written and read by the engine itself) is used rather
//! than the engine's portable record because the engine cannot always read back a record it
//! wrote: a thickened sheet, or a box turned and then moved, comes back with status 1007
//! (found 11 November 2026, to be reported upstream). The native format reads all of them
//! back. A body also keeps one placement for every move, turn, scale and mirror since it was
//! made, so moving is instant arithmetic.

use crate::{Curve, KernelError, Point, Similarity};
use serde::{Deserialize, Serialize};
use solidcore_ops::Body as EngineBody;
use solidcore_record::brep::Brep as Record;
use solidcore_tess::{MeshTolerance, plan};

/// The native format in its text form, which a `.curva` file can hold as it is.
const TEXT_FORMAT: bool = true;

/// How closely volumes and areas are worked out, as a share of the answer.
const MEASURE_ACCURACY: f64 = 1e-6;

/// A placement as the engine takes it: a point is a row, `p' = p·A + t`, with rows 0 to 2
/// where the axes go and row 3 the move.
type Placement = [[f64; 4]; 4];

const IN_PLACE: Placement = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn in_place() -> Placement {
    IN_PLACE
}

fn is_in_place(placement: &Placement) -> bool {
    *placement == IN_PLACE
}

pub(crate) fn failed(what: &str, error: impl std::fmt::Debug) -> KernelError {
    KernelError(format!("{what} failed: {error:?}"))
}

/// `first` followed by `then`.
fn followed_by(first: &Placement, then: &Placement) -> Placement {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| first[i][k] * then[k][j]).sum()))
}

/// A surface or solid: one or more faces joined along their edges.
///
/// The engine's body is read once (when made, or when a file opens) and shared between copies
/// behind an `Arc`, so copying a body, as every undo step does, copies no faces, and asking it
/// something reads nothing again. The native text a file holds is written only when the body is
/// first saved or compared (`docs/reviews/code-health-2026-11-14.md`, fix 1).
#[derive(Clone)]
pub struct Body {
    shared: std::sync::Arc<Shared>,
    placement: Placement,
}

/// What every copy of a body shares: the engine's body as made, and what is worked out from it
/// once.
struct Shared {
    /// The faces as made, without the placement. Never changed: operations work on a copy.
    engine: EngineBody,
    /// The faces in the engine's native text, written when first needed.
    native: std::sync::OnceLock<String>,
    /// The faces as the engine's portable record, without the placement, read when first needed.
    record: std::sync::OnceLock<Record>,
    /// The faces as made, without the placement, meshed for drawing, when first needed.
    mesh: std::sync::OnceLock<BodyMesh>,
    /// The box around the faces as made, without the placement, when first needed.
    bounds: std::sync::OnceLock<(Point, Point)>,
    /// The volume and the area of the faces as made, when first needed.
    volume: std::sync::OnceLock<f64>,
    area: std::sync::OnceLock<f64>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("placement", &self.placement)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Body {
    /// Two bodies are equal when they are placed alike and their faces are the same: at once
    /// when they share them, else by their native text.
    fn eq(&self, other: &Self) -> bool {
        self.placement == other.placement
            && (std::sync::Arc::ptr_eq(&self.shared, &other.shared)
                || matches!((self.native(), other.native()), (Ok(a), Ok(b)) if a == b))
    }
}

/// A body as a file holds it. Files from 10 and 11 November 2026 hold the portable record
/// instead; those are read once through it and saved the new way.
#[derive(Serialize, Deserialize)]
struct StoredBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record: Option<Record>,
    #[serde(default = "in_place", skip_serializing_if = "is_in_place")]
    placement: Placement,
}

impl<'de> Deserialize<'de> for Body {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = StoredBody::deserialize(deserializer)?;
        let body = match (stored.native, stored.record) {
            (Some(native), _) => Self::from_native(native),
            (None, Some(record)) => EngineBody::from_record(&record, true)
                .map_err(|e| format!("{e:?}"))
                .map_err(KernelError)
                .and_then(|(engine, _)| Self::made(&engine))
                .map_err(|e| e.0),
            (None, None) => Err("a body without faces".to_owned()),
        };
        let mut body = body.map_err(serde::de::Error::custom)?;
        body.placement = stored.placement;
        Ok(body)
    }
}

impl Serialize for Body {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let native = self.native().map_err(serde::ser::Error::custom)?;
        StoredBody {
            native: Some(native.to_owned()),
            record: None,
            placement: self.placement,
        }
        .serialize(serializer)
    }
}

/// A measurement kept in `slot`: worked out by `measure` the first time, then read.
fn measured(
    slot: &std::sync::OnceLock<f64>,
    measure: impl FnOnce() -> Result<f64, KernelError>,
) -> Result<f64, KernelError> {
    if let Some(value) = slot.get() {
        return Ok(*value);
    }
    let value = measure()?;
    let _ = slot.set(value);
    Ok(value)
}

pub(crate) fn native_of(body: &EngineBody) -> Result<String, KernelError> {
    let bytes = body
        .to_smb(TEXT_FORMAT)
        .map_err(|e| failed("storing a body", e))?;
    String::from_utf8(bytes).map_err(|e| failed("storing a body", e))
}

/// What a body looks like on screen: triangles to shade and edges to draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyMesh {
    /// Corner positions of the triangles.
    pub positions: Vec<Point>,
    /// One normal per position, pointing out of the body.
    pub normals: Vec<Point>,
    /// Triangles as three indices into `positions`.
    pub triangles: Vec<[u32; 3]>,
    /// The body's edges as runs of points; seams inside a smooth face are left out.
    pub edges: Vec<Vec<Point>>,
    /// For each of `edges`, its number among the body's edges, for picking it.
    pub edge_ids: Vec<usize>,
    /// For each of `triangles`, the number of the face it is part of, for picking faces.
    pub triangle_faces: Vec<usize>,
}

impl Body {
    /// A body the engine has just made.
    ///
    /// It is written to the native text and read back once, here: the engine cannot describe
    /// some fresh faces (an offset sheet's, found 14 November 2026: "a face surface with no
    /// NURBS form has no exact record") until its reader has turned them into NURBS. The text
    /// is kept, so saving writes nothing again.
    pub(crate) fn made(body: &EngineBody) -> Result<Self, KernelError> {
        Self::from_native(native_of(body)?).map_err(KernelError)
    }

    fn from_engine(body: &EngineBody) -> Result<Self, KernelError> {
        Self::made(body)
    }

    /// A body the engine has already read from its native text, in place.
    fn held(engine: EngineBody) -> Self {
        Self {
            shared: std::sync::Arc::new(Shared {
                engine,
                native: std::sync::OnceLock::new(),
                record: std::sync::OnceLock::new(),
                mesh: std::sync::OnceLock::new(),
                bounds: std::sync::OnceLock::new(),
                volume: std::sync::OnceLock::new(),
                area: std::sync::OnceLock::new(),
            }),
            placement: IN_PLACE,
        }
    }

    /// The body as the engine holds it, in its place, for operations elsewhere in the kernel.
    pub(crate) fn engine_body(&self) -> Result<EngineBody, KernelError> {
        self.engine()
    }

    /// A body from the engine's native text, in place. The text is read once, here.
    pub(crate) fn from_native(native: String) -> Result<Self, String> {
        let engine = EngineBody::from_smb(native.as_bytes().to_vec(), TEXT_FORMAT)
            .map_err(|e| format!("reading a body failed: {e:?}"))?;
        let body = Self::held(engine);
        let _ = body.shared.native.set(native);
        Ok(body)
    }

    /// The faces in the engine's native text, as a file holds them; written the first time it
    /// is asked for, then kept.
    pub(crate) fn native(&self) -> Result<&str, KernelError> {
        if self.shared.native.get().is_none() {
            let _ = self.shared.native.set(native_of(&self.shared.engine)?);
        }
        Ok(self.shared.native.get().map_or("", String::as_str))
    }

    /// The faces in place, as the engine's portable record, for reading their edges.
    pub(crate) fn record(&self) -> Result<Record, KernelError> {
        if is_in_place(&self.placement) {
            return self.unplaced_record().cloned();
        }
        let (record, _) = self
            .engine()?
            .to_record()
            .map_err(|e| failed("describing a body", e))?;
        Ok(record)
    }

    /// The faces as made, without the placement, as the engine's portable record; read the
    /// first time it is asked for, then kept.
    fn unplaced_record(&self) -> Result<&Record, KernelError> {
        if self.shared.record.get().is_none() {
            let (record, _) = self
                .shared
                .engine
                .to_record()
                .map_err(|e| failed("describing a body", e))?;
            let _ = self.shared.record.set(record);
        }
        self.shared
            .record
            .get()
            .ok_or_else(|| KernelError("describing a body failed".to_owned()))
    }

    /// The faces as made, without the placement.
    fn unplaced(&self) -> Self {
        Self {
            shared: std::sync::Arc::clone(&self.shared),
            placement: IN_PLACE,
        }
    }

    /// The body as the engine holds it, in its place: a copy of the shared one, which
    /// operations may change freely.
    fn engine(&self) -> Result<EngineBody, KernelError> {
        let body = self
            .shared
            .engine
            .copy()
            .map_err(|e| failed("copying a body", e))?;
        if !is_in_place(&self.placement) {
            body.transform(&self.placement)
                .map_err(|e| failed("placing a body", e))?;
        }
        Ok(body)
    }

    /// How much the placement enlarges lengths.
    fn scale(&self) -> f64 {
        self.placement[0][..3]
            .iter()
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt()
    }

    fn placed_point(&self, p: Point) -> Point {
        let m = &self.placement;
        std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
    }

    fn placed_direction(&self, d: Point) -> Point {
        let m = &self.placement;
        let turned: Point =
            std::array::from_fn(|j| d[0] * m[0][j] + d[1] * m[1][j] + d[2] * m[2][j]);
        crate::unit(turned).unwrap_or(turned)
    }

    /// A solid box with one corner at `origin` and sides along the world axes. Negative sizes
    /// run the other way, so any two opposite corners give the same box.
    pub fn cuboid(origin: Point, size: Point) -> Result<Self, KernelError> {
        let mut low = origin;
        let mut extent = size;
        for axis in 0..3 {
            if extent[axis] < 0.0 {
                low[axis] += extent[axis];
                extent[axis] = -extent[axis];
            }
        }
        let body = solidcore_ops::make_box(low, extent).map_err(|e| failed("a box", e))?;
        Self::from_engine(&body)
    }

    /// A solid box with one corner at `corner`, its sides along the directions `x` and `y`
    /// (at right angles, of length 1) and the direction straight out of them. `size` is how far
    /// it reaches along each; negative values reach the other way.
    pub fn cuboid_on(corner: Point, x: Point, y: Point, size: Point) -> Result<Self, KernelError> {
        let upright = Self::cuboid(corner, size)?;
        let plus = |direction: Point| [0, 1, 2].map(|k| corner[k] + direction[k]);
        let world = [corner, plus([1.0, 0.0, 0.0]), plus([0.0, 1.0, 0.0])];
        let on_plane = [corner, plus(x), plus(y)];
        match Similarity::between_frames(corner, world, on_plane) {
            Some(turn) => Ok(upright.mapped(&turn)),
            None => Err(KernelError("the box's plane has no direction".to_owned())),
        }
    }

    /// The body moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        let mut moved = self.clone();
        for (axis, amount) in by.iter().enumerate() {
            moved.placement[3][axis] += amount;
        }
        moved
    }

    /// The body turned, scaled or mirrored.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let origin = change.point([0.0; 3]);
        let row = |axis: Point| {
            let p = change.point(axis);
            [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2], 0.0]
        };
        let then = [
            row([1.0, 0.0, 0.0]),
            row([0.0, 1.0, 0.0]),
            row([0.0, 0.0, 1.0]),
            [origin[0], origin[1], origin[2], 1.0],
        ];
        Self {
            shared: std::sync::Arc::clone(&self.shared),
            placement: followed_by(&self.placement, &then),
        }
    }

    /// The body stretched by `factor` along the direction `axis`, about `origin`, as Rhino's
    /// Scale1D and ScaleNU do. Round faces become NURBS, as an uneven scale needs. The result
    /// keeps its new shape as made, with no placement.
    pub fn stretched(&self, origin: Point, axis: Point, factor: f64) -> Result<Self, KernelError> {
        let a = crate::unit(axis)
            .ok_or_else(|| KernelError("the direction to scale along has no length".to_owned()))?;
        // A = I + (f − 1)·a·aᵀ stretches along `a` only; it is symmetric, so rows and columns
        // agree, and the translation keeps `origin` where it is: t = o − o·A.
        let k = factor - 1.0;
        let m: [[f64; 3]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| f64::from(u8::from(i == j)) + k * a[i] * a[j])
        });
        let moved: Point =
            std::array::from_fn(|j| origin[j] - (0..3).map(|i| origin[i] * m[i][j]).sum::<f64>());
        let matrix = [
            [m[0][0], m[0][1], m[0][2], 0.0],
            [m[1][0], m[1][1], m[1][2], 0.0],
            [m[2][0], m[2][1], m[2][2], 0.0],
            [moved[0], moved[1], moved[2], 1.0],
        ];
        let body = self.engine()?;
        body.transform(&matrix)
            .map_err(|e| failed("scaling a body unevenly", e))?;
        Self::from_engine(&body)
    }

    /// Whether it closes in a volume, as a solid does, rather than being an open surface.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        // Closed means every edge is shared by two faces (a seam counts twice on its own face),
        // which is quick to read; measuring the volume is the fallback.
        match self.unplaced_record() {
            Ok(record) => {
                !record.faces.is_empty() && record.edges.iter().all(|edge| edge.uses.len() >= 2)
            }
            Err(_) => self.volume().is_ok_and(|volume| volume > 0.0),
        }
    }

    /// The enclosed volume; zero for an open surface. Worked out on the faces as made, then
    /// scaled, which is exact.
    pub fn volume(&self) -> Result<f64, KernelError> {
        let volume = measured(&self.shared.volume, || {
            self.shared
                .engine
                .volume(MEASURE_ACCURACY)
                .map_err(|e| failed("measuring volume", e))
        })?;
        Ok(volume * self.scale().powi(3))
    }

    /// The centre of its volume, or for a surface (no volume) of its area, integrated from the
    /// exact faces by the engine.
    pub fn centroid(&self) -> Result<Point, KernelError> {
        let properties = self
            .engine()?
            .mass_properties(MEASURE_ACCURACY)
            .map_err(|e| failed("finding the centroid", e))?;
        Ok(properties.centroid)
    }

    /// The area of all its faces.
    pub fn area(&self) -> Result<f64, KernelError> {
        let area = measured(&self.shared.area, || {
            self.shared
                .engine
                .area(MEASURE_ACCURACY)
                .map_err(|e| failed("measuring area", e))
        })?;
        Ok(area * self.scale().powi(2))
    }

    /// The lowest and highest corners of a box around it.
    pub fn bounds(&self) -> Result<(Point, Point), KernelError> {
        // A move, or a scale or mirror along the axes, keeps the box square to the axes, so the
        // box as made, placed, is still exact; a turn needs the faces placed and measured again.
        let m = &self.placement;
        let square = (0..3).all(|i| (0..3).all(|j| i == j || m[i][j] == 0.0));
        if square {
            let (low, high) = self.unplaced_bounds()?;
            let (a, b) = (self.placed_point(low), self.placed_point(high));
            return Ok((
                std::array::from_fn(|k| a[k].min(b[k])),
                std::array::from_fn(|k| a[k].max(b[k])),
            ));
        }
        self.engine()?
            .bounds()
            .map_err(|e| failed("measuring a body", e))
    }

    /// About how many bytes its faces hold, shared by every copy, for weighing undo steps:
    /// the native text and the mesh, when they have been made.
    #[must_use]
    pub fn held_bytes(&self) -> usize {
        let text = self.shared.native.get().map_or(0, String::len);
        let mesh = self.shared.mesh.get().map_or(0, |m| {
            (m.positions.len() + m.normals.len()) * 24
                + m.triangles.len() * 12
                + m.edges.iter().map(|e| e.len() * 24).sum::<usize>()
        });
        // The engine's own structure is about the size of its text.
        2 * text + mesh
    }

    /// Identifies the faces it shares with its copies: two bodies with the same key hold the
    /// same faces in memory once.
    #[must_use]
    pub fn faces_key(&self) -> usize {
        std::sync::Arc::as_ptr(&self.shared) as usize
    }

    /// How many faces, edges and corners it has.
    pub fn counts(&self) -> Result<[usize; 3], KernelError> {
        self.shared
            .engine
            .counts()
            .map_err(|e| failed("counting faces", e))
    }

    /// Triangles and edges for drawing, fine enough to look smooth at the body's size. The
    /// faces as made are meshed, then the mesh is placed.
    pub fn mesh(&self) -> Result<BodyMesh, KernelError> {
        let made = self.unplaced_mesh()?;
        if is_in_place(&self.placement) {
            return Ok(made.clone());
        }
        let point = |p: &Point| self.placed_point(*p);
        let direction = |d: &Point| self.placed_direction(*d);
        Ok(BodyMesh {
            positions: made.positions.iter().map(point).collect(),
            normals: made.normals.iter().map(direction).collect(),
            triangles: made.triangles.clone(),
            edges: made
                .edges
                .iter()
                .map(|run| run.iter().map(point).collect())
                .collect(),
            edge_ids: made.edge_ids.clone(),
            triangle_faces: made.triangle_faces.clone(),
        })
    }

    /// The faces as made, without the placement, meshed the first time it is asked for, then
    /// kept: picking an edge or drawing a moved copy meshes nothing again.
    fn unplaced_mesh(&self) -> Result<&BodyMesh, KernelError> {
        if self.shared.mesh.get().is_none() {
            let (low, high) = self.unplaced_bounds()?;
            let record = self.unplaced_record()?;
            let size = (0..3)
                .map(|axis| (high[axis] - low[axis]).powi(2))
                .sum::<f64>()
                .sqrt();
            let (tess, _) = plan(record, &MeshTolerance::display_of(size), size)
                .map_err(|e| failed("meshing a body", e))?;
            let (mesh, _) = tess.evaluate();
            let kept: Vec<usize> = (0..mesh.edges.len())
                .filter(|index| !mesh.seams.get(*index).copied().unwrap_or(false))
                .collect();
            let point = |v: &solidcore_record::Vec3| [v.x, v.y, v.z];
            let _ = self.shared.mesh.set(BodyMesh {
                positions: mesh.positions.iter().map(point).collect(),
                normals: mesh.normals.iter().map(point).collect(),
                triangles: mesh.triangles.clone(),
                edges: kept
                    .iter()
                    .map(|&index| mesh.edges[index].iter().map(point).collect())
                    .collect(),
                edge_ids: kept,
                triangle_faces: mesh.tri_face.iter().map(|&face| face as usize).collect(),
            });
        }
        self.shared
            .mesh
            .get()
            .ok_or_else(|| KernelError("meshing a body failed".to_owned()))
    }

    /// The box around the faces as made, without the placement; measured the first time it is
    /// asked for, then kept.
    fn unplaced_bounds(&self) -> Result<(Point, Point), KernelError> {
        if let Some(bounds) = self.shared.bounds.get() {
            return Ok(*bounds);
        }
        let bounds = self
            .shared
            .engine
            .bounds()
            .map_err(|e| failed("measuring a body", e))?;
        let _ = self.shared.bounds.set(bounds);
        Ok(bounds)
    }

    /// The body with the edges numbered `edges` (as in [`BodyMesh::edge_ids`]) rounded to
    /// `size`, or with `chamfer`, bevelled by it.
    pub fn blended(&self, edges: &[usize], size: f64, chamfer: bool) -> Result<Self, KernelError> {
        use solidcore_ops::{Blend, BlendOptions, Corners, Turn, blend_ordered};
        let unplaced = self.unplaced().engine()?;
        let (_, order) = unplaced
            .to_record()
            .map_err(|e| failed("reading edges", e))?;
        let kind = if chamfer {
            Blend::Chamfer
        } else {
            Blend::Round
        };
        blend_ordered(
            &unplaced,
            &order,
            kind,
            edges,
            &[size],
            &[],
            Corners::Filled,
            BlendOptions {
                turn: Turn::Mitre,
                ..BlendOptions::default()
            },
        )
        .map_err(|e| failed(if chamfer { "chamfering" } else { "filleting" }, e))?;
        Ok(Self {
            placement: self.placement,
            ..Self::made(&unplaced)?
        })
    }

    /// The solid hollowed to a wall `thickness` thick, inward, with the faces numbered `open`
    /// (as in [`BodyMesh::triangle_faces`]) taken away to open it.
    pub fn shelled(&self, open: &[usize], thickness: f64) -> Result<Self, KernelError> {
        let unplaced = self.unplaced().engine()?;
        let (_, order) = unplaced
            .to_record()
            .map_err(|e| failed("reading faces", e))?;
        // The engine's wall is outward for a positive thickness; Rhino's Shell goes inward.
        solidcore_ops::shell(&unplaced, &order, open, -thickness.abs())
            .map_err(|e| failed("shelling", e))?;
        Ok(Self {
            placement: self.placement,
            ..Self::made(&unplaced)?
        })
    }

    /// The edge numbered `index` (as in [`BodyMesh::edge_ids`]) as an exact curve: a line, an
    /// arc, an ellipse's part or the NURBS it lies on, trimmed to the edge. Edges the engine
    /// holds only as a traced intersection come back as the polyline through their mesh points.
    pub fn edge_curve(&self, index: usize) -> Result<Curve, KernelError> {
        use solidcore_record::brep::Curve as Kind;
        let record = self.record()?;
        let edge = record
            .edges
            .get(index)
            .ok_or_else(|| KernelError(format!("the body has no edge {index}")))?;
        let mesh = self.mesh()?;
        let traced = mesh
            .edge_ids
            .iter()
            .position(|id| *id == index)
            .and_then(|at| mesh.edges.get(at))
            .cloned()
            .unwrap_or_default();
        let corner = |at: usize| {
            record
                .vertices
                .get(edge.vertices[at] as usize)
                .map(|v| v.point)
                .ok_or_else(|| KernelError("an edge without its vertices".to_owned()))
        };
        let (start, end) = (corner(0)?, corner(1)?);
        let spline = |knots: &[f64], points: Vec<Point>, weights: Vec<f64>| {
            let degree = knots.len().saturating_sub(points.len() + 1);
            Curve::nurbs(degree, knots.to_vec(), points, weights)
        };
        let shape = edge.nurbs.as_ref().unwrap_or(&edge.curve);
        let whole = match shape {
            Kind::Line { .. } => return Curve::line(start, end),
            Kind::Circle { frame, radius } => {
                Curve::circle_on(frame.origin, *radius, frame.x_axis, frame.y_axis)?
            }
            Kind::Ellipse { frame, radii } => Curve::ellipse(
                frame.origin,
                frame.x_axis.map(|v| v * radii[0]),
                frame.y_axis.map(|v| v * radii[1]),
            )?,
            Kind::Polyline { points } => Curve::polyline(points.clone(), false)?,
            Kind::Spline { knots, points } => spline(knots, points.clone(), Vec::new())?,
            Kind::RationalSpline { knots, homogeneous } => spline(
                knots,
                homogeneous
                    .iter()
                    .map(|h| [h[0] / h[3], h[1] / h[3], h[2] / h[3]])
                    .collect(),
                homogeneous.iter().map(|h| h[3]).collect(),
            )?,
            Kind::SurfaceIntersection { .. } => return Curve::polyline(traced, false),
        };
        // The piece of the whole curve between the edge's ends that runs where the edge's mesh
        // points do.
        let closed_edge = crate::gap(start, end) < 1e-9;
        let pieces = if closed_edge {
            whole.cut_at(&[start])?
        } else {
            whole.cut_at(&[start, end])?
        };
        let middle = traced.get(traced.len() / 2).copied();
        let off = |piece: &Curve| -> f64 {
            let Some(middle) = middle else { return 0.0 };
            piece
                .sample()
                .map(|points| {
                    points
                        .iter()
                        .map(|p| crate::gap(*p, middle))
                        .fold(f64::INFINITY, f64::min)
                })
                .unwrap_or(f64::INFINITY)
        };
        let ends_match = |piece: &Curve| {
            piece.sample().is_ok_and(|points| {
                let (Some(a), Some(b)) = (points.first(), points.last()) else {
                    return false;
                };
                let near =
                    |p: Point, q: Point| crate::gap(p, q) < 1e-6 * (1.0 + crate::gap(start, end));
                closed_edge
                    || (near(*a, start) && near(*b, end))
                    || (near(*a, end) && near(*b, start))
            })
        };
        pieces
            .into_iter()
            .filter(ends_match)
            .min_by(|a, b| off(a).total_cmp(&off(b)))
            .ok_or_else(|| KernelError("could not trim the edge's curve".to_owned()))
    }
    /// The edges as curves, for drawing in wireframe, picking and snapping.
    pub fn edge_curves(&self) -> Result<Vec<Curve>, KernelError> {
        Ok(self
            .mesh()?
            .edges
            .into_iter()
            .filter_map(|run| match run.len() {
                0 | 1 => None,
                2 => Curve::line(run[0], run[1]).ok(),
                _ => Curve::polyline(run, false).ok(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use crate::{Similarity, gap};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    #[test]
    fn a_cones_centroid_is_a_quarter_up_from_its_base() {
        // A right triangle turned about the z axis: a cone of radius 3 and height 8, then moved.
        let outline = crate::Curve::polyline(
            vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 0.0, 8.0]],
            false,
        )
        .unwrap();
        let cone = Body::revolved(
            std::slice::from_ref(&outline),
            [0.0; 3],
            [0.0, 0.0, 1.0],
            std::f64::consts::TAU,
            false,
        )
        .unwrap()
        .translated([5.0, 0.0, 0.0]);
        let at = cone.centroid().unwrap();
        assert!(gap(at, [5.0, 0.0, 2.0]) < 1e-6, "{at:?}");
    }

    #[test]
    fn each_edge_comes_back_exact_and_the_one_asked_for() {
        // A box moved away from the origin: every edge is a straight line of its true length,
        // and runs where the mesh draws that edge.
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 5.0])
            .unwrap()
            .translated([10.0, 0.0, 0.0]);
        let mesh = cube.mesh().unwrap();
        for (run, id) in mesh.edges.iter().zip(&mesh.edge_ids) {
            let curve = cube.edge_curve(*id).unwrap();
            assert!(matches!(curve, crate::Curve::Line { .. }), "{curve:?}");
            let points = curve.sample().unwrap();
            let (a, b) = (run[0], run[run.len() - 1]);
            assert!(
                (gap(points[0], a) < 1e-9 && gap(points[1], b) < 1e-9)
                    || (gap(points[0], b) < 1e-9 && gap(points[1], a) < 1e-9)
            );
        }
        // A cylinder's rims are whole circles, exactly 2πr long.
        let circle = crate::Curve::circle([0.0; 3], 4.0).unwrap();
        let cylinder = Body::extruded(&[circle], [0.0, 0.0, 1.0], 6.0, true).unwrap();
        let mesh = cylinder.mesh().unwrap();
        let rims = mesh
            .edge_ids
            .iter()
            .filter_map(|id| cylinder.edge_curve(*id).ok())
            .filter(|curve| curve.is_closed().unwrap_or(false))
            .collect::<Vec<_>>();
        assert_eq!(rims.len(), 2);
        for rim in rims {
            let length = rim.length().unwrap();
            assert!(close(length, 8.0 * std::f64::consts::PI), "{length}");
        }
    }

    #[test]
    fn stretching_a_box_unevenly_multiplies_its_volume() {
        let cube = Body::cuboid([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]).unwrap();
        let wide = cube.stretched([0.0; 3], [1.0, 0.0, 0.0], 2.0).unwrap();
        let tall = wide.stretched([0.0; 3], [0.0, 0.0, 1.0], 3.0).unwrap();
        assert!(
            close(tall.volume().unwrap(), 6.0),
            "{}",
            tall.volume().unwrap()
        );
    }

    #[test]
    fn stretching_a_cylinder_makes_an_elliptic_one() {
        let ring = crate::Curve::circle([0.0; 3], 1.0).unwrap();
        let cylinder = Body::extruded(&[ring], [0.0, 0.0, 1.0], 1.0, true).unwrap();
        let oval = cylinder.stretched([0.0; 3], [1.0, 0.0, 0.0], 2.0).unwrap();
        let expected = 2.0 * std::f64::consts::PI;
        assert!((oval.volume().unwrap() - expected).abs() < 1e-4 * expected);
    }

    #[test]
    fn a_box_has_the_volume_and_area_of_its_sides() {
        let body = Body::cuboid([0.0; 3], [10.0, 20.0, 5.0]).unwrap();
        assert!(close(body.volume().unwrap(), 1000.0));
        assert!(close(body.area().unwrap(), 2.0 * (200.0 + 50.0 + 100.0)));
        assert_eq!(body.counts().unwrap(), [6, 12, 8]);
    }

    #[test]
    fn a_box_on_a_turned_plane_keeps_its_size() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let body =
            Body::cuboid_on([1.0, 2.0, 3.0], [s, s, 0.0], [-s, s, 0.0], [2.0, 3.0, -4.0]).unwrap();
        assert!(close(body.volume().unwrap(), 24.0));
        let (low, high) = body.bounds().unwrap();
        assert!(
            close(high[2], 3.0) && close(low[2], -1.0),
            "{low:?} {high:?}"
        );
    }

    #[test]
    fn opposite_corners_give_the_same_box() {
        let a = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let b = Body::cuboid([2.0, 3.0, 4.0], [-2.0, -3.0, -4.0]).unwrap();
        assert_eq!(a.bounds().unwrap(), b.bounds().unwrap());
    }

    #[test]
    fn moving_and_turning_keep_the_volume() {
        let body = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let moved = body.translated([10.0, 0.0, 0.0]);
        let (low, _) = moved.bounds().unwrap();
        assert!(close(low[0], 10.0));
        let turn = Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], 0.7);
        assert!(close(body.mapped(&turn).volume().unwrap(), 24.0));
        let doubled = Similarity::scaling([0.0; 3], 2.0);
        assert!(close(body.mapped(&doubled).volume().unwrap(), 192.0));
    }

    #[test]
    fn a_turned_box_can_be_moved_and_turned_again() {
        let body = Body::cuboid([1.0, 2.0, 3.0], [2.0, 3.0, 4.0]).unwrap();
        let about_corner = Similarity::rotation([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], 0.7);
        let turned = body.mapped(&about_corner);
        let moved = turned.translated([5.0, 1.0, 0.0]);
        let again = moved.mapped(&Similarity::rotation([0.0; 3], [1.0, 0.0, 0.0], 0.3));
        for b in [&turned, &moved, &again] {
            assert!(close(b.volume().unwrap(), 24.0));
            assert!(b.bounds().is_ok());
            assert_eq!(b.edge_curves().unwrap().len(), 12);
        }
        // Turning about its corner leaves that corner where it was, and moving takes it along.
        let (low, _) = turned.bounds().unwrap();
        assert!(close(low[1], 2.0) && close(low[2], 3.0), "{low:?}");
        let corners = moved.mesh().unwrap().positions;
        assert!(corners.iter().any(|p| gap(*p, [6.0, 3.0, 3.0]) < 1e-9));
    }

    #[test]
    fn a_fillet_on_one_edge_takes_off_the_corner_of_its_section() {
        let body = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let edge = body.mesh().unwrap().edge_ids[0];
        let round = body.blended(&[edge], 2.0, false).unwrap();
        let lost = (1.0 - std::f64::consts::PI / 4.0) * 4.0 * 10.0;
        assert!(
            close(round.volume().unwrap(), 1000.0 - lost),
            "{:?}",
            round.volume()
        );
        let bevel = body.blended(&[edge], 2.0, true).unwrap();
        assert!(close(bevel.volume().unwrap(), 1000.0 - 2.0 * 10.0));
    }

    #[test]
    fn a_turned_box_is_filleted_in_place() {
        let body = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0])
            .unwrap()
            .mapped(&Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], 0.5))
            .translated([20.0, 0.0, 0.0]);
        let edge = body.mesh().unwrap().edge_ids[3];
        let round = body.blended(&[edge], 1.0, false).unwrap();
        assert!(close(
            round.bounds().unwrap().0[2],
            body.bounds().unwrap().0[2]
        ));
        assert!(round.volume().unwrap() < 1000.0);
    }

    #[test]
    fn a_cube_shelled_with_its_top_open_keeps_its_walls() {
        let body = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let mesh = body.mesh().unwrap();
        // The top face: the one whose triangles all lie at z = 10.
        let top = (0..mesh.triangles.len())
            .find(|&t| {
                mesh.triangles[t]
                    .iter()
                    .all(|&c| (mesh.positions[c as usize][2] - 10.0).abs() < 1e-9)
            })
            .map(|t| mesh.triangle_faces[t])
            .unwrap();
        let shell = body.shelled(&[top], 1.0).unwrap();
        assert!(close(shell.volume().unwrap(), 1000.0 - 8.0 * 8.0 * 9.0));
    }

    #[test]
    fn a_box_shows_twelve_edges_and_shaded_faces() {
        let body = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        assert_eq!(body.edge_curves().unwrap().len(), 12);
        let mesh = body.mesh().unwrap();
        assert!(mesh.triangles.len() >= 12);
        assert_eq!(mesh.positions.len(), mesh.normals.len());
    }

    #[test]
    fn a_body_saved_as_a_portable_record_still_opens() {
        let body = Body::cuboid([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]).unwrap();
        let old = serde_json::json!({ "record": body.record().unwrap() });
        let back: Body = serde_json::from_value(old).unwrap();
        assert!(close(back.volume().unwrap(), 120.0));
    }

    #[test]
    fn a_body_comes_back_exactly_from_a_file() {
        let body = Body::cuboid([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])
            .unwrap()
            .mapped(&Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], 0.4));
        let text = serde_json::to_string(&body).unwrap();
        let back: Body = serde_json::from_str(&text).unwrap();
        assert_eq!(back, body);
    }
}
