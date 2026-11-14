//! The `SelColor` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};
use curva_document::Document;

/// The colour an object shows in: its layer's.
fn colour_of(doc: &Document, id: u64) -> Option<[f32; 3]> {
    let layer = doc.layer_of(id)?;
    doc.layers()
        .iter()
        .find(|l| l.id == layer)
        .map(|l| l.colour)
}

/// `SelColor`: selects every object showing in the same colour as the one picked.
#[derive(Default)]
pub struct SelColor;

impl Recipe for SelColor {
    const NAME: &'static str = "SelColor";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.selcolor.object").one()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let colour = objects(cx, 0).first().and_then(|id| colour_of(cx.doc, *id));
        let found: Vec<u64> = match colour {
            Some(colour) => cx
                .doc
                .selectable()
                .into_iter()
                .filter(|id| colour_of(cx.doc, *id) == Some(colour))
                .collect(),
            None => Vec::new(),
        };
        Ok(Made {
            message: Some(Message::with("done.select", vec![found.len().to_string()])),
            edits: vec![Edit::Select(found)],
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn select_by_colour_lock_and_group() {
        let mut doc = Document::default();
        let red = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(red, [1.0, 0.0, 0.0]);
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 1.0, 0.0], [1.0, 1.0, 0.0]).unwrap());
        let c = doc.add(Curve::line([0.0, 2.0, 0.0], [1.0, 2.0, 0.0]).unwrap());
        doc.move_to_layer(&[a, c], red);
        doc.select(&[a]);
        run(&mut doc, &["selcolor"]);
        assert_eq!(doc.selection(), vec![a, c]);
        doc.select(&[b]);
        run(&mut doc, &["lock"]);
        assert!(!doc.selectable().contains(&b));
        run(&mut doc, &["unlock"]);
        assert!(doc.selectable().contains(&b));
        doc.select(&[a, b]);
        run(&mut doc, &["group"]);
        let mut picked = doc.with_groups(&[a]);
        picked.sort_unstable();
        assert_eq!(picked, vec![a, b]);
        doc.select(&[a]);
        run(&mut doc, &["ungroup"]);
        assert_eq!(doc.with_groups(&[a]), vec![a]);
        // Locking is one undo step.
        doc.select(&[c]);
        run(&mut doc, &["lock"]);
        doc.undo();
        assert!(doc.selectable().contains(&c));
    }
}
