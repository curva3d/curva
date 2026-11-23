//! Rhino 8's KeepCreases for Bevel on SubDs. Measured (23 November 2026) on a 4 m SubD box of
//! 4 × 4 × 4 faces, bevelling the front edge (202, 0, 1)–(202, 0, 2) by 0.5 with KeepCreases=Yes:
//!
//! - a creased edge leaving a run point off the run (to its far point, or to a side point)
//!   creases that point's row, and at an open end the cap's two edges to the far point; a creased
//!   side edge stays creased, now ending at the row (one side edge creased: 6 creases);
//! - a creased edge of the run creases the strip's two long sides along it and, at an open end,
//!   the cap's edges to the far point (the picked edge alone creased: 6 creases);
//! - both together: 13 creases with the untouched rest of the line; nothing creased: none.
//!
//! With KeepCreases=No the creases at the run's points go with them.

use crate::SubD;
use crate::subd_bevel::Bevelled;
use std::collections::BTreeSet;

impl SubD {
    /// The edges, numbered as in `bevel`'s working cage, that KeepCreases creases.
    pub(crate) fn kept_creases(&self, bevel: &Bevelled) -> Vec<[u32; 2]> {
        let creased: BTreeSet<(u32, u32)> =
            self.creases().map(|[a, b]| (a.min(b), a.max(b))).collect();
        let is_creased = |a: u32, b: u32| creased.contains(&(a.min(b), a.max(b)));
        let points = &bevel.gone;
        let m = points.len();
        let closed = bevel.ends.is_none();
        let at = |i: usize| points[i % m];
        let on_run = |i: usize, w: u32| {
            (closed || i + 1 < m) && w == at(i + 1) || (closed || i > 0) && w == at(i + m - 1)
        };
        let mut out: Vec<[u32; 2]> = Vec::new();
        let caps = |i: usize, out: &mut Vec<[u32; 2]>| {
            let row = &bevel.rows[i];
            let far = match bevel.ends {
                Some((first, _)) if i == 0 => first,
                Some((_, last)) if i == m - 1 => last,
                _ => return,
            };
            out.push([row[0], far]);
            out.push([row[row.len() - 1], far]);
        };
        for (i, &v) in points.iter().enumerate() {
            let row = &bevel.rows[i];
            let [s1, s2] = bevel.sides[i];
            let off_run: Vec<u32> = creased
                .iter()
                .filter_map(|&(a, b)| (a == v).then_some(b).or((b == v).then_some(a)))
                .filter(|&w| !on_run(i, w))
                .collect();
            if !off_run.is_empty() {
                out.extend(row.windows(2).map(|w| [w[0], w[1]]));
                caps(i, &mut out);
            }
            if is_creased(v, s1) {
                out.push([s1, row[0]]);
            }
            if is_creased(v, s2) {
                out.push([s2, row[row.len() - 1]]);
            }
        }
        let joins = if closed { m } else { m - 1 };
        for i in 0..joins {
            let j = (i + 1) % m;
            if is_creased(points[i], points[j]) {
                let (ra, rb) = (&bevel.rows[i], &bevel.rows[j]);
                out.push([ra[0], rb[0]]);
                out.push([ra[ra.len() - 1], rb[rb.len() - 1]]);
                caps(i, &mut out);
                caps(j, &mut out);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::{BevelAmount, Frame, Point, SubD};

    fn index(i: usize) -> u32 {
        u32::try_from(i).unwrap()
    }

    /// Rhino's case: the 4 × 4 × 4 box, `creased` edges creased, the front edge at x = 202 from
    /// z = 1 to 2 bevelled by 0.5; the creases afterwards.
    fn creases_after(creased: &[[Point; 2]], keep: bool) -> usize {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [4, 4, 4]);
        let at = |p: Point| index(subd.cage().vertices().iter().position(|&q| q == p).unwrap());
        let edges: Vec<[u32; 2]> = creased.iter().map(|e| e.map(at)).collect();
        let edge = [at([202.0, 0.0, 1.0]), at([202.0, 0.0, 2.0])];
        let subd = subd.with_creases(&edges);
        let out = subd
            .bevelled_run(&edge, BevelAmount::Absolute(0.5), 3, 0.0, keep)
            .unwrap();
        out.creases().count()
    }

    fn line(z: f64) -> [Point; 2] {
        [[202.0, 0.0, z], [202.0, 0.0, z + 1.0]]
    }

    #[test]
    fn creases_are_kept_round_the_bevel_as_in_rhino() {
        let whole: Vec<[Point; 2]> = (0..4).map(|z| line(f64::from(z))).collect();
        assert_eq!(creases_after(&whole, true), 13);
        assert_eq!(creases_after(&whole, false), 1, "only the untouched edge");
        assert_eq!(
            creases_after(&[line(1.0)], true),
            6,
            "the picked edge alone"
        );
        assert_eq!(
            creases_after(&[line(0.0), line(2.0)], true),
            10,
            "the line's other edges"
        );
        let side = [[201.0, 0.0, 1.0], [202.0, 0.0, 1.0]];
        assert_eq!(creases_after(&[side], true), 6, "a side edge at one end");
        assert_eq!(creases_after(&[], true), 0);
    }
}
