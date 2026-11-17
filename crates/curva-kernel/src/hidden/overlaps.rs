//! Hidden lines: lines drawn twice, or hidden under visible ones, left out.

use super::*;

/// The drawn lines without those drawn twice, or hidden ones lying under visible ones: visible
/// lines first, then each line keeps only what no line already kept covers. Only kept lines
/// near a line can cover it, so only those are tried: whether a line is covered does not depend
/// on the order lines are tried in (#317).
pub(super) fn without_overlaps(
    mut drawn: Vec<DrawnLine>,
    options: Options,
    flat: f64,
) -> Vec<DrawnLine> {
    let mut kept: Vec<DrawnLine> = Vec::new();
    drawn.sort_by_key(|line| {
        (
            !line.visible,
            options.first_from.is_none_or(|from| line.source < from),
        )
    });
    let mut cells = Cells::over(&drawn, flat);
    for line in drawn {
        let near = cells.near(&line.curve);
        let over: Vec<&Curve> = near
            .iter()
            .map(|&i| &kept[i])
            .filter(|k| k.visible || !line.visible)
            .map(|k| &k.curve)
            .collect();
        let mut pieces = Vec::new();
        if let Some((a, b)) = ends(&line.curve) {
            for (p, q) in uncovered((a, b), &over, flat) {
                if let Ok(curve) = Curve::line(p, q) {
                    pieces.push(DrawnLine {
                        curve,
                        ..line.clone()
                    });
                }
            }
        } else if !over.iter().any(|k| lies_on(&line.curve, k, flat)) {
            pieces.push(line);
        }
        for piece in pieces {
            cells.add(kept.len(), &piece.curve);
            kept.push(piece);
        }
    }
    kept
}

/// The drawing divided into square-ish cells, about as many as there are lines, with the kept
/// lines whose boxes reach into each cell. A line's box is grown by twice the tolerance and a
/// hundredth of its size (its sample points can cut a little inside a curve), so a line that
/// comes within the tolerance of another shares a cell with it.
struct Cells {
    flat: f64,
    low: [f64; 2],
    size: [f64; 2],
    side: usize,
    cells: Vec<Vec<usize>>,
}

impl Cells {
    fn over(drawn: &[DrawnLine], flat: f64) -> Self {
        let mut low = [f64::INFINITY; 2];
        let mut high = [f64::NEG_INFINITY; 2];
        for line in drawn {
            let (a, b) = bounds2(&line.curve);
            for k in 0..2 {
                low[k] = low[k].min(a[k]);
                high[k] = high[k].max(b[k]);
            }
        }
        if !low[0].is_finite() || !low[1].is_finite() {
            low = [0.0; 2];
            high = [0.0; 2];
        }
        let side = ((drawn.len() as f64).sqrt().ceil() as usize).clamp(1, 1024);
        let size = [0, 1].map(|k| ((high[k] - low[k]) / side as f64).max(flat).max(1e-12));
        Self {
            flat,
            low,
            size,
            side,
            cells: vec![Vec::new(); side * side],
        }
    }

    /// The cells the grown box of `curve` reaches into.
    fn of(&self, curve: &Curve) -> Vec<usize> {
        let (a, b) = bounds2(curve);
        if !(a[0].is_finite() && a[1].is_finite() && b[0].is_finite() && b[1].is_finite()) {
            // No box: every cell, so nothing is missed.
            return (0..self.cells.len()).collect();
        }
        let grow = 2.0 * self.flat + 0.01 * (b[0] - a[0]).hypot(b[1] - a[1]);
        let at = |v: f64, k: usize| {
            let n = ((v - self.low[k]) / self.size[k]).floor();
            if n.is_nan() {
                0
            } else {
                (n.max(0.0) as usize).min(self.side - 1)
            }
        };
        let (x0, x1) = (at(a[0] - grow, 0), at(b[0] + grow, 0));
        let (y0, y1) = (at(a[1] - grow, 1), at(b[1] + grow, 1));
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| y * self.side + x))
            .collect()
    }

    fn add(&mut self, index: usize, curve: &Curve) {
        for cell in self.of(curve) {
            self.cells[cell].push(index);
        }
    }

    /// The kept lines sharing a cell with `curve`, in the order they were kept.
    fn near(&self, curve: &Curve) -> Vec<usize> {
        let mut near: Vec<usize> = self
            .of(curve)
            .into_iter()
            .flat_map(|cell| self.cells[cell].iter().copied())
            .collect();
        near.sort_unstable();
        near.dedup();
        near
    }
}

/// The parts of the line `a`–`b` that no line among `over` lying along it covers, each
/// longer than `tolerance`.
fn uncovered((a, b): (Point, Point), over: &[&Curve], tolerance: f64) -> Vec<(Point, Point)> {
    let span = sub(b, a);
    let total = length(span);
    if total <= tolerance {
        return Vec::new();
    }
    let dir = span.map(|c| c / total);
    // Where along `a`–`b` a point falls, and how far off the line it is.
    let place = |p: Point| {
        let t = dot(sub(p, a), dir);
        (t, length(sub(sub(p, a), dir.map(|c| c * t))))
    };
    let mut covers: Vec<(f64, f64)> = over
        .iter()
        .filter_map(|curve| ends(curve))
        .filter_map(|(p, q)| {
            let ((tp, op), (tq, oq)) = (place(p), place(q));
            (op <= tolerance && oq <= tolerance).then_some((tp.min(tq), tp.max(tq)))
        })
        .collect();
    covers.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut free = Vec::new();
    let mut at = 0.0;
    for (from, to) in covers {
        if from > at + tolerance {
            free.push((at, from.min(total)));
        }
        at = at.max(to);
        if at >= total - tolerance {
            break;
        }
    }
    if at < total - tolerance {
        free.push((at, total));
    }
    free.into_iter()
        .filter(|(s, e)| e - s > tolerance)
        .map(|(s, e)| {
            // The line's own ends exactly where it keeps them.
            let point = |t: f64| {
                if t <= tolerance {
                    a
                } else if t >= total - tolerance {
                    b
                } else {
                    plus(a, dir, t)
                }
            };
            (point(s), point(e))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(curve: Curve, visible: bool) -> DrawnLine {
        DrawnLine {
            curve,
            visible,
            kind: LineKind::Curve,
            source: 0,
        }
    }

    #[test]
    fn only_what_lies_on_a_kept_line_is_left_out_however_many_lines_there_are() {
        let line = |a: Point, b: Point| Curve::line(a, b).unwrap();
        let mut lines = Vec::new();
        // A row of 400 squares' bottom edges, each drawn twice, the second hidden.
        for i in 0..400 {
            let x = f64::from(i) * 20.0;
            lines.push(drawn(line([x, 0.0, 0.0], [x + 10.0, 0.0, 0.0]), true));
            lines.push(drawn(line([x, 0.0, 0.0], [x + 10.0, 0.0, 0.0]), false));
        }
        // A hidden line under the first four edges and the gaps between: only the gaps stay.
        lines.push(drawn(line([0.0, 0.0, 0.0], [70.0, 0.0, 0.0]), false));
        // A circle drawn twice, far off: kept once.
        let circle = Curve::circle([0.0, 500.0, 0.0], 5.0).unwrap();
        lines.push(drawn(circle.clone(), true));
        lines.push(drawn(circle, true));
        let kept = without_overlaps(lines, Options::default(), 1e-6);
        assert_eq!(kept.iter().filter(|l| l.visible).count(), 401);
        let hidden: Vec<(Point, Point)> = kept
            .iter()
            .filter(|l| !l.visible)
            .filter_map(|l| ends(&l.curve))
            .collect();
        assert_eq!(hidden.len(), 3);
        for ((a, b), from) in hidden.iter().zip([10.0, 30.0, 50.0]) {
            assert!((a[0] - from).abs() < 1e-9 && (b[0] - from - 10.0).abs() < 1e-9);
        }
    }
}
