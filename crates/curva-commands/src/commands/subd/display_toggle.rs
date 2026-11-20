//! Rhino's `SubDDisplayToggle` (spec `docs/specs/mesh-and-subd.md` §1, Stage B): every SubD shows
//! smooth or flat. Measured in Rhino 8: it switches every SubD in the model whatever is selected,
//! and says "SubD display changed from smooth to flat. 2 SubDs changed." It changes how SubDs show,
//! not their shapes, and it is undoable.

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::SubDDisplay;

/// The recipe.
#[derive(Default)]
pub struct SubDDisplayToggle;

impl Recipe for SubDDisplayToggle {
    const NAME: &'static str = "subddisplaytoggle";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        Vec::new()
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let subds: Vec<_> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.subd().map(|s| (o.id, s)))
            .collect();
        if subds.is_empty() {
            return Err(Message::new("error.subddisplaytoggle_none"));
        }
        // Any smooth SubD makes them all flat; only when all are flat do they all turn smooth.
        let to = if subds
            .iter()
            .any(|(_, s)| s.display() == SubDDisplay::Smooth)
        {
            SubDDisplay::Flat
        } else {
            SubDDisplay::Smooth
        };
        let mut made = Made::default();
        for (id, subd) in &subds {
            if subd.display() != to {
                made.replace
                    .push((*id, Shape::SubD((*subd).clone().with_display(to))));
            }
        }
        let (from, into) = match to {
            SubDDisplay::Flat => ("smooth", "flat"),
            SubDDisplay::Smooth => ("flat", "smooth"),
        };
        made.message = Some(Message::with(
            "done.subddisplaytoggle",
            vec![
                from.to_owned(),
                into.to_owned(),
                made.replace.len().to_string(),
            ],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, SubD, SubDDisplay};

    fn displays(doc: &Document) -> Vec<SubDDisplay> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| o.shape.subd().map(SubD::display))
            .collect()
    }

    #[test]
    fn every_subd_switches_whatever_is_selected_as_in_rhino() {
        let mut doc = Document::default();
        let cube = || SubD::subd_box(&Frame::world(), [2.0; 3], [1, 1, 1]);
        let first = doc.add_shape(Shape::SubD(cube()));
        doc.add_shape(Shape::SubD(cube()));
        doc.select(&[first]);
        run(&mut doc, &["subddisplaytoggle"]);
        assert_eq!(displays(&doc), vec![SubDDisplay::Flat; 2]);
        run(&mut doc, &["subddisplaytoggle"]);
        assert_eq!(displays(&doc), vec![SubDDisplay::Smooth; 2]);
        run(&mut doc, &["undo"]);
        assert_eq!(displays(&doc), vec![SubDDisplay::Flat; 2]);
    }
}
