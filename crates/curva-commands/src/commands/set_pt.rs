//! `SetPt`: lines objects up on one, two or all three coordinates of a point.

use super::project_to_cplane::flattened_onto;
use super::{ask_objects, curves_of, geometry_failed, ghost_results, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Grip};
use curva_kernel::{Curve, KernelError, Point, cross, dot, sub, unit};

const SET_X: &str = "SetX";
const SET_Y: &str = "SetY";
const SET_Z: &str = "SetZ";
const AXIS_NAMES: [&str; 3] = ["X", "Y", "Z"];

/// The running command.
pub struct SetPt {
    ids: Option<Vec<u64>>,
    /// Picked control points, which are lined up instead of whole objects, as in Rhino.
    grips: Vec<Grip>,
    /// Which coordinates are set, along the plane's X, Y and normal.
    setting: [bool; 3],
}

impl Default for SetPt {
    fn default() -> Self {
        Self {
            ids: None,
            grips: Vec::new(),
            setting: [true; 3],
        }
    }
}

impl SetPt {
    fn ask(&self) -> Step {
        let named: Vec<&str> = AXIS_NAMES
            .iter()
            .zip(self.setting)
            .filter(|(_, on)| *on)
            .map(|(name, _)| *name)
            .collect();
        Step::Ask(Request {
            prompt: Message::with("prompt.setpt.point", vec![named.join(", ")]),
            options: vec![SET_X, SET_Y, SET_Z],
            ..Request::new("prompt.setpt.point", Kind::Point)
        })
    }

    fn set(
        &self,
        doc: &Document,
        ids: &[u64],
        target: Point,
    ) -> Result<Vec<(u64, Curve)>, KernelError> {
        let (_, [x, y]) = doc.work_frame();
        let normal = unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0]);
        let axes = [x, y, normal];
        if !self.grips.is_empty() {
            let setting = self.setting;
            return doc.grips_changed(&self.grips, |p| {
                let mut moved = p;
                for (axis, on) in axes.iter().zip(setting) {
                    if on {
                        let by = dot(sub(target, p), *axis);
                        moved = [0, 1, 2].map(|k| moved[k] + axis[k] * by);
                    }
                }
                moved
            });
        }
        curves_of(doc, ids)
            .into_iter()
            .map(|(id, mut curve)| {
                for (axis, on) in axes.iter().zip(self.setting) {
                    if on {
                        curve = flattened_onto(&curve, target, *axis)?;
                    }
                }
                Ok((id, curve))
            })
            .collect()
    }
}

impl Command for SetPt {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let grips = doc.grip_selection();
            if answer.is_none() && !grips.is_empty() {
                self.ids = Some(grips.iter().map(|(id, _)| *id).collect());
                self.grips = grips;
                return self.ask();
            }
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.setpt.objects");
            }
            self.ids = Some(chosen);
            return self.ask();
        };
        match answer {
            Some(Answer::Option(word)) => {
                let index = [SET_X, SET_Y, SET_Z].iter().position(|w| *w == word);
                if let Some(index) = index {
                    let mut next = self.setting;
                    next[index] = !next[index];
                    // At least one coordinate stays set, or the command would do nothing.
                    if next.contains(&true) {
                        self.setting = next;
                    }
                }
                self.ask()
            }
            Some(Answer::Point(target)) => match self.set(doc, &ids, target) {
                Ok(made) => {
                    for (id, curve) in made {
                        doc.replace(id, curve);
                    }
                    Step::Done(Some(Message::with(
                        "done.setpt",
                        vec![ids.len().to_string()],
                    )))
                }
                Err(error) => geometry_failed(&error),
            },
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(cursor)) = (&self.ids, live.cursor) else {
            return Vec::new();
        };
        self.set(doc, ids, cursor)
            .map(ghost_results)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0, 0.0, 0.0], [2.0, 3.0, 1.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn setting_only_x_lines_the_points_up_on_that_x() {
        let mut doc = selected_line();
        run(&mut doc, &["setpt", "SetY", "SetZ", "5,9,9"]);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap(),
            [[5.0, 0.0, 0.0], [5.0, 3.0, 1.0]]
        );
    }

    #[test]
    fn the_last_coordinate_cannot_be_turned_off() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["setpt", "SetX", "SetY", "SetZ"]);
        assert!(processor.prompt().unwrap().contains('Z'));
    }

    #[test]
    fn picked_control_points_are_lined_up_instead_of_the_object() {
        let mut doc = Document::default();
        let id = doc
            .add(Curve::polyline(vec![[0.0; 3], [2.0, 1.0, 0.0], [4.0, 3.0, 0.0]], false).unwrap());
        doc.points_on(&[id]);
        doc.select_grips(&[(id, 1), (id, 2)]);
        run(&mut doc, &["setpt", "SetX", "SetZ", "9,0,0"]);
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap(),
            [[0.0; 3], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn setting_z_moves_a_flat_curve_without_rebuilding_it() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["setpt", "SetX", "SetY", "0,0,4"]);
        assert_eq!(
            *doc.state().objects[0].curve().unwrap(),
            Curve::circle([0.0, 0.0, 4.0], 1.0).unwrap()
        );
    }
}
