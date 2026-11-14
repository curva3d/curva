//! The `LinetypeCommand` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::{Document, Linetype};

/// `Linetype`: sets objects' linetype by name (ByLayer, Continuous, Dashed, Dots, DashDot,
/// Hidden, Center, Border).
#[derive(Default)]
pub struct LinetypeCommand {
    objects: Vec<u64>,
}

impl Command for LinetypeCommand {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.objects.is_empty() => {
                let Some(linetype) = Linetype::named(name.trim()) else {
                    return Step::Failed(Message::with("error.no_linetype", vec![name]));
                };
                doc.set_linetype(&self.objects, linetype);
                Step::Done(Some(Message::with(
                    "done.linetype",
                    vec![linetype.name().to_owned()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.linetype.objects");
                }
                Step::Ask(Request::new("prompt.linetype.name", Kind::Path))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Linetype};
    use curva_kernel::Curve;

    fn line(doc: &mut Document, y: f64) -> u64 {
        doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
    }

    #[test]
    fn linetype_print_width_and_user_text() {
        let mut doc = Document::default();
        let a = line(&mut doc, 0.0);
        doc.select(&[a]);
        run(&mut doc, &["linetype", "hidden"]);
        assert_eq!(doc.state().objects[0].linetype, Linetype::Hidden);
        doc.select(&[a]);
        run(&mut doc, &["printwidth", "0.35"]);
        assert_eq!(doc.print_width_of(a), Some(0.35));
        doc.select(&[a]);
        run(&mut doc, &["setusertext", "Material", "Oak"]);
        doc.select(&[a]);
        let processor = run(&mut doc, &["getusertext"]);
        assert!(processor.log().last().unwrap().contains("Material = Oak"));
    }
}
