//! Reading Rhino's SubDs (`ON_SubD`) into Curva's.
//!
//! The layout is openNURBS' `opennurbs_subd_archive.cpp`: a flag, then the SubD in an anonymous
//! chunk holding its levels, each in a chunk of its own with its points, edges and faces. Only
//! level 0, the cage, is read; finer levels are Rhino's saved subdivisions, worked out again by
//! Curva. Each component starts with its archive number, id and level, and ends with optional
//! "additions", each led by a size byte and the whole closed by 255. Points carry their tag and
//! place; edges their tag, sharpness and two points; faces their edges, each with a flag that
//! says it runs backwards, and optionally a colour and texture points.

use crate::archive::{Reader, Result, bad};
use curva_mesh::{Attribute, Mesh, Point, Sharpness, SubD};

/// The byte that ends a component's additions.
const END: u8 = 255;
/// The byte that marks an addition held in an anonymous chunk.
const CHUNKED: u8 = 254;

/// What a point record holds.
struct Vertex {
    tag: u8,
    at: Point,
}

/// What an edge record holds: its tag, its sharpness, and its two points' archive numbers.
struct Edge {
    tag: u8,
    sharpness: f64,
    ends: [u32; 2],
}

/// What a face record holds: its edges' archive numbers with their backwards flags, and its
/// optional colour and texture points.
struct Face {
    edges: Vec<(u32, bool)>,
    colour: Option<[u8; 4]>,
    texture: Option<Vec<Point>>,
}

/// One `ON_SubD`, its class data.
///
/// # Errors
/// When the record is damaged or of a version Curva does not read.
pub(crate) fn subd(r: &mut Reader) -> Result<SubD> {
    if r.u8()? == 0 {
        return Ok(SubD::default());
    }
    let (major, mut body) = {
        let (major, minor, body) = r.versioned(crate::archive::tcode::ANONYMOUS_CHUNK)?;
        let _ = minor;
        (major, body)
    };
    if major != 1 {
        return Err(bad(format!("a SubD of version {major}")));
    }
    let levels = body.u32()?;
    if levels == 0 {
        return Ok(SubD::default());
    }
    body.skip(3 * 4 + 6 * 8)?; // largest ids, and an unused box
    let (_, mut level) = body.anonymous()?;
    read_level(&mut level)
}

fn read_level(r: &mut Reader) -> Result<SubD> {
    let _index = r.u16()?;
    r.skip(3 + 6 * 8)?; // three size bytes and the cage's box
    let mut partition = [0u32; 4];
    for p in &mut partition {
        *p = r.u32()?;
    }
    if !(partition[0] <= partition[1]
        && partition[1] <= partition[2]
        && partition[2] <= partition[3])
        || (partition[3] - partition[0]) as usize > r.left()
    {
        return Err(bad("a SubD whose parts are numbered wrongly"));
    }
    let version = r.version;
    let mut vertices = Vec::with_capacity((partition[1] - partition[0]) as usize);
    for _ in partition[0]..partition[1] {
        base(r, version)?;
        let tag = r.u8()?;
        let at = r.f64s::<3>()?;
        let edges = r.u16()?;
        let faces = r.u16()?;
        skip_limit_points(r, faces)?;
        skip_list(r)?; // its edges
        skip_list(r)?; // its faces
        let _ = edges;
        finish(r, version)?;
        vertices.push(Vertex { tag, at });
    }
    let mut edges = Vec::with_capacity((partition[2] - partition[1]) as usize);
    for _ in partition[1]..partition[2] {
        base(r, version)?;
        let tag = r.u8()?;
        let _faces = r.u16()?;
        r.skip(2 * 8)?; // sector coefficients, worked out again by the library
        let sharpness = r.f64()?;
        let ends = list(r)?;
        let [Some(a), Some(b)] = [ends.first(), ends.get(1)].map(|e| e.map(|e| e.0)) else {
            return Err(bad("a SubD edge without two points"));
        };
        skip_list(r)?; // its faces
        let mut edge = Edge {
            tag,
            sharpness,
            ends: [a, b],
        };
        if version < 70 {
            r.u8()?;
        } else {
            // Rhino 8 adds the sharpness at the edge's other end.
            let mut size = r.u8()?;
            if version >= 80 && size == 8 {
                let other = r.f64()?;
                edge.sharpness = f64::midpoint(edge.sharpness, other);
                size = r.u8()?;
            }
            finish_from(r, size)?;
        }
        edges.push(edge);
    }
    let mut faces = Vec::with_capacity((partition[3] - partition[2]) as usize);
    for _ in partition[2]..partition[3] {
        base(r, version)?;
        let _level_zero = r.u32()?;
        let _parent = r.u32()?;
        let _count = r.u16()?;
        let edges = list(r)?;
        let mut face = Face {
            edges: edges
                .iter()
                .map(|&(id, flags)| (id, flags & 1 != 0))
                .collect(),
            colour: None,
            texture: None,
        };
        if version < 70 {
            r.u8()?;
        } else {
            face_additions(r, &mut face)?;
        }
        faces.push(face);
    }
    build(&partition, &vertices, &edges, &faces)
}

/// The start every component shares: its archive number, id and level, then (from version 70)
/// its additions.
fn base(r: &mut Reader, version: u32) -> Result<()> {
    r.skip(4 + 4 + 2)?;
    if version < 70 {
        if r.u8()? != 0 {
            r.skip(3 * 8)?;
        }
        if r.u8()? != 0 {
            r.skip(3 * 8)?;
        }
        return Ok(());
    }
    // Additions of sizes 24 (unused), 4 (group) and 5 (symmetry), each possibly absent.
    for size in [24u8, 4, 5] {
        match r.u8()? {
            0 => {}
            END => return Ok(()),
            s if s == size => r.skip(usize::from(s))?,
            _ => return Err(bad("a SubD part of unknown layout")),
        }
    }
    finish(r, version)
}

/// Reads the rest of a component's additions up to its end mark.
fn finish(r: &mut Reader, version: u32) -> Result<()> {
    if version < 70 {
        return Ok(());
    }
    let size = r.u8()?;
    finish_from(r, size)
}

fn finish_from(r: &mut Reader, mut size: u8) -> Result<()> {
    loop {
        match size {
            END => return Ok(()),
            CHUNKED => {
                r.chunk()?;
            }
            0 => {}
            s => r.skip(usize::from(s))?,
        }
        size = r.u8()?;
    }
}

/// A list of component references: a count, then each one's archive number and flags.
fn list(r: &mut Reader) -> Result<Vec<(u32, u8)>> {
    let count = usize::from(r.u16()?);
    if count * 5 > r.left() {
        return Err(bad("a SubD list longer than its record"));
    }
    (0..count).map(|_| Ok((r.u32()?, r.u8()?))).collect()
}

fn skip_list(r: &mut Reader) -> Result<()> {
    let count = usize::from(r.u16()?);
    r.skip(count * 5)
}

/// Limit points Rhino may have saved with a point: a flag, then each one's place, tangents,
/// normal and face.
fn skip_limit_points(r: &mut Reader, faces: u16) -> Result<()> {
    if r.u8()? == 0 {
        return Ok(());
    }
    let count = r.u32()?;
    if count == 0 || count > u32::from(faces) {
        return Err(bad("a SubD point with damaged limit points"));
    }
    r.skip(count as usize * (4 * 3 * 8 + 5))
}

/// A face's additions: pack rectangle (34), material (4), colour (4), pack id (4), and texture
/// points (4, then the points in runs of up to ten).
fn face_additions(r: &mut Reader, face: &mut Face) -> Result<()> {
    for (step, size) in [34u8, 4, 4, 4, 4].into_iter().enumerate() {
        match r.u8()? {
            END => return Ok(()),
            0 => continue,
            s if s == size => {}
            _ => return Err(bad("a SubD face of unknown layout")),
        }
        match step {
            2 => face.colour = Some(r.color()?),
            4 => {
                let corners = face.edges.len();
                let runs = r.u32()? as usize;
                if runs != corners / 10 {
                    return Err(bad("a SubD face with the wrong number of texture points"));
                }
                let mut points = Vec::with_capacity(corners);
                let left = corners % 10;
                for run in 0..runs + usize::from(left > 0) {
                    let n = if run < runs { 10 } else { left };
                    let size = r.u8()?;
                    if usize::from(size) != n * 24 {
                        return Err(bad("a SubD face's texture points are damaged"));
                    }
                    for _ in 0..n {
                        points.push(r.f64s::<3>()?);
                    }
                }
                face.texture = Some(points);
            }
            _ => r.skip(usize::from(size))?,
        }
    }
    finish(r, 70)
}

/// Curva's SubD from the records.
fn build(
    partition: &[u32; 4],
    vertices: &[Vertex],
    edges: &[Edge],
    faces: &[Face],
) -> Result<SubD> {
    let vertex_at = |archive: u32| -> Result<u32> {
        archive
            .checked_sub(partition[0])
            .filter(|&i| (i as usize) < vertices.len())
            .ok_or_else(|| bad("a SubD edge names a point it does not have"))
    };
    let edge_at = |archive: u32| -> Result<&Edge> {
        archive
            .checked_sub(partition[1])
            .and_then(|i| edges.get(i as usize))
            .ok_or_else(|| bad("a SubD face names an edge it does not have"))
    };
    let mut rings: Vec<Vec<u32>> = Vec::with_capacity(faces.len());
    for face in faces {
        let ring = face
            .edges
            .iter()
            .map(|&(id, backwards)| {
                let e = edge_at(id)?;
                vertex_at(if backwards { e.ends[1] } else { e.ends[0] })
            })
            .collect::<Result<Vec<u32>>>()?;
        rings.push(ring);
    }
    let points: Vec<Point> = vertices.iter().map(|v| v.at).collect();
    let mut cage = Mesh::new(points, &rings).map_err(|e| bad(e.to_string()))?;
    if faces.iter().all(|f| f.texture.is_some()) && !faces.is_empty() {
        let texture = faces
            .iter()
            .flat_map(|f| f.texture.iter().flatten().map(|p| [p[0], p[1]]))
            .collect();
        cage = cage
            .with_texture(Attribute::PerCorner(texture))
            .map_err(|e| bad(e.to_string()))?;
    }
    let mut subd = SubD::from_cage(cage);
    for e in edges {
        let ends = [vertex_at(e.ends[0])?, vertex_at(e.ends[1])?];
        // Rhino's edge tags: 2 crease; 1 and 4 smooth, possibly partly sharp.
        let sharpness = if e.tag == 2 {
            Sharpness::CREASE
        } else {
            Sharpness::new(e.sharpness)
        };
        subd = subd.with_sharpness(ends, sharpness);
    }
    // Rhino's point tags: 3 corner (crease and dart follow from the edges).
    let corners: Vec<u32> = (0..vertices.len())
        .filter(|&v| vertices[v].tag == 3)
        .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        .collect();
    subd = subd.with_corners(&corners);
    // `ON_Color`'s unset value is all ones; a face without a colour keeps none.
    let colours: Vec<Option<[f32; 3]>> = faces
        .iter()
        .map(|f| {
            f.colour
                .filter(|c| *c != [255; 4])
                .map(|c| [c[0], c[1], c[2]].map(|b| f32::from(b) / 255.0))
        })
        .collect();
    if colours.iter().any(Option::is_some) {
        subd = subd.clone().with_face_colours(colours).unwrap_or(subd);
    }
    Ok(subd)
}
