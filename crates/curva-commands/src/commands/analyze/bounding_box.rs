//! `BoundingBox`, as in Rhino: reports the box around objects of every kind (its lowest and highest
//! corners and its size) and draws it: a solid, or a flat rectangle around flat objects (#688).

use crate::commands::{ask_objects, geometry_failed, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct BoundingBox;

impl Command for BoundingBox {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.boundingbox.objects");
        };
        let boxes: Vec<_> = doc
            .state()
            .objects
            .iter()
            .filter(|o| chosen.contains(&o.id))
            .filter_map(|o| o.shape.bounds())
            .collect();
        let Some((low, high)) = boxes.into_iter().reduce(|(lo, hi), (l, h)| {
            (
                std::array::from_fn(|i| lo[i].min(l[i])),
                std::array::from_fn(|i| hi[i].max(h[i])),
            )
        }) else {
            return Step::Failed(Message::new("error.nothing_to_measure"));
        };
        let size: [f64; 3] = std::array::from_fn(|i| high[i] - low[i]);
        // The box drawn: a solid, or a rectangle around flat objects.
        let flat = (0..3).filter(|i| size[*i] <= 1e-9).collect::<Vec<_>>();
        let shape = match flat[..] {
            [] => curva_kernel::Body::cuboid(low, size),
            [axis] => {
                let corner = |a: f64, b: f64| -> curva_kernel::Point {
                    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                    let mut p = low;
                    p[u] += a * size[u];
                    p[v] += b * size[v];
                    p
                };
                curva_kernel::Curve::polyline(
                    vec![
                        corner(0.0, 0.0),
                        corner(1.0, 0.0),
                        corner(1.0, 1.0),
                        corner(0.0, 1.0),
                    ],
                    true,
                )
                .and_then(|ring| curva_kernel::Body::planar(&[ring]))
            }
            _ => Err(curva_kernel::KernelError("a line has no box".to_owned())),
        };
        match shape {
            Ok(body) => {
                doc.add_body(body);
            }
            Err(error) if flat.len() < 2 => return geometry_failed(&error),
            Err(_) => {}
        }
        let three = |p: [f64; 3]| p.map(|v| format!("{v:.3}")).join(",");
        let side = |i: usize| format!("{:.3}", size[i]);
        Step::Done(Some(Message::with(
            "done.boundingbox",
            vec![side(0), side(1), side(2), three(low), three(high)],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_reports_the_size_and_draws_the_box_as_in_rhino() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([1.0, 2.0, 0.0], [4.0, 2.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([1.0, 2.0, 0.0], [1.0, 7.0, 2.0]).unwrap());
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["boundingbox"]);
        assert!(processor.log()[0].contains("3.000 x 5.000 x 2.000"));
        // The box around them is drawn, as Rhino draws it.
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn a_solid_is_measured_and_boxed() {
        let mut doc = Document::default();
        let id = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[id]);
        let processor = run(&mut doc, &["boundingbox"]);
        assert!(
            processor.log()[0].contains("10.000 x 10.000 x 10.000"),
            "{:?}",
            processor.log()
        );
        let made = doc.state().objects.last().unwrap().shape.body().unwrap();
        assert!((made.volume().unwrap() - 1000.0).abs() < 1e-6);
    }
}
