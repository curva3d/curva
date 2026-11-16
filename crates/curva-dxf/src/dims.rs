//! Dimensions and leaders to and from DXF and DWG (spec `drawing-output.md` §3).
//!
//! A DIMENSION entity names a block that holds its drawing (AutoCAD DXF Reference, "DIMENSION":
//! the anonymous `*D` blocks), so each goes out as that block, drawn as Curva draws it (lines,
//! filled arrowheads, its value as text), and the entity with its measured points. Linear
//! dimensions measure in their plane, which DXF takes as the world's XY plane; one on any other
//! plane goes out as its drawing alone. Leaders go out as LEADER entities with their note as
//! text. Coming in, each kind becomes Curva's own dimension or leader from its points.

use crate::{P, cross, dot, p3, unit, v3};
use acadrust::entities::dimension::{
    Dimension, DimensionAligned, DimensionAngular2Ln, DimensionArc, DimensionDiameter,
    DimensionLinear, DimensionOrdinate, DimensionRadius,
};
use acadrust::entities::{EntityType, Leader as CadLeader, Line, MultiLeader, Solid};
use curva_document::State;
use curva_document::annotation::{
    AngleDimension, Annotation, ArcLengthDimension, Dimension as Linear, DimensionKind, Leader,
    OrdinateAxis, OrdinateDimension, RadialDimension,
};

/// A dimension's drawing as block contents: its lines, its arrowheads as filled triangles and
/// its value as text.
fn drawing(annotation: &Annotation, state: &State, space: Option<u64>) -> Option<Vec<EntityType>> {
    let parts = curva_document::shape::annotation_parts(annotation, state, space)?;
    let mut out: Vec<EntityType> = parts
        .lines
        .iter()
        .map(|(a, b)| EntityType::Line(Line::from_points(v3(*a), v3(*b))))
        .collect();
    for [tip, left, right] in &parts.arrows {
        let solid = Solid::new(v3(*tip), v3(*left), v3(*right), v3(*right));
        out.push(EntityType::Solid(solid));
    }
    out.extend(crate::annotation::text_out(
        &parts.text,
        &parts.text.content,
    ));
    Some(out)
}

/// Whether a plane is the world's XY plane, facing up.
fn flat(x: P, y: P) -> bool {
    unit(cross(x, y)).is_some_and(|n| n[2] > 1.0 - 1e-9)
}

/// A dimension's typed text for DXF: `<>` stands for the value there too.
fn typed(text: &Option<String>) -> Option<String> {
    text.clone()
}

/// The DIMENSION entity for a dimension, naming the block that holds its drawing, and that
/// block's contents; `None` for annotation that is not a dimension.
pub(crate) fn dimension_out(
    annotation: &Annotation,
    state: &State,
    space: Option<u64>,
    block: &str,
) -> Option<(EntityType, Vec<EntityType>)> {
    let contents = drawing(annotation, state, space)?;
    let parts = curva_document::shape::annotation_parts(annotation, state, space)?;
    let mut dimension = match annotation {
        Annotation::Dimension(d) => linear_out(d)?,
        Annotation::Radial(r) if r.diameter => {
            let far: P = std::array::from_fn(|k| 2.0 * r.centre[k] - r.on_curve[k]);
            let mut d = DimensionDiameter::new(v3(r.on_curve), v3(far));
            d.base.user_text = typed(&r.text);
            Dimension::Diameter(d)
        }
        Annotation::Radial(r) => {
            let mut d = DimensionRadius::new(v3(r.centre), v3(r.on_curve));
            d.base.user_text = typed(&r.text);
            Dimension::Radius(d)
        }
        Annotation::Angle(a) => {
            let mut d = DimensionAngular2Ln::new(v3(a.apex), v3(a.first), v3(a.second));
            d.dimension_arc = v3(a.arc_at);
            d.base.user_text = typed(&a.text);
            Dimension::Angular2Ln(d)
        }
        // AutoCAD measures from its definition point (the base) along the world's X or Y and
        // draws its own jog, so only a dimension on the world's plane goes out, without kinks.
        Annotation::Ordinate(o) if flat(o.x, o.y) && (o.x[0] - 1.0).abs() < 1e-9 => {
            let mut d =
                DimensionOrdinate::new(v3(o.point), v3(o.leader), o.axis == OrdinateAxis::X);
            d.definition_point = v3(o.base);
            d.base.definition_point = v3(o.base);
            d.base.actual_measurement = o.value();
            d.base.user_text = typed(&o.text);
            Dimension::Ordinate(d)
        }
        // Its angles are on the world's plane, so only one on that plane goes out as DIMARC.
        Annotation::ArcLength(a) if flat(a.x, a.y) => {
            let angle = |p: P| (p[1] - a.centre[1]).atan2(p[0] - a.centre[0]);
            let start = angle(a.start);
            let mut d = DimensionArc {
                definition_point: v3(a.arc_at),
                first_extension_point: v3(a.start),
                second_extension_point: v3(a.end),
                center_point: v3(a.centre),
                arc_start_parameter: start,
                arc_end_parameter: start + a.turn(),
                is_partial: a.partial,
                ..DimensionArc::default()
            };
            d.base.definition_point = v3(a.arc_at);
            if let Some((from, to)) = a.leader_points() {
                d.has_leader = true;
                d.first_leader_point = v3(from);
                d.second_leader_point = v3(to);
            }
            d.base.actual_measurement = a.value();
            d.base.user_text = typed(&a.text);
            Dimension::Arc(d)
        }
        _ => return None,
    };
    let base = base_of(&mut dimension);
    base.block_name = block.to_owned();
    base.text_middle_point = v3(parts.text.origin);
    base.style_name = "Standard".to_owned();
    Some((EntityType::Dimension(Box::new(dimension)), contents))
}

pub(crate) fn base_of(d: &mut Dimension) -> &mut acadrust::entities::dimension::DimensionBase {
    match d {
        Dimension::Aligned(x) => &mut x.base,
        Dimension::Linear(x) => &mut x.base,
        Dimension::Radius(x) => &mut x.base,
        Dimension::Diameter(x) => &mut x.base,
        Dimension::Angular2Ln(x) => &mut x.base,
        Dimension::Angular3Pt(x) => &mut x.base,
        Dimension::Ordinate(x) => &mut x.base,
        Dimension::Arc(x) => &mut x.base,
        Dimension::LargeRadial(x) => &mut x.base,
    }
}

/// A linear dimension as DXF's: aligned, or linear at the angle it measures along, with the
/// point where its dimension line meets the second extension line. Only in the world's XY plane.
fn linear_out(d: &Linear) -> Option<Dimension> {
    if !flat(d.x, d.y) {
        return None;
    }
    let along = d.direction();
    let across = cross([0.0, 0.0, 1.0], along);
    let offset = dot(std::array::from_fn(|k| d.line[k] - d.second[k]), across);
    let on_line: P = std::array::from_fn(|k| d.second[k] + across[k] * offset);
    let measured = d.value();
    Some(match d.kind {
        DimensionKind::Aligned => {
            let mut x = DimensionAligned::new(v3(d.first), v3(d.second));
            x.definition_point = v3(on_line);
            x.base.definition_point = v3(on_line);
            x.base.actual_measurement = measured;
            x.base.user_text = typed(&d.text);
            Dimension::Aligned(x)
        }
        _ => {
            let angle = along[1].atan2(along[0]);
            let mut x = DimensionLinear::rotated(v3(d.first), v3(d.second), angle);
            x.definition_point = v3(on_line);
            x.base.definition_point = v3(on_line);
            x.base.actual_measurement = measured;
            x.base.user_text = typed(&d.text);
            Dimension::Linear(x)
        }
    })
}

/// A leader as a LEADER entity through its points, with its note as text beside its end.
pub(crate) fn leader_out(l: &Leader, state: &State, space: Option<u64>) -> Vec<EntityType> {
    let mut leader = CadLeader::new();
    leader.vertices = l.points.iter().map(|p| v3(*p)).collect();
    leader.arrow_enabled = true;
    leader.normal = v3(unit(cross(l.x, l.y)).unwrap_or([0.0, 0.0, 1.0]));
    let mut out = vec![EntityType::Leader(leader)];
    if let Some(parts) =
        curva_document::shape::annotation_parts(&Annotation::Leader(l.clone()), state, space)
    {
        out.extend(crate::annotation::text_out(&parts.text, &l.content));
    }
    out
}

/// Typed dimension text from DXF, or none when it shows the value alone.
fn typed_in(base: &acadrust::entities::dimension::DimensionBase) -> Option<String> {
    let text = base.user_text.clone().unwrap_or_else(|| base.text.clone());
    let trimmed = text.trim();
    (!trimmed.is_empty() && trimmed != "<>").then_some(text)
}

/// A DIMENSION entity as Curva's dimension, on the world's XY plane: every kind DXF has.
pub(crate) fn dimension_in(d: &Dimension) -> Annotation {
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    match d {
        Dimension::Aligned(a) => Annotation::Dimension(Linear {
            kind: DimensionKind::Aligned,
            first: p3(&a.first_point),
            second: p3(&a.second_point),
            line: p3(&a.definition_point),
            x,
            y,
            style: 0,
            text: typed_in(&a.base),
        }),
        Dimension::Linear(l) => Annotation::Dimension(Linear {
            kind: DimensionKind::Rotated(l.rotation),
            first: p3(&l.first_point),
            second: p3(&l.second_point),
            line: p3(&l.definition_point),
            x,
            y,
            style: 0,
            text: typed_in(&l.base),
        }),
        Dimension::Radius(r) => Annotation::Radial(RadialDimension {
            diameter: false,
            centre: p3(&r.angle_vertex),
            on_curve: p3(&r.definition_point),
            text_at: p3(&r.base.text_middle_point),
            x,
            y,
            style: 0,
            text: typed_in(&r.base),
        }),
        Dimension::Diameter(r) => {
            let (a, b) = (p3(&r.angle_vertex), p3(&r.definition_point));
            Annotation::Radial(RadialDimension {
                diameter: true,
                centre: std::array::from_fn(|k| (a[k] + b[k]) / 2.0),
                on_curve: a,
                text_at: p3(&r.base.text_middle_point),
                x,
                y,
                style: 0,
                text: typed_in(&r.base),
            })
        }
        Dimension::Angular2Ln(a) => Annotation::Angle(AngleDimension {
            apex: p3(&a.angle_vertex),
            first: p3(&a.second_point),
            second: p3(&a.definition_point),
            arc_at: p3(&a.dimension_arc),
            x,
            y,
            style: 0,
            text: typed_in(&a.base),
        }),
        // An angle by three points: its vertex, a point on each side, the arc's place.
        Dimension::Angular3Pt(a) => Annotation::Angle(AngleDimension {
            apex: p3(&a.angle_vertex),
            first: p3(&a.first_point),
            second: p3(&a.second_point),
            arc_at: p3(&a.definition_point),
            x,
            y,
            style: 0,
            text: typed_in(&a.base),
        }),
        // A jogged radius (DIMJOGGED) as a plain one: its centre, the point it measures to and
        // its text; the jog and the moved centre it is drawn from are left out.
        Dimension::LargeRadial(r) => Annotation::Radial(RadialDimension {
            diameter: false,
            centre: p3(&r.definition_point),
            on_curve: p3(&r.chord_point),
            text_at: p3(&r.base.text_middle_point),
            x,
            y,
            style: 0,
            text: typed_in(&r.base),
        }),
        // An arc length (DIMARC): the arc's centre and ends, counter-clockwise from the first,
        // and a point on the dimension arc. A partial one measures between its ends as given and
        // keeps its flag; a leader is drawn from the dimension again, as AutoCAD places it.
        Dimension::Arc(a) => Annotation::ArcLength(ArcLengthDimension {
            centre: p3(&a.center_point),
            start: p3(&a.first_extension_point),
            end: p3(&a.second_extension_point),
            arc_at: p3(&a.definition_point),
            x,
            y,
            style: 0,
            text: typed_in(&a.base),
            partial: a.is_partial,
            leader: a.has_leader,
        }),
        Dimension::Ordinate(o) => Annotation::Ordinate(OrdinateDimension {
            base: p3(&o.definition_point),
            point: p3(&o.feature_location),
            leader: p3(&o.leader_endpoint),
            axis: if o.is_ordinate_type_x {
                OrdinateAxis::X
            } else {
                OrdinateAxis::Y
            },
            kinks: None,
            x,
            y,
            style: 0,
            text: typed_in(&o.base),
        }),
    }
}

/// A LEADER entity as Curva's leader, its note coming in as text of its own.
pub(crate) fn leader_in(l: &CadLeader) -> Option<Leader> {
    let normal = unit(p3(&l.normal)).unwrap_or([0.0, 0.0, 1.0]);
    let (x, y) = crate::object_axes(normal);
    (l.vertices.len() >= 2).then(|| Leader {
        points: l.vertices.iter().map(p3).collect(),
        content: String::new(),
        x,
        y,
        landing: false,
        style: 0,
    })
}

/// An MLEADER's leader lines as leaders: each line runs from its arrow through its points to its
/// root's connection point, where the landing starts. The note (formatting dropped) goes with the
/// first line; any others come in as lines alone.
pub(crate) fn multileader_in(m: &MultiLeader) -> Vec<Leader> {
    let context = &m.context;
    let normal = unit(p3(&context.text_normal)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, _) = crate::object_axes(normal);
    let along = p3(&context.text_direction);
    let level = dot(along, normal);
    let x = unit(std::array::from_fn(|k| along[k] - normal[k] * level)).unwrap_or(ox);
    let y = cross(normal, x);
    let mut note = if context.has_text_contents {
        crate::annotation::plain_mtext(&context.text_string)
    } else {
        String::new()
    };
    let near = |a: P, b: P| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9);
    let mut out = Vec::new();
    for root in &context.leader_roots {
        let connection = p3(&root.connection_point);
        for line in &root.lines {
            let mut points: Vec<P> = line.points.iter().map(p3).collect();
            if !points.last().is_some_and(|last| near(*last, connection)) {
                points.push(connection);
            }
            if points.len() < 2 {
                continue;
            }
            out.push(Leader {
                points,
                content: std::mem::take(&mut note),
                x,
                y,
                landing: m.enable_landing && root.landing_distance > 0.0,
                style: 0,
            });
        }
    }
    out
}

/// The dimension style a DIMENSION names.
pub(crate) fn style_name_of(d: &Dimension) -> &str {
    match d {
        Dimension::Aligned(x) => &x.base.style_name,
        Dimension::Linear(x) => &x.base.style_name,
        Dimension::Radius(x) => &x.base.style_name,
        Dimension::Diameter(x) => &x.base.style_name,
        Dimension::Angular2Ln(x) => &x.base.style_name,
        Dimension::Angular3Pt(x) => &x.base.style_name,
        Dimension::Ordinate(x) => &x.base.style_name,
        Dimension::Arc(x) => &x.base.style_name,
        Dimension::LargeRadial(x) => &x.base.style_name,
    }
}

#[cfg(test)]
mod tests {
    use crate::{Format, export, import, read};
    use curva_document::annotation::{
        AngleDimension, Annotation, Dimension, DimensionKind, Leader, RadialDimension,
    };
    use curva_document::{Document, Shape};

    fn annotations(doc: &Document) -> Vec<Annotation> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(a) => Some(a.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn arc_length_dimensions_go_out_and_come_back() {
        use curva_document::annotation::ArcLengthDimension;
        let mut doc = Document::default();
        // Radius 10 about (5, 5), from 300° on to 60°, past the X axis; the arc at radius 14.
        let on = |r: f64, degrees: f64| {
            let (s, c) = degrees.to_radians().sin_cos();
            [5.0 + r * c, 5.0 + r * s, 0.0]
        };
        let wanted = ArcLengthDimension {
            centre: [5.0, 5.0, 0.0],
            start: on(10.0, 300.0),
            end: on(10.0, 60.0),
            arc_at: on(14.0, 0.0),
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: 0,
            text: None,
            partial: false,
            leader: false,
        };
        doc.add_annotation(Annotation::ArcLength(wanted.clone()));
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            if let (Format::Dxf, Ok(path)) = (format, std::env::var("CURVA_DXF_ARC_SAMPLE")) {
                std::fs::write(path, &bytes).unwrap();
            }
            assert!(report.skipped.is_empty(), "{report:?}");
            let cad = read(&bytes, format).unwrap();
            let arc = cad
                .entities()
                .find_map(|e| match e {
                    acadrust::entities::EntityType::Dimension(d) => match d.as_ref() {
                        acadrust::entities::dimension::Dimension::Arc(a) => Some(a.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{format:?}: no ARC_DIMENSION"));
            // AutoCAD's own measure of it agrees.
            assert!(
                (arc.measurement() - wanted.value()).abs() < 1e-9,
                "{format:?}"
            );
            let mut back = Document::default();
            import(&cad, &mut back);
            assert_eq!(
                annotations(&back),
                vec![Annotation::ArcLength(wanted.clone())],
                "{format:?}"
            );
        }
    }

    #[test]
    fn a_partial_arc_length_with_a_leader_goes_out_and_comes_back() {
        use curva_document::annotation::ArcLengthDimension;
        let mut doc = Document::default();
        // Part of an arc of radius 10 about the origin, from 20° to 160°, with its leader.
        let on = |r: f64, degrees: f64| {
            let (s, c) = f64::to_radians(degrees).sin_cos();
            [r * c, r * s, 0.0]
        };
        let wanted = ArcLengthDimension {
            centre: [0.0; 3],
            start: on(10.0, 20.0),
            end: on(10.0, 160.0),
            arc_at: on(15.0, 90.0),
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: 0,
            text: None,
            partial: true,
            leader: true,
        };
        doc.add_annotation(Annotation::ArcLength(wanted.clone()));
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, _) = export(&doc, None, format).unwrap();
            let cad = read(&bytes, format).unwrap();
            let arc = cad
                .entities()
                .find_map(|e| match e {
                    acadrust::entities::EntityType::Dimension(d) => match d.as_ref() {
                        acadrust::entities::dimension::Dimension::Arc(a) => Some(a.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{format:?}: no ARC_DIMENSION"));
            assert!(arc.is_partial && arc.has_leader, "{format:?}");
            // The leader runs in from the dimension arc to the measured arc, straight up at 90°.
            let gap = |p: &acadrust::types::Vector3, q: [f64; 3]| {
                let p = crate::p3(p);
                ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
            };
            assert!(
                gap(&arc.first_leader_point, on(15.0, 90.0)) < 1e-9,
                "{format:?}"
            );
            assert!(
                gap(&arc.second_leader_point, on(10.0, 90.0)) < 1e-9,
                "{format:?}"
            );
            let mut back = Document::default();
            import(&cad, &mut back);
            assert_eq!(
                annotations(&back),
                vec![Annotation::ArcLength(wanted.clone())],
                "{format:?}"
            );
        }
    }

    #[test]
    fn ordinate_dimensions_go_out_and_come_back() {
        use curva_document::annotation::{OrdinateAxis, OrdinateDimension};
        let mut doc = Document::default();
        let ordinate = |leader, axis| OrdinateDimension {
            base: [10.0, 10.0, 0.0],
            point: [40.0, 25.0, 0.0],
            leader,
            axis,
            kinks: None,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: 0,
            text: None,
        };
        let wanted = [
            ordinate([45.0, 60.0, 0.0], OrdinateAxis::X),
            ordinate([0.0, 20.0, 0.0], OrdinateAxis::Y),
        ];
        for d in &wanted {
            doc.add_annotation(Annotation::Ordinate(d.clone()));
        }
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            if let (Format::Dxf, Ok(path)) = (format, std::env::var("CURVA_DXF_ORDINATE_SAMPLE")) {
                std::fs::write(path, &bytes).unwrap();
            }
            assert!(report.skipped.is_empty(), "{report:?}");
            let mut back = Document::default();
            import(&read(&bytes, format).unwrap(), &mut back);
            let got: Vec<OrdinateDimension> = annotations(&back)
                .into_iter()
                .filter_map(|a| match a {
                    Annotation::Ordinate(d) => Some(d),
                    _ => None,
                })
                .collect();
            assert_eq!(got, wanted, "{format:?}");
        }
    }

    #[test]
    fn three_point_angles_and_jogged_radii_come_in() {
        use acadrust::entities::EntityType;
        use acadrust::entities::dimension::{
            Dimension as Cad, DimensionAngular3Pt, DimensionLargeRadial,
        };
        use acadrust::types::Vector3;
        use acadrust::{CadDocument, DwgWriter, DxfWriter};
        // AutoCAD's angle by three points (vertex (0, 0), sides through (10, 0) and (0, 10), its
        // arc through (5, 5)) and a jogged radius (centre (50, 0), to (60, 0), text at (70, 5)).
        let mut cad = CadDocument::new();
        let mut angle = DimensionAngular3Pt::new(
            Vector3::ZERO,
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(0.0, 10.0, 0.0),
        );
        angle.definition_point = Vector3::new(5.0, 5.0, 0.0);
        let mut jogged = DimensionLargeRadial {
            definition_point: Vector3::new(50.0, 0.0, 0.0),
            chord_point: Vector3::new(60.0, 0.0, 0.0),
            override_center: Vector3::new(55.0, -20.0, 0.0),
            jog_point: Vector3::new(57.0, -10.0, 0.0),
            ..Default::default()
        };
        jogged.base.text_middle_point = Vector3::new(70.0, 5.0, 0.0);
        let _ = cad.add_entity(EntityType::Dimension(Box::new(Cad::Angular3Pt(angle))));
        let _ = cad.add_entity(EntityType::Dimension(Box::new(Cad::LargeRadial(jogged))));
        for format in [Format::Dxf, Format::Dwg] {
            let bytes = match format {
                Format::Dxf => DxfWriter::new(&cad).write_to_vec(),
                Format::Dwg => DwgWriter::write_to_vec(&cad),
            }
            .unwrap();
            if let (Format::Dxf, Ok(path)) = (format, std::env::var("CURVA_DXF_ANGLE3_SAMPLE")) {
                std::fs::write(path, &bytes).unwrap();
            }
            let mut doc = Document::default();
            let report = import(&read(&bytes, format).unwrap(), &mut doc);
            assert!(report.skipped.is_empty(), "{format:?}: {report:?}");
            let got = annotations(&doc);
            assert_eq!(got.len(), 2, "{format:?}");
            let Annotation::Angle(a) = &got[0] else {
                panic!("{format:?}: {:?}", got[0]);
            };
            assert_eq!(
                (a.apex, a.first, a.second, a.arc_at),
                (
                    [0.0; 3],
                    [10.0, 0.0, 0.0],
                    [0.0, 10.0, 0.0],
                    [5.0, 5.0, 0.0]
                ),
                "{format:?}"
            );
            assert!((a.value() - 90.0).abs() < 1e-9, "{format:?}: {}", a.value());
            let Annotation::Radial(r) = &got[1] else {
                panic!("{format:?}: {:?}", got[1]);
            };
            assert!(!r.diameter);
            assert_eq!(
                (r.centre, r.on_curve, r.text_at),
                ([50.0, 0.0, 0.0], [60.0, 0.0, 0.0], [70.0, 5.0, 0.0]),
                "{format:?}"
            );
        }
    }

    #[test]
    fn dimensions_and_leaders_go_out_and_come_back() {
        let mut doc = Document::default();
        let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let linear = |kind, text: Option<&str>| {
            Annotation::Dimension(Dimension {
                kind,
                first: [10.0, 10.0, 0.0],
                second: [60.0, 30.0, 0.0],
                line: [20.0, 50.0, 0.0],
                x,
                y,
                style: 0,
                text: text.map(str::to_owned),
            })
        };
        let wanted = [
            linear(DimensionKind::Horizontal, None),
            linear(DimensionKind::Aligned, Some("<> clear")),
            linear(DimensionKind::Rotated(0.3), None),
            Annotation::Radial(RadialDimension {
                diameter: true,
                centre: [100.0, 100.0, 0.0],
                on_curve: [103.0, 104.0, 0.0],
                text_at: [120.0, 120.0, 0.0],
                x,
                y,
                style: 0,
                text: None,
            }),
            Annotation::Angle(AngleDimension {
                apex: [0.0; 3],
                first: [10.0, 0.0, 0.0],
                second: [0.0, 5.0, 0.0],
                arc_at: [3.0, 3.0, 0.0],
                x,
                y,
                style: 0,
                text: None,
            }),
            Annotation::Leader(Leader {
                points: vec![[0.0, 0.0, 0.0], [10.0, 10.0, 0.0], [20.0, 10.0, 0.0]],
                content: "Oak floor".to_owned(),
                x,
                y,
                landing: true,
                style: 0,
            }),
        ];
        for a in &wanted {
            doc.add_annotation(a.clone());
        }
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            // For opening in Rhino or AutoCAD by hand.
            if let (Format::Dxf, Ok(path)) = (format, std::env::var("CURVA_DXF_SAMPLE")) {
                std::fs::write(path, &bytes).unwrap();
            }
            assert!(report.skipped.is_empty(), "{report:?}");
            let cad = read(&bytes, format).unwrap();
            assert!(
                cad.block_records.get("*D1").is_some(),
                "{format:?}: each dimension's drawing in a block"
            );
            let mut back = Document::default();
            import(&cad, &mut back);
            let got = annotations(&back);
            let value = |a: &Annotation| match a {
                Annotation::Dimension(d) => Some(d.value()),
                Annotation::Radial(r) => Some(r.value()),
                Annotation::Angle(a) => Some(a.value()),
                _ => None,
            };
            let values: Vec<f64> = got.iter().filter_map(value).collect();
            for w in wanted.iter().filter_map(value) {
                assert!(
                    values.iter().any(|v| (v - w).abs() < 1e-6),
                    "{format:?}: {w} in {values:?}"
                );
            }
            assert!(
                got.iter().any(|a| matches!(a, Annotation::Dimension(d) if d.text.as_deref() == Some("<> clear"))),
                "{format:?}: typed text"
            );
            assert!(
                got.iter()
                    .any(|a| matches!(a, Annotation::Leader(l) if l.points.len() == 3)),
                "{format:?}: the leader"
            );
            assert!(
                got.iter()
                    .any(|a| matches!(a, Annotation::Text(t) if t.content == "Oak floor")),
                "{format:?}: the leader's note"
            );
        }
    }
}
