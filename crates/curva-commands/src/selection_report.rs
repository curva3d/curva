//! What a selection holds, as Rhino says it when objects are picked (#520): "1 curve added to
//! selection.", "5 curves, 3 meshes added to selection." Rhino's wording and order, with open and
//! closed told apart for curves and polysurfaces, as the owner asked.

use crate::Message;
use curva_document::annotation::Annotation;
use curva_document::{Document, Shape};

/// The kinds, in the order Rhino lists them, each with its words for one and for several.
const KINDS: [(&str, &str); 12] = [
    ("kind.point.one", "kind.point.many"),
    ("kind.open_curve.one", "kind.open_curve.many"),
    ("kind.closed_curve.one", "kind.closed_curve.many"),
    ("kind.surface.one", "kind.surface.many"),
    ("kind.open_polysurface.one", "kind.open_polysurface.many"),
    (
        "kind.closed_polysurface.one",
        "kind.closed_polysurface.many",
    ),
    ("kind.mesh.one", "kind.mesh.many"),
    ("kind.subd.one", "kind.subd.many"),
    ("kind.text.one", "kind.text.many"),
    ("kind.dimension.one", "kind.dimension.many"),
    ("kind.leader.one", "kind.leader.many"),
    ("kind.block.one", "kind.block.many"),
];

/// Which of [`KINDS`] a shape is.
fn kind_of(shape: &Shape) -> usize {
    match shape {
        Shape::Curve(curve) => {
            if matches!(curve, curva_kernel::Curve::Point { .. }) {
                0
            } else if curve.is_closed().unwrap_or(false) {
                2
            } else {
                1
            }
        }
        Shape::Body(body) => {
            let faces = body.counts().map_or(1, |[faces, _, _]| faces);
            if faces <= 1 && !body.is_solid() {
                3
            } else if body.is_solid() {
                5
            } else {
                4
            }
        }
        Shape::Mesh(_) => 6,
        Shape::SubD(_) => 7,
        Shape::Annotation(Annotation::Text(_)) => 8,
        Shape::Annotation(Annotation::Leader(_)) => 10,
        Shape::Annotation(_) => 9,
        Shape::Instance(_) => 11,
    }
}

/// Each kind among the objects `ids`, with how many: "1 open curve", "3 meshes", in Rhino's order.
#[must_use]
pub fn counted_kinds(doc: &Document, ids: &[u64]) -> Vec<Message> {
    let mut counts = [0_usize; KINDS.len()];
    for object in doc.state().objects.iter().filter(|o| ids.contains(&o.id)) {
        counts[kind_of(&object.shape)] += 1;
    }
    KINDS
        .iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|((one, many), n)| {
            if n == 1 {
                Message::new(one)
            } else {
                Message::with(many, vec![n.to_string()])
            }
        })
        .collect()
}

/// The line the command bar shows when objects are added to or removed from the selection, as
/// Rhino does; `None` when nothing changed.
#[must_use]
pub fn selection_change(
    doc: &Document,
    before: &[u64],
    after: &[u64],
    text: &crate::Translations,
) -> Option<String> {
    let list = |ids: &[u64]| {
        counted_kinds(doc, ids)
            .iter()
            .map(|m| text.render(m))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let added: Vec<u64> = after
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    let removed: Vec<u64> = before
        .iter()
        .copied()
        .filter(|id| !after.contains(id))
        .collect();
    if !added.is_empty() {
        Some(text.render(&Message::with("note.selection.added", vec![list(&added)])))
    } else if !removed.is_empty() && !after.is_empty() {
        Some(text.render(&Message::with(
            "note.selection.removed",
            vec![list(&removed)],
        )))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::{Body, Curve};

    #[test]
    fn a_mix_is_counted_by_kind_in_rhinos_order() {
        let mut doc = Document::default();
        let mut ids = Vec::new();
        for i in 0..3 {
            let at = f64::from(i) * 5.0;
            ids.push(doc.add_shape(Shape::Mesh(curva_mesh::Mesh::mesh_box(
                &curva_mesh::Frame::world(),
                [1.0; 3],
                [1, 1, 1],
            ))));
            ids.push(doc.add(Curve::line([at, 0.0, 0.0], [at + 1.0, 0.0, 0.0]).unwrap()));
        }
        ids.push(doc.add(Curve::circle([0.0; 3], 2.0).unwrap()));
        ids.push(doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap()));
        let text = crate::Translations::english();
        let line = selection_change(&doc, &[], &ids, &text).unwrap();
        assert_eq!(
            line,
            "3 open curves, 1 closed curve, 1 closed polysurface, 3 meshes added to selection."
        );
        let one = selection_change(&doc, &ids[..1], &ids[..2], &text).unwrap();
        assert_eq!(one, "1 open curve added to selection.");
        let fewer = selection_change(&doc, &ids[..2], &ids[..1], &text).unwrap();
        assert_eq!(fewer, "1 open curve removed from selection.");
        assert_eq!(selection_change(&doc, &ids, &ids, &text), None);
    }
}
