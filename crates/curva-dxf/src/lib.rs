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

use acadrust::entities::{Arc, Circle, EntityType, Line, LwPolyline, Point, Polyline3D, Spline};
use acadrust::types::{Color, DxfVersion, LineWeight, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Layer};
use curva_document::annotation::Annotation;
use curva_document::fields::{FieldContext, resolve};
use curva_document::{Document, PrintWidth, Shape, Unit};
use curva_kernel::Curve;
use std::collections::BTreeMap;
use std::io::Cursor;

pub(crate) type P = curva_kernel::Point;

/// Which of the two files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// AutoCAD's text exchange file.
    Dxf,
    /// AutoCAD's own drawing file.
    Dwg,
}

impl Format {
    /// The format a file name's extension names, if either.
    #[must_use]
    pub fn of(path: &std::path::Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "dxf" => Some(Self::Dxf),
            "dwg" => Some(Self::Dwg),
            _ => None,
        }
    }
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

pub(crate) fn v3(p: P) -> Vector3 {
    Vector3::new(p[0], p[1], p[2])
}

pub(crate) fn p3(v: &Vector3) -> P {
    [v.x, v.y, v.z]
}

pub(crate) fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn unit(v: P) -> Option<P> {
    let length = dot(v, v).sqrt();
    (length > 1e-12).then(|| v.map(|c| c / length))
}

/// The X and Y axes of the object coordinate system for a normal, by the arbitrary axis
/// algorithm: X is the world Y (or Z) axis crossed with the normal.
pub(crate) fn object_axes(normal: P) -> (P, P) {
    let limit = 1.0 / 64.0;
    let x = if normal[0].abs() < limit && normal[1].abs() < limit {
        cross([0.0, 1.0, 0.0], normal)
    } else {
        cross([0.0, 0.0, 1.0], normal)
    };
    let x = unit(x).unwrap_or([1.0, 0.0, 0.0]);
    (x, cross(normal, x))
}

/// A world point in a normal's object coordinates.
pub(crate) fn to_object(p: P, normal: P) -> P {
    let (x, y) = object_axes(normal);
    [dot(p, x), dot(p, y), dot(p, normal)]
}

/// A point in a normal's object coordinates back in the world.
pub(crate) fn from_object(p: P, normal: P) -> P {
    let (x, y) = object_axes(normal);
    std::array::from_fn(|i| x[i] * p[0] + y[i] * p[1] + normal[i] * p[2])
}

/// The DXF unit code (`$INSUNITS`) for a model unit.
fn unit_code(unit: Unit) -> i16 {
    match unit {
        Unit::Inches => 1,
        Unit::Feet => 2,
        Unit::Millimetres => 4,
        Unit::Metres => 6,
    }
}

/// The model unit for a DXF unit code, if Curva has it.
fn unit_of(code: i16) -> Option<Unit> {
    match code {
        1 => Some(Unit::Inches),
        2 => Some(Unit::Feet),
        4 => Some(Unit::Millimetres),
        6 => Some(Unit::Metres),
        _ => None,
    }
}

/// AutoCAD's line weights, in hundredths of a millimetre: the only ones it accepts.
const LINE_WEIGHTS: [i16; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

/// The AutoCAD line weight nearest a print width in millimetres.
fn line_weight(millimetres: f64) -> LineWeight {
    let wanted = millimetres * 100.0;
    let nearest = LINE_WEIGHTS
        .iter()
        .copied()
        .min_by(|a, b| {
            (f64::from(*a) - wanted)
                .abs()
                .total_cmp(&(f64::from(*b) - wanted).abs())
        })
        .unwrap_or(0);
    LineWeight::Value(nearest)
}

/// A layer's DXF name: its full path, with `$` between levels (DXF names cannot hold `:`), as
/// Rhino writes sublayers.
fn dxf_layer_name(doc: &Document, id: u64) -> String {
    let path = doc.state().layer_path(id);
    let name: String = path
        .replace("::", "$")
        .chars()
        .map(|c| if "<>/\\\":;?*|=`".contains(c) { '_' } else { c })
        .collect();
    if name.is_empty() {
        "0".to_owned()
    } else {
        name
    }
}

/// The DXF entities for a curve: one for most, one per part for a compound.
fn entities_for(curve: &Curve) -> Vec<EntityType> {
    match curve {
        Curve::Point { at } => vec![EntityType::Point(Point::from_coords(at[0], at[1], at[2]))],
        Curve::Line { start, end } => {
            vec![EntityType::Line(Line::from_points(v3(*start), v3(*end)))]
        }
        Curve::Polyline { points, closed } => {
            let flat = points
                .first()
                .is_some_and(|first| points.iter().all(|p| (p[2] - first[2]).abs() < 1e-9));
            if flat {
                // A plan polyline, as Rhino writes one: lightweight, at its height.
                let mut polyline = LwPolyline::from_points(
                    points.iter().map(|p| Vector2::new(p[0], p[1])).collect(),
                );
                polyline.elevation = points[0][2];
                polyline.is_closed = *closed;
                vec![EntityType::LwPolyline(polyline)]
            } else {
                let mut polyline = Polyline3D::from_points(points.iter().map(|p| v3(*p)).collect());
                if *closed {
                    polyline.close();
                }
                vec![EntityType::Polyline3D(polyline)]
            }
        }
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => {
            let normal = unit(cross(*x, *y)).unwrap_or([0.0, 0.0, 1.0]);
            let mut circle = Circle::from_center_radius(v3(to_object(*centre, normal)), *radius);
            circle.normal = v3(normal);
            vec![EntityType::Circle(circle)]
        }
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } => {
            let normal = unit(cross(*x, *y)).unwrap_or([0.0, 0.0, 1.0]);
            let (ox, oy) = object_axes(normal);
            let start = dot(*x, oy).atan2(dot(*x, ox));
            let mut arc = Arc::new();
            arc.center = v3(to_object(*centre, normal));
            arc.radius = *radius;
            arc.start_angle = start;
            arc.end_angle = start + sweep;
            arc.normal = v3(normal);
            vec![EntityType::Arc(arc)]
        }
        Curve::Nurbs {
            degree,
            knots,
            points,
            weights,
        } => {
            let mut spline = Spline::new();
            spline.degree = i32::try_from(*degree).unwrap_or(3);
            spline.knots.clone_from(knots);
            spline.control_points = points.iter().map(|p| v3(*p)).collect();
            spline.flags.rational = weights.iter().any(|w| (w - 1.0).abs() > 1e-12);
            if spline.flags.rational {
                spline.weights.clone_from(weights);
            }
            vec![EntityType::Spline(spline)]
        }
        Curve::Compound { segments } => segments.iter().flat_map(entities_for).collect(),
        Curve::Interpolated { .. } => curve
            .to_nurbs()
            .map(|nurbs| entities_for(&nurbs))
            .unwrap_or_default(),
    }
}

/// A name AutoCAD takes for a layer or block: the characters it refuses become `_`.
fn dxf_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if "<>/\\\":;?*|=`".contains(c) { '_' } else { c })
        .collect();
    if clean.trim().is_empty() {
        "_".to_owned()
    } else {
        clean
    }
}

/// What an export is writing: the drawing, and what has been put in it so far.
struct Writer<'a> {
    cad: CadDocument,
    doc: &'a Document,
    fields: FieldContext,
    layers: BTreeMap<u64, String>,
    blocks: BTreeMap<u64, String>,
    /// How many dimension blocks (`*D1`, `*D2`, ...) are written.
    dimensions: usize,
    report: Report,
}

impl Writer<'_> {
    /// The DXF layer for a Curva layer, made the first time it is needed.
    fn layer(&mut self, id: u64) -> String {
        if let Some(name) = self.layers.get(&id) {
            return name.clone();
        }
        let name = dxf_layer_name(self.doc, id);
        if self.cad.layers.get(&name).is_none() {
            let mut entry = Layer::new(name.clone());
            if let Some(held) = self.doc.state().layer(id) {
                let [r, g, b] = held.colour.map(|c| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let byte = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
                    byte
                });
                entry.color = Color::Rgb { r, g, b };
                entry.flags.off = !held.visible;
                entry.flags.locked = held.locked;
                if let Some(width) = held.print_width {
                    entry.line_weight = line_weight(width);
                }
            }
            let _ = self.cad.layers.add(entry);
        }
        self.layers.insert(id, name.clone());
        name
    }

    /// The entities for one object, and the kind it is reported as.
    fn entities(&mut self, object: &curva_document::Object) -> (Vec<EntityType>, &'static str) {
        let state = self.doc.state();
        let (mut made, kind) = match &object.shape {
            Shape::Curve(curve) => (entities_for(curve), "curves"),
            Shape::Annotation(Annotation::Text(text)) => {
                let words = resolve(&text.content, state, object.space, &self.fields);
                (annotation::text_out(text, &words), "texts")
            }
            Shape::Annotation(Annotation::Hatch(hatch)) => (
                annotation::hatch_out(hatch, state).into_iter().collect(),
                "hatches",
            ),
            Shape::Annotation(
                a @ (Annotation::Dimension(_) | Annotation::Radial(_) | Annotation::Angle(_)),
            ) => {
                let name = format!("*D{}", self.dimensions + 1);
                let written = dims::dimension_out(a, state, object.space, &name)
                    .filter(|(_, contents)| !contents.is_empty())
                    .and_then(|(dimension, contents)| {
                        blocks::write_block(&mut self.cad, &name, contents)
                            .ok()
                            .map(|()| dimension)
                    });
                match written {
                    Some(dimension) => {
                        self.dimensions += 1;
                        (vec![dimension], "dimensions")
                    }
                    // Off the world's XY plane: the drawing alone.
                    _ => (
                        curva_document::shape::annotation_parts(a, state, object.space)
                            .map(|p| {
                                p.lines
                                    .iter()
                                    .map(|(s, e)| {
                                        EntityType::Line(Line::from_points(v3(*s), v3(*e)))
                                    })
                                    .chain(annotation::text_out(&p.text, &p.text.content))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        "dimensions",
                    ),
                }
            }
            Shape::Annotation(Annotation::Leader(l)) => {
                (dims::leader_out(l, state, object.space), "leaders")
            }
            Shape::Annotation(_) => (Vec::new(), "dots and other annotations"),
            Shape::Body(_) => (Vec::new(), "surfaces and solids"),
            Shape::Instance(instance) => {
                let name = self.block(instance.definition);
                (
                    name.and_then(|name| blocks::insert_out(instance, &name))
                        .map(EntityType::Insert)
                        .into_iter()
                        .collect(),
                    "block instances",
                )
            }
        };
        let layer = if made.is_empty() {
            String::new()
        } else {
            self.layer(object.layer)
        };
        for entity in &mut made {
            let common = entity.common_mut();
            common.layer.clone_from(&layer);
            match object.print_width {
                PrintWidth::Millimetres(width) => common.line_weight = line_weight(width),
                PrintWidth::Default => common.line_weight = LineWeight::Default,
                PrintWidth::ByLayer => {}
            }
        }
        (made, kind)
    }

    /// The block for a definition, written with its objects the first time it is needed.
    fn block(&mut self, definition: u64) -> Option<String> {
        if let Some(name) = self.blocks.get(&definition) {
            return Some(name.clone());
        }
        let held = self
            .doc
            .state()
            .blocks
            .iter()
            .find(|b| b.id == definition)?
            .clone();
        let mut name = dxf_name(&held.name);
        while self.cad.block_records.get(&name).is_some() {
            name.push('_');
        }
        // Claimed before its contents, so a definition that holds itself stops here.
        self.blocks.insert(definition, name.clone());
        let mut contents = Vec::new();
        for object in &held.objects {
            let (made, _) = self.entities(object);
            contents.extend(made);
        }
        blocks::write_block(&mut self.cad, &name, contents).ok()?;
        Some(name)
    }
}

/// The model (objects in the model, not on sheets), or the objects in `only`, written as a DXF
/// or DWG file. Block instances go out as INSERTs of blocks holding their definitions.
///
/// # Errors
///
/// When the file cannot be written.
pub fn export(
    doc: &Document,
    only: Option<&[u64]>,
    format: Format,
) -> Result<(Vec<u8>, Report), String> {
    let mut cad = CadDocument::with_version(DxfVersion::AC1015);
    cad.header.insertion_units = unit_code(doc.state().unit);
    let mut writer = Writer {
        cad,
        doc,
        fields: FieldContext::now(doc.path()),
        layers: BTreeMap::new(),
        blocks: BTreeMap::new(),
        dimensions: 0,
        report: Report::default(),
    };
    for object in &doc.state().objects {
        if only.is_some_and(|ids| !ids.contains(&object.id))
            || (only.is_none() && object.space.is_some())
        {
            continue;
        }
        let (made, kind) = writer.entities(object);
        if made.is_empty() {
            Report::count(&mut writer.report.skipped, kind);
            continue;
        }
        for entity in made {
            writer.cad.add_entity(entity).map_err(|e| e.to_string())?;
        }
        Report::count(&mut writer.report.brought, kind);
    }
    let bytes = match format {
        Format::Dxf => DxfWriter::new(&writer.cad).write_to_vec(),
        Format::Dwg => DwgWriter::write_to_vec(&writer.cad),
    }
    .map_err(|e| e.to_string())?;
    Ok((bytes, writer.report))
}

/// A DXF or DWG file read.
///
/// # Errors
///
/// When the file is not one, or is damaged.
pub fn read(bytes: &[u8], format: Format) -> Result<CadDocument, String> {
    let stream = Cursor::new(bytes.to_vec());
    match format {
        Format::Dxf => DxfReader::from_reader(stream)
            .and_then(DxfReader::read)
            .map_err(|e| e.to_string()),
        Format::Dwg => DwgReader::from_stream(stream)
            .read()
            .map_err(|e| e.to_string()),
    }
}

/// The curves a DXF entity draws, or the kind it is when left out.
fn curve_of(entity: &EntityType) -> Result<Vec<Curve>, &'static str> {
    let normal_of = |n: &Vector3| unit(p3(n)).unwrap_or([0.0, 0.0, 1.0]);
    let made = match entity {
        EntityType::Point(p) => Curve::Point {
            at: p3(&p.location),
        },
        EntityType::Line(l) => {
            return Ok(Curve::line(p3(&l.start), p3(&l.end)).into_iter().collect());
        }
        EntityType::Circle(c) => {
            let normal = normal_of(&c.normal);
            let (x, y) = object_axes(normal);
            Curve::circle_on(from_object(p3(&c.center), normal), c.radius, x, y)
                .map_err(|_| "circles")?
        }
        EntityType::Arc(a) => {
            let normal = normal_of(&a.normal);
            let (ox, oy) = object_axes(normal);
            let centre = from_object(p3(&a.center), normal);
            let turn = std::f64::consts::TAU;
            let sweep = (a.end_angle - a.start_angle).rem_euclid(turn);
            let sweep = if sweep < 1e-12 { turn } else { sweep };
            let (s, c) = a.start_angle.sin_cos();
            let x: P = std::array::from_fn(|i| ox[i] * c + oy[i] * s);
            Curve::Arc {
                centre,
                radius: a.radius,
                x,
                y: cross(normal, x),
                sweep,
            }
        }
        EntityType::LwPolyline(l) => {
            let normal = normal_of(&l.normal);
            let points: Vec<P> = l
                .vertices
                .iter()
                .map(|v| from_object([v.location.x, v.location.y, l.elevation], normal))
                .collect();
            if l.vertices.iter().any(|v| v.bulge.abs() > 1e-12) {
                let bulges: Vec<f64> = l.vertices.iter().map(|v| v.bulge).collect();
                return Ok(vec![bulged(&points, &bulges, l.is_closed, normal)?]);
            }
            Curve::polyline(points, l.is_closed).map_err(|_| "polylines")?
        }
        EntityType::Polyline3D(p) => Curve::polyline(
            p.vertices.iter().map(|v| p3(&v.position)).collect(),
            p.is_closed(),
        )
        .map_err(|_| "polylines")?,
        EntityType::Spline(s) => {
            let degree = usize::try_from(s.degree).map_err(|_| "splines")?;
            if s.control_points.is_empty() && s.fit_points.len() > 1 {
                Curve::interpolated(s.fit_points.iter().map(p3).collect(), s.flags.closed)
                    .map_err(|_| "splines")?
            } else {
                let weights = if s.flags.rational {
                    s.weights.clone()
                } else {
                    Vec::new()
                };
                Curve::nurbs(
                    degree,
                    s.knots.clone(),
                    s.control_points.iter().map(p3).collect(),
                    weights,
                )
                .map_err(|_| "splines")?
            }
        }
        EntityType::Text(_) | EntityType::MText(_) => return Err("texts"),
        EntityType::Dimension(_) => return Err("dimensions of other kinds"),
        EntityType::Hatch(_) => return Err("hatches"),
        EntityType::Insert(_) => return Err("block instances"),
        EntityType::Ellipse(_) => return Err("ellipses"),
        EntityType::Leader(_) | EntityType::MultiLeader(_) => return Err("leaders"),
        _ => return Err("other objects"),
    };
    Ok(vec![made])
}

/// A lightweight polyline with arc segments (a bulge is the tangent of a quarter of the arc's
/// angle, negative turning clockwise) as a compound of lines and arcs.
pub(crate) fn bulged(
    points: &[P],
    bulges: &[f64],
    closed: bool,
    normal: P,
) -> Result<Curve, &'static str> {
    let count = points.len();
    let spans = if closed {
        count
    } else {
        count.saturating_sub(1)
    };
    let mut segments = Vec::new();
    for i in 0..spans {
        let (a, b) = (points[i], points[(i + 1) % count]);
        let bulge = bulges[i];
        if bulge.abs() < 1e-12 {
            if let Ok(line) = Curve::line(a, b) {
                segments.push(line);
            }
            continue;
        }
        let angle = 4.0 * bulge.atan();
        let chord: P = std::array::from_fn(|k| b[k] - a[k]);
        let half = dot(chord, chord).sqrt() / 2.0;
        let radius = half / (angle / 2.0).sin().abs();
        // The centre sits off the chord's middle: to its left for a counter-clockwise arc
        // shorter than a half circle, to its right for a longer one.
        let middle: P = std::array::from_fn(|k| (a[k] + b[k]) / 2.0);
        let left = unit(cross(normal, chord)).ok_or("polylines")?;
        let offset = radius * (angle / 2.0).cos() * bulge.signum();
        let centre: P = std::array::from_fn(|k| middle[k] + left[k] * offset);
        let start = unit(std::array::from_fn(|k| a[k] - centre[k])).ok_or("polylines")?;
        let turning = if bulge > 0.0 {
            normal
        } else {
            normal.map(|c| -c)
        };
        segments.push(Curve::Arc {
            centre,
            radius,
            x: start,
            y: cross(turning, start),
            sweep: angle.abs(),
        });
    }
    Ok(Curve::Compound { segments })
}

/// What an import is reading into: the model, and the layers and blocks made so far.
struct Reader<'a> {
    cad: &'a CadDocument,
    layers: BTreeMap<String, u64>,
    blocks: BTreeMap<String, Option<u64>>,
}

impl Reader<'_> {
    /// The Curva layer for a DXF layer, made (with its colour) the first time it is needed; `$`
    /// in a name makes sublayers, as Rhino writes them.
    fn layer(&mut self, doc: &mut Document, name: &str) -> u64 {
        if let Some(id) = self.layers.get(name) {
            return *id;
        }
        let path = name.replace('$', "::");
        let id = doc.layer_at_path(if path.is_empty() { "0" } else { &path });
        if let Some(held) = self.cad.layers.get(name) {
            let colour = match held.color {
                Color::Rgb { r, g, b } => Some((r, g, b)),
                other => other.rgb(),
            };
            if let Some((r, g, b)) = colour {
                doc.set_layer_colour(id, [r, g, b].map(|c| f32::from(c) / 255.0));
            }
        }
        self.layers.insert(name.to_owned(), id);
        id
    }

    /// The Curva definition for a block, made from its entities the first time it is needed.
    fn block(&mut self, doc: &mut Document, name: &str) -> Option<u64> {
        if let Some(id) = self.blocks.get(name) {
            return *id;
        }
        // Claimed before its contents, so a block that holds itself stops here.
        self.blocks.insert(name.to_owned(), None);
        let cad = self.cad;
        let mut ids = Vec::new();
        for entity in cad.entities_in_block(name) {
            if matches!(entity, EntityType::Block(_) | EntityType::BlockEnd(_)) {
                continue;
            }
            let (shapes, _) = self.shapes(doc, entity);
            let layer = self.layer(doc, &entity.common().layer);
            for shape in shapes {
                ids.push(doc.add_shape_on(shape, Some(layer)));
            }
        }
        let (definition, instance) = doc.define_block(name.to_owned(), [0.0; 3], &ids);
        doc.remove(&[instance]);
        self.blocks.insert(name.to_owned(), Some(definition));
        Some(definition)
    }

    /// What one entity becomes, and the kind it is reported as.
    fn shapes(&mut self, doc: &mut Document, entity: &EntityType) -> (Vec<Shape>, &'static str) {
        match entity {
            EntityType::Text(text) => (
                annotation::text_in(text)
                    .map(|t| Shape::Annotation(Annotation::Text(t)))
                    .into_iter()
                    .collect(),
                "texts",
            ),
            EntityType::MText(text) => (
                annotation::mtext_in(text)
                    .map(|t| Shape::Annotation(Annotation::Text(t)))
                    .into_iter()
                    .collect(),
                "texts",
            ),
            EntityType::Hatch(hatch) => match annotation::hatch_in(hatch, doc.state()) {
                Some((made, pattern)) => {
                    if let Some(pattern) = pattern {
                        doc.add_hatch_pattern(pattern);
                    }
                    (vec![Shape::Annotation(Annotation::Hatch(made))], "hatches")
                }
                None => (Vec::new(), "hatches"),
            },
            EntityType::Dimension(d) => match dims::dimension_in(d) {
                Ok(a) => (vec![Shape::Annotation(a)], "dimensions"),
                Err(kind) => (Vec::new(), kind),
            },
            EntityType::Leader(l) => (
                dims::leader_in(l)
                    .map(|l| Shape::Annotation(Annotation::Leader(l)))
                    .into_iter()
                    .collect(),
                "leaders",
            ),
            EntityType::Insert(insert) => match self.block(doc, &insert.block_name) {
                Some(definition) if insert.column_count <= 1 && insert.row_count <= 1 => (
                    vec![Shape::Instance(curva_document::Instance {
                        definition,
                        placement: blocks::placement_in(insert),
                    })],
                    "block instances",
                ),
                _ => (Vec::new(), "block instances"),
            },
            other => match curve_of(other) {
                Ok(curves) => (curves.into_iter().map(Shape::Curve).collect(), "curves"),
                Err(kind) => (Vec::new(), kind),
            },
        }
    }
}

/// A DXF or DWG drawing's model space added to `doc`: its curves, points, text, hatches and
/// block instances (with their blocks as definitions) on layers of the same names, and the
/// model's unit taken from the file when `doc` is empty.
pub fn import(cad: &CadDocument, doc: &mut Document) -> Report {
    let mut report = Report::default();
    if doc.state().objects.is_empty()
        && let Some(unit) = unit_of(cad.header.insertion_units)
    {
        doc.set_unit(unit, false);
    }
    let mut reader = Reader {
        cad,
        layers: BTreeMap::new(),
        blocks: BTreeMap::new(),
    };
    // Only what model space owns: a file read back lists block contents among its entities too.
    let model = cad.block_records.get("*Model_Space").map(|b| b.handle);
    for entity in cad.entities().filter(|e| {
        let owner = e.common().owner_handle;
        owner.is_null() || Some(owner) == model
    }) {
        let (shapes, kind) = reader.shapes(doc, entity);
        if shapes.is_empty() {
            Report::count(&mut report.skipped, kind);
            continue;
        }
        let layer = reader.layer(doc, &entity.common().layer);
        for shape in shapes {
            doc.add_shape_on(shape, Some(layer));
            Report::count(&mut report.brought, kind);
        }
    }
    report
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
}
