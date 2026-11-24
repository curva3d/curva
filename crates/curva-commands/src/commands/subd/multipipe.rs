//! Rhino's `MultiPipe` for straight lines. Measured in Rhino 8 (23 November 2026, curves picked by
//! id): it asks "Select curves to pipe. Press Enter when done", then "Radius <1> ( Cap=No
//! Struts=1 )". Curves meeting end to end join into one pipe, and where three or more meet (or
//! one ends partway along another) a block joins them; each connected group gives one SubD. One
//! line of length 10 gives 12 points and 8 faces, a square corner 20, a T 29, and the curves
//! stay. See `curva_mesh::SubD::pipes`. Curved curves are not here yet.

use crate::Message;
use crate::commands::arguments::{number, objects};
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Point};
use curva_mesh::{PipeOptions, SubD};

const CAP: &str = "Cap";
const STRUTS: &str = "Struts";

/// The recipe.
#[derive(Default)]
pub struct MultiPipe;

/// A curve's straight segments, or `None` when it is curved.
fn segments_of(curve: &Curve) -> Option<Vec<[Point; 2]>> {
    match curve {
        Curve::Line { start, end } => Some(vec![[*start, *end]]),
        Curve::Polyline { points, closed } => {
            let mut out: Vec<[Point; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
            if *closed && points.len() > 2 {
                out.push([points[points.len() - 1], points[0]]);
            }
            Some(out)
        }
        Curve::Compound { segments } => {
            let parts: Option<Vec<Vec<[Point; 2]>>> = segments.iter().map(segments_of).collect();
            Some(parts?.concat())
        }
        _ => None,
    }
}

impl Recipe for MultiPipe {
    const NAME: &'static str = "multipipe";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(CAP, false),
        OptionDef::whole(STRUTS, 1, 1, 100),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.multipipe.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::number("prompt.multipipe.radius")
                    .remembered("Radius", 1.0)
                    .offering(&[CAP, STRUTS]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(radius) = number(cx, 1) else {
            return Ok(Made::default());
        };
        if radius <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let curves = curves_of(cx.doc, objects(cx, 0));
        let Some(first) = curves.first().map(|(id, _)| *id) else {
            return Err(Message::new("error.multipipe_none"));
        };
        let mut segments = Vec::new();
        for (_, curve) in &curves {
            segments
                .extend(segments_of(curve).ok_or_else(|| Message::new("error.multipipe_curved"))?);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let options = PipeOptions {
            radius,
            cap: cx.toggle(CAP),
            struts: cx.number(STRUTS) as usize,
        };
        let mut made = Made::default();
        let pipes = SubD::pipes(&segments, options)
            .map_err(|e| Message::with("error.geometry", vec![e.0]))?;
        for subd in pipes {
            made.derived.push(Derived {
                shape: Shape::SubD(subd),
                from: first,
                history: None,
            });
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    fn line(doc: &mut Document, a: [f64; 3], b: [f64; 3]) -> u64 {
        doc.add_shape(Shape::Curve(Curve::Line { start: a, end: b }))
    }

    #[test]
    fn two_lines_meeting_pipe_as_one_corner_as_in_rhino() {
        let mut doc = Document::default();
        let a = line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        let b = line(&mut doc, [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]);
        doc.select(&[a, b]);
        run(&mut doc, &["multipipe", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 3, "the curves stay");
        let cage = objects[2].shape.subd().unwrap().cage();
        assert_eq!((cage.vertex_count(), cage.face_count()), (20, 16));
    }

    #[test]
    fn a_junction_of_three_joins_as_in_rhino() {
        let mut doc = Document::default();
        let ids = [
            line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
            line(&mut doc, [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]),
            line(&mut doc, [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]),
        ];
        doc.select(&ids);
        run(&mut doc, &["multipipe", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 4, "one SubD, and the curves stay");
        let cage = objects[3].shape.subd().unwrap().cage();
        assert_eq!((cage.vertex_count(), cage.face_count()), (29, 24));
    }

    #[test]
    fn the_pipe_shows_while_the_radius_is_asked() {
        let mut doc = Document::default();
        let a = line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        doc.select(&[a]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "multipipe");
        assert!(!processor.preview(&doc, None, "2").is_empty());
    }
}
