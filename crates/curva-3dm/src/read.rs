//! Reading a `.3dm` file into a [`Model`].

use crate::archive::{Reader, Result, Uuid, bad, tcode};
use crate::model::{
    Attributes, Block, Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, BrepVertex, Curve, Extrusion,
    Geometry, Group, Layer, Model, NurbsCurve, NurbsSurface, Object, P3, Plane, Source, Surface,
};

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
    while !file.is_done() {
        let mut chunk = file.chunk()?;
        match chunk.tcode {
            tcode::SETTINGS_TABLE => settings(&mut chunk.body, &mut model)?,
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
            tcode::INSTANCE_DEFINITION_TABLE => {
                records(&mut chunk.body, tcode::INSTANCE_DEFINITION_RECORD, |body| {
                    if let Some(block) = block(body)? {
                        model.blocks.push(block);
                    }
                    Ok(())
                })?;
            }
            tcode::OBJECT_TABLE => {
                let mut objects = Vec::new();
                let mut skipped = Vec::new();
                records(&mut chunk.body, tcode::OBJECT_RECORD, |body| {
                    match object(body) {
                        Ok(Ok(found)) => objects.push(found),
                        Ok(Err(kind)) => skipped.push(kind),
                        // A damaged object is skipped, not the file.
                        Err(e) => skipped.push(format!("damaged object: {e}")),
                    }
                    Ok(())
                })?;
                model.objects = objects;
                for kind in skipped {
                    model.skip(&kind);
                }
            }
            tcode::ENDOFFILE => break,
            _ => {}
        }
    }
    Ok(model)
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

fn layer(record: &mut Reader<'_>) -> Result<Option<Layer>> {
    let (id, mut r) = class_object(record)?;
    if id != class::LAYER {
        return Ok(None);
    }
    let (major, minor) = r.chunk_version()?;
    if major != 1 {
        return Err(bad("a layer of an unknown version"));
    }
    let mode = r.i32()?;
    let _index = r.i32()?;
    let _iges = r.i32()?;
    let _material = r.i32()?;
    let _obsolete = r.i32()?;
    let c = r.color()?;
    r.skip(2 + 2 + 8 + 8)?;
    let name = r.wstring()?;
    let mut layer = Layer {
        name,
        id: Uuid::NIL,
        parent: Uuid::NIL,
        color: [c[0], c[1], c[2]],
        visible: mode != 1,
        locked: mode == 2,
        print_width: 0.0,
    };
    if minor >= 1 {
        layer.visible = r.bool()? && mode != 1;
    }
    if minor >= 2 {
        let _linetype = r.i32()?;
    }
    if minor >= 3 {
        let _plot_color = r.color()?;
        layer.print_width = r.f64()?;
    }
    if minor >= 4 {
        layer.locked = r.bool()? || mode == 2;
    }
    if minor >= 5 {
        layer.id = r.uuid()?;
    }
    if minor >= 6 {
        layer.parent = r.uuid()?;
    }
    Ok(Some(layer))
}

fn group(record: &mut Reader<'_>) -> Result<Option<Group>> {
    let (id, mut r) = class_object(record)?;
    if id != class::GROUP {
        return Ok(None);
    }
    let (_, minor) = r.chunk_version()?;
    let _index = r.i32()?;
    let name = r.wstring()?;
    let id = if minor >= 1 { r.uuid()? } else { Uuid::NIL };
    Ok(Some(Group { name, id }))
}

/// openNURBS' `ReadModelComponentAttributes`: the id, index and name a component carries.
fn component_attributes(r: &mut Reader<'_>) -> Result<(Uuid, String)> {
    let (_, _, mut body) = r.versioned(tcode::MODEL_ATTRIBUTES_CHUNK)?;
    let mut id = Uuid::NIL;
    let mut name = String::new();
    if body.u8()? == 1 {
        body.skip(12)?;
    }
    if body.u8()? == 1 {
        id = body.uuid()?;
    }
    if body.u8()? == 1 {
        body.skip(4)?;
    }
    if body.u8()? == 1 {
        body.skip(4)?;
    }
    if body.u8()? == 1 {
        name = body.wstring()?;
    }
    Ok((id, name))
}

fn uuids(r: &mut Reader<'_>) -> Result<Vec<Uuid>> {
    let n = r.count(16)?;
    (0..n).map(|_| r.uuid()).collect()
}

fn block(record: &mut Reader<'_>) -> Result<Option<Block>> {
    let (id, mut r) = class_object(record)?;
    if id != class::INSTANCE_DEFINITION {
        return Ok(None);
    }
    if r.version >= 70 || r.peek_tcode() == Some(tcode::ANONYMOUS_CHUNK) {
        block_v6(&mut r)
    } else {
        block_v5(&mut r)
    }
}

fn block_v6(r: &mut Reader<'_>) -> Result<Option<Block>> {
    let (_, mut body) = r.anonymous()?;
    let (id, name) = component_attributes(&mut body)?;
    let kind = body.u32()?;
    let _units = body.anonymous()?;
    let description = body.wstring()?;
    let _url = body.wstring()?;
    let _url_tag = body.wstring()?;
    bounding_box(&mut body)?;
    let objects = if body.bool()? {
        uuids(&mut body)?
    } else {
        Vec::new()
    };
    let mut linked = None;
    if body.bool()? {
        let (_, mut link) = body.anonymous()?;
        let (_, mut reference) = link.anonymous()?;
        let full = reference.wstring()?;
        let relative = reference.wstring()?;
        linked = Some(if full.is_empty() { relative } else { full });
    }
    // Linked-and-embedded definitions carry their objects; only plain links are links here.
    if kind != 3 {
        linked = None;
    }
    Ok(Some(Block {
        name,
        id,
        description,
        objects,
        linked,
    }))
}

fn block_v5(r: &mut Reader<'_>) -> Result<Option<Block>> {
    let (major, minor) = r.chunk_version()?;
    if major != 1 {
        return Ok(None);
    }
    let id = r.uuid()?;
    let objects = uuids(r)?;
    let name = r.wstring()?;
    let description = r.wstring()?;
    let _url = r.wstring()?;
    let _url_tag = r.wstring()?;
    bounding_box(r)?;
    let mut linked = None;
    if minor >= 1 {
        let kind = r.i32()?;
        let path = r.wstring()?;
        if kind == 3 && !path.is_empty() {
            linked = Some(path);
        }
    }
    Ok(Some(Block {
        name,
        id,
        description,
        objects,
        linked,
    }))
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
            3 | 4 | 10 => r.skip(4)?,
            6 => {
                let c = r.color()?;
                a.color = [c[0], c[1], c[2]];
            }
            7 => r.skip(4)?,
            8 => a.print_width = r.f64()?,
            9 | 14 | 16 | 17 => r.skip(1)?,
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
            19 => r.skip(1)?,
            20 => r.skip(16)?,
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
        class::MESH => return Ok(Err("mesh".to_owned())),
        class::POINT_CLOUD => return Ok(Err("point cloud".to_owned())),
        class::SUBD => return Ok(Err("SubD".to_owned())),
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

/// A curve object written with `WriteObject`.
fn curve(r: &mut Reader<'_>) -> Result<Curve> {
    let (id, mut data) = class_object(r)?;
    curve_of(id, &mut data)?.ok_or_else(|| bad(format!("a curve of an unknown kind {:02x?}", id.0)))
}

fn curve_of(id: Uuid, r: &mut Reader<'_>) -> Result<Option<Curve>> {
    Ok(Some(match id {
        class::LINE_CURVE => {
            r.chunk_version()?;
            let ends = [point(r)?, point(r)?];
            let domain = interval(r)?;
            Curve::Line { ends, domain }
        }
        class::POLYLINE_CURVE => {
            r.chunk_version()?;
            let n = r.count(24)?;
            let points = (0..n).map(|_| point(r)).collect::<Result<Vec<_>>>()?;
            let m = r.count(8)?;
            let params = (0..m).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
            Curve::Polyline { points, params }
        }
        class::ARC_CURVE => {
            r.chunk_version()?;
            let plane = plane(r)?;
            let radius = r.f64()?;
            r.skip(72)?;
            let angles = interval(r)?;
            let domain = interval(r)?;
            Curve::Arc {
                plane,
                radius,
                angles,
                domain,
            }
        }
        class::NURBS_CURVE => Curve::Nurbs(nurbs_curve(r)?),
        class::POLY_CURVE => {
            r.chunk_version()?;
            let count = r.count(1)?;
            r.skip(8)?;
            bounding_box(r)?;
            let m = r.count(8)?;
            let params = (0..m).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
            let segments = (0..count).map(|_| curve(r)).collect::<Result<Vec<_>>>()?;
            Curve::Poly { segments, params }
        }
        _ => return Ok(None),
    }))
}

fn nurbs_curve(r: &mut Reader<'_>) -> Result<NurbsCurve> {
    r.chunk_version()?;
    let dimension = usize::try_from(r.i32()?).map_err(|_| bad("a bad dimension"))?;
    let rational = r.i32()? != 0;
    let order = usize::try_from(r.i32()?).map_err(|_| bad("a bad order"))?;
    let count = usize::try_from(r.i32()?).map_err(|_| bad("a bad count"))?;
    if !(1..=3).contains(&dimension) || !(2..=20).contains(&order) || count < order {
        return Err(bad("a NURBS curve that cannot be"));
    }
    r.skip(8)?;
    bounding_box(r)?;
    let knot_count = r.count(8)?;
    let knots = (0..knot_count)
        .map(|_| r.f64())
        .collect::<Result<Vec<_>>>()?;
    let cv_count = r.count(8)?;
    let size = dimension + usize::from(rational);
    let mut points = Vec::with_capacity(cv_count);
    for _ in 0..cv_count {
        let mut p = [0.0, 0.0, 0.0, 1.0];
        for v in p.iter_mut().take(dimension) {
            *v = r.f64()?;
        }
        if rational {
            p[3] = r.f64()?;
        }
        points.push(p);
    }
    let _ = size;
    if knots.len() != count + order - 2 || points.len() != count {
        return Err(bad("a NURBS curve with the wrong number of knots"));
    }
    Ok(NurbsCurve {
        dimension,
        order,
        knots,
        points,
        rational,
    })
}

/// A surface object written with `WriteObject`.
fn surface(r: &mut Reader<'_>) -> Result<Surface> {
    let (id, mut data) = class_object(r)?;
    surface_of(id, &mut data)?.ok_or_else(|| bad("a surface of an unknown kind"))
}

fn surface_of(id: Uuid, r: &mut Reader<'_>) -> Result<Option<Surface>> {
    Ok(Some(match id {
        class::NURBS_SURFACE => {
            r.chunk_version()?;
            let _dimension = r.i32()?;
            let rational = r.i32()? != 0;
            let order = [r.i32()?, r.i32()?].map(|o| usize::try_from(o).unwrap_or(0));
            let count = [r.i32()?, r.i32()?].map(|c| usize::try_from(c).unwrap_or(0));
            if order.iter().any(|o| !(2..=20).contains(o))
                || count[0] < order[0]
                || count[1] < order[1]
            {
                return Err(bad("a NURBS surface that cannot be"));
            }
            r.skip(8)?;
            bounding_box(r)?;
            let mut knots = [Vec::new(), Vec::new()];
            for k in &mut knots {
                let n = r.count(8)?;
                *k = (0..n).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
            }
            let n = r.count(8)?;
            if n != count[0] * count[1] {
                return Err(bad("a NURBS surface with the wrong number of points"));
            }
            let mut points = Vec::with_capacity(count[0]);
            for _ in 0..count[0] {
                let mut row = Vec::with_capacity(count[1]);
                for _ in 0..count[1] {
                    let mut p = [0.0, 0.0, 0.0, 1.0];
                    for v in p.iter_mut().take(3) {
                        *v = r.f64()?;
                    }
                    if rational {
                        p[3] = r.f64()?;
                    }
                    row.push(p);
                }
                points.push(row);
            }
            for d in 0..2 {
                if knots[d].len() != count[d] + order[d] - 2 {
                    return Err(bad("a NURBS surface with the wrong number of knots"));
                }
            }
            Surface::Nurbs(NurbsSurface {
                order,
                knots,
                points,
                rational,
            })
        }
        class::PLANE_SURFACE => {
            let (_, minor) = r.chunk_version()?;
            let plane = plane(r)?;
            let domain = [interval(r)?, interval(r)?];
            let extents = if minor >= 1 {
                [interval(r)?, interval(r)?]
            } else {
                domain
            };
            Surface::Plane {
                plane,
                domain,
                extents,
            }
        }
        class::REV_SURFACE => {
            r.chunk_version()?;
            let axis = [point(r)?, point(r)?];
            let angles = interval(r)?;
            let domain = interval(r)?;
            bounding_box(r)?;
            let transposed = r.i32()? != 0;
            if r.u8()? == 0 {
                return Err(bad("a surface of revolution without its curve"));
            }
            let curve = Box::new(curve(r)?);
            Surface::Revolution {
                curve,
                axis,
                angles,
                domain,
                transposed,
            }
        }
        class::SUM_SURFACE => {
            r.chunk_version()?;
            let base = point(r)?;
            bounding_box(r)?;
            let a = curve(r)?;
            let b = curve(r)?;
            Surface::Sum {
                curves: [Box::new(a), Box::new(b)],
                base,
            }
        }
        _ => return Ok(None),
    }))
}

/// One of a B-rep's arrays of curves or surfaces: each is there or not.
fn optional_array<T>(
    r: &mut Reader<'_>,
    mut each: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<Vec<Option<T>>> {
    let mut chunk = r.expect(tcode::ANONYMOUS_CHUNK)?;
    let body = &mut chunk.body;
    body.chunk_version()?;
    let n = body.count(4)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if body.i32()? != 0 {
            out.push(Some(each(body)?));
        } else {
            out.push(None);
        }
    }
    Ok(out)
}

/// One of a B-rep's arrays of topology.
fn topology<'a, T>(
    r: &mut Reader<'a>,
    mut each: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut chunk = r.expect(tcode::ANONYMOUS_CHUNK)?;
    let body = &mut chunk.body;
    body.chunk_version()?;
    let n = body.count(4)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(each(body)?);
    }
    Ok(out)
}

fn ints(r: &mut Reader<'_>) -> Result<Vec<i32>> {
    let n = r.count(4)?;
    (0..n).map(|_| r.i32()).collect()
}

fn brep(r: &mut Reader<'_>) -> Result<Brep> {
    let (major, _) = r.chunk_version()?;
    if major != 3 {
        return Err(bad("a B-rep from before Rhino 3"));
    }
    let curves2 = optional_array(r, curve)?;
    let curves3 = optional_array(r, curve)?;
    let surfaces = optional_array(r, surface)?;
    let after_3 = r.version >= 3;
    let vertices = topology(r, |r| {
        let _index = r.i32()?;
        let point = point(r)?;
        let edges = ints(r)?;
        let tolerance = r.f64()?;
        Ok(BrepVertex {
            point,
            edges,
            tolerance,
        })
    })?;
    let edges = topology(r, |r| {
        let _index = r.i32()?;
        let curve = r.i32()?;
        let reversed = r.i32()? != 0;
        let proxy_domain = interval(r)?;
        let vertices = [r.i32()?, r.i32()?];
        let trims = ints(r)?;
        let tolerance = r.f64()?;
        let domain = if after_3 { interval(r)? } else { proxy_domain };
        Ok(BrepEdge {
            curve,
            reversed,
            proxy_domain,
            domain,
            vertices,
            trims,
            tolerance,
        })
    })?;
    let trims = topology(r, |r| {
        let _index = r.i32()?;
        let curve = r.i32()?;
        let proxy_domain = interval(r)?;
        let edge = r.i32()?;
        let vertices = [r.i32()?, r.i32()?];
        let reversed3d = r.i32()? != 0;
        let kind = r.i32()?;
        let iso = r.i32()?;
        let loop_index = r.i32()?;
        let tolerance = r.f64s::<2>()?;
        let (domain, proxy_reversed) = if after_3 {
            let domain = interval(r)?;
            let flags = r.bytes(8)?;
            r.skip(24)?;
            (domain, flags[0] == 1)
        } else {
            r.skip(48)?;
            (proxy_domain, false)
        };
        r.skip(16)?;
        Ok(BrepTrim {
            curve,
            proxy_domain,
            domain,
            proxy_reversed,
            edge,
            vertices,
            reversed3d,
            kind,
            iso,
            loop_index,
            tolerance,
        })
    })?;
    let loops = topology(r, |r| {
        let _index = r.i32()?;
        let trims = ints(r)?;
        let kind = r.i32()?;
        let face = r.i32()?;
        Ok(BrepLoop { trims, kind, face })
    })?;
    let faces = topology(r, |r| {
        let _index = r.i32()?;
        let loops = ints(r)?;
        let surface = r.i32()?;
        let reversed = r.i32()? != 0;
        let _material = r.i32()?;
        Ok(BrepFace {
            loops,
            surface,
            reversed,
        })
    })?;
    Ok(Brep {
        curves2,
        curves3,
        surfaces,
        vertices,
        edges,
        trims,
        loops,
        faces,
    })
}

fn extrusion(r: &mut Reader<'_>) -> Result<Extrusion> {
    let (_, minor, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let profile = curve(&mut body)?;
    let line = [point(&mut body)?, point(&mut body)?];
    let t = interval(&mut body)?;
    let up = point(&mut body)?;
    body.skip(2 + 48)?;
    let _path_domain = interval(&mut body)?;
    let _transposed = body.bool()?;
    let mut count = 1;
    let mut caps = [false, false];
    if minor >= 1 {
        count = body.i32()?;
        if minor >= 2 {
            caps = [body.bool()?, body.bool()?];
        }
    }
    let profiles = match profile {
        Curve::Poly { segments, .. } if count > 1 => segments,
        other => vec![other],
    };
    if minor < 2 {
        let closed = true;
        caps = [closed, closed];
    }
    let at = |s: f64| -> P3 { std::array::from_fn(|k| line[0][k] + s * (line[1][k] - line[0][k])) };
    Ok(Extrusion {
        path: [at(t[0]), at(t[1])],
        up,
        profiles,
        caps,
    })
}
