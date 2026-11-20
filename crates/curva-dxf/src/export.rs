//! Writing a model to DXF or DWG.

use crate::*;

/// A layer's DXF name: its full path, with `$` between levels (DXF names cannot hold `:`), as
/// Rhino writes sublayers.
pub(crate) fn dxf_layer_name(doc: &Document, id: u64) -> String {
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
pub(crate) fn entities_for(curve: &Curve) -> Vec<EntityType> {
    match curve {
        Curve::Point { at } => vec![EntityType::Point(Point::from_coords(at[0], at[1], at[2]))],
        Curve::Line { start, end } => {
            vec![EntityType::Line(Line::from_points(v3(*start), v3(*end)))]
        }
        Curve::Polyline { points, closed } => vec![polyline_entity(points, *closed)],
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
        } => vec![arc_entity(*centre, *radius, *x, *y, *sweep)],
        Curve::Nurbs {
            degree,
            knots,
            points,
            weights,
        } => vec![nurbs_entity(curve, *degree, knots, points, weights)],
        Curve::Compound { segments } => segments.iter().flat_map(entities_for).collect(),
        Curve::Interpolated { .. } => curve
            .to_nurbs()
            .map(|nurbs| entities_for(&nurbs))
            .unwrap_or_default(),
    }
}

/// A polyline: lightweight at its height when it is flat, as Rhino writes a plan polyline, and a
/// 3D polyline otherwise.
fn polyline_entity(points: &[P], closed: bool) -> EntityType {
    let flat = points
        .first()
        .is_some_and(|first| points.iter().all(|p| (p[2] - first[2]).abs() < 1e-9));
    if flat {
        // A plan polyline, as Rhino writes one: lightweight, at its height.
        let mut polyline =
            LwPolyline::from_points(points.iter().map(|p| Vector2::new(p[0], p[1])).collect());
        polyline.elevation = points[0][2];
        polyline.is_closed = closed;
        EntityType::LwPolyline(polyline)
    } else {
        let mut polyline = Polyline3D::from_points(points.iter().map(|p| v3(*p)).collect());
        if closed {
            polyline.close();
        }
        EntityType::Polyline3D(polyline)
    }
}

/// An arc in its object coordinate system, from its start angle there.
fn arc_entity(centre: P, radius: f64, x: P, y: P, sweep: f64) -> EntityType {
    let normal = unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, oy) = object_axes(normal);
    let start = dot(x, oy).atan2(dot(x, ox));
    let mut arc = Arc::new();
    arc.center = v3(to_object(centre, normal));
    arc.radius = radius;
    arc.start_angle = start;
    arc.end_angle = start + sweep;
    arc.normal = v3(normal);
    EntityType::Arc(arc)
}

/// A NURBS curve as a spline, or as an ellipse when it is an exact one, as AutoCAD and Rhino
/// draw them.
fn nurbs_entity(
    curve: &Curve,
    degree: usize,
    knots: &[f64],
    points: &[P],
    weights: &[f64],
) -> EntityType {
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
            return EntityType::Ellipse(ellipse);
        }
    }
    let mut spline = Spline::new();
    spline.degree = i32::try_from(degree).unwrap_or(3);
    spline.knots = knots.to_vec();
    spline.control_points = points.iter().map(|p| v3(*p)).collect();
    spline.flags.rational = weights.iter().any(|w| (w - 1.0).abs() > 1e-12);
    if spline.flags.rational {
        spline.weights = weights.to_vec();
    }
    EntityType::Spline(spline)
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
pub(crate) struct Writer<'a> {
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

    /// An object's user text as Rhino's DXF writes it, registering Rhino's application name the
    /// first time.
    fn user_text_out(
        &mut self,
        user_text: &BTreeMap<String, String>,
    ) -> acadrust::xdata::ExtendedDataRecord {
        if self.cad.app_ids.get(RHINO_APP).is_none() {
            let mut app = acadrust::tables::AppId::new(RHINO_APP);
            acadrust::tables::TableEntry::set_handle(&mut app, self.cad.allocate_handle());
            let _ = self.cad.app_ids.add(app);
        }
        let mut record = acadrust::xdata::ExtendedDataRecord::new(RHINO_APP);
        for (key, value) in user_text {
            record.add_value(XDataValue::ControlString("{".to_owned()));
            record.add_value(XDataValue::String(key.clone()));
            record.add_value(XDataValue::String(value.clone()));
            record.add_value(XDataValue::ControlString("}".to_owned()));
        }
        record
    }

    /// The DXF layer for a Curva layer, made the first time it is needed.
    fn layer(&mut self, id: u64) -> String {
        if let Some(name) = self.layers.get(&id) {
            return name.clone();
        }
        let name = dxf_layer_name(self.doc, id);
        if self.cad.layers.get(&name).is_none() {
            let mut entry = Layer::new(name.clone());
            // A handle now, so a viewport can name it among its frozen layers.
            entry.handle = self.cad.allocate_handle();
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
                a @ (Annotation::Dimension(_)
                | Annotation::Radial(_)
                | Annotation::Angle(_)
                | Annotation::Ordinate(_)
                | Annotation::ArcLength(_)),
            ) => (self.dimension(a, object), "dimensions"),
            Shape::Annotation(Annotation::Leader(l)) => {
                (dims::leader_out(l, state, object.space), "leaders")
            }
            Shape::Annotation(_) => (Vec::new(), "dots and other annotations"),
            Shape::Body(_) => (Vec::new(), "surfaces and solids"),
            // Not written to DXF yet (its MESH and POLYFACE entities are for later).
            Shape::Mesh(_) => (Vec::new(), "meshes"),
            Shape::SubD(_) => (Vec::new(), "SubDs"),
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
        self.name_styles(object, &mut made);
        self.stamp(object, &mut made);
        (made, kind)
    }

    /// A dimension of any kind as a DIMENSION entity with its drawing as an anonymous block, or,
    /// off the world's XY plane, the drawing alone.
    fn dimension(&mut self, a: &Annotation, object: &curva_document::Object) -> Vec<EntityType> {
        let state = self.doc.state();
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
                vec![dimension]
            }
            // Off the world's XY plane: the drawing alone.
            _ => curva_document::shape::annotation_parts(a, state, object.space)
                .map(|p| {
                    p.lines
                        .iter()
                        .map(|(s, e)| EntityType::Line(Line::from_points(v3(*s), v3(*e))))
                        .chain(annotation::text_out(&p.text, &p.text.content))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Annotation names its style: text by text style, dimensions and leaders by dimension
    /// style, both under the Curva style's name.
    fn name_styles(&mut self, object: &curva_document::Object, made: &mut [EntityType]) {
        if let Shape::Annotation(a) = &object.shape
            && !made.is_empty()
            && !matches!(a, Annotation::Hatch(_) | Annotation::Dot(_))
        {
            let name = self.style(a.style());
            for entity in made.iter_mut() {
                match entity {
                    EntityType::Text(t) => t.style.clone_from(&name),
                    EntityType::MText(t) => t.style.clone_from(&name),
                    EntityType::Dimension(d) => dims::base_of(d).style_name.clone_from(&name),
                    EntityType::Leader(l) => l.dimension_style.clone_from(&name),
                    _ => {}
                }
            }
        }
    }

    /// The object's layer, linetype, user text, colour and print width on its entities.
    fn stamp(&mut self, object: &curva_document::Object, made: &mut [EntityType]) {
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
        // User text as Rhino's DXF writes it: XDATA under "Rhino", each key and value in braces.
        let user_text =
            (!object.user_text.is_empty()).then(|| self.user_text_out(&object.user_text));
        for entity in made.iter_mut() {
            let common = entity.common_mut();
            if let Some(record) = &user_text {
                common.extended_data.upsert_record(record.clone());
            }
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

impl Writer<'_> {
    /// One layout: a DXF layout of its paper size, with its drawing in its paper space in
    /// millimetres and its details as viewports, numbered on from `next_viewport`.
    fn layout(
        &mut self,
        index: usize,
        layout: &curva_document::layouts::Layout,
        next_viewport: &mut i16,
    ) -> Result<(), String> {
        let doc = self.doc;
        let mm = layout.unit.millimetres();
        let name = layouts::layout_out(
            &mut self.cad,
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
            let (entities, kind) =
                if let Some(detail) = layout.details.iter().find(|d| d.frame == object.id) {
                    let Some(viewport) = self.viewport(detail, &sheet, *next_viewport) else {
                        continue;
                    };
                    *next_viewport += 1;
                    (vec![viewport], "details")
                } else {
                    self.entities(&sheet)
                };
            if entities.is_empty() {
                Report::count(&mut self.report.skipped, kind);
                continue;
            }
            for entity in entities {
                self.cad
                    .add_entity_to_layout(entity, &name)
                    .map_err(|e| e.to_string())?;
            }
            Report::count(&mut self.report.brought, kind);
        }
        Ok(())
    }

    /// A detail as a viewport the size of its frame (`sheet`, in millimetres), with the layers
    /// hidden in this detail only frozen in it. `None` when the frame has no size.
    fn viewport(
        &mut self,
        detail: &curva_document::layouts::Detail,
        sheet: &curva_document::Object,
        number: i16,
    ) -> Option<EntityType> {
        let (low, high) = sheet.shape.bounds()?;
        let model_mm = self.doc.state().unit.millimetres();
        let mut viewport = layouts::viewport_out(detail, low, high, model_mm, number);
        viewport.common.layer = self.layer(sheet.layer);
        for hidden in &detail.hidden_layers {
            let name = self.layer(*hidden);
            if let Some(entry) = self.cad.layers.get(&name) {
                viewport.freeze_layer(entry.handle);
            }
        }
        Some(EntityType::Viewport(Box::new(viewport)))
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
        let mut next_viewport: i16 = 2;
        for (index, layout) in doc.state().layouts.iter().enumerate() {
            writer.layout(index, layout, &mut next_viewport)?;
        }
    }
    let bytes = match format {
        Format::Dxf => DxfWriter::new(&writer.cad).write_to_vec(),
        Format::Dwg => DwgWriter::write_to_vec(&writer.cad),
    }
    .map_err(|e| e.to_string())?;
    Ok((bytes, writer.report))
}
