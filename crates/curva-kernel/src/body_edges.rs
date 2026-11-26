//! A body's edges as exact curves: one edge (`Body::edge_curve`), all of them in the mesh's
//! order for snapping, ties and printing (`Body::edge_curves`, #792), and with the faces that
//! meet at each, for drawings (`Body::exact_edges`).

use crate::body::Body;
use crate::{Curve, KernelError, Point};
use solidcore_record::brep::Brep as Record;

impl Body {
    /// The edge numbered `index` (as in [`crate::BodyMesh::edge_ids`]) as an exact curve: a line, an
    /// arc, an ellipse's part or the NURBS it lies on, trimmed to the edge. Edges the engine
    /// holds only as a traced intersection come back as the polyline through their mesh points.
    pub fn edge_curve(&self, index: usize) -> Result<Curve, KernelError> {
        let record = self.record()?;
        let mesh = self.mesh()?;
        let traced = mesh
            .edge_ids
            .iter()
            .position(|id| *id == index)
            .and_then(|at| mesh.edges.get(at))
            .cloned()
            .unwrap_or_default();
        exact_edge(&record, index, traced)
    }

    /// The edges as their exact curves, one for each of the mesh's edges and in its order (curve
    /// `i` is edge [`crate::BodyMesh::edge_ids`]`[i]`), for picking, snapping, ties and printing: a
    /// circular edge is a circle, whose ends, middle and centre snap as Rhino's do (#792). An
    /// edge that can't be made exact is its meshed run of points. Drawing keeps the meshed runs.
    pub fn edge_curves(&self) -> Result<Vec<Curve>, KernelError> {
        let record = self.record()?;
        let mesh = self.mesh()?;
        Ok(mesh
            .edge_ids
            .iter()
            .zip(mesh.edges)
            .map(|(&index, run)| {
                exact_edge(&record, index, run.clone()).unwrap_or_else(|_| meshed_run(run))
            })
            .collect())
    }
}

/// An edge's meshed run of points as a curve: a line for two, a polyline for more, a point for
/// one.
fn meshed_run(run: Vec<Point>) -> Curve {
    let fallback = Curve::Point {
        at: run.first().copied().unwrap_or([0.0; 3]),
    };
    match run.len() {
        0 | 1 => fallback,
        2 => Curve::line(run[0], run[1]).unwrap_or(fallback),
        _ => Curve::polyline(run, false).unwrap_or(fallback),
    }
}

/// Record edge `index` as an exact curve, trimmed to the edge where its meshed run of points,
/// `traced`, lies.
fn exact_edge(record: &Record, index: usize, traced: Vec<Point>) -> Result<Curve, KernelError> {
    use solidcore_record::brep::Curve as Kind;
    {
        let edge = record
            .edges
            .get(index)
            .ok_or_else(|| KernelError(format!("the body has no edge {index}")))?;
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
        // A circle, ellipse or line as itself, so a fillet's edge comes back an arc whose centre
        // ties and prints read directly; anything else as the NURBS the engine holds it on.
        let shape = match &edge.curve {
            Kind::Circle { .. } | Kind::Ellipse { .. } | Kind::Line { .. } => &edge.curve,
            other => edge.nurbs.as_ref().unwrap_or(other),
        };
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
        trimmed_to(&whole, start, end, &traced)
    }
}

/// The piece of `whole` between an edge's ends, `start` and `end`, that runs where its meshed
/// points, `traced`, do.
fn trimmed_to(
    whole: &Curve,
    start: Point,
    end: Point,
    traced: &[Point],
) -> Result<Curve, KernelError> {
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
            closed_edge || (near(*a, start) && near(*b, end)) || (near(*a, end) && near(*b, start))
        })
    };
    pieces
        .into_iter()
        .filter(ends_match)
        .min_by(|a, b| off(a).total_cmp(&off(b)))
        .ok_or_else(|| KernelError("could not trim the edge's curve".to_owned()))
}
impl Body {
    /// The body's edges as their exact curves (lines between their vertices, circles and
    /// splines as the engine holds them), each with the faces that meet at it, by number. For
    /// drawings, which want edges exact rather than as the display draws them.
    pub fn exact_edges(&self) -> Result<Vec<(Curve, Vec<usize>)>, KernelError> {
        use solidcore_record::brep::Curve as RecordCurve;
        let record = self.record()?;
        let point = |index: u32| {
            record
                .vertices
                .get(index as usize)
                .map(|v| v.point)
                .ok_or_else(|| KernelError("an edge's vertex is missing".to_owned()))
        };
        let mut out = Vec::new();
        for edge in &record.edges {
            let curve = match &edge.curve {
                RecordCurve::Line { .. } => {
                    Curve::line(point(edge.vertices[0])?, point(edge.vertices[1])?)?
                }
                curve => crate::from_record_curve(
                    edge.nurbs.as_ref().unwrap_or(curve),
                    Some(edge.interval),
                )?,
            };
            let faces = edge.uses.iter().map(|used| used.face as usize).collect();
            out.push((curve, faces));
        }
        Ok(out)
    }
}
