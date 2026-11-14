//! `Detail`: adds and manages detail views on a layout, as in Rhino 8 (spec `layouts.md` §2;
//! `rhino-reference.md`). A detail is a rectangle on the sheet that shows the model live.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::layouts::{Detail as DetailView, parse_scale};
use curva_kernel::{Curve, Point};

const PROJECTION: &str = "Projection";
const SCALE: &str = "Scale";
const LOCK: &str = "Lock";
const UNLOCK: &str = "Unlock";

/// The views a detail can look from, in the order `Projection` steps through them.
const PROJECTIONS: [&str; 4] = ["Top", "Front", "Right", "Perspective"];

/// Scales a new detail picks from to fit the model: 1:1, 1:2, 1:5, 1:10 and so on, and the same
/// enlarging.
fn standard_scales() -> Vec<f64> {
    let mut scales = Vec::new();
    for power in -4..=3 {
        for step in [1.0, 2.0, 5.0] {
            scales.push(step * 10f64.powi(power));
        }
    }
    scales.extend([25.0, 250.0].map(|n| 1.0 / n));
    scales.sort_by(f64::total_cmp);
    scales
}

/// Where the command is.
#[derive(Default)]
enum Stage {
    #[default]
    Corner,
    OtherCorner(Point),
    /// Lock, Unlock or Scale: which details.
    Details(&'static str),
    /// Scale: the scale for these details.
    ScaleOf(Vec<u64>),
}

/// The running command.
#[derive(Default)]
pub struct Detail {
    stage: Stage,
    projection: usize,
}

impl Detail {
    fn ask_corner(&self) -> Step {
        Step::Ask(Request {
            options: vec![PROJECTION, SCALE, LOCK, UNLOCK],
            option_labels: vec![
                format!("{PROJECTION}={}", PROJECTIONS[self.projection]),
                SCALE.to_owned(),
                LOCK.to_owned(),
                UNLOCK.to_owned(),
            ],
            ..Request::new("prompt.detail.corner", Kind::Point)
        })
    }

    /// The new detail: its frame on the sheet, and a view of the model's extents at a standard
    /// scale that fits them.
    fn make(&self, doc: &mut Document, layout: u64, a: Point, b: Point) -> Step {
        let low = [a[0].min(b[0]), a[1].min(b[1]), 0.0];
        let high = [a[0].max(b[0]), a[1].max(b[1]), 0.0];
        let corners = vec![low, [high[0], low[1], 0.0], high, [low[0], high[1], 0.0]];
        let Ok(rectangle) = Curve::polyline(corners, true) else {
            return Step::Failed(Message::new("error.detail_corners"));
        };
        let projection = PROJECTIONS[self.projection];
        let (centre, extent) = model_extent(doc, projection);
        let paper_mm = doc.layout(layout).map_or(1.0, |l| l.unit.millimetres());
        let model_mm = doc.state().unit.millimetres();
        let frame = [(high[0] - low[0]) * paper_mm, (high[1] - low[1]) * paper_mm];
        let scale = (projection != "Perspective").then(|| {
            // The largest standard scale at which the model, with a margin, fits the frame.
            let wanted = extent.map_or(1.0, |[w, h]| {
                (frame[0] / (w * model_mm * 1.1)).min(frame[1] / (h * model_mm * 1.1))
            });
            standard_scales()
                .into_iter()
                .rfind(|s| *s <= wanted)
                .unwrap_or(1e-4)
        });
        let distance = extent.map_or(100.0, |[w, h]| 2.0 * w.max(h));
        let frame_id = doc.add(rectangle);
        doc.add_detail(
            layout,
            DetailView {
                frame: frame_id,
                projection: projection.to_owned(),
                target: centre,
                distance,
                scale,
                display: if projection == "Perspective" {
                    "Shaded".to_owned()
                } else {
                    "Wireframe".to_owned()
                },
                locked: false,
                hidden_layers: Vec::new(),
            },
        );
        Step::Done(Some(match scale {
            Some(scale) => Message::with(
                "done.detail",
                vec![curva_document::layouts::scale_text(scale)],
            ),
            None => Message::new("done.detail_perspective"),
        }))
    }
}

/// The centre of the model's objects, and their width and height as seen from `projection`;
/// no extent for an empty model.
fn model_extent(doc: &Document, projection: &str) -> (Point, Option<[f64; 2]>) {
    let boxes: Vec<(Point, Point)> = doc
        .state()
        .objects
        .iter()
        .filter(|o| o.space.is_none() && !o.hidden)
        .filter_map(|o| o.shape.bounds())
        .collect();
    let Some(first) = boxes.first().copied() else {
        return ([0.0; 3], None);
    };
    let (low, high) = boxes.iter().fold(first, |(lo, hi), (l, h)| {
        (
            std::array::from_fn(|i| lo[i].min(l[i])),
            std::array::from_fn(|i| hi[i].max(h[i])),
        )
    });
    let size: Point = std::array::from_fn(|i| (high[i] - low[i]).max(1e-6));
    let seen = match projection {
        "Front" => [size[0], size[2]],
        "Right" => [size[1], size[2]],
        "Perspective" => [
            size[0].max(size[1]),
            size[2].max(size[0].max(size[1]) * 0.6),
        ],
        _ => [size[0], size[1]],
    };
    (
        std::array::from_fn(|i| (low[i] + high[i]) / 2.0),
        Some(seen),
    )
}

/// The frames of details among `ids`.
fn details_among(doc: &Document, ids: &[u64]) -> Vec<u64> {
    ids.iter()
        .copied()
        .filter(|id| doc.detail_of(*id).is_some())
        .collect()
}

impl Command for Detail {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(layout) = doc.space() else {
            return Step::Failed(Message::new("error.detail_no_layout"));
        };
        match (std::mem::take(&mut self.stage), answer) {
            (Stage::Corner, None) => self.ask_corner(),
            (Stage::Corner, Some(Answer::Option(PROJECTION))) => {
                self.projection = (self.projection + 1) % PROJECTIONS.len();
                self.ask_corner()
            }
            (Stage::Corner, Some(Answer::Option(word @ (SCALE | LOCK | UNLOCK)))) => {
                self.stage = Stage::Details(word);
                Step::Ask(Request::new("prompt.detail.which", Kind::Objects))
            }
            (Stage::Corner, Some(Answer::Point(a))) => {
                self.stage = Stage::OtherCorner(a);
                Step::Ask(Request {
                    from: Some(a),
                    corner: true,
                    preview: vec![a],
                    ..Request::new("prompt.detail.other_corner", Kind::Point)
                })
            }
            (Stage::OtherCorner(a), Some(Answer::Point(b))) => {
                if (a[0] - b[0]).abs() < 1e-9 || (a[1] - b[1]).abs() < 1e-9 {
                    return Step::Failed(Message::new("error.detail_corners"));
                }
                self.make(doc, layout, a, b)
            }
            (Stage::Details(word), Some(Answer::Objects(ids))) => {
                let frames = details_among(doc, &ids);
                if frames.is_empty() {
                    return Step::Failed(Message::new("error.detail_none_picked"));
                }
                match word {
                    SCALE => {
                        self.stage = Stage::ScaleOf(frames);
                        Step::Ask(Request::new("prompt.detail.scale", Kind::Path))
                    }
                    _ => {
                        let locked = word == LOCK;
                        for frame in &frames {
                            doc.change_detail(*frame, |d| d.locked = locked);
                        }
                        Step::Done(Some(Message::with(
                            if locked {
                                "done.detail_locked"
                            } else {
                                "done.detail_unlocked"
                            },
                            vec![frames.len().to_string()],
                        )))
                    }
                }
            }
            (Stage::ScaleOf(frames), Some(Answer::Path(text))) => {
                let Some(scale) = parse_scale(&text) else {
                    self.stage = Stage::ScaleOf(frames);
                    return Step::Failed(Message::with("error.detail_scale", vec![text]));
                };
                for frame in &frames {
                    doc.change_detail(*frame, |d| {
                        if d.projection != "Perspective" {
                            d.scale = Some(scale);
                        }
                    });
                }
                Step::Done(Some(Message::with(
                    "done.detail_scaled",
                    vec![curva_document::layouts::scale_text(scale)],
                )))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    /// A model with a 10 m wide box of lines, and an A3 layout open.
    fn model_and_sheet() -> Document {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [10_000.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 5_000.0, 0.0]).unwrap());
        run(&mut doc, &["Layout", ""]);
        doc
    }

    #[test]
    fn a_detail_frames_the_model_at_a_standard_scale_that_fits() {
        let mut doc = model_and_sheet();
        let sheet = doc.space().unwrap();
        run(&mut doc, &["Detail", "20,20", "220,170"]);
        let details = doc.details_on(sheet);
        assert_eq!(details.len(), 1);
        let (detail, corners) = &details[0];
        assert_eq!(*corners, [[20.0, 20.0], [220.0, 170.0]]);
        // 10 m and a margin in 200 mm fits at about 1:55, so 1:100 is the largest standard
        // scale that fits.
        assert_eq!(
            detail
                .scale
                .map(curva_document::layouts::scale_text)
                .as_deref(),
            Some("1:100")
        );
        assert_eq!(detail.target, [5_000.0, 2_500.0, 0.0]);
        // The frame is on the sheet, not in the model.
        assert_eq!(doc.state().objects.last().unwrap().space, Some(sheet));
    }

    #[test]
    fn details_are_scaled_locked_and_unlocked() {
        let mut doc = model_and_sheet();
        run(&mut doc, &["Detail", "20,20", "220,170"]);
        let frame = doc.state().objects.last().unwrap().id;
        doc.select(&[frame]);
        run(&mut doc, &["Detail", "Scale", "", "1:50"]);
        assert_eq!(doc.detail_of(frame).unwrap().1.scale, Some(0.02));
        doc.select(&[frame]);
        run(&mut doc, &["Detail", "Lock", ""]);
        assert!(doc.detail_of(frame).unwrap().1.locked);
        doc.select(&[frame]);
        run(&mut doc, &["Detail", "Unlock", ""]);
        assert!(!doc.detail_of(frame).unwrap().1.locked);
    }

    #[test]
    fn a_detail_needs_a_layout_open() {
        let mut doc = Document::default();
        run(&mut doc, &["Detail", "0,0", "10,10"]);
        assert!(doc.layouts().is_empty());
        assert!(doc.state().objects.is_empty());
    }
}
