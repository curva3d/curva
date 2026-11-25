//! The `Group` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};

/// `Group`: groups objects, so picking one picks them all.
#[derive(Default)]
pub struct Group;

impl Recipe for Group {
    const NAME: &'static str = "Group";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        // One object makes a group too, as in Rhino (#752).
        vec![StepDef::objects("prompt.group.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let chosen = objects(cx, 0).to_vec();
        Ok(Made {
            message: Some(Message::with("done.group", vec![chosen.len().to_string()])),
            edits: vec![Edit::Group(chosen)],
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
    fn one_object_makes_a_group_as_in_rhino() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[a]);
        let processor = run(&mut doc, &["group"]);
        assert!(!processor.is_running());
        assert!(
            processor.log().last().unwrap().contains('1'),
            "{:?}",
            processor.log()
        );
    }
}
