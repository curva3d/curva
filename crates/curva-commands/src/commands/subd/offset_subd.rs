//! Rhino's `OffsetSubD`: SubDs moved off themselves along their smooth surface's normals.
//! Measured in Rhino 8 (23 November 2026): after the SubDs, it asks "Select object to flip offset
//! direction. Press Enter when done ( Distance=1  Solid=No  BothSides=No  DeleteInput=No
//! FlipAll )"; each cage point moves the distance along its limit normal; Solid joins the offset
//! to the SubD with side quads round its border, BothSides offsets both ways, and the original
//! stays unless DeleteInput. SubDs picked at the second prompt go the other way.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_mesh::SubD;

const DISTANCE: &str = "Distance";
const SOLID: &str = "Solid";
const BOTH_SIDES: &str = "BothSides";
const DELETE_INPUT: &str = "DeleteInput";
const FLIP_ALL: &str = "FlipAll";
const OFFERED: &[&str] = &[DISTANCE, SOLID, BOTH_SIDES, DELETE_INPUT, FLIP_ALL];

/// The recipe.
#[derive(Default)]
pub struct OffsetSubD;

/// The offsets a SubD gives, as the options say: one or two shells, or one solid.
fn offsets(cx: &Context<'_>, subd: &SubD, d: f64) -> Result<Vec<SubD>, Message> {
    let failed = |e: String| Message::with("error.geometry", vec![e]);
    let (solid, both) = (cx.toggle(SOLID), cx.toggle(BOTH_SIDES));
    match (solid, both) {
        (false, false) => Ok(vec![subd.offset(d).map_err(|e| failed(e.to_string()))?]),
        (false, true) => Ok(vec![
            subd.offset(-d).map_err(|e| failed(e.to_string()))?,
            subd.offset(d).map_err(|e| failed(e.to_string()))?,
        ]),
        (true, false) => {
            let (low, high) = if d < 0.0 { (d, 0.0) } else { (0.0, d) };
            Ok(vec![
                subd.offset_solid(low, high)
                    .map_err(|e| failed(e.to_string()))?,
            ])
        }
        (true, true) => {
            let d = d.abs();
            Ok(vec![
                subd.offset_solid(-d, d)
                    .map_err(|e| failed(e.to_string()))?,
            ])
        }
    }
}

impl Recipe for OffsetSubD {
    const NAME: &'static str = "offsetsubd";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number(DISTANCE, 1.0),
        OptionDef::toggle(SOLID, false),
        OptionDef::toggle(BOTH_SIDES, false),
        OptionDef::toggle(DELETE_INPUT, false),
        OptionDef::toggle(FLIP_ALL, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.offsetsubd.objects")];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.offsetsubd.flip")
                    .optional()
                    .offering(OFFERED),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let flipped: Vec<u64> = match cx.values.get(1) {
            Some(Value::Objects(ids)) => ids.clone(),
            _ => Vec::new(),
        };
        let mut made = Made::default();
        for &id in objects(cx, 0).iter() {
            let Some(subd) = cx
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.subd())
            else {
                continue;
            };
            let flip = flipped.contains(&id) != cx.toggle(FLIP_ALL);
            let d = if flip { -1.0 } else { 1.0 } * cx.number(DISTANCE);
            for shape in offsets(cx, subd, d)? {
                made.derived.push(Derived {
                    shape: Shape::SubD(shape),
                    from: id,
                    history: None,
                });
            }
            if cx.toggle(DELETE_INPUT) {
                made.remove.push(id);
            }
        }
        if made.derived.is_empty() {
            return Err(Message::new("error.offsetsubd_none"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::{Document, Shape};
    use curva_mesh::{Mesh, SubD};

    fn strip() -> SubD {
        let points = vec![
            [300.0, 0.0, 0.0],
            [302.0, 0.0, 0.0],
            [304.0, 0.0, 0.0],
            [300.0, 2.0, 0.0],
            [302.0, 2.0, 0.0],
            [304.0, 2.0, 0.0],
        ];
        SubD::new(Mesh::new(points, &[vec![0, 1, 4, 3], vec![1, 2, 5, 4]]).unwrap())
    }

    fn offset(words: &[&str]) -> Document {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(strip()));
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offsetsubd");
        for word in words {
            processor.submit(&mut doc, word);
        }
        processor.submit(&mut doc, "");
        doc
    }

    fn zs(doc: &Document, k: usize) -> Vec<f64> {
        let subd = doc.state().objects[k].shape.subd().unwrap();
        let mut zs: Vec<f64> = subd.cage().vertices().iter().map(|p| p[2]).collect();
        zs.sort_by(f64::total_cmp);
        zs.dedup();
        zs
    }

    #[test]
    fn the_offset_shows_at_the_flip_prompt() {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(strip()));
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offsetsubd");
        assert!(!processor.preview(&doc, None, "").is_empty());
    }

    #[test]
    fn the_options_give_rhinos_offsets() {
        // Rhino 8 on an open strip: Enter puts a new SubD at z = 1 and keeps the strip; Solid
        // makes one of 10 quads on 12 points; BothSides makes two, at -1 and 1.
        let plain = offset(&[]);
        assert_eq!(plain.state().objects.len(), 2);
        assert_eq!(zs(&plain, 1), vec![1.0]);
        let solid = offset(&["Solid"]);
        let made = solid.state().objects[1].shape.subd().unwrap();
        assert_eq!(
            (made.cage().face_count(), made.cage().vertex_count()),
            (10, 12)
        );
        let both = offset(&["BothSides"]);
        assert_eq!(both.state().objects.len(), 3);
        assert_eq!(zs(&both, 1), vec![-1.0]);
        assert_eq!(zs(&both, 2), vec![1.0]);
        let gone = offset(&["DeleteInput"]);
        assert_eq!(gone.state().objects.len(), 1);
    }
}
