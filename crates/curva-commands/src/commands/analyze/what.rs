//! The `What` command.

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Curve;

/// `What`: what the chosen objects are, with their sizes.
#[derive(Default)]
pub struct What;

impl Recipe for What {
    const NAME: &'static str = "what";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.what.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let unit = cx.doc.state().unit;
        let described: Vec<String> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .map(|o| match &o.shape {
                Shape::Annotation(curva_document::annotation::Annotation::Text(text)) => {
                    format!("text \"{}\"", text.content.replace('\n', " "))
                }
                Shape::Instance(i) => {
                    let name = cx
                        .doc
                        .definitions()
                        .iter()
                        .find(|d| d.id == i.definition)
                        .map_or("?", |d| d.name.as_str());
                    format!("block instance of {name}")
                }
                Shape::Curve(curve) => {
                    let kind = match curve {
                        Curve::Point { at } => {
                            return format!("point at {:.3}, {:.3}, {:.3}", at[0], at[1], at[2]);
                        }
                        Curve::Line { .. } => "line",
                        Curve::Polyline { closed: true, .. } => "closed polyline",
                        Curve::Polyline { .. } => "polyline",
                        Curve::Circle { .. } => "circle",
                        Curve::Arc { .. } => "arc",
                        Curve::Nurbs { .. } => "control-point curve",
                        Curve::Compound { .. } => "joined curve",
                        Curve::Interpolated { .. } => "interpolated curve",
                    };
                    let length = curve
                        .length()
                        .map(|l| format!(", {} long", cx.doc.length_text(l)))
                        .unwrap_or_default();
                    format!("{kind}{length}")
                }
                Shape::Body(body) if body.is_solid() => {
                    let volume = body
                        .volume()
                        .map(|v| {
                            format!(", volume {}", curva_document::units::format_volume(v, unit))
                        })
                        .unwrap_or_default();
                    format!("solid{volume}")
                }
                Shape::Body(body) => {
                    let area = body
                        .area()
                        .map(|a| format!(", area {}", curva_document::units::format_area(a, unit)))
                        .unwrap_or_default();
                    format!("surface{area}")
                }
            })
            .collect();
        Ok(Made {
            message: Some(Message::with("done.what", vec![described.join("; ")])),
            ..Made::default()
        })
    }
}
