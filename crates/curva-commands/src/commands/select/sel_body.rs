//! `SelSrf`, `SelPolysrf`, `SelSolid`, `SelClosedSrf` and `SelOpenSrf`: add every surface or
//! solid of one kind that can be selected to the selection, as in Rhino, which sorts them by their
//! faces: a surface is one face, open or closed (a sphere is a surface), a polysurface several
//! joined, and a solid a closed polysurface (#679, measured in Rhino 8).

use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::Body;

/// Which bodies a command picks.
#[derive(Clone, Copy)]
enum Kind {
    /// One face, open or closed.
    Surface,
    /// Several faces joined.
    Polysurface,
    /// Several faces closing round a space.
    Solid,
    /// One face closing round a space, as a sphere.
    ClosedSurface,
    /// One face, open.
    OpenSurface,
}

impl Kind {
    fn takes(self, body: &Body) -> bool {
        let one_face = body.face_count().is_ok_and(|count| count == 1);
        match self {
            Self::Surface => one_face,
            Self::Polysurface => !one_face,
            Self::Solid => !one_face && body.is_solid(),
            Self::ClosedSurface => one_face && body.is_solid(),
            Self::OpenSurface => one_face && !body.is_solid(),
        }
    }
}

/// Adds the selectable bodies of `kind` to the selection.
fn select_bodies(doc: &mut Document, kind: Kind) -> Step {
    let open = doc.selectable();
    let found: Vec<u64> = doc
        .state()
        .objects
        .iter()
        .filter(|object| open.contains(&object.id))
        .filter(|object| object.shape.body().is_some_and(|b| kind.takes(b)))
        .map(|object| object.id)
        .collect();
    let mut selection = doc.selection();
    let added: Vec<u64> = found
        .into_iter()
        .filter(|id| !selection.contains(id))
        .collect();
    let count = added.len();
    selection.extend(added);
    doc.select(&selection);
    Step::Done(Some(Message::with("done.select", vec![count.to_string()])))
}

macro_rules! selector {
    ($name:ident, $kind:expr, $what:literal) => {
        #[doc = concat!("The running `", $what, "`.")]
        #[derive(Default)]
        pub struct $name;

        impl Command for $name {
            fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
                select_bodies(doc, $kind)
            }
        }
    };
}

selector!(SelSrf, Kind::Surface, "SelSrf");
selector!(SelPolysrf, Kind::Polysurface, "SelPolysrf");
selector!(SelSolid, Kind::Solid, "SelSolid");
selector!(SelClosedSrf, Kind::ClosedSurface, "SelClosedSrf");
selector!(SelOpenSrf, Kind::OpenSurface, "SelOpenSrf");

#[cfg(test)]
mod tests {
    use crate::commands::run;
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
    fn a_sphere_is_a_surface_and_a_box_a_solid_as_in_rhino() {
        let mut doc = Document::default();
        let (_, solid, surface) = scene(&mut doc);
        let sphere = doc.add_body(Body::sphere([10.0, 0.0, 0.0], 1.0).unwrap());
        for (command, expected) in [
            ("selsrf", vec![surface, sphere]),
            ("selsolid", vec![solid]),
            ("selpolysrf", vec![solid]),
            ("selclosedsrf", vec![sphere]),
            ("selopensrf", vec![surface]),
        ] {
            doc.select(&[]);
            run(&mut doc, &[command]);
            assert_eq!(doc.selection(), expected, "{command}");
        }
    }

    #[test]
    fn sel_srf_picks_only_open_surfaces() {
        let mut doc = Document::default();
        let (_, _, surface) = scene(&mut doc);
        run(&mut doc, &["selsrf"]);
        assert_eq!(doc.selection(), [surface]);
    }
}
