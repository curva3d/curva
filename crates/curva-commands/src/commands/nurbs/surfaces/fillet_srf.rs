//! The `FilletSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

const TRIM: &str = "Trim";

/// `input` trimmed back to where `fillet` meets it, as Rhino's Trim=Yes does (#732): split along
/// the fillet's edges lying on it, keeping the piece farthest from the fillet. `None` when the
/// fillet's edges do not cut it.
fn trimmed(input: &Body, fillet: &Body) -> Option<Body> {
    let edges: Vec<Curve> = fillet
        .exact_edges()
        .ok()?
        .into_iter()
        .map(|(curve, _)| curve)
        .filter(|curve| {
            let points = curve.divided(4).unwrap_or_default();
            !points.is_empty()
                && input
                    .on_surface(&points)
                    .is_ok_and(|on| on.iter().all(|x| *x))
        })
        .collect();
    if edges.is_empty() {
        return None;
    }
    let pieces =
        crate::commands::nurbs::surfaces::split_srf::split_pieces(input, &edges).ok()??;
    let middle = fillet.centroid().ok()?;
    pieces.into_iter().max_by(|x, y| {
        let away = |b: &Body| b.centroid().map_or(0.0, |c| curva_kernel::gap(c, middle));
        away(x).total_cmp(&away(y))
    })
}

/// `FilletSrf` (or, with `CHAMFER`, `ChamferSrf`): a fillet (or chamfer) surface of a radius
/// between two surfaces, on the sides facing each other.
#[derive(Default)]
pub struct FilletSrf<const CHAMFER: bool>;

impl<const CHAMFER: bool> Recipe for FilletSrf<CHAMFER> {
    const NAME: &'static str = if CHAMFER { "chamfersrf" } else { "filletsrf" };
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(TRIM, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.filletsrf.first").bodies().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.filletsrf.second").bodies().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.filletsrf.radius").remembered("Radius", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((id, a)), Some((other, b)), Some(radius)) =
            (first_body(cx, 0), first_body(cx, 1), number(cx, 2))
        else {
            return Ok(Made::default());
        };
        if radius <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let made = a
            .filleted_with(&b, radius, CHAMFER)
            .map_err(|e| failed(&e))?;
        // Both inputs trimmed back to the fillet, as Rhino does by default.
        let replace = if cx.toggle(TRIM) {
            [(id, &a), (other, &b)]
                .into_iter()
                .filter_map(|(input_id, input)| {
                    Some((input_id, Shape::Body(trimmed(input, &made)?)))
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(Made {
            replace,
            derived: vec![Derived {
                shape: Shape::Body(made),
                from: id,
                history: None,
            }],
            message: Some(Message::new(if CHAMFER {
                "done.chamfersrf"
            } else {
                "done.filletsrf"
            })),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use curva_document::Document;

    #[test]
    fn both_surfaces_are_trimmed_back_to_the_fillet_as_in_rhino() {
        let mut doc = Document::default();
        let mut processor = crate::Processor::default();
        for step in ["plane", "0,0,0", "10,10,0", "line", "0,0,0", "10,0,0"] {
            processor.submit(&mut doc, step);
        }
        let line = doc.state().objects[1].id;
        doc.select(&[line]);
        for step in ["extrudecrv", "10"] {
            processor.submit(&mut doc, step);
        }
        let floor = doc.state().objects[0].id;
        let wall = doc.state().objects.last().unwrap().id;
        processor.submit(&mut doc, "filletsrf");
        for picked in [floor, wall] {
            doc.select(&[picked]);
            processor.submit(&mut doc, "");
        }
        processor.submit(&mut doc, "1");
        let area = |id: u64| {
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .unwrap()
                .shape
                .body()
                .unwrap()
                .area()
                .unwrap()
        };
        // Rhino 8: the floor and the wall are trimmed to 90 each; the fillet is 15.708.
        assert!((area(floor) - 90.0).abs() < 1e-3, "floor {}", area(floor));
        assert!((area(wall) - 90.0).abs() < 1e-3, "wall {}", area(wall));
    }
}
