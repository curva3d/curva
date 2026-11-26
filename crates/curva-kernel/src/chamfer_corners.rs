//! Chamfered corners as Rhino makes them (#683, measured): where every edge meeting at a corner
//! is chamfered, the corner is cut flat through the points where the chamfers meet the faces
//! there (the corner moved one chamfer distance along each of two of its edges), leaving a small
//! triangle joining the chamfers.

use crate::{Body, Curve, KernelError, Point, cross, sub, unit};

/// How far past the corner's own triangle the cutting sheet reaches, as a share of its size,
/// so it cuts right through.
const REACH: f64 = 0.25;

impl Body {
    /// `chamfered`, made from `self` by chamfering the edges numbered `edges` by `size`, with
    /// each corner where all its edges were chamfered cut flat, as Rhino cuts it.
    pub(crate) fn chamfer_corners_cut(
        &self,
        chamfered: Body,
        edges: &[usize],
        size: f64,
    ) -> Result<Body, KernelError> {
        let record = self.record()?;
        let mut body = chamfered;
        for (v, vertex) in record.vertices.iter().enumerate() {
            let touching: Vec<usize> = record
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| e.vertices.contains(&(v as u32)))
                .map(|(k, _)| k)
                .collect();
            if touching.len() < 3 || touching.iter().any(|k| !edges.contains(k)) {
                continue;
            }
            // Along each straight edge from the corner, its direction.
            let Some(along) = touching
                .iter()
                .map(|&k| {
                    let e = &record.edges[k];
                    let other = e.vertices.iter().find(|w| **w != v as u32)?;
                    let far = record.vertices.get(*other as usize)?.point;
                    unit(sub(far, vertex.point))
                })
                .collect::<Option<Vec<Point>>>()
            else {
                continue;
            };
            // Where the chamfers meet the faces at the corner: one chamfer distance along each of
            // two edges.
            let points: Vec<Point> = [(0, 1), (1, 2), (2, 0)]
                .iter()
                .map(|&(i, j)| {
                    std::array::from_fn(|k| vertex.point[k] + (along[i][k] + along[j][k]) * size)
                })
                .collect();
            if let Some(cut) = corner_cut(&body, &points)? {
                body = cut;
            }
        }
        Ok(body)
    }
}

/// `body` with the corner beyond the plane through `points` cut away: split by a triangle a
/// little larger than theirs, keeping the larger piece. `None` when the sheet does not divide it.
fn corner_cut(body: &Body, points: &[Point]) -> Result<Option<Body>, KernelError> {
    if unit(cross(sub(points[1], points[0]), sub(points[2], points[0]))).is_none() {
        return Ok(None);
    }
    let middle: Point = std::array::from_fn(|i| points.iter().map(|p| p[i]).sum::<f64>() / 3.0);
    let grown: Vec<Point> = points
        .iter()
        .map(|p| std::array::from_fn(|i| middle[i] + (p[i] - middle[i]) * (1.0 + REACH)))
        .collect();
    let sheet = Body::planar(&[Curve::polyline(grown, true)?])?;
    let Some((one, two)) = body.split_by(&sheet)? else {
        return Ok(None);
    };
    let (v1, v2) = (one.volume()?, two.volume()?);
    Ok(Some(if v1 >= v2 { one } else { two }))
}

#[cfg(test)]
mod tests {
    use crate::Body;

    #[test]
    fn a_box_chamfered_on_every_edge_has_rhinos_corner_triangles() {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let all: Vec<usize> = (0..12).collect();
        let chamfered = cube.blended(&all, 1.0, true).unwrap();
        // Rhino 8: 26 faces, volume 945.3333.
        assert_eq!(chamfered.face_count().unwrap(), 26);
        assert!((chamfered.volume().unwrap() - 945.3333).abs() < 1e-3);
    }
}
