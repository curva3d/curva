//! What an object's pieces draw as: the kind of line of each shape, and a body's edges as curves.

use curva_document::Shape;
use curva_kernel::{Curve, Point};

use super::{IN_PLACE, Placement, placed};

/// The kind of line `shape`'s pieces draw as ([`crate::LineKind`]).
pub(super) fn line_kind_of(shape: &Shape) -> crate::LineKind {
    match shape {
        Shape::Body(_) => crate::LineKind::Edge,
        Shape::Mesh(_) => crate::LineKind::MeshWire,
        Shape::SubD(_) => crate::LineKind::SubdEdge,
        Shape::Annotation(_) => crate::LineKind::Annotation,
        _ => crate::LineKind::Curve,
    }
}

/// A mesh's edges as curves where `placement` puts them, each with its number among the body's
/// edges.
pub(super) fn edge_curves(
    mesh: &curva_kernel::BodyMesh,
    placement: &Placement,
) -> Vec<(Curve, Option<usize>)> {
    let place = |p: &Point| {
        if *placement == IN_PLACE {
            *p
        } else {
            placed(placement, *p)
        }
    };
    mesh.edges
        .iter()
        .zip(&mesh.edge_ids)
        .filter_map(|(run, id)| match run.len() {
            0 | 1 => None,
            2 => Curve::line(place(&run[0]), place(&run[1]))
                .ok()
                .map(|c| (c, Some(*id))),
            _ => Curve::polyline(run.iter().map(place).collect(), false)
                .ok()
                .map(|c| (c, Some(*id))),
        })
        .collect()
}
