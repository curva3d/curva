//! The `Block` command.

use crate::commands::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_kernel::Point;

/// `Block`: makes a block definition from objects: the objects, a base point, a name. The
/// objects become one instance of it.
#[derive(Default)]
pub struct Block {
    objects: Vec<u64>,
    base: Option<Point>,
}

impl Command for Block {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None if self.objects.is_empty() => {
                self.objects = doc.selection();
                if self.objects.is_empty() {
                    return ask_objects("prompt.block.objects");
                }
                Step::Ask(Request::new("prompt.block.base", Kind::Point))
            }
            Some(Answer::Objects(list)) if self.objects.is_empty() => {
                if list.is_empty() {
                    return ask_objects("prompt.block.objects");
                }
                self.objects = list;
                Step::Ask(Request::new("prompt.block.base", Kind::Point))
            }
            Some(Answer::Point(p)) if self.base.is_none() => {
                self.base = Some(p);
                Step::Ask(Request::new("prompt.block.name", Kind::Path))
            }
            Some(Answer::Path(name)) => {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return Step::Ask(Request::new("prompt.block.name", Kind::Path));
                }
                if doc.definition_named(&name).is_some() {
                    return Step::Failed(Message::with("error.block_name_taken", vec![name]));
                }
                let (_, instance) =
                    doc.define_block(name.clone(), self.base.unwrap_or([0.0; 3]), &self.objects);
                doc.select(&[instance]);
                Step::Done(Some(Message::with("done.block", vec![name])))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::{Body, Curve};

    fn chair(doc: &mut Document) -> Vec<u64> {
        vec![
            doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 0.2]).unwrap()),
            doc.add(Curve::line([0.0; 3], [0.0, 0.0, -1.0]).unwrap()),
        ]
    }

    #[test]
    fn make_insert_select_replace_and_explode_blocks() {
        let mut doc = Document::default();
        let ids = chair(&mut doc);
        doc.select(&ids);
        run(&mut doc, &["block", "0,0,0", "Chair"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(matches!(doc.state().objects[0].shape, Shape::Instance(_)));
        run(&mut doc, &["insert", "chair", "5,0,0", "2", "90"]);
        assert_eq!(doc.state().objects.len(), 2);
        run(&mut doc, &["selblockinstancenamed", "Chair"]);
        assert_eq!(doc.selection().len(), 2);
        // A second definition, and one instance swapped to it.
        let other = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        doc.select(&[other]);
        run(&mut doc, &["block", "0,0,0", "Lamp"]);
        let first = doc.state().objects[0].id;
        doc.select(&[first]);
        run(&mut doc, &["replaceblock", "lamp"]);
        let Shape::Instance(i) = &doc.state().objects[0].shape else {
            panic!()
        };
        assert_eq!(
            doc.definitions()
                .iter()
                .find(|d| d.id == i.definition)
                .unwrap()
                .name,
            "Lamp"
        );
        doc.select(&[first]);
        run(&mut doc, &["explodeblock"]);
        // The lamp's circle, placed (as an exact NURBS circle).
        assert!(
            doc.state()
                .objects
                .iter()
                .any(|o| o.curve().is_some_and(|c| c.is_closed().unwrap_or(false)))
        );
    }

    #[test]
    fn block_edit_changes_every_instance() {
        let mut doc = Document::default();
        let ids = chair(&mut doc);
        doc.select(&ids);
        run(&mut doc, &["block", "0,0,0", "Chair"]);
        run(&mut doc, &["insert", "Chair", "5,0,0", "", ""]);
        let first = doc.state().objects[0].id;
        doc.select(&[first]);
        run(&mut doc, &["blockedit"]);
        let editing = doc.selection();
        assert_eq!(editing.len(), 2);
        let leg = editing
            .iter()
            .copied()
            .find(|id| {
                doc.state()
                    .objects
                    .iter()
                    .find(|o| o.id == *id)
                    .unwrap()
                    .curve()
                    .is_some()
            })
            .unwrap();
        doc.remove(&[leg]);
        run(&mut doc, &["blockedit"]);
        let Shape::Instance(i) = &doc.state().objects[1].shape else {
            panic!()
        };
        assert_eq!(doc.instance_shapes(i).len(), 1);
    }
}
