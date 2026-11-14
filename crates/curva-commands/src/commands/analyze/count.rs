//! The `Count` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};

/// `Count`: how many objects are selected, by kind.
#[derive(Default)]
pub struct Count;

impl Command for Count {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        if ids.is_empty() {
            return ask_objects("prompt.count.objects");
        }
        let (mut curves, mut surfaces, mut solids, mut blocks) = (0, 0, 0, 0);
        for object in doc.state().objects.iter().filter(|o| ids.contains(&o.id)) {
            match &object.shape {
                Shape::Curve(_) => curves += 1,
                Shape::Body(b) if b.is_solid() => solids += 1,
                Shape::Body(_) => surfaces += 1,
                Shape::Instance(_) => blocks += 1,
            }
        }
        Step::Done(Some(Message::with(
            "done.count",
            [ids.len(), curves, surfaces, solids, blocks]
                .iter()
                .map(ToString::to_string)
                .collect(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Linetype, PrintWidth};
    use curva_kernel::{Body, Curve};

    fn line(doc: &mut Document, y: f64) -> u64 {
        doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
    }

    #[test]
    fn count_change_layer_and_match_properties() {
        let mut doc = Document::default();
        let a = line(&mut doc, 0.0);
        let b = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["count"]);
        assert!(processor.log().last().unwrap().contains('2'));
        let walls = doc.add_layer("Walls".to_owned());
        doc.select(&[a]);
        run(&mut doc, &["changelayer", "walls"]);
        assert_eq!(doc.layer_of(a), Some(walls));
        doc.set_linetype(&[a], Linetype::Dashed);
        doc.set_print_width(&[a], PrintWidth::Millimetres(0.5));
        let c = line(&mut doc, 2.0);
        doc.select(&[c]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "matchproperties");
        doc.select(&[a]);
        processor.submit(&mut doc, "");
        let matched = doc.state().objects.iter().find(|o| o.id == c).unwrap();
        assert_eq!(
            (matched.layer, matched.linetype, matched.print_width),
            (walls, Linetype::Dashed, PrintWidth::Millimetres(0.5))
        );
    }
}
