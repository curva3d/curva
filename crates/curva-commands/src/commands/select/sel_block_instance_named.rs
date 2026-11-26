//! The `SelBlockInstanceNamed` command.

use super::sel_name::matches;
use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `SelBlockInstanceNamed`: selects the instances of the definitions whose name matches, whatever
/// its case, with `*` for any run of letters and `?` for any one, as Rhino 8 matches them (#772).
#[derive(Default)]
pub struct SelBlockInstanceNamed;

impl Command for SelBlockInstanceNamed {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.selblock.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let pattern = name.trim();
                let definitions: Vec<u64> = doc
                    .definitions()
                    .iter()
                    .filter(|d| matches(pattern, &d.name))
                    .map(|d| d.id)
                    .collect();
                if definitions.is_empty() {
                    return Step::Failed(Message::with("error.no_block", vec![name]));
                }
                let open = doc.selectable();
                let found: Vec<u64> = definitions
                    .iter()
                    .flat_map(|definition| doc.instances_of(*definition))
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
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_wildcard_selects_the_instances_of_every_matching_block_as_in_rhino() {
        let mut doc = Document::default();
        for (name, x) in [("TstA", 0.0), ("TstB", 5.0), ("Other", 10.0)] {
            let circle = doc.add(Curve::circle([x, 0.0, 0.0], 1.0).unwrap());
            doc.select(&[circle]);
            run(&mut doc, &["block", &format!("{x},0,0"), name]);
        }
        // Rhino 8: Tst* selects the two Tst instances.
        run(&mut doc, &["selblockinstancenamed", "Tst*"]);
        assert_eq!(doc.selection().len(), 2);
        run(&mut doc, &["selblockinstancenamed", "tsta"]);
        assert_eq!(doc.selection().len(), 1);
    }
}
