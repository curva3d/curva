//! A surface carried on past an edge as Rhino's ExtendSrf does with `Smooth` (#751, measured in
//! Rhino 8): the surface's own spans continue past the edge, its knots pushed out in parameter
//! by the typed distance over the surface's speed across the edge. A rational surface (an arc's)
//! goes further than that in Rhino by a factor still unknown, so it is left to the engine.

use crate::body::Body;
use crate::surface::NurbsSurface;
use crate::surface_edit::Side;
use crate::{KernelError, Spline, gap};

impl Body {
    /// The one-face, untrimmed, non-rational surface carried on smoothly past `edge` by
    /// `distance`, as Rhino's smooth ExtendSrf does; `None` for anything else.
    pub(crate) fn smoothly_extended(
        &self,
        edge: usize,
        distance: f64,
    ) -> Result<Option<Body>, KernelError> {
        let record = self.record()?;
        let Ok((face, side, _)) = self.edge_side(edge) else {
            return Ok(None);
        };
        if record.faces.len() != 1 || record.faces[face].loops.len() != 1 {
            return Ok(None);
        }
        let surface = NurbsSurface::of_face(&record, face)?;
        let rational = surface
            .points
            .iter()
            .flatten()
            .any(|p| (p[3] - 1.0).abs() > 1e-12);
        let box_matches = surface
            .domain()
            .iter()
            .flatten()
            .zip(record.faces[face].domain.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-9 * (1.0 + a.abs()));
        if rational || !box_matches {
            return Ok(None);
        }
        let across_u = matches!(side, Side::U0 | Side::U1);
        let at_end = matches!(side, Side::U1 | Side::V1);
        // The curves running across the edge: columns for a u side, rows for a v side.
        let mut curves: Vec<Spline> = if across_u {
            (0..surface.points[0].len())
                .map(|j| Spline {
                    degree: surface.degree[0],
                    knots: surface.u_knots.clone(),
                    points: surface.points.iter().map(|row| row[j]).collect(),
                })
                .collect()
        } else {
            surface
                .points
                .iter()
                .map(|row| Spline {
                    degree: surface.degree[1],
                    knots: surface.v_knots.clone(),
                    points: row.clone(),
                })
                .collect()
        };
        if !at_end {
            curves = curves.iter().map(Spline::reversed).collect();
        }
        // The speed across the edge at its middle: Rhino's distance over it is the parameter
        // added (a cubic extrusion's edge: 5 over 1.2254 is 4.0803, as measured).
        let [[u0, u1], [v0, v1]] = surface.domain();
        let middle = if across_u {
            surface.iso_v((v0 + v1) / 2.0)
        } else {
            surface.iso_u((u0 + u1) / 2.0)
        };
        let middle = if at_end { middle } else { middle.reversed() };
        let (_, end) = middle.domain();
        let speed = {
            let d = middle.derivative(end);
            gap(d, [0.0; 3])
        };
        if speed < 1e-12 {
            return Ok(None);
        }
        let by = distance / speed;
        let mut longer: Vec<Spline> = curves.iter().map(|c| c.extended_end(by)).collect();
        if !at_end {
            longer = longer.iter().map(Spline::reversed).collect();
        }
        let mut out = surface.clone();
        if across_u {
            out.u_knots.clone_from(&longer[0].knots);
            out.points = (0..longer[0].points.len())
                .map(|i| longer.iter().map(|c| c.points[i]).collect())
                .collect();
        } else {
            out.v_knots.clone_from(&longer[0].knots);
            out.points = longer.into_iter().map(|c| c.points).collect();
        }
        out.to_body().map(Some)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Carry, Curve};

    #[test]
    fn a_cubic_extrusion_carries_on_by_the_distance_over_its_speed_as_in_rhino() {
        // Rhino 8: the cubic through (0,0) (3,4) (8,4) (12,0) extruded 5 up, extended 5 past its
        // end: the parameter grows by 5 / 1.2254 = 4.0803, the last point (15.1984, -4.5772).
        let points = vec![[0.0; 3], [3.0, 4.0, 0.0], [8.0, 4.0, 0.0], [12.0, 0.0, 0.0]];
        let curve = Curve::control_point(points, 3, false).unwrap();
        let wall = Body::extruded(&[curve], [0.0, 0.0, 1.0], 5.0, false).unwrap();
        let edge = (0..wall.record_edge_count())
            .find(|e| {
                let points = wall.edge_curve(*e).unwrap().sample().unwrap();
                points
                    .iter()
                    .all(|p| (p[0] - 12.0).abs() < 1e-9 && p[1].abs() < 1e-9)
            })
            .unwrap();
        let longer = wall.extended_at(edge, 5.0, Carry::Natural).unwrap();
        let (low, high) = longer.bounds().unwrap();
        assert!(
            (high[0] - 15.1984).abs() < 1e-3 && (low[1] + 4.5772).abs() < 1e-3,
            "{low:?} {high:?}"
        );
    }
}
