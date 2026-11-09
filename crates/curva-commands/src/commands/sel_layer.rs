//! `SelLayer`: selects the objects on a named layer.

use super::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelLayer;

impl Command for SelLayer {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.sellayer.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let Some(layer) = doc
                    .layers()
                    .iter()
                    .find(|layer| layer.name.eq_ignore_ascii_case(&name))
                    .map(|layer| layer.id)
                else {
                    return Step::Failed(Message::with("error.no_layer", vec![name]));
                };
                let open = doc.selectable();
                let found: Vec<u64> = doc
                    .state()
                    .objects
                    .iter()
                    .filter(|o| open.contains(&o.id) && o.layer == layer)
                    .map(|o| o.id)
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
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_selects_what_is_on_the_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Walls".to_owned());
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.move_to_layer(&[a], layer);
        run(&mut doc, &["sellayer", "walls"]);
        assert_eq!(doc.selection(), [a]);
    }

    #[test]
    fn an_unknown_layer_is_reported() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["sellayer", "nothing"]);
        assert!(processor.log()[0].contains("nothing"));
    }
}
