//! The `Insert` command.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::blocks::{followed_by, moved_by};
use curva_document::{Document, Placement};
use curva_kernel::Point;

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

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

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
