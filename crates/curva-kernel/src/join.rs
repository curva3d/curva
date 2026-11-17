//! Joining curves that meet end to end.

#[allow(clippy::wildcard_imports)]
use super::*;

/// How close two curve ends must be to join, in millimetres.
pub(crate) const JOIN_TOLERANCE: f64 = 1e-3;

/// One open curve ready to be chained to others.
pub(crate) struct JoinPiece {
    index: usize,
    curve: Curve,
    start: Point,
    end: Point,
}

impl JoinPiece {
    pub(crate) fn of(index: usize, curve: &Curve) -> Option<Self> {
        if matches!(curve, Curve::Polyline { closed: true, .. }) {
            return None;
        }
        let points = curve.sample().ok()?;
        let (start, end) = (*points.first()?, *points.last()?);
        (gap(start, end) > JOIN_TOLERANCE).then(|| Self {
            index,
            curve: curve.clone(),
            start,
            end,
        })
    }

    pub(crate) fn tail(&self, reversed: bool) -> Point {
        if reversed { self.start } else { self.end }
    }

    pub(crate) fn head(&self, reversed: bool) -> Point {
        self.tail(!reversed)
    }
}

/// Grows a chain of pieces from both its ends, returning each piece used and whether it is
/// reversed.
pub(crate) fn chain_from(
    pieces: &[JoinPiece],
    used: &mut [bool],
    seed: usize,
) -> Vec<(usize, bool)> {
    let mut chain = vec![(seed, false)];
    used[seed] = true;
    for grow_tail in [true, false] {
        loop {
            let &(at, flipped) = if grow_tail {
                chain.last()
            } else {
                chain.first()
            }
            .unwrap_or(&(seed, false));
            let open = if grow_tail {
                pieces[at].tail(flipped)
            } else {
                pieces[at].head(flipped)
            };
            let next = (0..pieces.len()).find_map(|candidate| {
                if used[candidate] {
                    return None;
                }
                [false, true].into_iter().find_map(|reversed| {
                    let meeting = if grow_tail {
                        pieces[candidate].head(reversed)
                    } else {
                        pieces[candidate].tail(reversed)
                    };
                    (gap(open, meeting) <= JOIN_TOLERANCE).then_some((candidate, reversed))
                })
            });
            let Some(link) = next else {
                break;
            };
            used[link.0] = true;
            if grow_tail {
                chain.push(link);
            } else {
                chain.insert(0, link);
            }
        }
    }
    chain
}

pub(crate) fn joined_straight(
    pieces: &[JoinPiece],
    chain: &[(usize, bool)],
) -> Result<Curve, KernelError> {
    let mut points: Vec<Point> = Vec::new();
    for &(at, reversed) in chain {
        let mut run = pieces[at].curve.sample()?;
        if reversed {
            run.reverse();
        }
        let skip = usize::from(!points.is_empty());
        points.extend(run.into_iter().skip(skip));
    }
    let closed = points.len() > 3 && gap(points[0], points[points.len() - 1]) <= JOIN_TOLERANCE;
    if closed {
        points.pop();
    }
    Curve::polyline(points, closed)
}

pub(crate) fn joined_smooth(
    pieces: &[JoinPiece],
    chain: &[(usize, bool)],
) -> Result<Curve, KernelError> {
    let mut segments = chain
        .iter()
        .map(|&(at, _)| match pieces[at].curve.to_nurbs()? {
            Curve::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => nurbs_data(degree, &knots, &points, &weights),
            _ => Err(KernelError("that curve cannot be joined".to_owned())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let senses: Vec<bool> = chain.iter().map(|&(_, reversed)| !reversed).collect();
    let engine =
        BSplineCurve::create_by_joining(&mut segments, Some(&senses)).map_err(engine_error)?;
    curve_from_engine(&engine)
}

pub(crate) fn joined_mixed(
    pieces: &[JoinPiece],
    chain: &[(usize, bool)],
) -> Result<Curve, KernelError> {
    let mut segments = Vec::new();
    for &(at, reversed) in chain {
        let piece = if reversed {
            pieces[at].curve.reversed()
        } else {
            pieces[at].curve.clone()
        };
        match piece {
            Curve::Compound { segments: inner } => segments.extend(inner),
            single => segments.push(single),
        }
    }
    Curve::compound(segments)
}

/// Joins curves that meet end to end. Returns each new curve with the indexes of the curves it
/// replaces; curves that meet nothing are left out. Straight curves join into polylines and
/// smooth curves into NURBS curves. Any other mix keeps its parts in a compound curve.
pub fn join(curves: &[Curve]) -> Result<Vec<(Vec<usize>, Curve)>, KernelError> {
    let pieces: Vec<JoinPiece> = curves
        .iter()
        .enumerate()
        .filter_map(|(index, curve)| JoinPiece::of(index, curve))
        .collect();
    let mut used = vec![false; pieces.len()];
    let mut made = Vec::new();
    for seed in 0..pieces.len() {
        if used[seed] {
            continue;
        }
        let chain = chain_from(&pieces, &mut used, seed);
        if chain.len() < 2 {
            continue;
        }
        let all = |test: fn(&Curve) -> bool| chain.iter().all(|&(at, _)| test(&pieces[at].curve));
        let curve = if all(|c| matches!(c, Curve::Line { .. } | Curve::Polyline { .. })) {
            joined_straight(&pieces, &chain)?
        } else if all(|c| matches!(c, Curve::Interpolated { .. } | Curve::Nurbs { .. })) {
            joined_smooth(&pieces, &chain)?
        } else {
            joined_mixed(&pieces, &chain)?
        };
        let replaced = chain.iter().map(|&(at, _)| pieces[at].index).collect();
        made.push((replaced, curve));
    }
    Ok(made)
}

/// The curves joined where they meet end to end, with the ones that join nothing kept as they
/// are: every curve given is in the result once.
pub fn join_all(curves: &[Curve]) -> Result<Vec<Curve>, KernelError> {
    let joined = join(curves)?;
    let mut used = vec![false; curves.len()];
    for (indices, _) in &joined {
        for &i in indices {
            used[i] = true;
        }
    }
    let mut out: Vec<Curve> = joined.into_iter().map(|(_, curve)| curve).collect();
    out.extend(
        curves
            .iter()
            .zip(&used)
            .filter(|(_, used)| !**used)
            .map(|(curve, _)| curve.clone()),
    );
    Ok(out)
}
