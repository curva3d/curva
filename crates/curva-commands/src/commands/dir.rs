//! `Dir`: shows which way curves run with arrows, and flips the ones clicked, as in Rhino.

use super::{ask_objects, curves_of, plane_normal, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, cross, gap, sub, unit};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;
/// Where along each curve an arrow sits, as fractions of its length.
const ARROW_PLACES: [f64; 3] = [0.25, 0.5, 0.75];
/// An arrow's length as a fraction of its curve's length, so arrows suit any size of curve.
const ARROW_SHARE: f64 = 0.08;
const FLIP: &str = "Flip";

/// The running command.
#[derive(Default)]
pub struct Dir {
    ids: Option<Vec<u64>>,
    flipped: usize,
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [0, 1, 2].map(|k| p[k] + direction[k] * amount)
}

/// Arrows along a curve pointing the way it runs: a shaft along the curve and two barbs.
fn arrows(curve: &Curve, normal: Point) -> Vec<Curve> {
    let Ok(path) = curve.sample() else {
        return Vec::new();
    };
    let lengths: Vec<f64> = path.windows(2).map(|pair| gap(pair[0], pair[1])).collect();
    let total: f64 = lengths.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let size = total * ARROW_SHARE;
    let mut made = Vec::new();
    for share in ARROW_PLACES {
        let mut left = total * share;
        for (index, length) in lengths.iter().enumerate() {
            if left > *length {
                left -= length;
                continue;
            }
            let Some(along) = unit(sub(path[index + 1], path[index])) else {
                break;
            };
            let tip = plus(path[index], along, left);
            // The barbs lie in the drawing plane, or across the curve if it runs straight out of it.
            let side = unit(cross(normal, along))
                .or_else(|| unit(cross([1.0, 0.0, 0.0], along)))
                .or_else(|| unit(cross([0.0, 1.0, 0.0], along)))
                .unwrap_or([0.0, 0.0, 1.0]);
            let back = plus(tip, along, -size);
            made.extend(Curve::line(back, tip));
            for sign in [1.0, -1.0] {
                let barb = plus(plus(tip, along, -size * 0.4), side, sign * size * 0.25);
                made.extend(Curve::line(barb, tip));
            }
            break;
        }
    }
    made
}

fn ask_click(flipped: bool) -> Step {
    Step::Ask(Request {
        prompt: Message::new(if flipped {
            "prompt.dir.flipped"
        } else {
            "prompt.dir.click"
        }),
        allow_empty: true,
        options: vec![FLIP],
        ..Request::new("prompt.dir.click", Kind::PickPart)
    })
}

impl Command for Dir {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.dir.objects");
            }
            self.ids = Some(chosen);
            return ask_click(false);
        };
        match answer {
            Some(Answer::Empty) => Step::Done(Some(Message::with(
                "done.dir",
                vec![self.flipped.to_string()],
            ))),
            Some(Answer::Option(_)) => {
                for (id, curve) in curves_of(doc, &ids) {
                    doc.replace(id, curve.reversed());
                    self.flipped += 1;
                }
                ask_click(true)
            }
            Some(Answer::Number(_)) => ask_click(false),
            Some(Answer::Point(at)) => {
                let hit = curves_of(doc, &ids).into_iter().find(|(_, curve)| {
                    curve
                        .closest_point(at)
                        .is_ok_and(|p| gap(p, at) <= ON_CURVE)
                });
                match hit {
                    Some((id, curve)) => {
                        doc.replace(id, curve.reversed());
                        self.flipped += 1;
                        ask_click(true)
                    }
                    None => ask_click(false),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, _live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let normal = plane_normal(doc);
        curves_of(doc, ids)
            .iter()
            .flat_map(|(_, curve)| arrows(curve, normal))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    fn two_lines() -> (Document, u64, u64) {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 5.0, 0.0], [10.0, 5.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        (doc, a, b)
    }

    fn start_of(doc: &Document, id: u64) -> [f64; 3] {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.curve().unwrap().sample().unwrap()[0]
    }

    #[test]
    fn arrows_point_the_way_each_curve_runs() {
        let (mut doc, ..) = two_lines();
        let processor = run(&mut doc, &["dir"]);
        let shown = processor.preview(&doc, None, "");
        assert_eq!(shown.len(), 18);
        // Each arrow's shaft ends at its tip, further along +X than where it starts.
        let shaft = shown[0].sample().unwrap();
        assert!(shaft[1][0] > shaft[0][0]);
        assert!(gap(shaft[1], [2.5, 0.0, 0.0]) < 1e-9);
    }

    #[test]
    fn clicking_a_curve_flips_only_that_one() {
        let (mut doc, a, b) = two_lines();
        let mut processor = run(&mut doc, &["dir"]);
        processor.click(&mut doc, [4.0, 5.0, 0.0]);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        assert_eq!(start_of(&doc, a), [0.0; 3]);
        assert_eq!(start_of(&doc, b), [10.0, 5.0, 0.0]);
        processor.submit(&mut doc, "undo");
        assert_eq!(start_of(&doc, b), [0.0, 5.0, 0.0]);
    }

    #[test]
    fn flip_reverses_them_all() {
        let (mut doc, a, b) = two_lines();
        run(&mut doc, &["dir", "f", ""]);
        assert_eq!(start_of(&doc, a), [10.0, 0.0, 0.0]);
        assert_eq!(start_of(&doc, b), [10.0, 5.0, 0.0]);
    }
}
