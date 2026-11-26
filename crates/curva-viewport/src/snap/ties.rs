//! Ties (#296, decision 0035): which object, part and place an End, Mid, Center or Point snap
//! landed on, so a dimension drawn from it can follow the object when it moves or changes.

use curva_document::tie::{Place, Tie, place_on};
use curva_kernel::{Curve, Point};

use super::{Site, SnapKind};

/// A site that holds no tie.
pub(crate) const NO_TIE: u32 = u32::MAX;

/// Ties each End, Mid, Center and Point site of `curve` to its place: `curve` is part `part` of
/// object `object` (a surface's or solid's edge by number, or `None` for a whole curve). Each
/// tied site moves to its place exactly, so the tie's point and the snap's are the same, and its
/// tie is added to `ties`.
pub(crate) fn tie_sites(
    sites: &mut [Site],
    ties: &mut Vec<Tie>,
    object: u64,
    part: Option<usize>,
    curve: &Curve,
) {
    for site in sites {
        let Some((part, place, at)) = place_of_site(curve, part, site.kind, site.point) else {
            continue;
        };
        let Ok(number) = u32::try_from(ties.len()) else {
            return;
        };
        site.point = at;
        site.tie = number;
        ties.push(Tie {
            object,
            part,
            place,
            at,
        });
    }
}

/// Appends `other`'s sites and ties to `sites` and `ties`, renumbering the ties it holds.
pub(crate) fn append(sites: &mut Vec<Site>, ties: &mut Vec<Tie>, other: (&[Site], &[Tie])) {
    let offset = u32::try_from(ties.len()).unwrap_or(NO_TIE);
    sites.extend(other.0.iter().map(|site| Site {
        tie: if site.tie == NO_TIE {
            NO_TIE
        } else {
            site.tie.saturating_add(offset)
        },
        ..*site
    }));
    ties.extend_from_slice(other.1);
}

/// Drops every tie: for an object whose pieces cannot be tied, such as the parts of a block
/// instance.
pub(crate) fn forget(sites: &mut [Site], ties: &mut Vec<Tie>) {
    for site in sites {
        site.tie = NO_TIE;
    }
    ties.clear();
}

/// The part, place and exact point a site of `kind` near `point` on `curve` stands for: a
/// line's or polyline's corners and segment middles by number, a point object's only corner,
/// another curve's start, end, middle or centre, the nearest of them where several could be.
/// A joined curve's places are its segments'. `None` for the other kinds of snap.
fn place_of_site(
    curve: &Curve,
    part: Option<usize>,
    kind: SnapKind,
    point: Point,
) -> Option<(Option<usize>, Place, Point)> {
    if let Curve::Compound { segments } = curve {
        // Each segment is a part of its own: the one whose place is nearest.
        return segments
            .iter()
            .enumerate()
            .filter_map(|(i, segment)| {
                let (_, place, at) = place_of_site(segment, None, kind, point)?;
                Some((Some(i), place, at))
            })
            .filter(|_| part.is_none())
            .min_by(|a, b| gap(a.2, point).total_cmp(&gap(b.2, point)));
    }
    let corners = match curve {
        Curve::Point { .. } => 1,
        Curve::Line { .. } => 2,
        Curve::Polyline { points, .. } => points.len(),
        _ => 0,
    };
    let places: Vec<Place> = match kind {
        SnapKind::Point if corners == 1 => vec![Place::Corner(0)],
        SnapKind::End if corners > 1 => (0..corners).map(Place::Corner).collect(),
        SnapKind::Mid if corners > 1 => (0..corners).map(Place::SegmentMid).collect(),
        SnapKind::End => vec![Place::Start, Place::End],
        SnapKind::Mid => vec![Place::Mid],
        SnapKind::Center => vec![Place::Center],
        _ => return None,
    };
    places
        .into_iter()
        .filter_map(|place| Some((part, place, place_on(curve, place)?)))
        .min_by(|a, b| gap(a.2, point).total_cmp(&gap(b.2, point)))
}

fn gap(a: Point, b: Point) -> f64 {
    curva_kernel::gap(a, b)
}
