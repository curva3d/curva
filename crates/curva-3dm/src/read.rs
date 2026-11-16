//! Reading a `.3dm` file into a [`Model`].

use crate::archive::{Reader, Result, Uuid, bad, tcode};
use crate::model::{
    AngularDimension, Attributes, Block, Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, BrepVertex,
    ClippingPlane, Curve, Detail, DimStyle, Extrusion, Geometry, Group, Hatch, HatchLine,
    HatchPattern, Layer, Leader, LinearDimension, Linetype, Model, NamedCPlane, NurbsCurve,
    NurbsSurface, Object, P3, Page, Plane, RadialDimension, Source, Surface, Text, View,
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
    pub const DOCUMENT_USER_STRINGS: Uuid = Uuid::parse("06F3218E-F5EC-4f6c-B74C-14583F0ED7BC");
    pub const TEXT_DOT: Uuid = Uuid::parse("74198302-CDF4-4f95-9609-6D684F22AB37");
    pub const HATCH: Uuid = Uuid::parse("0559733B-5332-49d1-A936-0532AC76ADE5");
    pub const HATCH_PATTERN: Uuid = Uuid::parse("064E7C91-35F6-4734-A446-79FF7CD659E1");
    pub const TEXT: Uuid = Uuid::parse("57376349-62A9-4A16-B411-A46BCD544790");
    pub const DIM_LINEAR: Uuid = Uuid::parse("E550882B-F44D-4154-A1EF-6E50CBBBF543");
    pub const DIM_RADIAL: Uuid = Uuid::parse("FC749C2F-4C00-41FD-9840-26D94F047AD3");
    pub const DIM_ANGULAR: Uuid = Uuid::parse("D417786B-F6CD-4F12-9E1F-063F414DBEB6");
    pub const LEADER: Uuid = Uuid::parse("945BF594-6FF9-4F5C-BFC0-B3AF528F29D2");
    pub const DETAIL: Uuid = Uuid::parse("C8C66EFA-B3CB-4e00-9440-2AD66203379E");
    pub const CLIPPING_PLANE_SURFACE: Uuid = Uuid::parse("DBC5A584-CE3F-4170-98A8-497069CA5C36");
    pub const DIM_STYLE: Uuid = Uuid::parse("67AA51A5-791D-4BEC-8AED-D23B462B6F87");
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
            tcode::DIMSTYLE_TABLE => {
                records(&mut chunk.body, tcode::DIMSTYLE_RECORD, |body| {
                    if let Some(style) = dim_style(body)? {
                        model.dim_styles.push(style);
                    }
                    Ok(())
                })?;
            }
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

/// A list's count, which must fit in what is left (each item takes at least a chunk's 12 bytes).
fn list_count(r: &mut Reader<'_>) -> Result<usize> {
    let count = usize::try_from(r.i32()?).map_err(|_| bad("a list of a negative count"))?;
    if count > r.left() / 12 {
        return Err(bad("a list longer than its chunk"));
    }
    Ok(count)
}

/// A construction plane as openNURBS writes one: its plane, grid settings and name.
fn cplane(r: &mut Reader<'_>) -> Result<NamedCPlane> {
    r.chunk_version()?;
    let plane = plane(r)?;
    let _grid_spacing = r.f64()?;
    let _snap_spacing = r.f64()?;
    let _grid_lines = r.i32()?;
    let _thick_every = r.i32()?;
    let name = r.wstring()?;
    Ok(NamedCPlane { name, plane })
}

fn named_cplanes(r: &mut Reader<'_>) -> Result<Vec<NamedCPlane>> {
    let count = list_count(r)?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut chunk = r.expect(tcode::VIEW_CPLANE)?;
        out.push(cplane(&mut chunk.body)?);
    }
    Ok(out)
}

fn named_views(r: &mut Reader<'_>) -> Result<Vec<View>> {
    let count = list_count(r)?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut chunk = r.expect(tcode::VIEW_RECORD)?;
        out.push(view(&mut chunk.body)?);
    }
    Ok(out)
}

/// A view: tagged parts, of which Curva takes the camera, the target and the name.
fn view(r: &mut Reader<'_>) -> Result<View> {
    view_with_attributes(r).map(|(view, _)| view)
}

/// What a view's attributes (version 1.2 and later) add: its display mode's id, and whether its
/// projection is locked (version 1.3).
#[derive(Clone, Copy, Debug, Default)]
struct ViewExtras {
    id: Uuid,
    display: Uuid,
    locked: bool,
}

/// A view, and its display mode and lock.
fn view_with_attributes(r: &mut Reader<'_>) -> Result<(View, ViewExtras)> {
    let mut camera = None;
    let mut target = None;
    let mut extras = ViewExtras::default();
    let mut name = String::new();
    while !r.is_done() {
        let mut part = r.chunk()?;
        match part.tcode {
            tcode::ENDOFTABLE => break,
            tcode::VIEW_VIEWPORT => {
                extras.id = viewport_id(&mut part.body.clone()).unwrap_or(Uuid::NIL);
                camera = Some(viewport(&mut part.body)?);
            }
            // Files before the viewport held its own target keep it here.
            tcode::VIEW_TARGET => target = Some(point(&mut part.body)?),
            tcode::VIEW_NAME => name = part.body.wstring()?,
            tcode::VIEW_ATTRIBUTES => {
                if let Ok(found) = view_extras(&mut part.body) {
                    extras.display = found.display;
                    extras.locked = found.locked;
                }
            }
            _ => {}
        }
    }
    let (mut view, own_target) = camera.ok_or_else(|| bad("a view without its camera"))?;
    view.name = name;
    view.target = own_target.or(target).unwrap_or_else(|| {
        // No target at all: a point in front of the camera, halfway through its depth.
        let depth = 0.5 * (view.frustum[4] + view.frustum[5]);
        std::array::from_fn(|k| view.location[k] + depth * view.direction[k])
    });
    Ok((view, extras))
}

/// A view's attributes past its kind and paper size: version 1.2's display mode id (after the
/// obsolete parent id and nested position), then the page settings and version 1.3's lock.
fn view_extras(r: &mut Reader<'_>) -> Result<ViewExtras> {
    let (_, minor) = r.chunk_version()?;
    let mut extras = ViewExtras::default();
    if minor < 2 {
        return Ok(extras);
    }
    r.skip(4 + 8 * 2 + 16 + 8 * 6)?; // kind, paper size, parent id, nested position
    extras.display = r.uuid()?;
    if minor >= 3 {
        r.chunk()?; // page settings
        extras.locked = r.bool()?;
    }
    Ok(extras)
}

/// A viewport's id (version 1.1 and later), past its flags, projection, camera, frustum and
/// port; nil before.
fn viewport_id(r: &mut Reader<'_>) -> Result<Uuid> {
    let (_, minor) = r.chunk_version()?;
    r.skip(4 * 4 + 6 * 24 + 6 * 8 + 6 * 4)?;
    if minor >= 1 { r.uuid() } else { Ok(Uuid::NIL) }
}

/// openNURBS' viewport: projection, camera frame and frustum, and in later versions its own
/// target point.
fn viewport(r: &mut Reader<'_>) -> Result<(View, Option<P3>)> {
    let (major, minor) = r.chunk_version()?;
    if major != 1 {
        return Err(bad("a viewport of an unknown version"));
    }
    let _valid_camera = r.i32()?;
    let _valid_frustum = r.i32()?;
    let _valid_port = r.i32()?;
    let projection = r.i32()?;
    let location = point(r)?;
    let direction = point(r)?;
    let up = point(r)?;
    for _ in 0..3 {
        point(r)?; // the camera's frame, worked out again from direction and up
    }
    let frustum = r.f64s::<6>()?;
    r.skip(6 * 4)?; // the port, in pixels
    let mut target = None;
    if minor >= 1 {
        r.uuid()?;
        if minor >= 2 {
            r.skip(5)?; // locks and symmetry
            if minor >= 3 {
                let t = point(r)?;
                // openNURBS writes an unset point as -1.23432101234321e308.
                if t.iter().all(|v| v.is_finite() && v.abs() < 1e300) {
                    target = Some(t);
                }
            }
        }
    }
    let usable = |v: P3| v.iter().all(|c| c.is_finite()) && v.iter().any(|c| *c != 0.0);
    if !location.iter().all(|c| c.is_finite()) || !usable(direction) || !usable(up) {
        return Err(bad("a view with a broken camera"));
    }
    Ok((
        View {
            name: String::new(),
            location,
            direction,
            up,
            target: location,
            parallel: projection == 1,
            frustum,
        },
        target,
    ))
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

/// The details (by viewport id) a layer is turned off in, from its per-viewport settings
/// (`ON__LayerExtensions` user data); empty when it has none or they cannot be read.
fn layer_off_in(rest: &mut Reader<'_>) -> Vec<Uuid> {
    let mut out = Vec::new();
    while !rest.is_done() {
        let Ok(mut chunk) = rest.chunk() else {
            break;
        };
        if chunk.tcode != tcode::OPENNURBS_CLASS_USERDATA {
            continue;
        }
        let read = |body: &mut Reader<'_>| -> Result<Vec<Uuid>> {
            body.chunk_version()?;
            let mut header = body.expect(tcode::OPENNURBS_CLASS_USERDATA_HEADER)?;
            if header.body.uuid()? != class::LAYER_EXTENSIONS {
                return Ok(Vec::new());
            }
            let mut data = body.expect(tcode::ANONYMOUS_CHUNK)?;
            let (_, mut list) = data.body.anonymous()?;
            let count = list.count(4)?;
            let mut off = Vec::new();
            for _ in 0..count {
                let (_, mut setting) = list.anonymous()?;
                let bits = setting.u32()?;
                if bits == 0 {
                    continue;
                }
                let viewport = setting.uuid()?;
                if bits & 2 != 0 {
                    setting.skip(4)?; // colour
                }
                if bits & 4 != 0 {
                    setting.skip(4)?; // print colour
                }
                if bits & 8 != 0 {
                    setting.skip(8)?; // print width
                }
                if bits & 16 != 0 && setting.u8()? == 2 {
                    off.push(viewport);
                }
            }
            Ok(off)
        };
        if let Ok(found) = read(&mut chunk.body) {
            out.extend(found);
        }
    }
    out
}

fn layer(record: &mut Reader<'_>) -> Result<Option<Layer>> {
    let (id, mut r, mut rest) = class_object_rest(record)?;
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
        linetype: -1,
        hidden_in: Vec::new(),
    };
    if minor >= 1 {
        layer.visible = r.bool()? && mode != 1;
    }
    if minor >= 2 {
        layer.linetype = r.i32()?;
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
    layer.hidden_in = layer_off_in(&mut rest);
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

/// Whether a hatch line or pattern is written the old (Rhino 5) way: always before version 60,
/// never after, and in version 60 files only when the next chunk is not an anonymous one
/// (openNURBS' `Internal_UseHatchReadV5`).
fn old_hatch_io(r: &Reader<'_>) -> bool {
    match r.version {
        v if v < 60 => true,
        60 => r.peek_tcode() != Some(tcode::ANONYMOUS_CHUNK),
        _ => false,
    }
}

fn doubles(r: &mut Reader<'_>) -> Result<Vec<f64>> {
    let n = r.count(8)?;
    (0..n).map(|_| r.f64()).collect()
}

/// A hatch line (`ON_HatchLine::Read`).
fn hatch_line(r: &mut Reader<'_>) -> Result<HatchLine> {
    let read = |r: &mut Reader<'_>| -> Result<HatchLine> {
        let angle = r.f64()?;
        let base = r.f64s::<2>()?;
        let offset = r.f64s::<2>()?;
        let dashes = doubles(r)?;
        Ok(HatchLine {
            angle,
            base,
            offset,
            dashes,
        })
    };
    if old_hatch_io(r) {
        r.chunk_version()?;
        read(r)
    } else {
        let (_, mut body) = r.anonymous()?;
        read(&mut body)
    }
}

/// A hatch pattern table record (`ON_HatchPattern::Read`, or `ReadV5` in older files).
fn hatch_pattern(record: &mut Reader<'_>) -> Result<Option<HatchPattern>> {
    let (id, mut r) = class_object(record)?;
    if id != class::HATCH_PATTERN {
        return Ok(None);
    }
    let lines_of = |r: &mut Reader<'_>, count: usize| -> Result<Vec<HatchLine>> {
        (0..count).map(|_| hatch_line(r)).collect()
    };
    if old_hatch_io(&r) {
        let (_, minor) = r.chunk_version()?;
        let _index = r.i32()?;
        let kind = r.i32()?;
        let name = r.wstring()?;
        let description = r.wstring()?;
        let lines = if kind == 1 {
            let count = usize::try_from(r.i32()?).unwrap_or(0);
            lines_of(&mut r, count)?
        } else {
            Vec::new()
        };
        let id = if minor >= 2 { r.uuid()? } else { Uuid::NIL };
        return Ok(Some(HatchPattern {
            id,
            name,
            description,
            solid: kind != 1,
            lines,
        }));
    }
    let (_, mut body) = r.anonymous()?;
    let (id, name) = model_component(&mut body)?;
    let kind = body.u32()?;
    let description = body.wstring()?;
    let mut lines_chunk = body.expect(tcode::ANONYMOUS_CHUNK)?;
    let count = usize::try_from(lines_chunk.body.u32()?).unwrap_or(0);
    let lines = lines_of(&mut lines_chunk.body, count)?;
    Ok(Some(HatchPattern {
        id,
        name,
        description,
        solid: kind != 1,
        lines,
    }))
}

/// A linetype table record (`ON_Linetype::Read`): Rhino 6 and later write version 2 (the
/// component's attributes, then the segments), Rhino 5 version 1 (index, name, segments, id).
fn linetype(record: &mut Reader<'_>) -> Result<Option<Linetype>> {
    let (class, mut r) = class_object(record)?;
    if class != class::LINETYPE {
        return Ok(None);
    }
    let (major, minor, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let segments = |body: &mut Reader<'_>| -> Result<Vec<(f64, bool)>> {
        let n = body.count(12)?;
        (0..n)
            .map(|_| Ok((body.f64()?, body.i32()? == 0)))
            .collect()
    };
    if major == 1 {
        let _index = body.i32()?;
        let name = body.wstring()?;
        let segments = segments(&mut body)?;
        let id = if minor >= 1 { body.uuid()? } else { Uuid::NIL };
        return Ok(Some(Linetype { id, name, segments }));
    }
    let (id, name) = component_attributes(&mut body)?;
    let segments = segments(&mut body)?;
    Ok(Some(Linetype { id, name, segments }))
}

/// openNURBS' `ReadModelComponentAttributes` for components written since Rhino 6: a bit set
/// saying which of id, parent id, index, name and status follow. Returns the id and name.
fn model_component(r: &mut Reader<'_>) -> Result<(Uuid, String)> {
    let (_, mut body) = r.anonymous()?;
    let bits = body.u32()?;
    let mut id = Uuid::NIL;
    let mut name = String::new();
    if bits & 0x01 != 0 {
        id = body.uuid()?;
    }
    if bits & 0x02 != 0 {
        body.uuid()?;
    }
    if bits & 0x04 != 0 {
        body.i32()?;
    }
    if bits & 0x08 != 0 {
        name = body.wstring()?;
    }
    Ok((id, name))
}

/// A hatch (`ON_Hatch::Read`): its plane, scale, turn, pattern, outlines and base point.
fn hatch(r: &mut Reader<'_>) -> Result<Hatch> {
    let (_, minor) = r.chunk_version()?;
    let plane = plane(r)?;
    let scale = r.f64()?;
    let rotation = r.f64()?;
    let pattern = r.i32()?;
    let count = usize::try_from(r.i32()?).map_err(|_| bad("a bad loop count"))?;
    let mut loops = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        r.chunk_version()?;
        let outer = r.i32()? == 0;
        loops.push((outer, curve(r)?));
    }
    let base = if minor >= 2 { r.f64s::<2>()? } else { [0.0; 2] };
    Ok(Hatch {
        plane,
        scale,
        rotation,
        pattern,
        loops,
        base,
    })
}

/// A Rhino 6+ annotation style (`ON_DimStyle::Write`, version 1), read as far as the notes'
/// alignment: its id and name, sizes (the seventh is the text height), then every field in
/// order up to the alignments it also keeps in Rhino 5's numbering.
fn dim_style_fields(r: &mut Reader<'_>) -> Result<DimStyle> {
    let (minor, mut b) = r.anonymous()?;
    let (id, name) = component_attributes(&mut b)?;
    let sizes = b.f64s::<7>()?;
    // Text alignment (obsolete), angle format, length format, angle resolution.
    b.skip(4 * 4)?;
    let length_resolution = b.i32()?;
    b.i32()?; // the ignored text style index of Rhino 6 files
    b.f64()?; // length factor
    b.bool()?; // alternate units
    b.f64()?; // alternate length factor
    b.skip(4 * 2)?; // alternate length format and resolution
    for _ in 0..4 {
        b.wstring()?; // prefixes and suffixes
    }
    b.f64()?; // dimension line extension
    b.skip(2)?; // extension lines suppressed
    b.uuid()?; // parent style
    b.i32()?; // legacy override count
    if b.bool()? {
        let count = b.count(1)?;
        b.skip(count)?; // which fields are overridden
    }
    b.skip(4 * 2 + 8 * 4)?; // tolerance format, resolution, upper, lower, height, baseline
    b.skip(1 + 4 + 4)?; // text mask on, fill type, colour
    b.skip(8 + 4 + 16)?; // dimension scale, its source, source style
    b.skip(4 + 16 + 4 + 16 + 2 + 16)?; // colour sources and colours, plot weights
    b.skip(8 + 1)?; // fixed extension length
    b.skip(8 + 4 + 8)?; // text rotation, alternate tolerance resolution, tolerance height
    b.skip(2 + 4 + 4 + 8)?; // arrows suppressed, text move, arc symbol, stack height
    b.skip(4 + 8 * 3)?; // stack format, roundings
    b.skip(4 * 4 + 1)?; // zero suppression, alternate below
    b.skip(4 * 3 + 16 * 3)?; // arrow types and blocks
    b.i32()?; // leader content type (obsolete)
    let v_align = b.u32()?;
    b.skip(4 + 4 + 4 + 8 + 1 + 8)?; // leader alignment, angle style, curve, angle, landing
    let h_align = b.u32()?;
    let mut font = None;
    if minor >= 2 {
        b.u32()?; // leader horizontal alignment
        b.skip(2)?; // draw forward, signed ordinate
        b.chunk()?; // the style's scale
        b.u32()?; // unit system
        font = font_name(&mut b).ok().flatten();
    }
    Ok(DimStyle {
        id,
        name,
        text_height: sizes[6],
        h_align,
        v_align,
        extension_extension: sizes[0],
        extension_offset: sizes[1],
        arrow_size: sizes[2],
        text_gap: sizes[5],
        length_resolution,
        font,
    })
}

/// A font's family name (`ON_Font::Read`, Rhino 6 and later): its characteristics, then its
/// Windows name as a UTF-8 string chunk, which names the family.
fn font_name(r: &mut Reader<'_>) -> Result<Option<String>> {
    let (_, mut f) = r.anonymous()?;
    f.u32()?; // weight, style, stretch, underline and strikethrough
    let mut name = f.expect(tcode::UTF8_STRING_CHUNK)?;
    if name.body.u8()? != 1 {
        return Ok(None);
    }
    let left = name.body.left();
    let text = String::from_utf8_lossy(name.body.bytes(left)?)
        .trim_end_matches('\0')
        .trim()
        .to_owned();
    Ok((!text.is_empty()).then_some(text))
}

/// A dimension style table record; Rhino 5's older form is left out.
fn dim_style(record: &mut Reader<'_>) -> Result<Option<DimStyle>> {
    let (id, mut r) = class_object(record)?;
    if id != class::DIM_STYLE || r.version < 60 || r.peek_tcode() != Some(tcode::ANONYMOUS_CHUNK) {
        return Ok(None);
    }
    dim_style_fields(&mut r).map(Some)
}

/// A note (`ON_Text::Read`, then `ON_Annotation::Internal_ReadAnnotation` and
/// `ON_TextContent::Read`).
/// The part every annotation shares (`ON_Annotation::Internal_ReadAnnotation`): its words, the
/// turn they carry, its style, plane, kind (openNURBS' annotation type, from version 1) and the
/// style it sets for itself (from version 2).
struct AnnotationCore {
    rich: String,
    rotation: f64,
    style: Uuid,
    plane: Plane,
    kind: u32,
    own: Option<DimStyle>,
}

fn annotation_core(r: &mut Reader<'_>) -> Result<AnnotationCore> {
    let (version, mut a) = r.anonymous()?;
    let (_, mut content) = a.anonymous()?;
    let rich = content.wstring()?;
    let _obsolete_plane = plane(&mut content)?;
    let _width = content.f64()?;
    let rotation = content.f64()?;
    let style = a.uuid()?;
    let plane = plane(&mut a)?;
    let kind = if version >= 1 { a.u32()? } else { 0 };
    let mut own = None;
    if version >= 2 {
        let (_, mut overrides) = a.anonymous()?;
        if overrides.bool()? {
            let (id, mut data) = class_object(&mut overrides)?;
            if id == class::DIM_STYLE {
                own = Some(dim_style_fields(&mut data)?);
            }
        }
    }
    Ok(AnnotationCore {
        rich,
        rotation,
        style,
        plane,
        kind,
        own,
    })
}

fn text(r: &mut Reader<'_>) -> Result<Text> {
    let (_, mut outer) = r.anonymous()?;
    let core = annotation_core(&mut outer)?;
    Ok(Text {
        rich: core.rich,
        plane: core.plane,
        style: core.style,
        own: core.own,
        rotation: core.rotation,
    })
}

/// A linear dimension (`ON_DimLinear::Read` around `ON_Dimension::Internal_ReadDimension`):
/// the annotation, its typed text, then its second point and dimension line point in its plane.
fn linear_dimension(r: &mut Reader<'_>) -> Result<LinearDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let core = annotation_core(&mut d)?;
    let text = d.wstring()?;
    let second = outer.f64s::<2>()?;
    let line = outer.f64s::<2>()?;
    Ok(LinearDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        rotated: core.kind == 5,
        text,
        second,
        line,
    })
}

/// The part dimensions share after the annotation (`ON_Dimension::Internal_ReadDimension`):
/// returns the annotation and the typed text; the rest is how Rhino draws it by default.
fn dimension_core(d: &mut Reader<'_>) -> Result<(AnnotationCore, String)> {
    let core = annotation_core(d)?;
    let text = d.wstring()?;
    Ok((core, text))
}

/// A radius or diameter dimension (`ON_DimRadial::Read`): the dimension, then the point on the
/// circle and the text point, in its plane (centred on the circle).
fn radial_dimension(r: &mut Reader<'_>) -> Result<RadialDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let (core, text) = dimension_core(&mut d)?;
    let radius_point = outer.f64s::<2>()?;
    let text_point = outer.f64s::<2>()?;
    Ok(RadialDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        diameter: core.kind == 3,
        text,
        radius_point,
        text_point,
    })
}

/// An angle dimension (`ON_DimAngular::Read`): the dimension, its sides' directions, where
/// its extension lines start, and a point on its arc, in its plane (at the corner).
fn angular_dimension(r: &mut Reader<'_>) -> Result<AngularDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let (core, text) = dimension_core(&mut d)?;
    let sides = [outer.f64s::<2>()?, outer.f64s::<2>()?];
    let offsets = [outer.f64()?, outer.f64()?];
    let arc_point = outer.f64s::<2>()?;
    Ok(AngularDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        text,
        sides,
        offsets,
        arc_point,
    })
}

/// A leader (`ON_Leader::Read`, version 1): the annotation, then its points in its plane.
fn leader(r: &mut Reader<'_>) -> Result<Leader> {
    let (_, mut outer) = r.anonymous()?;
    let core = annotation_core(&mut outer)?;
    let count = outer.count(16)?;
    let points = (0..count)
        .map(|_| outer.f64s::<2>())
        .collect::<Result<Vec<_>>>()?;
    Ok(Leader {
        plane: core.plane,
        style: core.style,
        own: core.own,
        rich: core.rich,
        points,
        landing: true,
    })
}

/// Rhino 5's annotation (`ON_OBSOLETE_V5_Annotation::Read` and its kinds' `Read`): its kind, plane,
/// points in that plane, typed words and text height; an angle dimension adds its angle and
/// radius. Files from openNURBS of October 2007 on wrap each part in a chunk; older ones start
/// with a bare chunk version. The style is named by its index in Rhino 5's style table, which
/// Curva does not read, so the annotation takes the default style (a note keeps its height and
/// alignment as its own).
fn v5_annotation(r: &mut Reader<'_>) -> Result<std::result::Result<Geometry, String>> {
    let chunked = r.peek_tcode() == Some(tcode::ANONYMOUS_CHUNK);
    // A kind's own chunk, then the shared part's; older files have only a bare chunk version.
    let mut a = if chunked {
        r.anonymous()?.1.anonymous()?.1
    } else {
        r.chunk_version()?;
        r.clone()
    };
    let kind = a.u32()?;
    let _display = a.u32()?;
    let plane = plane(&mut a)?;
    let count = a.count(16)?;
    let points = (0..count)
        .map(|_| a.f64s::<2>())
        .collect::<Result<Vec<_>>>()?;
    let words = a.wstring()?;
    let _user_positioned = a.u32()?;
    let _style_index = a.i32()?;
    let height = a.f64()?;
    let justification = if chunked { a.u32()? } else { 0 };
    let point = |i: usize| points.get(i).copied().unwrap_or([0.0, 0.0]);
    let less = |p: [f64; 2], q: [f64; 2]| [p[0] - q[0], p[1] - q[1]];
    Ok(Ok(match kind {
        // A note: Rhino 5 put an unjustified note's top left at its plane's origin.
        7 => {
            let (h_align, v_align) = if justification == 0 {
                (0, 3)
            } else {
                let h = if justification & 2 != 0 {
                    1
                } else if justification & 4 != 0 {
                    2
                } else {
                    0
                };
                let v = if justification & (1 << 18) != 0 {
                    3
                } else if justification & (1 << 17) != 0 {
                    5
                } else {
                    7
                };
                (h, v)
            };
            Geometry::Text(Text {
                rich: words,
                plane,
                style: Uuid::NIL,
                own: Some(DimStyle {
                    id: Uuid::NIL,
                    name: String::new(),
                    text_height: height,
                    h_align,
                    v_align,
                    arrow_size: 1.0,
                    extension_extension: 1.0,
                    extension_offset: 0.5,
                    text_gap: 0.25,
                    length_resolution: 2,
                    font: None,
                }),
                rotation: 0.0,
            })
        }
        // Linear (1) and aligned (2): the first point at [0] (the plane's origin), the second at
        // [2], the dimension line from [1] to [3], always along the plane's X. An aligned one's
        // plane runs from its first point to its second; when it does not, Rhino measures it
        // along X as well.
        1 | 2 => {
            let first = point(0);
            let (from, to) = (point(1), point(3));
            let middle = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
            let along_x = kind == 1 || (point(2)[1] - first[1]).abs() > 1e-9;
            Geometry::Dimension(LinearDimension {
                plane: Plane {
                    origin: std::array::from_fn(|k| {
                        plane.origin[k] + plane.x[k] * first[0] + plane.y[k] * first[1]
                    }),
                    ..plane
                },
                style: Uuid::NIL,
                own: None,
                rotated: along_x,
                text: words,
                second: less(point(2), first),
                line: less(middle, first),
            })
        }
        // Diameter (4) and radius (5): the centre at [0], the arrow at [1], the text at [2].
        4 | 5 => {
            let centre = point(0);
            Geometry::Radial(RadialDimension {
                plane: Plane {
                    origin: std::array::from_fn(|k| {
                        plane.origin[k] + plane.x[k] * centre[0] + plane.y[k] * centre[1]
                    }),
                    ..plane
                },
                style: Uuid::NIL,
                own: None,
                diameter: kind == 4,
                text: words,
                radius_point: less(point(1), centre),
                text_point: less(point(2), centre),
            })
        }
        // An angle (3): the corner at the plane's origin, a point on each side at [1] and [2], a
        // point on the arc at [3].
        3 => {
            let length = |p: [f64; 2]| (p[0] * p[0] + p[1] * p[1]).sqrt();
            let (start, end) = (point(1), point(2));
            Geometry::Angular(AngularDimension {
                plane,
                style: Uuid::NIL,
                own: None,
                text: words,
                sides: [start, end],
                offsets: [length(start), length(end)],
                arc_point: point(3),
            })
        }
        // A leader (6): the arrow's tip first, the words at the last point.
        6 => Geometry::Leader(Leader {
            plane,
            style: Uuid::NIL,
            own: None,
            rich: words,
            points,
            landing: false,
        }),
        other => return Ok(Err(format!("Rhino 5 annotation of kind {other}"))),
    }))
}

/// The model's views in the settings' view list (those not pages or nested in one): their
/// names and viewport ids.
fn model_views(r: &mut Reader<'_>) -> Result<Vec<(String, Uuid)>> {
    let count = list_count(r)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let mut record = r.expect(tcode::VIEW_RECORD)?;
        let mut id = Uuid::NIL;
        let mut name = String::new();
        let mut kind = 0;
        while !record.body.is_done() {
            let mut part = record.body.chunk()?;
            match part.tcode {
                tcode::ENDOFTABLE => break,
                tcode::VIEW_VIEWPORT => id = viewport_id(&mut part.body)?,
                tcode::VIEW_NAME => name = part.body.wstring()?,
                tcode::VIEW_ATTRIBUTES => {
                    let (_, minor) = part.body.chunk_version()?;
                    if minor >= 1 {
                        kind = part.body.i32()?;
                    }
                }
                _ => {}
            }
        }
        if kind == 0 {
            out.push((name, id));
        }
    }
    Ok(out)
}

/// The layouts among the views of the settings' view list: views of openNURBS' page type
/// (`ON_3dmView` with `ON::page_view_type`), with their viewport id, name and paper size.
fn pages(r: &mut Reader<'_>) -> Result<Vec<Page>> {
    let count = list_count(r)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let mut record = r.expect(tcode::VIEW_RECORD)?;
        let mut id = Uuid::NIL;
        let mut name = String::new();
        let mut page = None;
        let mut user_text = Vec::new();
        while !record.body.is_done() {
            let mut part = record.body.chunk()?;
            match part.tcode {
                tcode::ENDOFTABLE => break,
                tcode::VIEW_VIEWPORT => {
                    let (_, minor) = part.body.chunk_version()?;
                    // Valid flags and projection, the camera's six vectors, frustum and port.
                    part.body.skip(4 * 4 + 6 * 24 + 6 * 8 + 6 * 4)?;
                    if minor >= 1 {
                        id = part.body.uuid()?;
                    }
                }
                // The viewport's user data, where Rhino keeps layout user text.
                tcode::VIEW_VIEWPORT_USERDATA => user_text = self::user_text(&mut part.body),
                tcode::VIEW_NAME => name = part.body.wstring()?,
                tcode::VIEW_ATTRIBUTES => {
                    let (_, minor) = part.body.chunk_version()?;
                    if minor >= 1 && part.body.i32()? == 1 {
                        page = Some([part.body.f64()?, part.body.f64()?]);
                    }
                }
                _ => {}
            }
        }
        if let Some(size_mm) = page {
            out.push(Page {
                id,
                name,
                size_mm,
                user_text,
            });
        }
    }
    Ok(out)
}

/// The page unit in the settings' attributes (version 1.1 and later): after the linetype
/// scale, plot colour and its source, and the current linetype and its source, the page's
/// units and tolerances in their own chunk.
fn page_unit(r: &mut Reader<'_>) -> Result<Option<u32>> {
    let (major, minor) = r.chunk_version()?;
    if major != 1 || minor < 1 {
        return Ok(None);
    }
    r.skip(8 + 4 + 4 + 4 + 4)?;
    let (_, _, mut units) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let version = units.i32()?;
    (100..200)
        .contains(&version)
        .then(|| units.u32())
        .transpose()
}

/// A detail view (`ON_DetailView::Read`): its camera, its outline, and its scale.
fn detail(r: &mut Reader<'_>) -> Result<Detail> {
    let (_, minor, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let (_, _, mut view_chunk) = body.versioned(tcode::ANONYMOUS_CHUNK)?;
    let (view, extras) = view_with_attributes(&mut view_chunk)?;
    let (_, _, mut outline) = body.versioned(tcode::ANONYMOUS_CHUNK)?;
    let boundary = nurbs_curve(&mut outline)?;
    let ratio = if minor >= 1 { body.f64()? } else { 0.0 };
    Ok(Detail {
        view,
        boundary,
        ratio,
        id: extras.id,
        display: extras.display,
        locked: extras.locked,
    })
}

/// A clipping plane (`ON_ClippingPlaneSurface::Read`): its plane surface, then the clipping
/// plane: the first viewport, its id, the plane, whether it is on, and from version 1.1 all
/// its viewports.
fn clipping_plane(r: &mut Reader<'_>) -> Result<ClippingPlane> {
    let (_, mut body) = r.anonymous()?;
    let mut surface = body.expect(tcode::ANONYMOUS_CHUNK)?;
    let (_, minor) = surface.body.chunk_version()?;
    let _plane = plane(&mut surface.body)?;
    let domain = [interval(&mut surface.body)?, interval(&mut surface.body)?];
    let extents = if minor >= 1 {
        [interval(&mut surface.body)?, interval(&mut surface.body)?]
    } else {
        domain
    };
    let (_, clip_minor, mut clip) = body.versioned(tcode::ANONYMOUS_CHUNK)?;
    let first = clip.uuid()?;
    let id = clip.uuid()?;
    let plane = plane(&mut clip)?;
    let enabled = clip.bool()?;
    let mut viewports = Vec::new();
    if clip_minor >= 1 {
        let (_, mut list) = clip.anonymous()?;
        let n = list.count(16)?;
        for _ in 0..n {
            viewports.push(list.uuid()?);
        }
    }
    if viewports.is_empty() && first != Uuid::NIL {
        viewports.push(first);
    }
    Ok(ClippingPlane {
        plane,
        extents,
        viewports,
        id,
        enabled,
    })
}

/// The model's notes in the properties table (`ON_3dmNotes`): whether they are HTML, then the
/// words.
fn notes(table: &mut Reader<'_>) -> Result<Option<String>> {
    while !table.is_done() {
        let mut chunk = table.chunk()?;
        match chunk.tcode {
            tcode::ENDOFTABLE => break,
            tcode::PROPERTIES_NOTES => {
                chunk.body.chunk_version()?;
                let _html = chunk.body.i32()?;
                return Ok(Some(chunk.body.wstring()?));
            }
            _ => {}
        }
    }
    Ok(None)
}

/// A user table: its plug-in's id, then its record. Curva reads the one openNURBS keeps the
/// document user text in: an `ON_DocumentUserStringList` object whose user data holds the keys and
/// values.
fn user_table(table: &mut Reader<'_>) -> Result<Option<Vec<(String, String)>>> {
    let mut id_chunk = table.expect(tcode::USER_TABLE_UUID)?;
    if id_chunk.body.uuid()? != class::DOCUMENT_USER_STRINGS {
        return Ok(None);
    }
    let mut record = table.expect(tcode::USER_RECORD)?;
    let mut object = record.body.expect(tcode::OPENNURBS_CLASS)?;
    // The class id and data come first; the user data, with the keys and values, after.
    object.body.expect(tcode::OPENNURBS_CLASS_UUID)?;
    object.body.expect(tcode::OPENNURBS_CLASS_DATA)?;
    Ok(Some(user_text(&mut object.body)))
}
