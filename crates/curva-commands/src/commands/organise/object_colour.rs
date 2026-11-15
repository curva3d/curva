//! The `ObjectColour` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// A colour as typed: `ByLayer` (`None`), red, green and blue from 0 to 255 (`255,0,0`), or
/// `#rrggbb`. `Err` for anything else.
fn colour_typed(text: &str) -> Result<Option<[f32; 3]>, ()> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("bylayer") {
        return Ok(None);
    }
    let bytes: Vec<u8> = if let Some(hex) = text.strip_prefix('#') {
        if hex.len() != 6 || !hex.is_ascii() {
            return Err(());
        }
        (0..3)
            .map(|k| u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).map_err(|_| ()))
            .collect::<Result<_, _>>()?
    } else {
        text.split(',')
            .map(|part| part.trim().parse::<u8>().map_err(|_| ()))
            .collect::<Result<_, _>>()?
    };
    match bytes.as_slice() {
        [r, g, b] => Ok(Some([*r, *g, *b].map(|c| f32::from(c) / 255.0))),
        _ => Err(()),
    }
}

/// `ObjectColour`: gives objects a colour of their own (`255,0,0` or `#ff0000`), or `ByLayer`
/// to show in their layer's again, as Rhino's Properties do.
#[derive(Default)]
pub struct ObjectColour {
    objects: Vec<u64>,
}

impl Command for ObjectColour {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) if !self.objects.is_empty() => {
                let Ok(colour) = colour_typed(&text) else {
                    return Step::Failed(Message::with("error.object_colour", vec![text]));
                };
                doc.set_colour(&self.objects, colour);
                Step::Done(Some(Message::with(
                    "done.object_colour",
                    vec![self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.object_colour.objects");
                }
                Step::Ask(Request::new("prompt.object_colour.value", Kind::Path))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::colour_typed;
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    #[test]
    fn colours_are_typed_as_numbers_or_hex_or_by_layer() {
        assert_eq!(colour_typed("255, 0, 0"), Ok(Some([1.0, 0.0, 0.0])));
        assert_eq!(colour_typed("#00ff00"), Ok(Some([0.0, 1.0, 0.0])));
        assert_eq!(colour_typed("ByLayer"), Ok(None));
        assert!(colour_typed("red").is_err());
        assert!(colour_typed("1,2").is_err());
        assert!(colour_typed("#12345").is_err());
    }

    #[test]
    fn objects_take_a_colour_of_their_own_and_give_it_back() {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::Curve(
            Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap(),
        ));
        doc.select(&[id]);
        run(&mut doc, &["ObjectColour", "#0000ff"]);
        let state = doc.state();
        assert_eq!(state.objects[0].colour, Some([0.0, 0.0, 1.0]));
        assert_eq!(state.object_colour(&state.objects[0]), [0.0, 0.0, 1.0]);
        doc.select(&[id]);
        run(&mut doc, &["ObjectColour", "ByLayer"]);
        assert_eq!(doc.state().objects[0].colour, None);
    }
}
