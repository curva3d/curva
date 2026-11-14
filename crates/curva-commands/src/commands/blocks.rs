//! The block commands (spec `docs/specs/blocks.md`): `Block`, `Insert`, `BlockManager`,
//! `BlockEdit`, `ExplodeBlock`, `ReplaceBlock` and `SelBlockInstanceNamed`.

use super::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::blocks::{followed_by, moved_by};
use curva_document::{Document, Placement, Shape};
use curva_kernel::Point;

fn instances_among(doc: &Document, ids: &[u64]) -> Vec<u64> {
    doc.state()
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id) && matches!(o.shape, Shape::Instance(_)))
        .map(|o| o.id)
        .collect()
}

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

/// `Insert`: places an instance of a block definition, or of a `.curva` file as a new
/// definition (with `Linked`, it follows the file): the name or file, the insertion point, the
/// scale and the rotation.
#[derive(Default)]
pub struct Insert {
    definition: Option<u64>,
    linked: bool,
    at: Option<Point>,
    scale: Option<f64>,
}

impl Insert {
    fn ask_name(&self) -> Step {
        Step::Ask(Request {
            options: vec!["Linked"],
            ..Request::new(
                if self.linked {
                    "prompt.insert.linked"
                } else {
                    "prompt.insert.name"
                },
                Kind::Path,
            )
        })
    }

    fn placement(&self, angle_degrees: f64) -> Placement {
        let s = self.scale.unwrap_or(1.0);
        let (c, n) = (
            angle_degrees.to_radians().cos(),
            angle_degrees.to_radians().sin(),
        );
        let turn_and_scale: Placement = [
            [s * c, s * n, 0.0, 0.0],
            [-s * n, s * c, 0.0, 0.0],
            [0.0, 0.0, s, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        followed_by(&turn_and_scale, &moved_by(self.at.unwrap_or([0.0; 3])))
    }
}

/// The objects of a `.curva` file, to become a definition.
fn objects_of_file(path: &str) -> Result<Vec<curva_document::Object>, Message> {
    let opened = Document::open(std::path::Path::new(path))
        .map_err(|e| Message::with("error.insert_file", vec![path.to_owned(), e.0]))?;
    Ok(opened
        .state()
        .objects
        .iter()
        .map(|o| curva_document::Object::clone(o))
        .collect())
}

impl Command for Insert {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask_name(),
            Some(Answer::Option("Linked")) => {
                self.linked = !self.linked;
                self.ask_name()
            }
            Some(Answer::Path(text)) if self.definition.is_none() => {
                let text = text.trim().trim_matches('"').to_owned();
                if let Some(found) = doc.definition_named(&text) {
                    self.definition = Some(found.id);
                } else if text.to_lowercase().ends_with(".curva") {
                    let objects = match objects_of_file(&text) {
                        Ok(objects) => objects,
                        Err(message) => return Step::Failed(message),
                    };
                    let name = std::path::Path::new(&text)
                        .file_stem()
                        .map_or_else(|| text.clone(), |s| s.to_string_lossy().into_owned());
                    let linked = self.linked.then(|| text.clone());
                    self.definition = Some(doc.add_definition(name, objects, linked));
                } else {
                    return Step::Failed(Message::with("error.no_block", vec![text]));
                }
                Step::Ask(Request::new("prompt.insert.point", Kind::Point))
            }
            Some(Answer::Point(p)) if self.at.is_none() => {
                self.at = Some(p);
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.insert.scale", Kind::Number)
                })
            }
            Some(Answer::Number(_) | Answer::Empty) if self.scale.is_none() => {
                let scale = match answer {
                    Some(Answer::Number(n)) if n > 0.0 => n,
                    _ => 1.0,
                };
                self.scale = Some(scale);
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.insert.rotation", Kind::Number)
                })
            }
            Some(Answer::Number(_) | Answer::Empty) => {
                let angle = match answer {
                    Some(Answer::Number(n)) => n,
                    _ => 0.0,
                };
                let Some(definition) = self.definition else {
                    return unexpected();
                };
                let id = doc.insert_block(definition, self.placement(angle));
                doc.select(&[id]);
                Step::Done(Some(Message::new("done.insert")))
            }
            _ => unexpected(),
        }
    }
}

/// `BlockManager`: opens the Blocks panel.
#[derive(Default)]
pub struct BlockManager;

impl Command for BlockManager {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowBlocks)
    }
}

/// `BlockEdit`: edits a block definition in place through one of its instances; run again to
/// save it, every instance following.
#[derive(Default)]
pub struct BlockEdit;

impl Command for BlockEdit {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if doc.block_editing().is_some() {
            doc.finish_block_edit();
            return Step::Done(Some(Message::new("done.blockedit_saved")));
        }
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let Some(&instance) = instances_among(doc, &chosen).first() else {
            if chosen.is_empty() {
                return ask_objects("prompt.blockedit.instance");
            }
            return Step::Failed(Message::new("error.not_instance"));
        };
        match doc.start_block_edit(instance) {
            Some(editing) => {
                doc.select(&editing);
                Step::Done(Some(Message::new("done.blockedit_started")))
            }
            None => Step::Failed(Message::new("error.not_instance")),
        }
    }
}

/// `ExplodeBlock`: turns instances back into the objects of their definitions.
#[derive(Default)]
pub struct ExplodeBlock;

impl Command for ExplodeBlock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let instances = instances_among(doc, &chosen);
        if instances.is_empty() {
            return ask_objects("prompt.explodeblock.objects");
        }
        let made = doc.explode_instances(&instances);
        doc.select(&made);
        Step::Done(Some(Message::with(
            "done.explodeblock",
            vec![instances.len().to_string()],
        )))
    }
}

/// `ReplaceBlock`: points instances at another definition: the instances, then its name.
#[derive(Default)]
pub struct ReplaceBlock {
    instances: Vec<u64>,
}

impl Command for ReplaceBlock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.instances.is_empty() => {
                let Some(definition) = doc.definition_named(name.trim()).map(|d| d.id) else {
                    return Step::Failed(Message::with("error.no_block", vec![name]));
                };
                let count = doc.replace_definition(&self.instances, definition);
                Step::Done(Some(Message::with(
                    "done.replaceblock",
                    vec![count.to_string()],
                )))
            }
            Some(Answer::Objects(list)) => {
                self.instances = instances_among(doc, &list);
                Step::Ask(Request::new("prompt.replaceblock.name", Kind::Path))
            }
            None => {
                self.instances = instances_among(doc, &doc.selection());
                if self.instances.is_empty() {
                    return ask_objects("prompt.replaceblock.objects");
                }
                Step::Ask(Request::new("prompt.replaceblock.name", Kind::Path))
            }
            _ => unexpected(),
        }
    }
}

/// `SelBlockInstanceNamed`: selects the instances of a definition by its name.
#[derive(Default)]
pub struct SelBlockInstanceNamed;

impl Command for SelBlockInstanceNamed {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.selblock.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let Some(definition) = doc.definition_named(name.trim()).map(|d| d.id) else {
                    return Step::Failed(Message::with("error.no_block", vec![name]));
                };
                let open = doc.selectable();
                let found: Vec<u64> = doc
                    .instances_of(definition)
                    .into_iter()
                    .filter(|id| open.contains(id))
                    .collect();
                doc.select(&found);
                Step::Done(Some(Message::with(
                    "done.select",
                    vec![found.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
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

    #[test]
    fn insert_reads_a_curva_file_as_a_block() {
        let folder = std::env::temp_dir().join(format!("curva-insert-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("part.curva");
        let mut part = Document::default();
        part.add(Curve::line([0.0; 3], [3.0, 0.0, 0.0]).unwrap());
        part.save(&path).unwrap();
        let mut doc = Document::default();
        run(
            &mut doc,
            &["insert", "Linked", path.to_str().unwrap(), "0,0,0", "", ""],
        );
        assert_eq!(doc.definitions().len(), 1);
        assert_eq!(doc.definitions()[0].linked.as_deref(), path.to_str());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
