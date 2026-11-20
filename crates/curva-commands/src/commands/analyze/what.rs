//! The `What` command.

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_document::annotation::Annotation;
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
        let described: Vec<String> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .map(|o| described(o, cx.doc))
            .collect();
        Ok(Made {
            message: Some(Message::with("done.what", vec![described.join("; ")])),
            ..Made::default()
        })
    }
}

/// One object, as `What` describes it.
fn described(o: &curva_document::Object, doc: &curva_document::Document) -> String {
    match &o.shape {
        Shape::Annotation(a) => annotation_described(a, doc),
        Shape::Curve(curve) => curve_described(curve, doc),
        Shape::Body(body) => body_described(body, doc.state().unit),
        Shape::Instance(i) => {
            let name = doc
                .definitions()
                .iter()
                .find(|d| d.id == i.definition)
                .map_or("?", |d| d.name.as_str());
            format!("block instance of {name}")
        }
        Shape::Mesh(mesh) => format!(
            "{}mesh of {} points and {} faces",
            if mesh.is_closed() { "closed " } else { "" },
            mesh.vertices().len(),
            mesh.faces().count()
        ),
        Shape::SubD(subd) => format!(
            "SubD of {} cage points and {} faces",
            subd.cage().vertices().len(),
            subd.cage().faces().count()
        ),
    }
}

/// An annotation: its kind and what it says or measures.
fn annotation_described(a: &Annotation, doc: &curva_document::Document) -> String {
    match a {
        Annotation::Text(text) => {
            format!("text \"{}\"", text.content.replace('\n', " "))
        }
        Annotation::Dimension(d) => {
            format!("dimension of {}", doc.length_text(d.value()))
        }
        Annotation::Radial(r) => {
            let kind = if r.diameter { "diameter" } else { "radius" };
            format!("{kind} dimension of {}", doc.length_text(r.value()))
        }
        Annotation::Angle(a) => {
            format!("angle dimension of {:.2}°", a.value())
        }
        Annotation::Ordinate(o) => {
            let axis = match o.axis {
                curva_document::annotation::OrdinateAxis::X => "X",
                curva_document::annotation::OrdinateAxis::Y => "Y",
            };
            format!(
                "ordinate dimension of {} in {axis}",
                doc.length_text(o.value())
            )
        }
        Annotation::ArcLength(a) => {
            format!("arc length dimension of {}", doc.length_text(a.value()))
        }
        Annotation::Leader(l) => {
            format!("leader \"{}\"", l.content.replace('\n', " "))
        }
        Annotation::Dot(d) => {
            format!("dot \"{}\"", d.content.replace('\n', " "))
        }
        Annotation::Hatch(h) => {
            format!("hatch of {} at scale {}", h.pattern, h.scale)
        }
        Annotation::ScaleBar(_) => "scale bar".to_owned(),
        Annotation::Table(t) => match t.kind {
            curva_document::sheet::TableKind::SheetIndex => "sheet index".to_owned(),
            curva_document::sheet::TableKind::Revisions(_) => "revision table".to_owned(),
        },
    }
}

/// A curve: its kind and length, or a point and where it is.
fn curve_described(curve: &Curve, doc: &curva_document::Document) -> String {
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
        .map(|l| format!(", {} long", doc.length_text(l)))
        .unwrap_or_default();
    format!("{kind}{length}")
}

/// A surface with its area, or a solid with its volume.
fn body_described(body: &curva_kernel::Body, unit: curva_document::Unit) -> String {
    if body.is_solid() {
        let volume = body
            .volume()
            .map(|v| format!(", volume {}", curva_document::units::format_volume(v, unit)))
            .unwrap_or_default();
        format!("solid{volume}")
    } else {
        let area = body
            .area()
            .map(|a| format!(", area {}", curva_document::units::format_area(a, unit)))
            .unwrap_or_default();
        format!("surface{area}")
    }
}
