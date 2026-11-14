//! The `Section` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Curve, cross, sub, unit};

/// `Section`: curves where a vertical plane through a line cuts surfaces and solids: the line's
/// two points, the plane standing up out of the construction plane.
#[derive(Default)]
pub struct Section;

impl Recipe for Section {
    const NAME: &'static str = "section";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.section.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.section.start"));
        }
        if let Some(a) = point(cx, 1) {
            steps.push(StepDef::point("prompt.section.end").from(a));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 1), point(cx, 2)) else {
            return Ok(Made::default());
        };
        let normal =
            unit(cross(sub(b, a), up(cx))).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let cut = body.section(a, normal).map_err(|e| failed(&e))?;
            made.derived.extend(added_curves(cut, id));
        }
        made.guides = Curve::line(a, b).ok().into_iter().collect();
        made.message = Some(Message::with(
            "done.section",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn curves_made(doc: &Document, before: usize) -> Vec<Curve> {
        doc.state().objects[before..]
            .iter()
            .filter_map(|o| o.curve().cloned())
            .collect()
    }

    #[test]
    fn section_contour_intersect_and_project() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap());
        doc.select(&[cube]);
        run(&mut doc, &["section", "1,-1", "1,5"]);
        let cut = curves_made(&doc, 1);
        assert_eq!(cut.len(), 1);
        assert!(
            (cut[0].length().unwrap() - 14.0).abs() < 1e-6,
            "{}",
            cut[0].length().unwrap()
        );
        let before = doc.state().objects.len();
        doc.select(&[cube]);
        run(&mut doc, &["contour", "0,0,0", "0,0,1", "1"]);
        // Planes at z = 0, 1, 2, 3, 4.
        assert_eq!(curves_made(&doc, before).len(), 5);
        let other = doc.add_body(Body::cuboid([1.0, 1.0, 1.0], [2.0, 3.0, 4.0]).unwrap());
        let before = doc.state().objects.len();
        doc.select(&[cube, other]);
        run(&mut doc, &["intersect"]);
        assert!(!curves_made(&doc, before).is_empty());
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "5,5"]);
        let floor = doc.state().objects[0].id;
        let wire = doc.add(line([1.0, 1.0, 3.0], [4.0, 1.0, 3.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[wire]);
        processor.submit(&mut doc, "project");
        doc.select(&[floor]);
        processor.submit(&mut doc, "");
        let shadow = curves_made(&doc, 2);
        assert_eq!(shadow.len(), 1);
        assert!(
            shadow[0]
                .sample()
                .unwrap()
                .iter()
                .all(|p| p[2].abs() < 1e-9)
        );
    }
}
