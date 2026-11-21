//! How Fill closes one hole, as Rhino 8's patch styles: the faces across a loop of points, for
//! SubDs ([`crate::SubD::filled_at`]) and meshes ([`crate::Mesh::filled_at`]) alike.

use crate::{FillStyle, Mesh, MeshError, Point};

fn midpoint(points: &[Point]) -> Point {
    #[allow(clippy::cast_precision_loss)]
    let n = points.len().max(1) as f64;
    let sum = points
        .iter()
        .fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
    sum.map(|c| c / n)
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// The faces closing the hole round `around` (in the order a face across it must run), in
/// `style`: each face lists positions in `around`, and `around.len()` stands for the one new
/// middle point, which is returned when the style makes one.
pub(crate) fn patch(
    around: &[Point],
    style: FillStyle,
) -> Result<(Vec<Vec<u32>>, Option<Point>), MeshError> {
    let n = around.len();
    match style {
        FillStyle::SingleFace => Ok((vec![(0..index(n)).collect()], None)),
        FillStyle::Triangulated => {
            // The hole as one face, split as TriangulateMesh splits faces.
            let one = Mesh::new(around.to_vec(), &[(0..index(n)).collect::<Vec<_>>()])?;
            let (split, _) = one.triangulated();
            Ok((split.faces().map(<[u32]>::to_vec).collect(), None))
        }
        FillStyle::TriangleFan | FillStyle::Automatic => {
            let centre = index(n);
            let step = if style == FillStyle::Automatic { 2 } else { 1 };
            let mut faces = Vec::new();
            let mut k = 0;
            while k < n {
                let take = if k + step < n || n.is_multiple_of(step) {
                    step
                } else {
                    1
                };
                let mut face = vec![centre];
                face.extend((0..=take).map(|s| index((k + s) % n)));
                faces.push(face);
                k += take;
            }
            Ok((faces, Some(midpoint(around))))
        }
    }
}

impl Mesh {
    /// The holes whose open edges are all among `edges` closed in `style`, as Rhino 8's Fill on a
    /// mesh, and how many: the patch is a piece of its own, its points copied from the hole's, so
    /// it is joined but not welded (measured: an open 2 × 2 × 2 box's top hole, all eight edges
    /// picked, gives 24 quads on 34 points; one edge picked fills nothing). Edges name their two
    /// points; points at one place count as one.
    ///
    /// # Errors
    /// When a patch is not a valid mesh.
    pub fn filled_at(
        &self,
        edges: &[[u32; 2]],
        style: FillStyle,
    ) -> Result<(Self, usize), MeshError> {
        let places = self.places();
        let place = |v: u32| places[v as usize];
        let picked: std::collections::BTreeSet<(u32, u32)> = edges
            .iter()
            .map(|&[a, b]| {
                let (p, q) = (place(a), place(b));
                (p.min(q), p.max(q))
            })
            .collect();
        let mut pieces: Vec<Mesh> = Vec::new();
        for ring in self.hole_loops() {
            let n = ring.len();
            let every_edge = (0..n).all(|k| {
                let (p, q) = (ring[k].0, ring[(k + 1) % n].0);
                picked.contains(&(p.min(q), p.max(q)))
            });
            if n < 3 || !every_edge {
                continue;
            }
            let mut around: Vec<Point> = ring
                .iter()
                .map(|&(_, v)| self.vertices()[v as usize])
                .collect();
            let (faces, middle) = patch(&around, style)?;
            around.extend(middle);
            let piece = Mesh::new(around, &faces)?;
            pieces.push(if self.normals().is_none() {
                piece
            } else {
                piece.with_rebuilt_normals()
            });
        }
        let filled = pieces.len();
        if filled == 0 {
            return Ok((self.clone(), 0));
        }
        let mut all: Vec<&Mesh> = vec![self];
        all.extend(pieces.iter());
        Ok((Mesh::joined(&all)?, filled))
    }
}

#[cfg(test)]
mod tests {
    use crate::{FillStyle, Frame, Mesh};

    /// A 4 m box mesh of 2 × 2 faces a side, welded, without its four top faces.
    fn open_box() -> Mesh {
        let whole = Mesh::mesh_box(&Frame::world(), [4.0; 3], [2, 2, 2]);
        let rest: Vec<Vec<u32>> = whole
            .faces()
            .filter(|f| !f.iter().all(|&v| whole.vertices()[v as usize][2] > 3.99))
            .map(<[u32]>::to_vec)
            .collect();
        let kept = Mesh::new(whole.vertices().to_vec(), &rest).unwrap();
        kept.without_unused_vertices().0
    }

    fn border(mesh: &Mesh) -> Vec<[u32; 2]> {
        mesh.hole_loops()
            .into_iter()
            .flat_map(|ring| {
                let n = ring.len();
                (0..n)
                    .map(move |k| [ring[k].1, ring[(k + 1) % n].1])
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn the_whole_loop_picked_fills_as_rhino_does() {
        // Rhino 8: 20 quads on 25 points become 24 quads on 34, closed.
        let open = open_box();
        assert_eq!((open.face_count(), open.vertex_count()), (20, 25));
        let edges = border(&open);
        assert_eq!(edges.len(), 8);
        let (filled, holes) = open.filled_at(&edges, FillStyle::Automatic).unwrap();
        assert_eq!(holes, 1);
        assert_eq!((filled.face_count(), filled.vertex_count()), (24, 34));
        assert!(filled.faces().all(|f| f.len() == 4));
        assert!(filled.is_closed());
    }

    #[test]
    fn one_edge_picked_fills_nothing_as_in_rhino() {
        let open = open_box();
        let edges = border(&open);
        let (same, holes) = open.filled_at(&edges[..1], FillStyle::Automatic).unwrap();
        assert_eq!(holes, 0);
        assert_eq!(same.face_count(), 20);
    }
}
