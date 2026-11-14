//! `SelName`: selects the objects with a given name.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelName;

impl Command for SelName {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.selname.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let open = doc.selectable();
                let found: Vec<u64> = doc
                    .state()
                    .objects
                    .iter()
                    .filter(|o| open.contains(&o.id) && o.name.eq_ignore_ascii_case(&name))
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
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_finds_a_name_whatever_its_case() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.rename(&[a], "Beam");
        run(&mut doc, &["selname", "beam"]);
        assert_eq!(doc.selection(), [a]);
    }
}
