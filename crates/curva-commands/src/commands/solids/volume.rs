//! `Volume`: measures closed solids, each and in total, as in Rhino.

use crate::commands::{ask_objects, bodies_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Volume;

/// `value` to six significant figures, without trailing zeros.
fn figures(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let decimals = (5 - value.abs().log10().floor() as i32).max(0) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

impl Command for Volume {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let bodies = bodies_of(doc, &chosen);
        if bodies.is_empty() {
            return ask_objects("prompt.volume.objects");
        }
        if bodies.iter().any(|(_, body)| !body.is_solid()) {
            return Step::Failed(Message::new("error.volume_needs_solids"));
        }
        let mut each = Vec::new();
        for (_, body) in &bodies {
            match body.volume() {
                Ok(volume) => each.push(volume),
                Err(error) => return geometry_failed(&error),
            }
        }
        let total: f64 = each.iter().sum();
        let message = if each.len() == 1 {
            Message::with("done.volume", vec![figures(total)])
        } else {
            let listed: Vec<String> = each.iter().map(|v| figures(*v)).collect();
            Message::with("done.volumes", vec![listed.join(", "), figures(total)])
        };
        Step::Done(Some(message))
    }
}

#[cfg(test)]
mod tests {
    use super::figures;
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn it_measures_a_cube_and_a_cylinder_and_adds_them() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        let ring = Curve::circle([30.0, 0.0, 0.0], 5.0).unwrap();
        let cylinder = doc.add_body(Body::extruded(&[ring], [0.0, 0.0, 1.0], 10.0, true).unwrap());
        doc.select(&[cube]);
        let log = run(&mut doc, &["volume"]).log().to_vec();
        assert!(log.iter().any(|line| line.contains("1000 ")), "{log:?}");
        doc.select(&[cube, cylinder]);
        let log = run(&mut doc, &["volume"]).log().to_vec();
        assert!(
            log.iter()
                .any(|line| line.contains("785.398") && line.contains("1785.4 ")),
            "{log:?}"
        );
    }

    #[test]
    fn an_open_surface_is_refused() {
        let mut doc = Document::default();
        let square = Curve::polyline(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            true,
        )
        .unwrap();
        let id = doc.add_body(Body::planar(&[square]).unwrap());
        doc.select(&[id]);
        let log = run(&mut doc, &["volume"]).log().to_vec();
        assert!(log.iter().any(|line| line.contains("closed")), "{log:?}");
    }

    #[test]
    fn six_significant_figures() {
        assert_eq!(figures(785.398_163), "785.398");
        assert_eq!(figures(1000.0), "1000");
        assert_eq!(figures(0.001_234_567), "0.00123457");
    }
}
