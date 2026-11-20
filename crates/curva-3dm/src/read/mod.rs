//! Reading a `.3dm` file into a [`Model`].

use crate::archive::{Reader, Result, Uuid, bad, tcode};
use crate::model::{
    AngularDimension, Attributes, Block, Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, BrepVertex,
    ClippingPlane, Curve, Detail, DimStyle, Extrusion, Geometry, Group, Hatch, HatchLine,
    HatchPattern, Layer, Leader, LinearDimension, Linetype, Model, NamedCPlane, NurbsCurve,
    NurbsSurface, Object, OrdinateDimension, P3, Page, Plane, RadialDimension, Source, Surface,
    Text, View,
};

mod annotation;
mod geometry;
mod mesh;
mod subd;
mod tables;
mod views;
use annotation::*;
use geometry::*;
use tables::*;
#[allow(clippy::wildcard_imports)]
use views::*;

/// Class ids of the objects Curva reads and writes (openNURBS `ON_OBJECT_IMPLEMENT`).
#[allow(missing_docs)]
pub mod class {
    use crate::archive::Uuid;
    pub const POINT: Uuid = Uuid::parse("C3101A1D-F157-11d3-BFE7-0010830122F0");
    pub const LINE_CURVE: Uuid = Uuid::parse("4ED7D4DB-E947-11d3-BFE5-0010830122F0");
    pub const POLYLINE_CURVE: Uuid = Uuid::parse("4ED7D4E6-E947-11d3-BFE5-0010830122F0");
    pub const ARC_CURVE: Uuid = Uuid::parse("CF33BE2A-09B4-11d4-BFFB-0010830122F0");
    pub const NURBS_CURVE: Uuid = Uuid::parse("4ED7D4DD-E947-11d3-BFE5-0010830122F0");
    pub const POLY_CURVE: Uuid = Uuid::parse("4ED7D4E0-E947-11d3-BFE5-0010830122F0");
    pub const NURBS_SURFACE: Uuid = Uuid::parse("4ED7D4DE-E947-11d3-BFE5-0010830122F0");
    pub const PLANE_SURFACE: Uuid = Uuid::parse("4ED7D4DF-E947-11d3-BFE5-0010830122F0");
    pub const REV_SURFACE: Uuid = Uuid::parse("A16220D3-163B-11d4-8000-0010830122F0");
    pub const SUM_SURFACE: Uuid = Uuid::parse("C4CD5359-446D-4690-9FF5-29059732472B");
    pub const BREP: Uuid = Uuid::parse("60B5DBC5-E660-11d3-BFE4-0010830122F0");
    pub const EXTRUSION: Uuid = Uuid::parse("36F53175-72B8-4d47-BF1F-B4E6FC24F4B9");
    pub const INSTANCE_REF: Uuid = Uuid::parse("F9CFB638-B9D4-4340-87E3-C56E7865D96A");
    pub const INSTANCE_DEFINITION: Uuid = Uuid::parse("26F8BFF6-2618-417f-A158-153D64A94989");
    pub const LAYER: Uuid = Uuid::parse("95809813-E985-11d3-BFE5-0010830122F0");
    pub const GROUP: Uuid = Uuid::parse("721D9F97-3645-44c4-8BE6-B2CF697D25CE");
    pub const ATTRIBUTES: Uuid = Uuid::parse("A828C015-09F5-477c-8665-F0482F5D6996");
    pub const MESH: Uuid = Uuid::parse("4ED7D4E4-E947-11d3-BFE5-0010830122F0");
    pub const POINT_CLOUD: Uuid = Uuid::parse("2488F347-F8FA-11d3-BFEC-0010830122F0");
    pub const SUBD: Uuid = Uuid::parse("F09BA4D9-455B-42C3-BA3B-E6CCACEF853B");
    pub const USER_STRING_LIST: Uuid = Uuid::parse("CE28DE29-F4C5-4faa-A50A-C3A6849B6329");
    pub const DOCUMENT_USER_STRINGS: Uuid = Uuid::parse("06F3218E-F5EC-4f6c-B74C-14583F0ED7BC");
    pub const TEXT_DOT: Uuid = Uuid::parse("74198302-CDF4-4f95-9609-6D684F22AB37");
    pub const HATCH: Uuid = Uuid::parse("0559733B-5332-49d1-A936-0532AC76ADE5");
    pub const HATCH_PATTERN: Uuid = Uuid::parse("064E7C91-35F6-4734-A446-79FF7CD659E1");
    pub const TEXT: Uuid = Uuid::parse("57376349-62A9-4A16-B411-A46BCD544790");
    pub const DIM_LINEAR: Uuid = Uuid::parse("E550882B-F44D-4154-A1EF-6E50CBBBF543");
    pub const DIM_RADIAL: Uuid = Uuid::parse("FC749C2F-4C00-41FD-9840-26D94F047AD3");
    pub const DIM_ANGULAR: Uuid = Uuid::parse("D417786B-F6CD-4F12-9E1F-063F414DBEB6");
    pub const DIM_ORDINATE: Uuid = Uuid::parse("03124828-4C9B-4D28-9A82-664DDDE7A14F");
    pub const LEADER: Uuid = Uuid::parse("945BF594-6FF9-4F5C-BFC0-B3AF528F29D2");
    pub const DETAIL: Uuid = Uuid::parse("C8C66EFA-B3CB-4e00-9440-2AD66203379E");
    pub const CLIPPING_PLANE_SURFACE: Uuid = Uuid::parse("DBC5A584-CE3F-4170-98A8-497069CA5C36");
    pub const DIM_STYLE: Uuid = Uuid::parse("67AA51A5-791D-4BEC-8AED-D23B462B6F87");
    /// Rhino 5's annotation style (`ON_V5x_DimStyle`).
    pub const V5_DIM_STYLE: Uuid = Uuid::parse("81BD83D5-7120-41c4-9A57-C449336FF12C");
    pub const LINETYPE: Uuid = Uuid::parse("26F10A24-7D13-4F05-8FDA-8E364DAF8EA6");
    // Rhino 5's annotation (openNURBS' `ON_OBSOLETE_V5_*`, Rhino 5's `ON_TextEntity2` and kin).
    pub const V5_DIM_LINEAR: Uuid = Uuid::parse("BD57F33B-A1B2-46e9-9C6E-AF09D30FFDDE");
    pub const V5_DIM_RADIAL: Uuid = Uuid::parse("B2B683FC-7964-4e96-B1F9-9B356A76B08B");
    pub const V5_DIM_ANGULAR: Uuid = Uuid::parse("841BC40B-A971-4a8e-94E5-BBA26D67348E");
    pub const V5_TEXT: Uuid = Uuid::parse("46F75541-F46B-48be-AA7E-B353BBE068A7");
    pub const V5_LEADER: Uuid = Uuid::parse("14922B7A-5B65-4f11-8345-D415A9637129");
    /// The class an older id stands for, as openNURBS reads Rhino's older classes as today's.
    #[must_use]
    pub fn current(id: Uuid) -> Uuid {
        const NURBS_CURVES: [Uuid; 2] = [
            Uuid::parse("5EAF1119-0B51-11d4-BFFE-0010830122F0"),
            Uuid::parse("76A709D5-1550-11d4-8000-0010830122F0"),
        ];
        const NURBS_SURFACES: [Uuid; 2] = [
            Uuid::parse("4760C817-0BE3-11d4-BFFE-0010830122F0"),
            Uuid::parse("FA4FD4B5-1613-11d4-8000-0010830122F0"),
        ];
        if NURBS_CURVES.contains(&id) {
            NURBS_CURVE
        } else if NURBS_SURFACES.contains(&id) {
            NURBS_SURFACE
        } else if id == Uuid::parse("EF638317-154B-11d4-8000-0010830122F0") {
            POLY_CURVE
        } else if id == Uuid::parse("F06FC243-A32A-4608-9DD8-A7D2C4CE2A36") {
            BREP
        } else if id == Uuid::parse("0A8401B6-4D34-4b99-8615-1B4E723DC4E5") {
            REV_SURFACE
        } else if id == Uuid::parse("665F6331-2A66-4cce-81D0-B5EEBD9B5417") {
            SUM_SURFACE
        } else {
            id
        }
    }

    /// openNURBS' own application id, which owns user strings.
    pub const OPENNURBS4: Uuid = Uuid::parse("17b3ecda-17ba-4e45-9e67-a2b8d9be520d");
    pub const OPENNURBS5: Uuid = Uuid::parse("c8cda597-d957-4625-a4b3-a0b510fc30d4");
    pub const LAYER_EXTENSIONS: Uuid = Uuid::parse("3E4904E6-E930-4fbc-AA42-EBD407AEFE3B");
}

/// Reads a whole file.
///
/// # Errors
/// If it is not a `.3dm` file or is damaged where it matters.
pub fn read(data: &[u8]) -> Result<Model> {
    let (version, mut file) = crate::start(data)?;
    if version < 50 && version != 4 && version != 5 {
        return Err(bad(format!(
            "a Rhino {version} file: Curva reads files from Rhino 4 on"
        )));
    }
    let mut model = Model {
        version,
        unit: 2,
        tolerance: 0.001,
        angle_tolerance: 1f64.to_radians(),
        ..Model::default()
    };
    // Rhino 5's fonts and styles by their index in their tables.
    let mut v5_fonts: Vec<(i32, String)> = Vec::new();
    let mut v5_styles: Vec<(i32, Uuid)> = Vec::new();
    while !file.is_done() {
        let mut chunk = file.chunk()?;
        match chunk.tcode {
            tcode::SETTINGS_TABLE => settings(&mut chunk.body, &mut model)?,
            tcode::PROPERTIES_TABLE => {
                if let Ok(Some(found)) = notes(&mut chunk.body) {
                    model.notes = found;
                }
            }
            // A user table Curva cannot read is left out, not the file.
            tcode::USER_TABLE => {
                if let Ok(Some(pairs)) = user_table(&mut chunk.body) {
                    model.document_text.extend(pairs);
                }
            }
            tcode::LAYER_TABLE
            | tcode::GROUP_TABLE
            | tcode::HATCHPATTERN_TABLE
            | tcode::LINETYPE_TABLE
            | tcode::INSTANCE_DEFINITION_TABLE => component_table(&mut chunk, &mut model)?,
            // Rhino 5's fonts, which its styles name by index; later files keep a style's font
            // in the style.
            tcode::FONT_TABLE if version < 60 => {
                records(&mut chunk.body, tcode::FONT_RECORD, |body| {
                    if let Ok(Some(font)) = v5_font(body) {
                        v5_fonts.push(font);
                    }
                    Ok(())
                })?;
            }
            tcode::DIMSTYLE_TABLE => {
                dim_styles_in(&mut chunk.body, &mut model, &v5_fonts, &mut v5_styles)?;
            }
            tcode::OBJECT_TABLE => objects_in(&mut chunk.body, &mut model, &v5_styles)?,
            tcode::ENDOFFILE => break,
            _ => {}
        }
    }
    Ok(model)
}

/// One of the tables of components (layers, groups, hatch patterns, linetypes, block
/// definitions) into the model.
fn component_table(chunk: &mut crate::archive::Chunk<'_>, model: &mut Model) -> Result<()> {
    match chunk.tcode {
        tcode::LAYER_TABLE => records(&mut chunk.body, tcode::LAYER_RECORD, |body| {
            if let Some(layer) = layer(body)? {
                model.layers.push(layer);
            }
            Ok(())
        })?,
        tcode::GROUP_TABLE => records(&mut chunk.body, tcode::GROUP_RECORD, |body| {
            if let Some(group) = group(body)? {
                model.groups.push(group);
            }
            Ok(())
        })?,
        tcode::HATCHPATTERN_TABLE => {
            records(&mut chunk.body, tcode::HATCHPATTERN_RECORD, |body| {
                if let Some(pattern) = hatch_pattern(body)? {
                    model.hatch_patterns.push(pattern);
                }
                Ok(())
            })?;
        }
        tcode::LINETYPE_TABLE => {
            records(&mut chunk.body, tcode::LINETYPE_RECORD, |body| {
                if let Some(linetype) = linetype(body)? {
                    model.linetypes.push(linetype);
                }
                Ok(())
            })?;
        }
        tcode::INSTANCE_DEFINITION_TABLE => {
            records(&mut chunk.body, tcode::INSTANCE_DEFINITION_RECORD, |body| {
                if let Some(block) = block(body)? {
                    model.blocks.push(block);
                }
                Ok(())
            })?;
        }
        _ => {}
    }
    Ok(())
}

/// The annotation style table into the model; a Rhino 5 style's font by its index in
/// `v5_fonts`, and its index kept in `v5_styles` for the annotation naming it.
fn dim_styles_in(
    body: &mut Reader<'_>,
    model: &mut Model,
    v5_fonts: &[(i32, String)],
    v5_styles: &mut Vec<(i32, Uuid)>,
) -> Result<()> {
    records(body, tcode::DIMSTYLE_RECORD, |body| {
        match dim_style(body)? {
            Some((mut style, Some((index, font)))) => {
                style.font = v5_fonts
                    .iter()
                    .find(|(i, _)| *i == font)
                    .map(|(_, name)| name.clone());
                v5_styles.push((index, style.id));
                model.dim_styles.push(style);
            }
            Some((style, None)) => model.dim_styles.push(style),
            None => {}
        }
        Ok(())
    })
}

/// The object table into the model: a damaged object is skipped, not the file; Rhino 5's
/// annotation names its style by index in `v5_styles`.
fn objects_in(body: &mut Reader<'_>, model: &mut Model, v5_styles: &[(i32, Uuid)]) -> Result<()> {
    let mut objects = Vec::new();
    let mut skipped = Vec::new();
    records(body, tcode::OBJECT_RECORD, |body| {
        match object(body) {
            Ok(Ok(found)) => objects.push(found),
            Ok(Err(kind)) => skipped.push(kind),
            // A damaged object is skipped, not the file.
            Err(e) => skipped.push(format!("damaged object: {e}")),
        }
        Ok(())
    })?;
    model.objects = objects;
    if model.version < 60 {
        for object in &mut model.objects {
            name_v5_style(&mut object.geometry, v5_styles);
        }
    }
    for kind in skipped {
        model.skip(&kind);
    }
    Ok(())
}

/// Each record of a table, by `what`, up to the table's end.
fn records<'a>(
    table: &mut Reader<'a>,
    what: u32,
    mut each: impl FnMut(&mut Reader<'a>) -> Result<()>,
) -> Result<()> {
    while !table.is_done() {
        let mut chunk = table.chunk()?;
        if chunk.tcode == tcode::ENDOFTABLE {
            break;
        }
        if chunk.tcode == what {
            each(&mut chunk.body)?;
        }
    }
    Ok(())
}

fn settings(table: &mut Reader<'_>, model: &mut Model) -> Result<()> {
    while !table.is_done() {
        let mut chunk = table.chunk()?;
        match chunk.tcode {
            tcode::ENDOFTABLE => break,
            tcode::SETTINGS_UNITSANDTOLS => {
                let body = &mut chunk.body;
                let version = body.i32()?;
                if (100..200).contains(&version) {
                    model.unit = body.u32()?;
                    model.tolerance = body.f64()?;
                    model.angle_tolerance = body.f64()?;
                }
            }
            tcode::SETTINGS_ATTRIBUTES => {
                if let Ok(unit) = page_unit(&mut chunk.body) {
                    model.page_unit = unit;
                }
            }
            // A damaged list loses its views or planes, not the file.
            tcode::SETTINGS_NAMED_CPLANE_LIST => {
                if let Ok(planes) = named_cplanes(&mut chunk.body) {
                    model.cplanes = planes;
                }
            }
            tcode::SETTINGS_VIEW_LIST => {
                if let Ok(views) = model_views(&mut chunk.body.clone()) {
                    model.model_views = views;
                }
                if let Ok(found) = pages(&mut chunk.body) {
                    model.pages = found;
                }
            }
            tcode::SETTINGS_NAMED_VIEW_LIST => {
                if let Ok(views) = named_views(&mut chunk.body) {
                    model.views = views;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// An object written with openNURBS' `WriteObject`: its class id and the reader of its data.
pub(crate) fn class_object<'a>(reader: &mut Reader<'a>) -> Result<(Uuid, Reader<'a>)> {
    let mut chunk = reader.expect(tcode::OPENNURBS_CLASS)?;
    let mut id_chunk = chunk.body.expect(tcode::OPENNURBS_CLASS_UUID)?;
    let id = class::current(id_chunk.body.uuid()?);
    let data = chunk.body.expect(tcode::OPENNURBS_CLASS_DATA)?;
    Ok((id, data.body))
}

fn point(r: &mut Reader<'_>) -> Result<P3> {
    r.f64s::<3>()
}

fn interval(r: &mut Reader<'_>) -> Result<[f64; 2]> {
    r.f64s::<2>()
}

fn plane(r: &mut Reader<'_>) -> Result<Plane> {
    let origin = point(r)?;
    let x = point(r)?;
    let y = point(r)?;
    let z = point(r)?;
    let _equation = r.f64s::<4>()?;
    Ok(Plane { origin, x, y, z })
}

fn bounding_box(r: &mut Reader<'_>) -> Result<()> {
    r.skip(48)
}

/// An object written with `WriteObject`: its class id, the reader of its data, and the reader
/// of what follows (its user data).
fn class_object_rest<'a>(reader: &mut Reader<'a>) -> Result<(Uuid, Reader<'a>, Reader<'a>)> {
    let mut chunk = reader.expect(tcode::OPENNURBS_CLASS)?;
    let mut id_chunk = chunk.body.expect(tcode::OPENNURBS_CLASS_UUID)?;
    let id = class::current(id_chunk.body.uuid()?);
    let data = chunk.body.expect(tcode::OPENNURBS_CLASS_DATA)?;
    Ok((id, data.body, chunk.body))
}

/// An object record: its geometry and attributes, or what kind of thing was skipped.
fn object(record: &mut Reader<'_>) -> Result<std::result::Result<Object, String>> {
    let mut geometry = None;
    let mut skipped = None;
    let mut attributes = Attributes::default();
    while !record.is_done() {
        let code = record.peek_tcode();
        match code {
            Some(tcode::OPENNURBS_CLASS) => {
                let (id, mut data) = class_object(record)?;
                match geometry_of(id, &mut data)? {
                    Ok(g) => geometry = Some(g),
                    Err(kind) => skipped = Some(kind),
                }
            }
            Some(tcode::OBJECT_RECORD_ATTRIBUTES) => {
                let mut chunk = record.chunk()?;
                attributes = read_attributes(&mut chunk.body)?;
            }
            Some(tcode::OBJECT_RECORD_ATTRIBUTES_USERDATA) => {
                let mut chunk = record.chunk()?;
                attributes.user_text = user_text(&mut chunk.body);
            }
            Some(tcode::OBJECT_RECORD_END) => break,
            _ => {
                record.chunk()?;
            }
        }
    }
    Ok(match (geometry, skipped) {
        (Some(geometry), _) => Ok(Object {
            geometry,
            attributes,
        }),
        (None, Some(kind)) => Err(kind),
        (None, None) => Err("empty object".to_owned()),
    })
}

/// The user text among an object's user data, if any.
fn user_text(r: &mut Reader<'_>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while !r.is_done() {
        let Ok(mut chunk) = r.chunk() else {
            break;
        };
        if chunk.tcode != tcode::OPENNURBS_CLASS_USERDATA {
            continue;
        }
        let read = |body: &mut Reader<'_>| -> Result<Vec<(String, String)>> {
            let _version = body.chunk_version()?;
            let mut header = body.expect(tcode::OPENNURBS_CLASS_USERDATA_HEADER)?;
            let class = header.body.uuid()?;
            if class != class::USER_STRING_LIST {
                return Ok(Vec::new());
            }
            let mut data = body.expect(tcode::ANONYMOUS_CHUNK)?;
            let (_, mut list) = data.body.anonymous()?;
            let n = list.count(8)?;
            let mut pairs = Vec::with_capacity(n);
            for _ in 0..n {
                let (_, mut entry) = list.anonymous()?;
                let key = entry.wstring()?;
                let value = entry.wstring()?;
                pairs.push((key, value));
            }
            Ok(pairs)
        };
        if let Ok(pairs) = read(&mut chunk.body) {
            out.extend(pairs);
        }
    }
    out
}

fn source(byte: u8) -> Source {
    match byte {
        1 => Source::Object,
        3 => Source::Parent,
        _ => Source::Layer,
    }
}

/// Object attributes, as far as Curva uses them: openNURBS writes them as numbered items in
/// order, so reading stops at the first item Curva does not need to go past.
fn read_attributes(r: &mut Reader<'_>) -> Result<Attributes> {
    let mut a = Attributes::default();
    let (major, _) = r.chunk_version()?;
    if major != 2 {
        // Rhino 3 attributes: id, layer, material, colour, and more Curva does not need.
        a.id = r.uuid()?;
        a.layer = usize::try_from(r.i32()?).unwrap_or(0);
        return Ok(a);
    }
    a.id = r.uuid()?;
    a.layer = usize::try_from(r.i32()?).unwrap_or(0);
    loop {
        let item = r.u8()?;
        match item {
            0 => break,
            1 => a.name = r.wstring()?,
            2 => {
                r.wstring()?;
            }
            3 => a.linetype = r.i32()?,
            4 | 10 => r.skip(4)?,
            6 => {
                let c = r.color()?;
                a.color = [c[0], c[1], c[2]];
            }
            // Rendering attributes, in a chunk of their own.
            5 => {
                r.chunk()?;
            }
            7 => r.skip(4)?,
            8 => a.print_width = r.f64()?,
            9 | 14 | 16 => r.skip(1)?,
            17 => a.linetype_source = source(r.u8()?),
            11 => a.visible = r.bool()?,
            12 => a.mode = r.u8()? & 0x0F,
            13 => a.color_source = source(r.u8()?),
            15 => a.print_width_source = source(r.u8()?),
            18 => {
                let n = r.count(4)?;
                for _ in 0..n {
                    if let Ok(g) = usize::try_from(r.i32()?) {
                        a.groups.push(g);
                    }
                }
            }
            // openNURBS' active space: 1 the model, 2 a page.
            19 => a.page_space = r.u8()? == 2,
            20 => a.viewport = r.uuid()?,
            // Display material references, two ids each.
            21 => {
                let n = r.count(32)?;
                r.skip(n * 32)?;
            }
            22 | 30 => r.skip(4)?,
            23..=27 | 35 | 37 | 40 | 41 => r.skip(1)?,
            29 => a.section_source = r.u8()?,
            31..=33 => r.skip(8)?,
            34 => {
                r.color()?;
            }
            36 => r.skip(16 * 8)?,
            39 => a.section = Some(crate::sections::read(r)?),
            // Rendering attributes and later items: nothing more Curva needs.
            _ => break,
        }
    }
    Ok(a)
}

/// The geometry of an object of class `id`, or the kind of thing skipped.
fn geometry_of(id: Uuid, r: &mut Reader<'_>) -> Result<std::result::Result<Geometry, String>> {
    Ok(Ok(match id {
        class::POINT => {
            r.chunk_version()?;
            Geometry::Point(point(r)?)
        }
        class::BREP => Geometry::Brep(brep(r)?),
        class::EXTRUSION => Geometry::Extrusion(extrusion(r)?),
        class::INSTANCE_REF => {
            r.chunk_version()?;
            let block = r.uuid()?;
            let mut xform = [[0.0; 4]; 4];
            for row in &mut xform {
                *row = r.f64s::<4>()?;
            }
            Geometry::Instance { block, xform }
        }
        class::HATCH => Geometry::Hatch(hatch(r)?),
        class::TEXT => Geometry::Text(text(r)?),
        class::DIM_LINEAR => Geometry::Dimension(linear_dimension(r)?),
        class::DIM_RADIAL => Geometry::Radial(radial_dimension(r)?),
        class::DIM_ANGULAR => Geometry::Angular(angular_dimension(r)?),
        class::DIM_ORDINATE => Geometry::Ordinate(ordinate_dimension(r)?),
        class::LEADER => Geometry::Leader(leader(r)?),
        class::V5_TEXT
        | class::V5_DIM_LINEAR
        | class::V5_DIM_RADIAL
        | class::V5_DIM_ANGULAR
        | class::V5_LEADER => match v5_annotation(r)? {
            Ok(geometry) => geometry,
            Err(kind) => return Ok(Err(kind)),
        },
        class::DETAIL => Geometry::Detail(detail(r)?),
        class::CLIPPING_PLANE_SURFACE => Geometry::ClippingPlane(clipping_plane(r)?),
        class::TEXT_DOT => {
            let (_, minor) = r.chunk_version()?;
            let at = point(r)?;
            let height = r.i32()?;
            let text = r.wstring()?;
            let font = r.wstring()?;
            let _display = r.i32()?;
            let secondary = if minor >= 1 {
                r.wstring()?
            } else {
                String::new()
            };
            Geometry::Dot {
                at,
                text,
                secondary,
                height,
                font,
            }
        }
        class::MESH => Geometry::Mesh(mesh::mesh(r)?),
        class::POINT_CLOUD => return Ok(Err("point cloud".to_owned())),
        class::SUBD => Geometry::SubD(subd::subd(r)?),
        _ => {
            if let Some(curve) = curve_of(id, r)? {
                Geometry::Curve(curve)
            } else if let Some(surface) = surface_of(id, r)? {
                Geometry::Surface(surface)
            } else {
                return Ok(Err(format!("other object {:02x?}", id.0)));
            }
        }
    }))
}
