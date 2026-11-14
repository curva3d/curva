//! The `ShowEdges` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};

/// `ShowEdges`: shows the edges of surfaces and polysurfaces, the open (naked) ones only with
/// `NakedEdges` (the default), and counts them; Enter ends it.
#[derive(Default)]
pub struct ShowEdges;

impl Recipe for ShowEdges {
    const NAME: &'static str = "showedges";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("NakedEdges", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.showedges.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.showedges.done").optional());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut guides = Vec::new();
        let (mut all, mut naked) = (0, 0);
        for (_, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let open = body.naked_edges().map_err(|e| failed(&e))?;
            let mesh = body.mesh().map_err(|e| failed(&e))?;
            all += mesh.edge_ids.len();
            naked += open.len();
            let shown: Vec<usize> = if cx.toggle("NakedEdges") {
                open
            } else {
                mesh.edge_ids.clone()
            };
            for edge in shown {
                if let Ok(curve) = body.edge_curve(edge) {
                    guides.push(curve);
                }
            }
        }
        Ok(Made {
            guides,
            message: Some(Message::with(
                "done.showedges",
                vec![all.to_string(), naked.to_string()],
            )),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn show_edges_counts_and_plan_turns_the_view() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        let processor = run(&mut doc, &["showedges", ""]);
        assert!(
            processor
                .log()
                .iter()
                .any(|line| line.contains("4 edges, 4 of them naked")),
            "{:?}",
            processor.log()
        );
        let mut processor = run(&mut doc, &["plan"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::Plan));
        let mut processor = run(&mut doc, &["zebra"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::Zebra));
    }
}
