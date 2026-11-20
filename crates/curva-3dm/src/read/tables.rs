//! Reading a file's tables: layers (with their section styles and per-detail settings), groups
//! and block definitions.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The details (by viewport id) a layer is turned off in, from its per-viewport settings
/// (`ON__LayerExtensions` user data); empty when it has none or they cannot be read.
pub(super) fn layer_off_in(rest: &mut Reader<'_>) -> Vec<Uuid> {
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

pub(super) fn layer(record: &mut Reader<'_>) -> Result<Option<Layer>> {
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
        section: None,
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
    if minor >= 15 {
        layer.section = layer_section(&mut r)?;
    }
    layer.hidden_in = layer_off_in(&mut rest);
    Ok(Some(layer))
}

/// The section style among a version 1.15 layer's items: after whether it is expanded, its
/// rendering attributes and its display material, as Rhino 8 writes them.
pub(super) fn layer_section(r: &mut Reader<'_>) -> Result<Option<crate::sections::SectionStyle>> {
    r.bool()?;
    r.chunk()?;
    r.uuid()?;
    loop {
        match r.u8()? {
            34 => {
                r.bool()?;
            }
            35 => return Ok(Some(crate::sections::read(r)?)),
            // 0 ends the items; others are not written by Rhino 8.
            _ => return Ok(None),
        }
    }
}

pub(super) fn group(record: &mut Reader<'_>) -> Result<Option<Group>> {
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
pub(super) fn component_attributes(r: &mut Reader<'_>) -> Result<(Uuid, String)> {
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

pub(super) fn uuids(r: &mut Reader<'_>) -> Result<Vec<Uuid>> {
    let n = r.count(16)?;
    (0..n).map(|_| r.uuid()).collect()
}

pub(super) fn block(record: &mut Reader<'_>) -> Result<Option<Block>> {
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

pub(super) fn block_v6(r: &mut Reader<'_>) -> Result<Option<Block>> {
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

pub(super) fn block_v5(r: &mut Reader<'_>) -> Result<Option<Block>> {
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
