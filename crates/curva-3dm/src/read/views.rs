//! Views, construction planes, pages, details, clipping planes and notes as Rhino writes them.

#[allow(clippy::wildcard_imports)]
use super::*;

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

pub(super) fn named_cplanes(r: &mut Reader<'_>) -> Result<Vec<NamedCPlane>> {
    let count = list_count(r)?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut chunk = r.expect(tcode::VIEW_CPLANE)?;
        out.push(cplane(&mut chunk.body)?);
    }
    Ok(out)
}

pub(super) fn named_views(r: &mut Reader<'_>) -> Result<Vec<View>> {
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

/// The model's views in the settings' view list (those not pages or nested in one): their
/// names and viewport ids.
pub(super) fn model_views(r: &mut Reader<'_>) -> Result<Vec<(String, Uuid)>> {
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

/// The cameras of the model's views in the settings' view list (those not pages or nested in
/// one), each named as its view is.
pub(super) fn model_cameras(r: &mut Reader<'_>) -> Result<Vec<View>> {
    let count = list_count(r)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let record = r.expect(tcode::VIEW_RECORD)?;
        let model_view = model_views_kind(&mut record.body.clone())? == 0;
        if let (true, Ok((view, _))) = (model_view, view_with_attributes(&mut record.body.clone()))
        {
            out.push(view);
        }
    }
    Ok(out)
}

/// A view record's kind (openNURBS' view type: 0 for a model view, else a page or nested one).
fn model_views_kind(r: &mut Reader<'_>) -> Result<i32> {
    let mut kind = 0;
    while !r.is_done() {
        let mut part = r.chunk()?;
        match part.tcode {
            tcode::ENDOFTABLE => break,
            tcode::VIEW_ATTRIBUTES => {
                let (_, minor) = part.body.chunk_version()?;
                if minor >= 1 {
                    kind = part.body.i32()?;
                }
            }
            _ => {}
        }
    }
    Ok(kind)
}

/// The layouts among the views of the settings' view list: views of openNURBS' page type
/// (`ON_3dmView` with `ON::page_view_type`), with their viewport id, name and paper size.
pub(super) fn pages(r: &mut Reader<'_>) -> Result<Vec<Page>> {
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
pub(super) fn page_unit(r: &mut Reader<'_>) -> Result<Option<u32>> {
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
pub(super) fn detail(r: &mut Reader<'_>) -> Result<Detail> {
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
pub(super) fn clipping_plane(r: &mut Reader<'_>) -> Result<ClippingPlane> {
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
pub(super) fn notes(table: &mut Reader<'_>) -> Result<Option<String>> {
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
pub(super) fn user_table(table: &mut Reader<'_>) -> Result<Option<Vec<(String, String)>>> {
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
