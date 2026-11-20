//! Reading DXF or DWG into a model.

use crate::*;

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
pub(crate) fn curve_of(entity: &EntityType) -> Result<Vec<Curve>, &'static str> {
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
        EntityType::Arc(a) => arc_of(a),
        EntityType::LwPolyline(l) => return lw_polyline_of(l),
        EntityType::Polyline2D(l) => return polyline_2d_of(l),
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
        EntityType::Spline(s) => spline_of(s)?,
        EntityType::Text(_) | EntityType::MText(_) => return Err("texts"),
        EntityType::Dimension(_) => return Err("dimensions of other kinds"),
        EntityType::Hatch(_) => return Err("hatches"),
        EntityType::Insert(_) => return Err("block instances"),
        EntityType::Ellipse(e) => ellipse_of(e)?,
        EntityType::Leader(_) => return Err("leaders"),
        EntityType::MultiLeader(_) => return Err("multileaders"),
        _ => return Err("other objects"),
    };
    Ok(vec![made])
}

/// An entity's normal as a unit vector, or the world's Z when it has none.
fn normal_of(n: &Vector3) -> P {
    unit(p3(n)).unwrap_or([0.0, 0.0, 1.0])
}

/// An arc: centred in its object coordinates, turning anticlockwise from its start angle.
fn arc_of(a: &Arc) -> Curve {
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

/// A lightweight polyline, lying in its normal's plane at its elevation, with bulges as arcs.
fn lw_polyline_of(l: &LwPolyline) -> Result<Vec<Curve>, &'static str> {
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
    Ok(vec![
        Curve::polyline(points, l.is_closed).map_err(|_| "polylines")?,
    ])
}

/// The older, heavy polylines: 2D ones lie in their normal's plane at their elevation, with
/// bulges as arcs; the others hold world points.
fn polyline_2d_of(l: &acadrust::entities::Polyline2D) -> Result<Vec<Curve>, &'static str> {
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
    Ok(vec![
        Curve::polyline(points, l.is_closed()).map_err(|_| "polylines")?,
    ])
}

/// A spline: fitted through its points when it has only those, otherwise from its control
/// points, knots and weights.
fn spline_of(s: &Spline) -> Result<Curve, &'static str> {
    let degree = usize::try_from(s.degree).map_err(|_| "splines")?;
    if s.control_points.is_empty() && s.fit_points.len() > 1 {
        // AutoCAD fits through the points with the spline's end tangents (zero when
        // unset) and its knot parameterization: 0 chord, 1 square root, 2 uniform.
        let spacing = match s.knot_parameterization {
            1 => Spacing::SquareRoot,
            2 => Spacing::Uniform,
            _ => Spacing::Chord,
        };
        Curve::fitted(
            s.fit_points.iter().map(p3).collect(),
            s.flags.closed,
            Some(p3(&s.begin_tangent)),
            Some(p3(&s.end_tangent)),
            spacing,
        )
        .map_err(|_| "splines")
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
        .map_err(|_| "splines")
    }
}

/// An ellipse's centre and major axis are world points; it turns from its start parameter
/// to its end one, anticlockwise about its normal.
fn ellipse_of(e: &Ellipse) -> Result<Curve, &'static str> {
    let normal = normal_of(&e.normal);
    let major = p3(&e.major_axis);
    let minor: P = cross(normal, major).map(|v| v * e.minor_axis_ratio);
    let turn = std::f64::consts::TAU;
    let sweep = (e.end_parameter - e.start_parameter).rem_euclid(turn);
    let sweep = if sweep < 1e-12 { turn } else { sweep };
    Curve::elliptical_arc(p3(&e.center), major, minor, e.start_parameter, sweep)
        .map_err(|_| "ellipses")
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
pub(crate) struct Reader<'a> {
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
    /// A block instance's attributes as notes where they show, each on its own layer, as Rhino
    /// brings them in beside the instance.
    fn attribute_notes(&mut self, doc: &mut Document, entity: &EntityType) -> Vec<(Shape, u64)> {
        let EntityType::Insert(insert) = entity else {
            return Vec::new();
        };
        insert
            .attributes
            .iter()
            .filter_map(|a| {
                let note = annotation::attribute_text(a)?;
                Some((
                    Shape::Annotation(Annotation::Text(note)),
                    self.layer(doc, &a.common.layer),
                ))
            })
            .collect()
    }

    fn properties(&mut self, doc: &mut Document, id: u64, entity: &EntityType) {
        // User text: Rhino's, kept as XDATA; and a block instance's attributes, by tag, as Rhino
        // reads them.
        if let Some(record) = entity.common().extended_data.get_record(RHINO_APP) {
            for (key, value) in user_text_in(&record.values) {
                doc.set_user_text(&[id], &key, &value);
            }
        }
        if let EntityType::Insert(insert) = entity {
            for attribute in &insert.attributes {
                if !attribute.tag.trim().is_empty() {
                    doc.set_user_text(&[id], attribute.tag.trim(), &attribute.value);
                }
            }
        }
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

    /// A multileader's leaders, and its block content as an instance of the block where the
    /// MLEADER puts it.
    fn multileader(
        &mut self,
        doc: &mut Document,
        m: &acadrust::entities::MultiLeader,
    ) -> Vec<Shape> {
        let mut made: Vec<Shape> = dims::multileader_in(m)
            .into_iter()
            .map(|l| Shape::Annotation(Annotation::Leader(l)))
            .collect();
        // Block content: an instance of the block where the MLEADER puts it.
        let c = &m.context;
        let block = c.block_content_handle.or(m.block_content_handle);
        if c.has_block_contents
            && let Some(name) = block.and_then(|h| {
                self.cad
                    .block_records
                    .iter()
                    .find(|b| b.handle == h)
                    .map(|b| b.name.clone())
            })
            && let Some(definition) = self.block(doc, &name)
        {
            let scale = p3(&c.block_content_scale);
            let scale = scale.map(|s| if s == 0.0 { 1.0 } else { s });
            made.push(Shape::Instance(curva_document::Instance {
                definition,
                placement: blocks::placement(
                    p3(&c.block_content_normal),
                    c.block_rotation,
                    scale,
                    p3(&c.block_content_location),
                ),
            }));
        }
        made
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
            EntityType::Dimension(d) => {
                (vec![Shape::Annotation(dims::dimension_in(d))], "dimensions")
            }
            EntityType::Leader(l) => (
                dims::leader_in(l)
                    .map(|l| Shape::Annotation(Annotation::Leader(l)))
                    .into_iter()
                    .collect(),
                "leaders",
            ),
            EntityType::MultiLeader(m) => (self.multileader(doc, m), "leaders"),
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
    reader.model_space(doc, &mut report);
    // Layouts: each with its paper space's drawing on its sheet, and its viewports as details.
    let open = doc.space();
    for sheet in layouts::layouts_in(cad) {
        reader.sheet(doc, &sheet, &mut report);
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

/// A shape moved by a sheet's offset, when it has one.
fn shifted(shape: Shape, [ox, oy]: [f64; 2]) -> Shape {
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
}

impl Reader<'_> {
    /// What model space owns, added to the model: a file read back lists block contents among
    /// its entities too.
    fn model_space(&mut self, doc: &mut Document, report: &mut Report) {
        let model = self.cad.block_records.get("*Model_Space").map(|b| b.handle);
        let cad = self.cad;
        for entity in cad.entities().filter(|e| {
            let owner = e.common().owner_handle;
            owner.is_null() || Some(owner) == model
        }) {
            self.entity(doc, entity, None, [0.0, 0.0], report);
        }
    }

    /// One entity's shapes and attribute notes added on `layer` (or its own layer, made only when
    /// it has shapes), moved by `offset`, and counted.
    fn entity(
        &mut self,
        doc: &mut Document,
        entity: &EntityType,
        layer: Option<u64>,
        offset: [f64; 2],
        report: &mut Report,
    ) {
        let (shapes, kind) = self.shapes(doc, entity);
        if shapes.is_empty() {
            Report::count(&mut report.skipped, kind);
            return;
        }
        let layer = layer.unwrap_or_else(|| self.layer(doc, &entity.common().layer));
        for shape in shapes {
            let id = doc.add_shape_on(shifted(shape, offset), Some(layer));
            self.properties(doc, id, entity);
            Report::count(&mut report.brought, kind);
        }
        for (note, on) in self.attribute_notes(doc, entity) {
            doc.add_shape_on(shifted(note, offset), Some(on));
            Report::count(&mut report.brought, "texts");
        }
    }

    /// A layout: its sheet, its paper space's drawing on it, and its viewports as details. The
    /// first viewport of a paper space is its own, the size of the sheet; the others are its
    /// details (AutoCAD DXF Reference, "VIEWPORT").
    fn sheet(&mut self, doc: &mut Document, sheet: &layouts::LayoutIn, report: &mut Report) {
        let model_mm = doc.state().unit.millimetres();
        let id = doc.add_layout_sized(
            sheet.name.clone(),
            sheet.size,
            curva_document::layouts::PaperUnit::Millimetres,
        );
        doc.set_space(Some(id));
        let mut own_seen = false;
        let cad = self.cad;
        for entity in cad.entities_in_block(&sheet.block) {
            if matches!(entity, EntityType::Block(_) | EntityType::BlockEnd(_)) {
                continue;
            }
            let layer = self.layer(doc, &entity.common().layer);
            if let Some(viewport) = layouts::is_viewport(entity) {
                if !own_seen {
                    own_seen = true;
                    continue;
                }
                if let Some((frame, mut detail)) = layouts::viewport_in(viewport, model_mm) {
                    // The layers frozen in this viewport only are hidden in the detail.
                    for handle in &viewport.frozen_layers {
                        let name = cad
                            .layers
                            .iter()
                            .find(|l| l.handle == *handle)
                            .map(|l| l.name.clone());
                        if let Some(name) = name {
                            detail.hidden_layers.push(self.layer(doc, &name));
                        }
                    }
                    detail.frame =
                        doc.add_shape_on(shifted(Shape::Curve(frame), sheet.offset), Some(layer));
                    doc.add_detail(id, detail);
                    Report::count(&mut report.brought, "details");
                }
                continue;
            }
            self.entity(doc, entity, Some(layer), sheet.offset, report);
        }
    }
}
