//! The `ExtendSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Carry;

/// `ExtendSrf`: lengthens a surface past a picked edge by a distance, `Smooth` (the surface
/// carried on, the default) or `Line` (straight along its tangent).
#[derive(Default)]
pub struct ExtendSrf;

impl Recipe for ExtendSrf {
    const NAME: &'static str = "extendsrf";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode("Smooth"), OptionDef::mode("Line")];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.extendsrf.edge").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.extendsrf.distance").remembered("Distance", 10.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Edges(edges)), Some(distance)) = (cx.values.first(), number(cx, 1)) else {
            return Ok(Made::default());
        };
        if distance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let how = if cx.mode == Some("Line") {
            Carry::Tangent
        } else {
            Carry::Natural
        };
        let mut made = Made::default();
        for (id, edge) in edges {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let longer = body
                .extended_at(*edge, distance, how)
                .map_err(|e| failed(&e))?;
            made.replace.push((*id, Shape::Body(longer)));
        }
        made.message = Some(Message::with(
            "done.extendsrf",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    #[test]
    fn extend_and_untrim_surfaces() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,4"]);
        let id = doc.state().objects[0].id;
        let edge = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .mesh()
            .unwrap()
            .edge_ids[0];
        doc.select_edges(&[(id, edge)]);
        run(&mut doc, &["extendsrf", "", "2"]);
        assert!((last_body(&doc).area().unwrap() - 24.0).abs() < 1e-6);
        let mut doc = Document::default();
        let holed = Body::planar(&[
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
            Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap(),
        ])
        .unwrap();
        let hole_edge = (0..holed.record_edge_count())
            .find(|e| holed.edge_curve(*e).unwrap().is_closed().unwrap())
            .unwrap();
        let id = doc.add_body(holed);
        doc.select_edges(&[(id, hole_edge)]);
        run(&mut doc, &["untrim", ""]);
        assert!((last_body(&doc).area().unwrap() - 16.0).abs() < 1e-6);
    }
}
