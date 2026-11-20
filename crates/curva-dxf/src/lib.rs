//! DXF and DWG files (spec `drawing-output.md` §3), through `acadrust`: the model's curves and
//! points written with their layers (colour, print width, on and locked) and unit, and curves,
//! points and layers read back into a model. Written as AutoCAD 2000 (AC1015) by default, as
//! Rhino's default export scheme is.
//!
//! Circles, arcs and planar polylines lie in an "object coordinate system" fixed by their plane's
//! normal through AutoCAD's arbitrary axis algorithm (AutoCAD DXF Reference, "Arbitrary Axis
//! Algorithm"), so they are turned into and out of it here.

mod annotation;
mod blocks;
mod dims;
mod export;
pub mod format;
mod import;
mod layouts;
mod ocs;
mod units;

pub use export::export;
pub(crate) use export::*;
pub use format::Format;
pub(crate) use import::*;
pub use import::{import, read};
pub(crate) use ocs::*;
pub(crate) use units::*;

use acadrust::entities::{
    Arc, Circle, Ellipse, EntityType, Line, LwPolyline, Point, Polyline3D, Spline,
};
use acadrust::tables::{DimStyle, LineType, LineTypeElement, TextStyle};
use acadrust::types::{Color, DxfVersion, LineWeight, Vector2, Vector3};
use acadrust::xdata::XDataValue;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Layer};
use curva_document::annotation::Annotation;
use curva_document::fields::{FieldContext, resolve};
use curva_document::{Document, Linetype, PrintWidth, Shape, Unit};
use curva_kernel::{Curve, Spacing};
use std::collections::BTreeMap;
use std::io::Cursor;

pub(crate) type P = curva_kernel::Point;

/// The application name Rhino keeps objects' user text under in DXF and DWG.
pub(crate) const RHINO_APP: &str = "Rhino";

/// User text from Rhino's XDATA: each key and value is a pair of strings in braces.
pub(crate) fn user_text_in(values: &[XDataValue]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut open: Option<Vec<String>> = None;
    for value in values {
        match value {
            XDataValue::ControlString(c) if c == "{" => open = Some(Vec::new()),
            XDataValue::ControlString(c) if c == "}" => {
                if let Some(mut pair) = open.take().filter(|p| p.len() == 2) {
                    let value = pair.pop().unwrap_or_default();
                    let key = pair.pop().unwrap_or_default();
                    if !key.trim().is_empty() {
                        out.push((key, value));
                    }
                }
            }
            XDataValue::String(s) => {
                if let Some(pair) = &mut open {
                    pair.push(s.clone());
                }
            }
            _ => {}
        }
    }
    out
}

/// What a read or write brought across and what it left out, for the command bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Objects brought across, by kind.
    pub brought: Vec<(String, usize)>,
    /// Things left out, by kind.
    pub skipped: Vec<(String, usize)>,
}

impl Report {
    fn count(list: &mut Vec<(String, usize)>, kind: &str) {
        match list.iter_mut().find(|(k, _)| k == kind) {
            Some((_, n)) => *n += 1,
            None => list.push((kind.to_owned(), 1)),
        }
    }

    /// The report as one line: what came across, then what was left out.
    #[must_use]
    pub fn summary(&self) -> String {
        let list = |items: &[(String, usize)]| {
            items
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut out = if self.brought.is_empty() {
            "nothing".to_owned()
        } else {
            list(&self.brought)
        };
        if !self.skipped.is_empty() {
            out.push_str("; left out ");
            out.push_str(&list(&self.skipped));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Document {
        let mut doc = Document::default();
        let walls = doc.add_layer("Walls".to_owned());
        doc.set_layer_colour(walls, [1.0, 0.0, 0.0]);
        doc.set_layer_print_width(walls, Some(0.5));
        let doors = doc.add_sublayer(walls, "Doors".to_owned());
        let on = |doc: &mut Document, curve: Curve, layer| {
            doc.add_shape_on(Shape::Curve(curve), Some(layer));
        };
        on(
            &mut doc,
            Curve::line([0.0; 3], [5000.0, 0.0, 0.0]).unwrap(),
            walls,
        );
        on(
            &mut doc,
            Curve::polyline(
                vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]],
                true,
            )
            .unwrap(),
            walls,
        );
        on(
            &mut doc,
            Curve::polyline(vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]], false).unwrap(),
            walls,
        );
        on(
            &mut doc,
            Curve::circle([1.0, 2.0, 0.0], 3.0).unwrap(),
            doors,
        );
        // A quarter arc in the front (XZ) plane, so it needs object coordinates.
        on(
            &mut doc,
            Curve::Arc {
                centre: [5.0, 0.0, 5.0],
                radius: 2.0,
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                sweep: std::f64::consts::FRAC_PI_2,
            },
            doors,
        );
        on(
            &mut doc,
            Curve::nurbs(
                2,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                vec![[0.0; 3], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
                vec![1.0, 0.5, 1.0],
            )
            .unwrap(),
            doors,
        );
        on(
            &mut doc,
            Curve::Point {
                at: [7.0, 8.0, 9.0],
            },
            walls,
        );
        doc
    }

    /// The ends of every curve in the model, in order, for comparing two models.
    fn ends(doc: &Document) -> Vec<(P, P)> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| o.curve())
            .map(|c| {
                let s = c.sample().unwrap();
                (s[0], s[s.len() - 1])
            })
            .collect()
    }

    /// Whether two models hold curves with the same ends, in any order (a DWG keeps 3D
    /// polylines after the rest).
    fn close(a: &[(P, P)], b: &[(P, P)]) -> bool {
        let near = |x: P, y: P| (0..3).all(|i| (x[i] - y[i]).abs() < 1e-6);
        a.len() == b.len()
            && a.iter()
                .all(|x| b.iter().any(|y| near(x.0, y.0) && near(x.1, y.1)))
    }

    #[test]
    fn curves_and_layers_go_out_and_come_back_as_dxf_and_dwg() {
        let doc = model();
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            assert_eq!(report.brought, vec![("curves".to_owned(), 7)]);
            let cad = read(&bytes, format).unwrap();
            assert_eq!(cad.header.insertion_units, 4, "millimetres");
            let red = cad.layers.get("Walls").expect("the layer");
            assert_eq!(
                red.color.rgb(),
                Some((255, 0, 0)),
                "red, by index or true colour"
            );
            assert_eq!(red.line_weight, LineWeight::Value(50));
            assert!(
                cad.layers.get("Walls$Doors").is_some(),
                "sublayers as Rhino names them"
            );
            let mut back = Document::default();
            let report = import(&cad, &mut back);
            assert_eq!(report.brought, vec![("curves".to_owned(), 7)], "{format:?}");
            assert!(
                close(&ends(&doc), &ends(&back)),
                "{format:?}: the same curves"
            );
            let doors = back
                .state()
                .layers
                .iter()
                .find(|l| l.name == "Doors")
                .unwrap();
            assert_eq!(
                back.state().layer(doors.parent.unwrap()).unwrap().name,
                "Walls"
            );
        }
    }

    #[test]
    fn arcs_off_the_plan_keep_their_plane() {
        let doc = model();
        let (bytes, _) = export(&doc, None, Format::Dxf).unwrap();
        let mut back = Document::default();
        import(&read(&bytes, Format::Dxf).unwrap(), &mut back);
        let arc = back
            .state()
            .objects
            .iter()
            .find_map(|o| match o.curve() {
                Some(Curve::Arc {
                    centre,
                    x,
                    y,
                    sweep,
                    ..
                }) => Some((*centre, *x, *y, *sweep)),
                _ => None,
            })
            .unwrap();
        let near = |a: P, b: P| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
        assert!(near(arc.0, [5.0, 0.0, 5.0]));
        assert!(
            near(arc.1, [1.0, 0.0, 0.0]) && near(arc.2, [0.0, 0.0, 1.0]),
            "{arc:?}"
        );
        assert!((arc.3 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn ellipses_go_out_as_ellipses_and_come_back_exact() {
        let whole = Curve::ellipse([1.0, 2.0, 0.0], [0.0, 6.0, 0.0], [-2.0, 0.0, 0.0]).unwrap();
        let part =
            Curve::elliptical_arc([0.0; 3], [4.0, 0.0, 0.0], [0.0, 0.0, 1.5], 0.7, 2.0).unwrap();
        let mut doc = Document::default();
        doc.add_shape(Shape::Curve(whole.clone()));
        doc.add_shape(Shape::Curve(part.clone()));
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            let ellipses: Vec<&Ellipse> = cad
                .entities()
                .filter_map(|e| match e {
                    EntityType::Ellipse(e) => Some(e),
                    _ => None,
                })
                .collect();
            assert_eq!(ellipses.len(), 2, "{format:?}");
            assert!((ellipses[0].minor_axis_ratio - 2.0 / 6.0).abs() < 1e-9);
            let mut back = Document::default();
            let report = import(&cad, &mut back);
            assert_eq!(report.brought, vec![("curves".to_owned(), 2)], "{format:?}");
            for (sent, came) in [&whole, &part].into_iter().zip(&back.state().objects) {
                let (a, b) = (
                    sent.sample().unwrap(),
                    came.curve().unwrap().sample().unwrap(),
                );
                let near = |p: P, q: P| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-7);
                assert!(near(a[0], b[0]) && near(a[a.len() - 1], b[b.len() - 1]));
                let found = came
                    .curve()
                    .unwrap()
                    .as_ellipse()
                    .expect("still an ellipse");
                let length = |v: P| dot(v, v).sqrt();
                let sent = sent.as_ellipse().unwrap();
                assert!((length(found.major) - length(sent.major)).abs() < 1e-7);
                assert!((length(found.minor) - length(sent.minor)).abs() < 1e-7);
                assert!((found.sweep - sent.sweep).abs() < 1e-7, "{format:?}");
            }
        }
    }

    #[test]
    fn what_cannot_go_is_reported() {
        let mut doc = Document::default();
        doc.add_shape(Shape::Body(
            curva_kernel::Body::cuboid([0.0; 3], [1.0; 3]).unwrap(),
        ));
        let (_, report) = export(&doc, None, Format::Dxf).unwrap();
        assert_eq!(report.skipped, vec![("surfaces and solids".to_owned(), 1)]);
        assert!(report.summary().contains("left out 1 surfaces and solids"));
    }

    #[test]
    fn a_polyline_with_bulges_becomes_lines_and_arcs() {
        let mut polyline = LwPolyline::new();
        polyline.add_point_with_bulge(Vector2::new(0.0, 0.0), 0.0);
        polyline.add_point_with_bulge(Vector2::new(10.0, 0.0), 1.0);
        polyline.add_point(Vector2::new(10.0, 10.0));
        let mut cad = CadDocument::with_version(DxfVersion::AC1015);
        cad.add_entity(EntityType::LwPolyline(polyline)).unwrap();
        let mut doc = Document::default();
        import(&cad, &mut doc);
        let Some(Curve::Compound { segments }) = doc.state().objects[0].curve() else {
            panic!("not a compound");
        };
        // A bulge of 1 is a half circle, turning counter-clockwise from (10, 0) to (10, 10).
        let Curve::Arc {
            centre,
            radius,
            sweep,
            ..
        } = &segments[1]
        else {
            panic!("not an arc");
        };
        assert!((radius - 5.0).abs() < 1e-9 && (sweep - std::f64::consts::PI).abs() < 1e-9);
        assert!((centre[0] - 10.0).abs() < 1e-9 && (centre[1] - 5.0).abs() < 1e-9);
        let along = segments[1].sample().unwrap();
        let last = along[along.len() - 1];
        assert!((last[0] - 10.0).abs() < 1e-6 && (last[1] - 10.0).abs() < 1e-6);
        // Counter-clockwise from (10, 0) to (10, 10) about (10, 5) passes through (15, 5).
        assert!(along.iter().any(|p| (p[0] - 15.0).abs() < 1e-3));
    }

    #[test]
    fn linetypes_widths_and_layer_states_come_back_as_dxf_and_dwg() {
        let mut doc = Document::default();
        let hidden = doc.add_layer("Hidden lines".to_owned());
        doc.set_layer_linetype(hidden, Linetype::Hidden);
        doc.set_layer_print_width(hidden, Some(0.25));
        let axes = doc.add_layer("Axes".to_owned());
        doc.set_layer_linetype(axes, Linetype::Center);
        doc.set_layer_locked(axes, true);
        let off = doc.add_layer("Off".to_owned());
        doc.set_layer_visible(off, false);
        let line = |y: f64| Shape::Curve(Curve::line([0.0, y, 0.0], [100.0, y, 0.0]).unwrap());
        doc.add_shape_on(line(0.0), Some(hidden));
        let border = doc.add_shape_on(line(10.0), Some(axes));
        doc.set_linetype(&[border], Linetype::Border);
        doc.set_print_width(&[border], PrintWidth::Millimetres(0.7));
        let dots = doc.add_shape_on(line(20.0), Some(off));
        doc.set_linetype(&[dots], Linetype::Dotted);
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            let pattern: Vec<f64> = cad
                .line_types
                .get("Hidden")
                .expect("the linetype table holds it")
                .elements
                .iter()
                .map(|e| e.length)
                .collect();
            assert_eq!(pattern, [1.5, -1.0], "{format:?}: Curva's pattern");
            let mut back = Document::default();
            let report = import(&cad, &mut back);
            let state = back.state();
            let layer = |n: &str| state.layers.iter().find(|l| l.name == n).unwrap();
            assert_eq!(
                layer("Hidden lines").linetype,
                Linetype::Hidden,
                "{format:?}"
            );
            assert_eq!(layer("Hidden lines").print_width, Some(0.25));
            assert_eq!(layer("Axes").linetype, Linetype::Center);
            assert!(layer("Axes").locked);
            assert!(!layer("Off").visible);
            let kinds: Vec<(Linetype, PrintWidth)> = state
                .objects
                .iter()
                .map(|o| (o.linetype, o.print_width))
                .collect();
            assert_eq!(
                kinds,
                [
                    (Linetype::ByLayer, PrintWidth::ByLayer),
                    (Linetype::Border, PrintWidth::Millimetres(0.7)),
                    (Linetype::Dotted, PrintWidth::ByLayer),
                ],
                "{format:?}: {:?}",
                report.skipped
            );
        }
    }

    #[test]
    fn user_text_goes_out_as_rhinos_and_comes_back_as_dxf_and_dwg() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.set_user_text(&[line], "Room", "Kitchen");
        doc.set_user_text(&[line], "Fire rating", "{60} min");
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            assert!(cad.app_ids.get(RHINO_APP).is_some(), "{format:?}");
            let mut back = Document::default();
            import(&cad, &mut back);
            let state = back.state();
            let text = &state.objects[0].user_text;
            assert_eq!(
                text.get("Room").map(String::as_str),
                Some("Kitchen"),
                "{format:?}"
            );
            assert_eq!(
                text.get("Fire rating").map(String::as_str),
                Some("{60} min"),
                "{format:?}"
            );
        }
    }

    #[test]
    fn block_attributes_come_in_as_user_text_and_notes() {
        use acadrust::entities::AttributeEntity;
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.define_block("Door".to_owned(), [0.0; 3], &[line]);
        let (bytes, _) = export(&doc, None, Format::Dxf).unwrap();
        let mut cad = read(&bytes, Format::Dxf).unwrap();
        let mut tags = Layer::new("Tags");
        tags.handle = cad.allocate_handle();
        let _ = cad.layers.add(tags);
        let handles: Vec<_> = cad
            .entities()
            .filter_map(|e| match e {
                EntityType::Insert(i) => Some(i.common.handle),
                _ => None,
            })
            .collect();
        for h in handles {
            if let Some(EntityType::Insert(i)) = cad.get_entity_mut(h) {
                let mut width = AttributeEntity::new("WIDTH".to_owned(), "900".to_owned())
                    .with_height(2.5)
                    .with_position(Vector3::new(1.0, 2.0, 0.0));
                width.common.layer = "Tags".to_owned();
                let mut hidden = AttributeEntity::new("CODE".to_owned(), "D-01".to_owned());
                hidden.flags.invisible = true;
                i.attributes.push(width);
                i.attributes.push(hidden);
            }
        }
        for format in [Format::Dxf, Format::Dwg] {
            let bytes = match format {
                Format::Dxf => DxfWriter::new(&cad).write_to_vec(),
                Format::Dwg => DwgWriter::write_to_vec(&cad),
            }
            .unwrap();
            let mut back = Document::default();
            let report = import(&read(&bytes, format).unwrap(), &mut back);
            assert!(
                report.brought.contains(&("texts".to_owned(), 1)),
                "{format:?}: {report:?}"
            );
            let state = back.state();
            let instance = state
                .objects
                .iter()
                .find(|o| matches!(o.shape, Shape::Instance(_)))
                .unwrap();
            assert_eq!(
                instance.user_text.get("WIDTH").map(String::as_str),
                Some("900")
            );
            assert_eq!(
                instance.user_text.get("CODE").map(String::as_str),
                Some("D-01")
            );
            let note = state
                .objects
                .iter()
                .find_map(|o| match &o.shape {
                    Shape::Annotation(Annotation::Text(t)) => Some((t.clone(), o.layer)),
                    _ => None,
                })
                .expect("the shown attribute as a note");
            assert_eq!(note.0.content, "900");
            assert!((note.0.origin[0] - 1.0).abs() < 1e-9 && (note.0.height - 2.5).abs() < 1e-9);
            assert_eq!(state.layer(note.1).unwrap().name, "Tags", "{format:?}");
        }
    }

    #[test]
    fn autocads_dot_and_unknown_linetypes_come_in() {
        let mut cad = CadDocument::new();
        let mut layer = Layer::new("Fence");
        layer.line_type = "FENCELINE1".to_owned();
        let _ = cad.layers.add(layer);
        let mut dotted = Layer::new("Dots");
        dotted.line_type = "DOT".to_owned();
        let _ = cad.layers.add(dotted);
        let mut a = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        a.common.layer = "Fence".to_owned();
        let mut b = Line::from_points(Vector3::new(0.0, 1.0, 0.0), Vector3::new(1.0, 1.0, 0.0));
        b.common.layer = "Dots".to_owned();
        let _ = cad.add_entity(EntityType::Line(a));
        let _ = cad.add_entity(EntityType::Line(b));
        let mut doc = Document::default();
        let report = import(&cad, &mut doc);
        let state = doc.state();
        let layer = |n: &str| state.layers.iter().find(|l| l.name == n).unwrap();
        assert_eq!(layer("Fence").linetype, Linetype::Continuous);
        assert_eq!(layer("Dots").linetype, Linetype::Dotted);
        assert!(
            report
                .skipped
                .iter()
                .any(|(k, n)| k.contains("\"FENCELINE1\"") && *n == 1),
            "{:?}",
            report.skipped
        );
    }

    #[test]
    fn multileaders_come_in_as_leaders_with_their_note() {
        use acadrust::entities::MultiLeader;
        let mut cad = CadDocument::new();
        let mut m = MultiLeader::with_text(
            r"{\fArial|b1;Oak} floor",
            Vector3::new(20.0, 10.0, 0.0),
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)],
        );
        m.context.has_text_contents = true;
        m.context.text_direction = Vector3::new(1.0, 0.0, 0.0);
        m.context.text_normal = Vector3::new(0.0, 0.0, 1.0);
        let root = m.add_leader_root();
        root.connection_point = Vector3::new(20.0, 10.0, 0.0);
        root.create_line(vec![Vector3::new(5.0, 30.0, 0.0)]);
        let _ = cad.add_entity(EntityType::MultiLeader(Box::new(m)));
        for format in [Format::Dxf, Format::Dwg] {
            let bytes = match format {
                Format::Dxf => DxfWriter::new(&cad).write_to_vec(),
                Format::Dwg => DwgWriter::write_to_vec(&cad),
            }
            .unwrap();
            let mut doc = Document::default();
            let report = import(&read(&bytes, format).unwrap(), &mut doc);
            assert_eq!(
                report.brought,
                vec![("leaders".to_owned(), 2)],
                "{format:?}"
            );
            let leaders: Vec<_> = doc
                .state()
                .objects
                .iter()
                .filter_map(|o| match &o.shape {
                    Shape::Annotation(Annotation::Leader(l)) => Some(l.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(leaders[0].content, "Oak floor", "{format:?}");
            assert_eq!(
                leaders[0].points,
                vec![[0.0; 3], [10.0, 10.0, 0.0], [20.0, 10.0, 0.0]],
                "{format:?}: to the connection point"
            );
            assert_eq!(leaders[1].content, "", "the note goes with one line");
            assert_eq!(leaders[1].points, vec![[5.0, 30.0, 0.0], [20.0, 10.0, 0.0]]);
        }
    }

    #[test]
    fn a_multileaders_block_comes_in_where_it_puts_it() {
        use acadrust::entities::{Circle, MultiLeader};
        use acadrust::types::Vector3;
        // An MLEADER whose content is the block "Tag" (a circle of radius 2 at its base point),
        // at (20, 10), twice the size and a quarter turn round.
        let mut cad = CadDocument::new();
        crate::blocks::write_block(
            &mut cad,
            "Tag",
            vec![EntityType::Circle(Circle::from_center_radius(
                Vector3::ZERO,
                2.0,
            ))],
        )
        .unwrap();
        let tag = cad.block_records.get("Tag").unwrap().handle;
        let mut m = MultiLeader::new();
        m.context.has_text_contents = false;
        m.context.has_block_contents = true;
        m.context.block_content_handle = Some(tag);
        m.block_content_handle = Some(tag);
        m.context.block_content_location = Vector3::new(20.0, 10.0, 0.0);
        m.context.block_content_normal = Vector3::new(0.0, 0.0, 1.0);
        m.context.block_content_scale = Vector3::new(2.0, 2.0, 2.0);
        m.context.block_rotation = std::f64::consts::FRAC_PI_2;
        let root = m.add_leader_root();
        root.connection_point = Vector3::new(18.0, 10.0, 0.0);
        root.create_line(vec![Vector3::new(0.0, 0.0, 0.0)]);
        let _ = cad.add_entity(EntityType::MultiLeader(Box::new(m)));
        for format in [Format::Dxf, Format::Dwg] {
            let bytes = match format {
                Format::Dxf => DxfWriter::new(&cad).write_to_vec(),
                Format::Dwg => DwgWriter::write_to_vec(&cad),
            }
            .unwrap();
            let mut doc = Document::default();
            import(&read(&bytes, format).unwrap(), &mut doc);
            let state = doc.state();
            let leaders = state
                .objects
                .iter()
                .filter(|o| matches!(o.shape, Shape::Annotation(Annotation::Leader(_))))
                .count();
            assert_eq!(leaders, 1, "{format:?}");
            let instance = state
                .objects
                .iter()
                .find_map(|o| match &o.shape {
                    Shape::Instance(i) => Some(*i),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{format:?}: the block"));
            let block = state
                .blocks
                .iter()
                .find(|b| b.id == instance.definition)
                .unwrap();
            assert_eq!(block.name, "Tag", "{format:?}");
            let m = instance.placement;
            let near = |a: [f64; 4], b: [f64; 4]| (0..4).all(|k| (a[k] - b[k]).abs() < 1e-9);
            assert!(
                near(m[0], [0.0, 2.0, 0.0, 0.0])
                    && near(m[1], [-2.0, 0.0, 0.0, 0.0])
                    && near(m[3], [20.0, 10.0, 0.0, 1.0]),
                "{format:?}: {m:?}"
            );
        }
    }

    #[test]
    fn a_fit_point_spline_comes_in_with_its_end_tangents_and_spacing() {
        use acadrust::types::Vector3;
        let fit = [
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [20.0, 0.0, 0.0],
            [30.0, 4.0, 0.0],
        ];
        let mut spline = Spline::from_fit_points(fit.iter().map(|p| v3(*p)).collect());
        spline.begin_tangent = Vector3::new(0.0, 1.0, 0.0);
        spline.end_tangent = Vector3::new(0.0, -1.0, 0.0);
        spline.knot_parameterization = 1;
        let mut cad = CadDocument::new();
        let _ = cad.add_entity(EntityType::Spline(spline));
        let expected = Curve::fitted(
            fit.to_vec(),
            false,
            Some([0.0, 1.0, 0.0]),
            Some([0.0, -1.0, 0.0]),
            Spacing::SquareRoot,
        )
        .unwrap();
        for format in [Format::Dxf, Format::Dwg] {
            let bytes = match format {
                Format::Dxf => DxfWriter::new(&cad).write_to_vec(),
                Format::Dwg => DwgWriter::write_to_vec(&cad),
            }
            .unwrap();
            let mut doc = Document::default();
            import(&read(&bytes, format).unwrap(), &mut doc);
            let curves: Vec<&Curve> = doc
                .state()
                .objects
                .iter()
                .filter_map(|o| match &o.shape {
                    Shape::Curve(c) => Some(c),
                    _ => None,
                })
                .collect();
            assert_eq!(curves, vec![&expected], "{format:?}");
        }
    }

    #[test]
    fn object_colours_and_the_documents_own_linetypes_come_back_as_dxf_and_dwg() {
        let mut doc = Document::default();
        doc.set_unit(Unit::Metres, false);
        let walls = doc.add_layer("Walls".to_owned());
        let wall = doc.add_linetype("Wall dash", &[8.0, 2.0, 1.0, 2.0]);
        doc.set_layer_linetype(walls, wall);
        let line = |y: f64| Shape::Curve(Curve::line([0.0, y, 0.0], [10.0, y, 0.0]).unwrap());
        let red = doc.add_shape_on(line(0.0), Some(walls));
        doc.set_colour(&[red], Some([1.0, 0.0, 0.0]));
        doc.add_shape_on(line(1.0), Some(walls));
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            let lengths: Vec<f64> = cad
                .line_types
                .get("Wall dash")
                .expect("written")
                .elements
                .iter()
                .map(|e| e.length)
                .collect();
            let near = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
            assert!(
                near(&lengths, &[0.008, -0.002, 0.001, -0.002]),
                "{format:?}: millimetres in metres, {lengths:?}"
            );
            let mut back = Document::default();
            import(&cad, &mut back);
            let state = back.state();
            let wall_back = state.linetype_named("Wall dash").expect("read");
            assert!(near(
                state.linetype_pattern(wall_back),
                &[8.0, 2.0, 1.0, 2.0]
            ));
            let walls_back = state.layers.iter().find(|l| l.name == "Walls").unwrap();
            assert_eq!(walls_back.linetype, wall_back, "{format:?}");
            let colours: Vec<Option<[f32; 3]>> = state.objects.iter().map(|o| o.colour).collect();
            assert_eq!(colours, [Some([1.0, 0.0, 0.0]), None], "{format:?}");
        }
    }

    #[test]
    fn annotation_styles_go_out_and_come_back_as_dxf_and_dwg() {
        use curva_document::annotation::{
            AnnotationStyle, Dimension as Dim, DimensionKind, Text, TextAlign,
        };
        let mut doc = Document::default();
        let plans = doc.add_annotation_style(AnnotationStyle {
            id: 0,
            name: "Plans".to_owned(),
            font: Some("Arial".to_owned()),
            text_height: 3.5,
            imperial: false,
            arrow_size: Some(2.0),
            extension_offset: Some(1.0),
            extension_extension: Some(1.5),
            text_gap: Some(0.8),
            linear_resolution: Some(1),
        });
        doc.add_annotation(Annotation::Dimension(Dim {
            kind: DimensionKind::Horizontal,
            first: [0.0; 3],
            second: [5000.0, 0.0, 0.0],
            line: [0.0, 800.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: plans,
            text: None,
        }));
        doc.add_annotation(Annotation::Text(Text {
            content: "Kitchen".to_owned(),
            origin: [0.0, 2000.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height: 3.5,
            font: None,
            align: TextAlign::Left,
            style: plans,
        }));
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            let text = cad.text_styles.get("Plans").expect("the text style");
            assert_eq!(text.true_type_font, "Arial", "{format:?}");
            let dim = cad.dim_styles.get("Plans").expect("the dimension style");
            assert_eq!(
                (
                    dim.dimtxt, dim.dimasz, dim.dimexo, dim.dimexe, dim.dimgap, dim.dimdec
                ),
                (3.5, 2.0, 1.0, 1.5, 0.8, 1),
                "{format:?}"
            );
            assert_eq!(
                dim.dimtxsty, "Plans",
                "{format:?}: its text style, by handle"
            );
            // A DWG keeps the suffix as written; acadrust leaves an empty one out of a DXF and
            // reads it back as its own "<>".
            if format == Format::Dwg {
                assert!(dim.dimpost.is_empty(), "no suffix after the value");
            }
            let mut back = Document::default();
            import(&cad, &mut back);
            let styles: Vec<AnnotationStyle> = back
                .annotation_styles()
                .into_iter()
                .filter(|s| s.name == "Plans")
                .collect();
            assert_eq!(styles.len(), 1, "{format:?}");
            let style = &styles[0];
            assert_eq!(style.font.as_deref(), Some("Arial"));
            assert_eq!(style.text_height, 3.5);
            assert_eq!(style.arrow_size, Some(2.0));
            assert_eq!(style.extension_offset, Some(1.0));
            assert_eq!(style.extension_extension, Some(1.5));
            assert_eq!(style.text_gap, Some(0.8));
            assert_eq!(style.linear_resolution, Some(1));
            let named: Vec<u64> = back
                .state()
                .objects
                .iter()
                .filter_map(|o| match &o.shape {
                    Shape::Annotation(a) => Some(a.style()),
                    _ => None,
                })
                .collect();
            assert_eq!(named, [style.id, style.id], "{format:?}");
        }
    }
}
