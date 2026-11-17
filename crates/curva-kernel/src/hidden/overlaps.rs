//! Hidden lines: lines drawn twice, or hidden under visible ones, left out.

use super::*;

/// The drawn lines without those drawn twice, or hidden ones lying under visible ones: visible
/// lines first, then each line keeps only what no line already kept covers.
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
    for line in drawn {
        let over: Vec<&Curve> = kept
            .iter()
            .filter(|k| k.visible || !line.visible)
            .map(|k| &k.curve)
            .collect();
        if let Some((a, b)) = ends(&line.curve) {
            for (p, q) in uncovered((a, b), &over, flat) {
                if let Ok(curve) = Curve::line(p, q) {
                    kept.push(DrawnLine {
                        curve,
                        ..line.clone()
                    });
                }
            }
        } else if !over.iter().any(|k| lies_on(&line.curve, k, flat)) {
            kept.push(line);
        }
    }
    kept
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
