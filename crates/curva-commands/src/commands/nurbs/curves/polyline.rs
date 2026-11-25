//! `Polyline`, as in Rhino: straight segments through points, ended by Enter, or by `Close` once
//! there are three points. `Undo` takes back the last point. Each point takes typed distances and
//! angles from the one before. Rhino's arc segments (`Mode`), `Helpers` and `PersistentClose` are
//! still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, point_run, run_points};
use curva_document::Shape;
use curva_kernel::Curve;

const CLOSE: &str = "Close";

/// The recipe.
#[derive(Default)]
pub struct Polyline;

/// Whether the last point given is back on the first, after at least two others: then the
/// polyline closes and the command ends, as in Rhino.
fn closes_on_start(points: &[curva_kernel::Point]) -> bool {
    points.len() >= 4 && curva_kernel::gap(points[0], points[points.len() - 1]) < 1e-9
}

impl Recipe for Polyline {
    const NAME: &'static str = "polyline";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::finish(CLOSE), OptionDef::undo()];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = point_run(cx, "prompt.polyline.start", "prompt.polyline.next", 2);
        // A point back on the start closes it: nothing more is asked.
        if closes_on_start(&run_points(cx)) {
            steps.pop();
            return steps;
        }
        let count = run_points(cx).len();
        let offered: &[&'static str] = match count {
            0 => &[],
            1 | 2 => &["Undo"],
            _ => &[CLOSE, "Undo"],
        };
        if let Some(last) = steps.last_mut() {
            *last = last.clone().offering(offered);
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut points = run_points(cx);
        let closed = cx.mode == Some(CLOSE) || closes_on_start(&points);
        if closes_on_start(&points) {
            points.pop();
        }
        if points.len() < 2 {
            return Ok(Made::default());
        }
        let count = points.len();
        let polyline = Curve::polyline(points, closed)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(polyline)],
            message: Some(Message::with("done.polyline", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn the_points_given_are_offered_to_snap_to_so_it_can_close_on_its_start() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["polyline", "0,0", "10,0", "10,10"]);
        let preview = &processor.current_request().unwrap().preview;
        assert_eq!(preview.first(), Some(&[0.0, 0.0, 0.0]));
        assert_eq!(preview.last(), Some(&[10.0, 10.0, 0.0]));
    }

    #[test]
    fn the_start_point_picked_again_closes_it_and_ends_the_command() {
        let mut doc = Document::default();
        let processor = run(
            &mut doc,
            &["polyline", "0,0,0", "10,0,0", "10,10,0", "0,0,0"],
        );
        assert!(!processor.is_running());
        let curve = doc.state().objects[0].curve().unwrap();
        assert!(curve.is_closed().unwrap());
        assert!((curve.length().unwrap() - 34.142_135_6).abs() < 1e-6);
    }

    #[test]
    fn enter_finishes_an_open_polyline() {
        let mut doc = Document::default();
        run(&mut doc, &["polyline", "0,0", "10,0", "10,10", ""]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 20.0).abs() < 1e-9);
    }

    #[test]
    fn close_adds_the_last_segment() {
        let mut doc = Document::default();
        run(&mut doc, &["pl", "0,0", "10,0", "10,10", "0,10", "close"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 40.0).abs() < 1e-9);
    }

    #[test]
    fn enter_needs_two_points_first() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["pl", "0,0", ""]);
        assert!(processor.is_running());
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn undo_takes_back_the_last_point_and_distances_can_be_typed() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["pl", "0,0", "50,50", "undo", "10<0", "10<90", ""],
        );
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 20.0).abs() < 1e-9, "{length}");
    }
}
