//! `SelSrf` and `SelSolid`: add every open surface, or every closed solid, that can be
//! selected to the selection, as in Rhino.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// Adds the selectable bodies that are (`solid`) or are not closed to the selection.
fn select_bodies(doc: &mut Document, solid: bool) -> Step {
    let open = doc.selectable();
    let found: Vec<u64> = doc
        .state()
        .objects
        .iter()
        .filter(|object| open.contains(&object.id))
        .filter(|object| object.shape.body().is_some_and(|b| b.is_solid() == solid))
        .map(|object| object.id)
        .collect();
    let mut selection = doc.selection();
    let added = found.iter().filter(|id| !selection.contains(id)).count();
    selection.extend(
        found
            .iter()
            .filter(|id| !selection.contains(id))
            .copied()
            .collect::<Vec<_>>(),
    );
    doc.select(&selection);
    Step::Done(Some(Message::with("done.select", vec![added.to_string()])))
}

/// The running `SelSrf`.
#[derive(Default)]
pub struct SelSrf;

impl Command for SelSrf {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        select_bodies(doc, false)
    }
}

/// The running `SelSolid`.
#[derive(Default)]
pub struct SelSolid;

impl Command for SelSolid {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        select_bodies(doc, true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn scene(doc: &mut Document) -> (u64, u64, u64) {
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let solid = doc.add_body(Body::cuboid([5.0, 0.0, 0.0], [1.0, 1.0, 1.0]).unwrap());
        let square = Curve::polyline(
            vec![
                [0.0, 5.0, 0.0],
                [1.0, 5.0, 0.0],
                [1.0, 6.0, 0.0],
                [0.0, 6.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let surface = doc.add_body(Body::planar(&[square]).unwrap());
        (line, solid, surface)
    }

    #[test]
    fn sel_solid_picks_only_solids_and_adds_to_the_selection() {
        let mut doc = Document::default();
        let (line, solid, _) = scene(&mut doc);
        doc.select(&[line]);
        run(&mut doc, &["selsolid"]);
        assert_eq!(doc.selection(), [line, solid]);
    }

    #[test]
    fn sel_srf_picks_only_open_surfaces() {
        let mut doc = Document::default();
        let (_, _, surface) = scene(&mut doc);
        run(&mut doc, &["selsrf"]);
        assert_eq!(doc.selection(), [surface]);
    }
}
