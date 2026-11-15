//! Text and hatches to and from DXF and DWG (spec `drawing-output.md` §3).
//!
//! - Text goes out as one TEXT entity per line, at its baseline, as Rhino writes notes, with its
//!   fields worked out; TEXT and MTEXT come in as notes (MTEXT's formatting codes dropped).
//! - Hatches go out as HATCH entities with their outlines as polylines and their pattern's lines
//!   at the hatch's scale and turn, as AutoCAD needs them to draw; they come in with lines, arcs
//!   and polylines (with bulges) as outlines, and a pattern Curva knows by name or one made from
//!   the file's pattern lines.
//!
//! Positions on a plane are in AutoCAD's object coordinates (see the crate's notes).

use crate::{P, cross, dot, from_object, object_axes, p3, to_object, unit, v3};
use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, Hatch as CadHatch, HatchPattern, HatchPatternLine, PolylineEdge,
};
use acadrust::entities::text::{TextHorizontalAlignment, TextVerticalAlignment};
use acadrust::entities::{EntityType, MText, Text as CadText};
use acadrust::types::Vector2;
use curva_document::annotation::{Hatch, LINE_SPACING, Text, TextAlign};
use curva_document::hatch::{Pattern, PatternLine};
use curva_document::{State, Unit};
use curva_kernel::Curve;

/// The TEXT entities for a note whose words are already worked out.
pub(crate) fn text_out(text: &Text, words: &str) -> Vec<EntityType> {
    let normal = unit(cross(text.x, text.y)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, oy) = object_axes(normal);
    let rotation = dot(text.x, oy).atan2(dot(text.x, ox));
    let (horizontal, aligned) = match text.align {
        TextAlign::Left => (TextHorizontalAlignment::Left, false),
        TextAlign::Centre => (TextHorizontalAlignment::Center, true),
        TextAlign::Right => (TextHorizontalAlignment::Right, true),
    };
    words
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(row, line)| {
            #[allow(clippy::cast_precision_loss)]
            let down = row as f64 * text.height * LINE_SPACING;
            let at: P = std::array::from_fn(|i| text.origin[i] - text.y[i] * down);
            let mut entity = CadText::new();
            entity.value = line.to_owned();
            entity.insertion_point = v3(to_object(at, normal));
            entity.alignment_point = aligned.then(|| v3(to_object(at, normal)));
            entity.height = text.height;
            entity.rotation = rotation;
            entity.normal = v3(normal);
            entity.horizontal_alignment = horizontal;
            entity.vertical_alignment = TextVerticalAlignment::Baseline;
            EntityType::Text(entity)
        })
        .collect()
}

/// A TEXT entity as a note.
pub(crate) fn text_in(entity: &CadText) -> Option<Text> {
    let normal = unit(p3(&entity.normal)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, oy) = object_axes(normal);
    let (sin, cos) = entity.rotation.sin_cos();
    let x: P = std::array::from_fn(|i| ox[i] * cos + oy[i] * sin);
    let y = cross(normal, x);
    let align = match entity.horizontal_alignment {
        TextHorizontalAlignment::Center | TextHorizontalAlignment::Middle => TextAlign::Centre,
        TextHorizontalAlignment::Right => TextAlign::Right,
        _ => TextAlign::Left,
    };
    let anchor = match (&entity.alignment_point, align) {
        (Some(point), TextAlign::Centre | TextAlign::Right) => point,
        _ => &entity.insertion_point,
    };
    // Notes sit on their baseline; text placed by its top or middle moves down to it.
    let drop = match entity.vertical_alignment {
        TextVerticalAlignment::Top => entity.height,
        TextVerticalAlignment::Middle => entity.height / 2.0,
        _ => 0.0,
    };
    let at = from_object(p3(anchor), normal);
    (entity.height > 0.0 && !entity.value.trim().is_empty()).then(|| Text {
        content: entity.value.clone(),
        origin: std::array::from_fn(|i| at[i] - y[i] * drop),
        x,
        y,
        height: entity.height,
        font: None,
        align,
        style: 0,
    })
}

/// MTEXT's words without its formatting: `\P` is a new line, `\~` a space, `\\` a backslash, and
/// codes such as `\fArial|b0;` or `\H2.5;` and grouping braces are dropped (AutoCAD DXF Reference,
/// "MTEXT", inline codes).
pub(crate) fn plain_mtext(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => match chars.next() {
                Some('P') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                // Underline, overline and strike-through switches carry nothing more.
                Some('L' | 'l' | 'O' | 'o' | 'K' | 'k') => {}
                // Every other code runs to its semicolon.
                Some(_) => {
                    for d in chars.by_ref() {
                        if d == ';' {
                            break;
                        }
                    }
                }
                None => {}
            },
            other => out.push(other),
        }
    }
    out
}

/// An MTEXT entity as a note, its first line's baseline one text height below its top.
pub(crate) fn mtext_in(entity: &MText) -> Option<Text> {
    let normal = unit(p3(&entity.normal)).unwrap_or([0.0, 0.0, 1.0]);
    let x = match &entity.dwg_x_direction {
        Some(direction) => unit(p3(direction)),
        None => None,
    }
    .unwrap_or_else(|| {
        let (ox, oy) = object_axes(normal);
        let (sin, cos) = entity.rotation.sin_cos();
        std::array::from_fn(|i| ox[i] * cos + oy[i] * sin)
    });
    let y = cross(normal, x);
    let content = plain_mtext(&entity.value);
    let at = p3(&entity.insertion_point);
    (entity.height > 0.0 && !content.trim().is_empty()).then(|| Text {
        content,
        origin: std::array::from_fn(|i| at[i] - y[i] * entity.height),
        x,
        y,
        height: entity.height,
        font: None,
        align: TextAlign::Left,
        style: 0,
    })
}

/// How many model units one millimetre is.
fn units_per_millimetre(unit: Unit) -> f64 {
    1.0 / unit.millimetres()
}

/// A hatch as a HATCH entity, with its pattern's lines as AutoCAD draws them: turned by the
/// hatch's rotation and sized by its scale, in model units.
pub(crate) fn hatch_out(hatch: &Hatch, state: &State) -> Option<EntityType> {
    let normal = unit(cross(hatch.x, hatch.y)).unwrap_or([0.0, 0.0, 1.0]);
    let patterns = state.patterns();
    let pattern = patterns
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(&hatch.pattern));
    let mut entity = match pattern {
        Some(p) if !p.is_solid() => {
            let k = hatch.scale * units_per_millimetre(state.unit);
            let base = to_object(hatch.base, normal);
            // The hatch's own X, as an angle in object coordinates.
            let (ox, oy) = object_axes(normal);
            let turn = dot(hatch.x, oy).atan2(dot(hatch.x, ox)) + hatch.rotation;
            let mut cad = HatchPattern::new(p.name.clone());
            cad.description.clone_from(&p.description);
            for line in &p.lines {
                let angle = turn + line.angle.to_radians();
                let (s, c) = angle.sin_cos();
                let (ts, tc) = turn.sin_cos();
                let origin = [line.origin[0] * k, line.origin[1] * k];
                cad.add_line(HatchPatternLine {
                    angle,
                    base_point: Vector2::new(
                        base[0] + origin[0] * tc - origin[1] * ts,
                        base[1] + origin[0] * ts + origin[1] * tc,
                    ),
                    // The next line's start: along the line by the shift, across by the spacing.
                    offset: Vector2::new(
                        (line.shift * c - line.spacing * s) * k,
                        (line.shift * s + line.spacing * c) * k,
                    ),
                    dash_lengths: line.dashes.iter().map(|d| d * k).collect(),
                });
            }
            let mut cad_hatch = CadHatch::with_pattern(cad);
            cad_hatch.pattern_angle = hatch.rotation;
            cad_hatch.pattern_scale = hatch.scale;
            cad_hatch
        }
        _ => CadHatch::solid(),
    };
    entity.normal = v3(normal);
    for outline in &hatch.loops {
        let points = outline.sample().ok()?;
        let flat: Vec<Vector2> = points
            .iter()
            .map(|p| {
                let q = to_object(*p, normal);
                Vector2::new(q[0], q[1])
            })
            .collect();
        if let Some(first) = points.first() {
            entity.elevation = to_object(*first, normal)[2];
        }
        let mut path = BoundaryPath::new();
        path.flags.set_polyline(true);
        path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(flat, true)));
        entity.paths.push(path);
    }
    (!entity.paths.is_empty()).then(|| EntityType::Hatch(Box::new(entity)))
}

/// One edge of a hatch outline as a curve in the world.
fn edge_in(edge: &BoundaryEdge, normal: P, elevation: f64) -> Vec<Curve> {
    let world = |x: f64, y: f64| from_object([x, y, elevation], normal);
    match edge {
        BoundaryEdge::Line(line) => Curve::line(
            world(line.start.x, line.start.y),
            world(line.end.x, line.end.y),
        )
        .into_iter()
        .collect(),
        BoundaryEdge::CircularArc(arc) => {
            // A clockwise edge gives its angles measured clockwise, so they turn back here.
            let (ox, oy) = object_axes(normal);
            let tau = std::f64::consts::TAU;
            let (start, sweep, turning) = if arc.counter_clockwise {
                (
                    arc.start_angle,
                    (arc.end_angle - arc.start_angle).rem_euclid(tau),
                    normal,
                )
            } else {
                (
                    -arc.start_angle,
                    (arc.end_angle - arc.start_angle).rem_euclid(tau),
                    normal.map(|c| -c),
                )
            };
            let sweep = if sweep < 1e-12 { tau } else { sweep };
            let (s, c) = start.sin_cos();
            let x: P = std::array::from_fn(|i| ox[i] * c + oy[i] * s);
            vec![Curve::Arc {
                centre: world(arc.center.x, arc.center.y),
                radius: arc.radius,
                x,
                y: cross(turning, x),
                sweep,
            }]
        }
        BoundaryEdge::Polyline(polyline) => {
            let points: Vec<P> = polyline.vertices.iter().map(|v| world(v.x, v.y)).collect();
            if polyline.has_bulge() {
                let bulges: Vec<f64> = polyline.vertices.iter().map(|v| v.z).collect();
                crate::bulged(&points, &bulges, polyline.is_closed, normal)
                    .into_iter()
                    .collect()
            } else {
                Curve::polyline(points, polyline.is_closed)
                    .into_iter()
                    .collect()
            }
        }
        _ => Vec::new(),
    }
}

/// A HATCH entity as a hatch, and a pattern to add when the file's is not one Curva has.
pub(crate) fn hatch_in(entity: &CadHatch, state: &State) -> Option<(Hatch, Option<Pattern>)> {
    let normal = unit(p3(&entity.normal)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, oy) = object_axes(normal);
    let mut loops = Vec::new();
    for path in &entity.paths {
        let parts: Vec<Curve> = path
            .edges
            .iter()
            .flat_map(|e| edge_in(e, normal, entity.elevation))
            .collect();
        match parts.len() {
            0 => {}
            1 => loops.extend(parts),
            _ => loops.push(Curve::Compound { segments: parts }),
        }
    }
    if loops.is_empty() {
        return None;
    }
    let mm = state.unit.millimetres();
    let known = state
        .patterns()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(&entity.pattern.name));
    let (pattern, scale, added) = if entity.is_solid {
        ("Solid".to_owned(), 1.0, None)
    } else if let Some(known) = known {
        // The scale from the first line's spacing, as written, against the pattern's own.
        let scale = match (entity.pattern.lines.first(), known.lines.first()) {
            (Some(line), Some(own)) if own.spacing.abs() > 1e-12 => {
                let (s, c) = line.angle.sin_cos();
                let across = (-line.offset.x * s + line.offset.y * c).abs();
                across * mm / own.spacing.abs()
            }
            _ => entity.pattern_scale,
        };
        (known.name, scale, None)
    } else {
        // A pattern of the file's own, kept in millimetres at a scale of one.
        let lines = entity
            .pattern
            .lines
            .iter()
            .map(|line| {
                let (s, c) = line.angle.sin_cos();
                PatternLine {
                    angle: line.angle.to_degrees(),
                    origin: [line.base_point.x * mm, line.base_point.y * mm],
                    shift: (line.offset.x * c + line.offset.y * s) * mm,
                    spacing: (-line.offset.x * s + line.offset.y * c) * mm,
                    dashes: line.dash_lengths.iter().map(|d| d * mm).collect(),
                }
            })
            .collect();
        let name = if entity.pattern.name.trim().is_empty() {
            "Imported".to_owned()
        } else {
            entity.pattern.name.clone()
        };
        let own = Pattern {
            name: name.clone(),
            description: entity.pattern.description.clone(),
            lines,
        };
        (name, 1.0, Some(own))
    };
    let base = from_object([0.0, 0.0, entity.elevation], normal);
    let rotation = if added.is_some() {
        0.0
    } else {
        entity.pattern_angle
    };
    Some((
        Hatch {
            loops,
            pattern,
            scale,
            rotation,
            base,
            x: ox,
            y: oy,
        },
        added,
    ))
}

#[cfg(test)]
mod tests {
    use super::plain_mtext;
    use crate::{Format, export, import, read};
    use curva_document::annotation::{Annotation, Hatch, Text, TextAlign};
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    fn notes(doc: &Document) -> Vec<Text> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(Annotation::Text(t)) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    fn hatches(doc: &Document) -> Vec<Hatch> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(Annotation::Hatch(h)) => Some(h.clone()),
                _ => None,
            })
            .collect()
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
    }

    fn round_trip(doc: &Document, format: Format) -> Document {
        let (bytes, _) = export(doc, None, format).unwrap();
        let mut back = Document::default();
        import(&read(&bytes, format).unwrap(), &mut back);
        back
    }

    #[test]
    fn notes_go_out_line_by_line_and_come_back() {
        let mut doc = Document::default();
        let (s, c) = 0.5_f64.sin_cos();
        doc.add_annotation(Annotation::Text(Text {
            content: "Hall\nKitchen".to_owned(),
            origin: [10.0, 20.0, 0.0],
            x: [c, s, 0.0],
            y: [-s, c, 0.0],
            height: 3.5,
            font: None,
            align: TextAlign::Centre,
            style: 0,
        }));
        for format in [Format::Dxf, Format::Dwg] {
            let back = notes(&round_trip(&doc, format));
            assert_eq!(back.len(), 2, "{format:?}: one note per line");
            assert_eq!(back[0].content, "Hall");
            assert_eq!(back[1].content, "Kitchen");
            assert!(
                near(back[0].origin, [10.0, 20.0, 0.0]),
                "{:?}",
                back[0].origin
            );
            assert!(near(back[0].x, [c, s, 0.0]), "turned with the note");
            assert!((back[0].height - 3.5).abs() < 1e-9);
            assert_eq!(back[0].align, TextAlign::Centre);
            // The second line one line spacing below the first, across the turned note.
            let down = 3.5 * curva_document::annotation::LINE_SPACING;
            assert!(near(
                back[1].origin,
                [10.0 + s * down, 20.0 - c * down, 0.0]
            ));
        }
    }

    #[test]
    fn fields_go_out_worked_out() {
        let mut doc = Document::default();
        doc.set_document_text("Project", "Pier");
        doc.add_annotation(Annotation::Text(Text {
            content: "%<DocumentText(\"Project\")>%".to_owned(),
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height: 2.5,
            font: None,
            align: TextAlign::Left,
            style: 0,
        }));
        assert_eq!(notes(&round_trip(&doc, Format::Dxf))[0].content, "Pier");
    }

    #[test]
    fn hatches_keep_their_outline_pattern_scale_and_turn() {
        let mut doc = Document::default();
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [100.0, 0.0, 0.0],
                [100.0, 100.0, 0.0],
                [0.0, 100.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let hatch = |pattern: &str, scale: f64, rotation: f64| Hatch {
            loops: vec![square.clone()],
            pattern: pattern.to_owned(),
            scale,
            rotation,
            base: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
        };
        doc.add_annotation(Annotation::Hatch(hatch("ANSI31", 2.0, 0.3)));
        doc.add_annotation(Annotation::Hatch(hatch("Solid", 1.0, 0.0)));
        for format in [Format::Dxf, Format::Dwg] {
            let back = hatches(&round_trip(&doc, format));
            assert_eq!(back.len(), 2, "{format:?}");
            assert_eq!(back[0].pattern, "ANSI31");
            assert!(
                (back[0].scale - 2.0).abs() < 1e-6,
                "{format:?}: {}",
                back[0].scale
            );
            assert!((back[0].rotation - 0.3).abs() < 1e-6);
            assert_eq!(back[1].pattern, "Solid");
            let corners = back[0].loops[0].sample().unwrap();
            assert!(corners.iter().any(|p| near(*p, [100.0, 100.0, 0.0])));
        }
    }

    #[test]
    fn a_pattern_curva_lacks_comes_in_with_the_hatch() {
        use acadrust::entities::hatch::{
            BoundaryEdge, BoundaryPath, Hatch as CadHatch, HatchPattern, HatchPatternLine, LineEdge,
        };
        use acadrust::types::Vector2;
        let mut pattern = HatchPattern::new("BRICKS");
        pattern.add_line(HatchPatternLine {
            angle: 0.0,
            base_point: Vector2::new(0.0, 0.0),
            offset: Vector2::new(0.0, 5.0),
            dash_lengths: vec![10.0, -2.0],
        });
        let mut cad_hatch = CadHatch::with_pattern(pattern);
        let mut path = BoundaryPath::new();
        let corners = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            path.add_edge(BoundaryEdge::Line(LineEdge {
                start: Vector2::new(a.0, a.1),
                end: Vector2::new(b.0, b.1),
            }));
        }
        cad_hatch.paths.push(path);
        let mut cad = acadrust::CadDocument::new();
        cad.add_entity(acadrust::entities::EntityType::Hatch(Box::new(cad_hatch)))
            .unwrap();
        let mut doc = Document::default();
        import(&cad, &mut doc);
        let made = &hatches(&doc)[0];
        assert_eq!(made.pattern, "BRICKS");
        let own = doc
            .state()
            .patterns()
            .into_iter()
            .find(|p| p.name == "BRICKS")
            .unwrap();
        assert!(
            (own.lines[0].spacing - 5.0).abs() < 1e-9,
            "millimetres, as drawn"
        );
        assert_eq!(own.lines[0].dashes, vec![10.0, -2.0]);
    }

    #[test]
    fn mtext_formatting_is_dropped() {
        assert_eq!(
            plain_mtext(r"{\fArial|b1;Ground}\Pfloor\~plan \\ {\H2.5;x}"),
            "Ground\nfloor plan \\ x"
        );
    }
}
