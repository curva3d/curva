//! Where a SubD's cage parts lie on its smooth surface, for drawing picked parts as Rhino does
//! (#334): each cage edge's run of points on the surface, each cage point's limit point, and the
//! surface with each quad's cage face.

use crate::smooth::{cage_edge_lines, on_surface};
use crate::{Mesh, Point, SmoothError, SubD};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// The shortest way from `from` to `to` along `next`, both ends included; empty when there is
/// none.
fn path(next: &HashMap<u32, Vec<u32>>, from: u32, to: u32) -> Vec<u32> {
    let mut came: HashMap<u32, u32> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(here) = queue.pop_front() {
        if here == to {
            let mut way = vec![to];
            while let Some(&back) = came.get(way.last().unwrap_or(&to)) {
                if back == *way.last().unwrap_or(&to) {
                    break;
                }
                way.push(back);
            }
            way.reverse();
            return way;
        }
        for &n in next.get(&here).into_iter().flatten() {
            if let std::collections::hash_map::Entry::Vacant(slot) = came.entry(n) {
                slot.insert(here);
                queue.push_back(n);
            }
        }
    }
    Vec::new()
}

impl SubD {
    /// Every cage edge as it lies on the smooth surface, `levels` subdivisions fine: keyed by its
    /// two cage points, the lower first, each run going from the lower point's limit point to the
    /// higher's. One call refines the SubD once for all its edges.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn edges_on_surface(
        &self,
        levels: u8,
    ) -> Result<BTreeMap<(u32, u32), Vec<Point>>, SmoothError> {
        let (result, centre, refined) = self.refine(levels)?;
        let on = on_surface(&result, centre, &refined)?;
        // The library numbers each level's points from the level before's last, so cage point
        // `v` is fine point `fine - cage + v`. An edge is the shortest way between its ends along
        // the fine edges lying on cage edges: any other way crosses two cage edges or more.
        let cage = u32::try_from(self.cage().vertex_count()).unwrap_or(u32::MAX);
        let fine = u32::try_from(refined.len()).unwrap_or(u32::MAX);
        let first = fine.saturating_sub(cage);
        let mut next: HashMap<u32, Vec<u32>> = HashMap::new();
        for [a, b] in cage_edge_lines(&result) {
            next.entry(a).or_default().push(b);
            next.entry(b).or_default().push(a);
        }
        Ok(self
            .edge_uses()
            .into_keys()
            .map(|(a, b)| {
                let run = path(&next, first + a, first + b);
                ((a, b), run.iter().map(|&v| on[v as usize]).collect())
            })
            .collect())
    }

    /// One cage edge, from `a` to `b`, as it lies on the smooth surface ([`SubD::edges_on_surface`]
    /// for one edge); `None` when `a` and `b` are not joined by an edge.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn edge_on_surface(
        &self,
        levels: u8,
        [a, b]: [u32; 2],
    ) -> Result<Option<Vec<Point>>, SmoothError> {
        let mut runs = self.edges_on_surface(levels)?;
        Ok(runs.remove(&(a.min(b), a.max(b))).map(|mut run| {
            if a > b {
                run.reverse();
            }
            run
        }))
    }

    /// Each cage point's point on the smooth surface (its limit point), in cage order, found
    /// `levels` subdivisions fine. A point on no edge keeps its cage place.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn limit_points(&self, levels: u8) -> Result<Vec<Point>, SmoothError> {
        let mut points = self.cage().vertices().to_vec();
        for ((a, b), run) in self.edges_on_surface(levels)? {
            if let (Some(&first), Some(&last)) = (run.first(), run.last()) {
                points[a as usize] = first;
                points[b as usize] = last;
            }
        }
        Ok(points)
    }

    /// The smooth surface ([`SubD::surface`]) and, for each of its quads, the cage face it lies
    /// in.
    ///
    /// # Errors
    /// When the library refuses the cage.
    pub fn surface_with_faces(&self, levels: u8) -> Result<(Mesh, Vec<u32>), SmoothError> {
        self.surface_and_roots(levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    fn gap(a: Point, b: Point) -> f64 {
        (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
    }

    fn cube() -> SubD {
        SubD::subd_box(&Frame::world(), [4.0; 3], [2, 2, 2])
    }

    #[test]
    fn every_edge_runs_between_its_points_limit_points() {
        let subd = cube();
        let runs = subd.edges_on_surface(3).unwrap();
        assert_eq!(runs.len(), subd.edge_uses().len());
        let limits = subd.limit_points(3).unwrap();
        for ((a, b), run) in &runs {
            // Three levels: 2³ pieces, 9 points from end to end.
            assert_eq!(run.len(), 9);
            assert!(gap(run[0], limits[*a as usize]) < 1e-12);
            assert!(gap(run[8], limits[*b as usize]) < 1e-12);
        }
        // Every limit point lies on the smooth surface's points.
        let surface = subd.surface(3).unwrap();
        for p in &limits {
            let near = surface
                .vertices()
                .iter()
                .map(|&q| gap(*p, q))
                .fold(f64::MAX, f64::min);
            assert!(near < 1e-9, "{near}");
        }
    }

    #[test]
    fn one_edge_runs_the_way_asked() {
        let subd = cube();
        let (a, b) = *subd.edge_uses().keys().next().unwrap();
        let forward = subd.edge_on_surface(2, [a, b]).unwrap().unwrap();
        let backward = subd.edge_on_surface(2, [b, a]).unwrap().unwrap();
        assert_eq!(forward.first(), backward.last());
        assert!(subd.edge_on_surface(2, [a, a]).unwrap().is_none());
    }

    #[test]
    fn moving_points_keeps_everything_else() {
        let (a, b) = *cube().edge_uses().keys().next().unwrap();
        let subd = cube().with_creases(&[[a, b]]);
        let moved = subd.with_cage_points_moved(&[(a, [9.0, 9.0, 9.0]), (99_999, [0.0; 3])]);
        assert_eq!(moved.cage().vertices()[a as usize], [9.0, 9.0, 9.0]);
        assert_eq!(moved.cage().vertex_count(), subd.cage().vertex_count());
        assert!(moved.cage().faces().eq(subd.cage().faces()));
        assert!(moved.sharpness(a, b).is_crease());
        // The cage's corners are the same, so a drawing can keep its triangle layout.
        assert_eq!(moved.cage().corners(), subd.cage().corners());
    }

    #[test]
    fn each_quad_knows_its_cage_face() {
        let subd = cube();
        let (surface, faces) = subd.surface_with_faces(2).unwrap();
        assert_eq!(faces.len(), surface.face_count());
        // Two levels: each cage quad becomes 16 quads.
        let cage_faces = u32::try_from(subd.cage().face_count()).unwrap();
        for f in 0..cage_faces {
            assert_eq!(faces.iter().filter(|&&g| g == f).count(), 16);
        }
    }
}
