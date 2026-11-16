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
mod layouts;

use acadrust::entities::{
    Arc, Circle, Ellipse, EntityType, Line, LwPolyline, Point, Polyline3D, Spline,
};
use acadrust::tables::{DimStyle, LineType, LineTypeElement, TextStyle};
use acadrust::types::{Color, DxfVersion, LineWeight, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Layer};
use curva_document::annotation::Annotation;
use curva_document::fields::{FieldContext, resolve};
use curva_document::{Document, Linetype, PrintWidth, Shape, Unit};
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

/// A colour channel from 0 to 1 as a byte.
fn byte(c: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

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
            // Exact ellipses go out as ellipses, as AutoCAD and Rhino draw them.
            if let Some(found) = curve.as_ellipse() {
                let length = |v: P| dot(v, v).sqrt();
                if let Some(normal) = unit(cross(found.major, found.minor)) {
                    let mut ellipse = Ellipse::new();
                    ellipse.center = v3(found.centre);
                    ellipse.major_axis = v3(found.major);
                    ellipse.minor_axis_ratio = length(found.minor) / length(found.major);
                    ellipse.normal = v3(normal);
                    ellipse.start_parameter = found.start;
                    ellipse.end_parameter = found.start + found.sweep;
                    return vec![EntityType::Ellipse(ellipse)];
                }
            }
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
pub(crate) fn dxf_name(name: &str) -> String {
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
    /// The DXF linetype name for one of Curva's linetypes, its pattern added to the linetype
    /// table the first time it is needed (dashes positive, gaps negative, in drawing units).
    fn linetype(&mut self, linetype: Linetype) -> String {
        if linetype.is_by_layer() {
            return "ByLayer".to_owned();
        }
        let state = self.doc.state();
        let name = state.linetype_name(linetype);
        // Curva's lengths are millimetres on paper; a DXF's are drawing units, as Rhino writes them.
        let per_mm = 1.0 / state.unit.millimetres();
        let pattern: Vec<f64> = state
            .linetype_pattern(linetype)
            .iter()
            .map(|l| l * per_mm)
            .collect();
        if pattern.is_empty() {
            return "Continuous".to_owned();
        }
        if self.cad.line_types.get(&name).is_none() {
            let mut entry = LineType::new(name.clone());
            for (k, length) in pattern.iter().enumerate() {
                entry.add_element(if k % 2 == 0 {
                    LineTypeElement::dash(*length)
                } else {
                    LineTypeElement::space(*length)
                });
            }
            entry.pattern_length = pattern.iter().sum();
            let _ = self.cad.line_types.add(entry);
        }
        name
    }

    /// The DXF text style and dimension style for a Curva annotation style, both under its
    /// name, made the first time it is needed: the font (a TrueType family, as AutoCAD names
    /// one), and the sizes in drawing units with the decimals.
    fn style(&mut self, id: u64) -> String {
        let style = self.doc.annotation_style(id);
        let name = style.name.clone();
        if self.cad.text_styles.get(&name).is_none() {
            let mut text = TextStyle::new(name.clone());
            text.handle = self.cad.allocate_handle();
            if let Some(family) = &style.font {
                text.true_type_font.clone_from(family);
                text.font_file = format!("{family}.ttf");
            }
            let _ = self.cad.text_styles.add(text);
        }
        if self.cad.dim_styles.get(&name).is_none() {
            let scale = style.scale_to(self.doc.state().unit, None);
            let mut dim = DimStyle::new(name.clone());
            dim.dimtxt = style.text_height * scale;
            dim.dimasz = style.arrow_size() * scale;
            dim.dimexo = style.extension_offset() * scale;
            dim.dimexe = style.extension_extension() * scale;
            dim.dimgap = style.text_gap() * scale;
            dim.dimdec = i16::try_from(style.linear_resolution()).unwrap_or(2);
            dim.dimtxsty.clone_from(&name);
            // AutoCAD follows the text style by handle (group 340), not by name.
            if let Some(text) = self.cad.text_styles.get(&name) {
                dim.dimtxsty_handle = text.handle;
            }
            dim.dimscale = 1.0;
            // No suffix: acadrust's "<>" means the value alone to AutoCAD, but Rhino shows it.
            dim.dimpost = String::new();
            let _ = self.cad.dim_styles.add(dim);
        }
        name
    }

    /// The DXF layer for a Curva layer, made the first time it is needed.
    fn layer(&mut self, id: u64) -> String {
        if let Some(name) = self.layers.get(&id) {
            return name.clone();
        }
        let name = dxf_layer_name(self.doc, id);
        if self.cad.layers.get(&name).is_none() {
            let mut entry = Layer::new(name.clone());
            if let Some(held) = self.doc.state().layer(id) {
                let [r, g, b] = held.colour.map(byte);
                entry.color = Color::Rgb { r, g, b };
                entry.flags.off = !held.visible;
                entry.flags.locked = held.locked;
                if let Some(width) = held.print_width {
                    entry.line_weight = line_weight(width);
                }
                entry.line_type = self.linetype(held.linetype);
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
        // Annotation names its style: text by text style, dimensions and leaders by
        // dimension style, both under the Curva style's name.
        if let Shape::Annotation(a) = &object.shape
            && !made.is_empty()
            && !matches!(a, Annotation::Hatch(_) | Annotation::Dot(_))
        {
            let name = self.style(a.style());
            for entity in &mut made {
                match entity {
                    EntityType::Text(t) => t.style.clone_from(&name),
                    EntityType::MText(t) => t.style.clone_from(&name),
                    EntityType::Dimension(d) => dims::base_of(d).style_name.clone_from(&name),
                    EntityType::Leader(l) => l.dimension_style.clone_from(&name),
                    _ => {}
                }
            }
        }
        let layer = if made.is_empty() {
            String::new()
        } else {
            self.layer(object.layer)
        };
        let linetype = if made.is_empty() || object.linetype.is_by_layer() {
            None
        } else {
            Some(self.linetype(object.linetype))
        };
        for entity in &mut made {
            let common = entity.common_mut();
            common.layer.clone_from(&layer);
            if let Some(name) = &linetype {
                common.linetype.clone_from(name);
            }
            if let Some(colour) = object.colour {
                let [r, g, b] = colour.map(byte);
                common.color = Color::Rgb { r, g, b };
            }
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
    // Layouts: each a DXF layout of its paper size, its drawing in its paper space in
    // millimetres, and its details as viewports.
    if only.is_none() {
        let model_mm = doc.state().unit.millimetres();
        let mut next_viewport: i16 = 2;
        for (index, layout) in doc.state().layouts.iter().enumerate() {
            let mm = layout.unit.millimetres();
            let name = layouts::layout_out(
                &mut writer.cad,
                &layout.name,
                index == 0,
                [layout.size[0] * mm, layout.size[1] * mm],
            );
            let to_mm = [
                [mm, 0.0, 0.0, 0.0],
                [0.0, mm, 0.0, 0.0],
                [0.0, 0.0, mm, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            for object in doc
                .state()
                .objects
                .iter()
                .filter(|o| o.space == Some(layout.id))
            {
                let mut sheet = curva_document::Object::clone(object);
                if (mm - 1.0).abs() > 1e-12
                    && let Some(shape) = curva_document::blocks::shape_placed(&sheet.shape, &to_mm)
                {
                    sheet.shape = shape;
                }
                let made =
                    if let Some(detail) = layout.details.iter().find(|d| d.frame == object.id) {
                        let Some((low, high)) = sheet.shape.bounds() else {
                            continue;
                        };
                        let mut viewport =
                            layouts::viewport_out(detail, low, high, model_mm, next_viewport);
                        next_viewport += 1;
                        viewport.common.layer = writer.layer(object.layer);
                        (vec![EntityType::Viewport(Box::new(viewport))], "details")
                    } else {
                        writer.entities(&sheet)
                    };
                let (entities, kind) = made;
                if entities.is_empty() {
                    Report::count(&mut writer.report.skipped, kind);
                    continue;
                }
                for entity in entities {
                    writer
                        .cad
                        .add_entity_to_layout(entity, &name)
                        .map_err(|e| e.to_string())?;
                }
                Report::count(&mut writer.report.brought, kind);
            }
        }
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
        // The older, heavy polylines: 2D ones lie in their normal's plane at their elevation, with
        // bulges as arcs; the others hold world points.
        EntityType::Polyline2D(l) => {
            let normal = normal_of(&l.normal);
            let points: Vec<P> = l
                .vertices
                .iter()
                .map(|v| from_object([v.location.x, v.location.y, l.elevation], normal))
                .collect();
            if l.vertices.iter().any(|v| v.bulge.abs() > 1e-12) {
                let bulges: Vec<f64> = l.vertices.iter().map(|v| v.bulge).collect();
                return Ok(vec![bulged(&points, &bulges, l.is_closed(), normal)?]);
            }
            Curve::polyline(points, l.is_closed()).map_err(|_| "polylines")?
        }
        EntityType::Polyline(p) => Curve::polyline(
            p.vertices.iter().map(|v| p3(&v.location)).collect(),
            p.is_closed(),
        )
        .map_err(|_| "polylines")?,
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
        // An ellipse's centre and major axis are world points; it turns from its start parameter
        // to its end one, anticlockwise about its normal.
        EntityType::Ellipse(e) => {
            let normal = normal_of(&e.normal);
            let major = p3(&e.major_axis);
            let minor: P = cross(normal, major).map(|v| v * e.minor_axis_ratio);
            let turn = std::f64::consts::TAU;
            let sweep = (e.end_parameter - e.start_parameter).rem_euclid(turn);
            let sweep = if sweep < 1e-12 { turn } else { sweep };
            Curve::elliptical_arc(p3(&e.center), major, minor, e.start_parameter, sweep)
                .map_err(|_| "ellipses")?
        }
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
    /// Linetypes Curva does not have, by name, and how many layers and objects used them.
    unknown_linetypes: BTreeMap<String, usize>,
}

impl Reader<'_> {
    /// The linetype for a DXF linetype name, matched ignoring case: one of Curva's own
    /// (AutoCAD's `DOT` is Curva's dotted line, `ByBlock` follows the layer), or the
    /// document's own with the file's pattern. A name the file gives no pattern for is drawn
    /// continuous and counted.
    fn linetype(&mut self, doc: &mut Document, name: &str) -> Linetype {
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("ByBlock") {
            return Linetype::ByLayer;
        }
        if name.eq_ignore_ascii_case("DOT") {
            return Linetype::Dotted;
        }
        if let Some(found) = doc.state().linetype_named(name) {
            return found;
        }
        let Some(entry) = self.cad.line_types.get(name) else {
            *self.unknown_linetypes.entry(name.to_owned()).or_default() += 1;
            return Linetype::Continuous;
        };
        let segments: Vec<(f64, bool)> = entry
            .elements
            .iter()
            .map(|e| (e.length.abs(), e.length >= 0.0))
            .collect();
        let mm = doc.state().unit.millimetres();
        let segments: Vec<(f64, bool)> = segments.iter().map(|(l, d)| (l * mm, *d)).collect();
        doc.add_linetype(name, &curva_document::alternating(&segments))
    }

    /// The Curva annotation style for a DXF style name: one of the same name kept, otherwise
    /// one made from the dimension style of that name (its text style's font, its sizes from
    /// drawing units to the style's millimetres, or inches in an imperial model, its decimals),
    /// or from a text style alone (its font and fixed height).
    fn style(&mut self, doc: &mut Document, name: &str) -> Option<u64> {
        use curva_document::annotation::AnnotationStyle;
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let dim = self.cad.dim_styles.get(name);
        // The text style of the same name, else the one the dimension style names.
        let text = self
            .cad
            .text_styles
            .get(name)
            .or_else(|| dim.and_then(|d| self.cad.text_styles.get(d.dimtxsty.as_str())));
        if dim.is_none() && text.is_none() {
            return None;
        }
        let unit = doc.state().unit;
        let imperial = matches!(unit, Unit::Feet | Unit::Inches);
        let per_unit = unit.millimetres() / if imperial { 25.4 } else { 1.0 };
        let font = text
            .map(|t| t.true_type_font.trim().to_owned())
            .filter(|f| !f.is_empty());
        let height = dim
            .map(|d| d.dimtxt)
            .or_else(|| text.map(|t| t.height))
            .filter(|h| *h > 0.0)
            .map_or(2.5, |h| h * per_unit);
        let sized = |v: f64| Some(v * per_unit);
        let style = AnnotationStyle {
            id: 0,
            name: name.to_owned(),
            font,
            text_height: height,
            imperial,
            arrow_size: dim.and_then(|d| sized(d.dimasz)),
            extension_offset: dim.and_then(|d| sized(d.dimexo)),
            extension_extension: dim.and_then(|d| sized(d.dimexe)),
            text_gap: dim.and_then(|d| sized(d.dimgap)),
            linear_resolution: dim.and_then(|d| u8::try_from(d.dimdec).ok()),
        };
        Some(doc.add_annotation_style(style))
    }

    /// An object's own colour, linetype, line weight and annotation style, as its entity gives
    /// them.
    fn properties(&mut self, doc: &mut Document, id: u64, entity: &EntityType) {
        let style_name = match entity {
            EntityType::Text(t) => Some(t.style.as_str()),
            EntityType::MText(t) => Some(t.style.as_str()),
            EntityType::Dimension(d) => Some(dims::style_name_of(d)),
            EntityType::Leader(l) => Some(l.dimension_style.as_str()),
            _ => None,
        };
        if let Some(style) = style_name.and_then(|n| self.style(doc, n)) {
            doc.set_annotation_style_of(&[id], style);
        }
        let common = entity.common();
        let linetype = self.linetype(doc, &common.linetype);
        if !linetype.is_by_layer() {
            doc.set_linetype(&[id], linetype);
        }
        let colour = match common.color {
            Color::ByLayer | Color::ByBlock => None,
            Color::Rgb { r, g, b } => Some((r, g, b)),
            other => other.rgb(),
        };
        if let Some((r, g, b)) = colour {
            doc.set_colour(&[id], Some([r, g, b].map(|c| f32::from(c) / 255.0)));
        }
        match common.line_weight {
            LineWeight::Value(v) if v >= 0 => doc.set_print_width(
                &[id],
                PrintWidth::Millimetres(PrintWidth::clamped(f64::from(v) / 100.0)),
            ),
            LineWeight::Default => doc.set_print_width(&[id], PrintWidth::Default),
            _ => {}
        }
    }

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
            if held.flags.off || held.flags.frozen {
                doc.set_layer_visible(id, false);
            }
            if held.flags.locked {
                doc.set_layer_locked(id, true);
            }
            if let LineWeight::Value(v) = held.line_weight
                && v >= 0
            {
                doc.set_layer_print_width(id, Some(f64::from(v) / 100.0));
            }
            let line_type = held.line_type.clone();
            let linetype = self.linetype(doc, &line_type);
            if !linetype.is_by_layer() {
                doc.set_layer_linetype(id, linetype);
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
                let id = doc.add_shape_on(shape, Some(layer));
                self.properties(doc, id, entity);
                ids.push(id);
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
        unknown_linetypes: BTreeMap::new(),
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
            let id = doc.add_shape_on(shape, Some(layer));
            reader.properties(doc, id, entity);
            Report::count(&mut report.brought, kind);
        }
    }
    // Layouts: each with its paper space's drawing on its sheet, and its viewports as details.
    let model_mm = doc.state().unit.millimetres();
    let open = doc.space();
    for sheet in layouts::layouts_in(cad) {
        let id = doc.add_layout_sized(
            sheet.name,
            sheet.size,
            curva_document::layouts::PaperUnit::Millimetres,
        );
        doc.set_space(Some(id));
        let [ox, oy] = sheet.offset;
        let shifted = |shape: Shape| {
            if ox == 0.0 && oy == 0.0 {
                return shape;
            }
            let by = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ox, oy, 0.0, 1.0],
            ];
            curva_document::blocks::shape_placed(&shape, &by).unwrap_or(shape)
        };
        // The first viewport of a paper space is its own, the size of the sheet; the others
        // are its details (AutoCAD DXF Reference, "VIEWPORT").
        let mut own_seen = false;
        for entity in cad.entities_in_block(&sheet.block) {
            if matches!(entity, EntityType::Block(_) | EntityType::BlockEnd(_)) {
                continue;
            }
            let layer = reader.layer(doc, &entity.common().layer);
            if let Some(viewport) = layouts::is_viewport(entity) {
                if !own_seen {
                    own_seen = true;
                    continue;
                }
                if let Some((frame, mut detail)) = layouts::viewport_in(viewport, model_mm) {
                    detail.frame = doc.add_shape_on(shifted(Shape::Curve(frame)), Some(layer));
                    doc.add_detail(id, detail);
                    Report::count(&mut report.brought, "details");
                }
                continue;
            }
            let (shapes, kind) = reader.shapes(doc, entity);
            if shapes.is_empty() {
                Report::count(&mut report.skipped, kind);
                continue;
            }
            for shape in shapes {
                let id = doc.add_shape_on(shifted(shape), Some(layer));
                reader.properties(doc, id, entity);
                Report::count(&mut report.brought, kind);
            }
        }
    }
    doc.set_space(open);
    for (name, uses) in reader.unknown_linetypes {
        report.skipped.push((
            format!("linetype \"{name}\", which Curva does not have (drawn continuous)"),
            uses,
        ));
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
