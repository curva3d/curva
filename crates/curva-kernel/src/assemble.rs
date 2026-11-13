//! Putting bodies' faces together into one body and sewing them where their edges meet, taking
//! some faces of a body out on their own, and finding the edges only one face uses.

use crate::KernelError;
use crate::body::{Body, failed, native_of};
use solidcore_ops::Body as EngineBody;
use solidcore_record::brep::{Brep as Record, Loop, Region, Shell};

/// How close edges must be to be sewn together, in millimetres.
pub const SEW_TOLERANCE: f64 = 1e-3;

fn engine_body(record: &Record) -> Result<EngineBody, KernelError> {
    // Trusted: the parts each come from the engine, so the healer is not needed (and would lose
    // the record order).
    EngineBody::from_record(record, true)
        .map(|(body, _)| body)
        .map_err(|e| failed("building a body", e))
}

/// All parts' faces, edges and points in one record, nothing sewn yet.
fn merged(records: &[Record]) -> Result<Record, KernelError> {
    let first = records
        .first()
        .ok_or_else(|| KernelError("nothing to put together".to_owned()))?;
    let mut out = Record {
        version: first.version,
        tolerance: records.iter().map(|r| r.tolerance).fold(0.0, f64::max),
        regions: vec![Region {
            shells: Vec::new(),
            void: false,
        }],
        shells: Vec::new(),
        faces: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
    };
    for record in records {
        let shell_at = out.shells.len() as u32;
        let face_at = out.faces.len() as u32;
        let edge_at = out.edges.len() as u32;
        let vertex_at = out.vertices.len() as u32;
        for (index, region) in record.regions.iter().enumerate() {
            let shells = region.shells.iter().map(|s| s + shell_at);
            if index == 0 {
                out.regions[0].shells.extend(shells);
            } else {
                out.regions.push(Region {
                    shells: shells.collect(),
                    void: region.void,
                });
            }
        }
        for shell in &record.shells {
            out.shells.push(match shell {
                Shell::Faces { faces, wires } => Shell::Faces {
                    faces: faces
                        .iter()
                        .map(|f| {
                            let mut f = *f;
                            f.face += face_at;
                            f
                        })
                        .collect(),
                    wires: wires.iter().map(|w| w + edge_at).collect(),
                },
                Shell::Wire { edges } => Shell::Wire {
                    edges: edges.iter().map(|e| e + edge_at).collect(),
                },
                Shell::Vertex { vertex } => Shell::Vertex {
                    vertex: vertex + vertex_at,
                },
            });
        }
        for face in &record.faces {
            let mut face = face.clone();
            for lp in &mut face.loops {
                match lp {
                    Loop::Edges { edges } => {
                        for e in edges {
                            e.edge += edge_at;
                        }
                    }
                    Loop::Vertex { vertex } => *vertex += vertex_at,
                }
            }
            out.faces.push(face);
        }
        for edge in &record.edges {
            let mut edge = edge.clone();
            for v in &mut edge.vertices {
                *v += vertex_at;
            }
            for used in &mut edge.uses {
                used.face += face_at;
            }
            out.edges.push(edge);
        }
        out.vertices.extend(record.vertices.iter().cloned());
    }
    Ok(out)
}

/// Only the faces `keep` of a record, with the edges and points they use, as one open shell.
fn subset(record: &Record, keep: &[usize]) -> Result<Record, KernelError> {
    let mut face_map = vec![None; record.faces.len()];
    let mut kept_faces = Vec::new();
    for &index in keep {
        if index < record.faces.len() && face_map[index].is_none() {
            face_map[index] = Some(kept_faces.len() as u32);
            kept_faces.push(index);
        }
    }
    if kept_faces.is_empty() {
        return Err(KernelError("no faces to keep".to_owned()));
    }
    // Which way each kept face is used, from the shell that held it.
    let mut forward = vec![true; record.faces.len()];
    for shell in &record.shells {
        if let Shell::Faces { faces, .. } = shell {
            for f in faces {
                forward[f.face as usize] = f.forward;
            }
        }
    }
    let mut edge_map = vec![None; record.edges.len()];
    let mut vertex_map = vec![None; record.vertices.len()];
    let mut out = Record {
        version: record.version,
        tolerance: record.tolerance,
        regions: vec![Region {
            shells: vec![0],
            void: false,
        }],
        shells: Vec::new(),
        faces: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
    };
    let vertex = |old: u32, out: &mut Record, map: &mut Vec<Option<u32>>| -> u32 {
        if let Some(new) = map[old as usize] {
            return new;
        }
        let new = out.vertices.len() as u32;
        out.vertices.push(record.vertices[old as usize].clone());
        map[old as usize] = Some(new);
        new
    };
    for &index in &kept_faces {
        let mut face = record.faces[index].clone();
        for lp in &mut face.loops {
            match lp {
                Loop::Edges { edges } => {
                    for e in edges {
                        let old = e.edge as usize;
                        let new = if let Some(new) = edge_map[old] {
                            new
                        } else {
                            let mut edge = record.edges[old].clone();
                            for v in &mut edge.vertices {
                                *v = vertex(*v, &mut out, &mut vertex_map);
                            }
                            edge.uses = edge
                                .uses
                                .iter()
                                .filter_map(|used| {
                                    face_map[used.face as usize].map(|face| {
                                        let mut used = *used;
                                        used.face = face;
                                        used
                                    })
                                })
                                .collect();
                            let new = out.edges.len() as u32;
                            out.edges.push(edge);
                            edge_map[old] = Some(new);
                            new
                        };
                        e.edge = new;
                    }
                }
                Loop::Vertex { vertex: v } => *v = vertex(*v, &mut out, &mut vertex_map),
            }
        }
        out.faces.push(face);
    }
    out.shells.push(Shell::Faces {
        faces: kept_faces
            .iter()
            .enumerate()
            .map(|(new, &old)| solidcore_record::brep::OrientedFace {
                face: new as u32,
                forward: forward[old],
            })
            .collect(),
        wires: Vec::new(),
    });
    Ok(out)
}

impl Body {
    /// The faces of all `parts` as one body, sewn where their edges meet within
    /// [`SEW_TOLERANCE`]; faces that close round a space make a solid.
    pub fn assembled(parts: &[Body]) -> Result<Body, KernelError> {
        let records = parts
            .iter()
            .map(Body::record)
            .collect::<Result<Vec<_>, _>>()?;
        let body = engine_body(&merged(&records)?)?;
        let (sewn, _) = solidcore_ops::stitch::stitch(body, SEW_TOLERANCE)
            .map_err(|e| failed("sewing faces", e))?;
        Ok(Body::from_native(native_of(&sewn)?))
    }

    /// Only the faces numbered `keep` (as in [`crate::BodyMesh::triangle_faces`]), as a surface
    /// of their own.
    pub fn faces_only(&self, keep: &[usize]) -> Result<Body, KernelError> {
        let body = engine_body(&subset(&self.record()?, keep)?)?;
        let (sewn, _) = solidcore_ops::stitch::stitch(body, SEW_TOLERANCE)
            .map_err(|e| failed("sewing faces", e))?;
        Ok(Body::from_native(native_of(&sewn)?))
    }

    /// How many faces it has.
    pub fn face_count(&self) -> Result<usize, KernelError> {
        Ok(self.record()?.faces.len())
    }

    /// The edges only one face uses, the open border of a surface (numbered as in
    /// [`crate::BodyMesh::edge_ids`]).
    pub fn naked_edges(&self) -> Result<Vec<usize>, KernelError> {
        Ok(self
            .record()?
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.uses.len() == 1)
            .map(|(index, _)| index)
            .collect())
    }

    /// The body with faces that share an edge and lie on one surface merged into one, as
    /// Rhino's MergeAllFaces.
    pub fn faces_merged(&self) -> Result<Body, KernelError> {
        let engine = self.engine_body()?;
        let (_, order) = engine.to_record().map_err(|e| failed("reading faces", e))?;
        let merged = solidcore_ops::stitch::merge_faces(engine, &order, &[])
            .map_err(|e| failed("merging faces", e))?;
        Ok(Body::from_native(native_of(&merged)?))
    }
}

impl Body {
    /// A surface pulled straight along `direction` (of any length) by `distance` into a solid,
    /// as Rhino's ExtrudeSrf: the surface, a moved copy of it, and the sides its open edges
    /// sweep, sewn together.
    pub fn extruded_straight(
        &self,
        direction: crate::Point,
        distance: f64,
    ) -> Result<Body, KernelError> {
        let along = crate::unit(direction)
            .ok_or_else(|| KernelError("the direction has no length".to_owned()))?;
        let by = along.map(|v| v * distance);
        let mut parts = vec![self.clone(), self.translated(by)];
        for edge in self.naked_edges()? {
            let curve = self.edge_curve(edge)?;
            parts.push(Body::extruded(&[curve], along, distance, false)?);
        }
        let solid = Body::assembled(&parts)?;
        if !solid.is_solid() {
            return Err(KernelError(
                "the extruded faces did not close into a solid".to_owned(),
            ));
        }
        Ok(solid)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    fn square(size: f64) -> Body {
        let outline = Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [size, 0.0, 0.0],
                [size, size, 0.0],
                [0.0, size, 0.0],
            ],
            true,
        )
        .unwrap();
        Body::planar(&[outline]).unwrap()
    }

    #[test]
    fn a_flat_square_extrudes_into_a_box() {
        let solid = square(10.0)
            .extruded_straight([0.0, 0.0, 1.0], 5.0)
            .unwrap();
        let volume = solid.volume().unwrap();
        assert!((volume - 500.0).abs() < 1e-3, "{volume}");
        // Slanted, the volume is the base times the height.
        let slanted = square(10.0)
            .extruded_straight([1.0, 0.0, 1.0], 2f64.sqrt() * 5.0)
            .unwrap();
        let volume = slanted.volume().unwrap();
        assert!((volume - 500.0).abs() < 1e-3, "{volume}");
    }

    #[test]
    fn a_curved_surface_extrudes_into_a_solid() {
        // A surface lofted between two arcs, bulging upward, pulled up by 3.
        let low = Curve::interpolated(
            vec![[0.0, 0.0, 0.0], [5.0, 0.0, 2.0], [10.0, 0.0, 0.0]],
            false,
        )
        .unwrap();
        let high = Curve::interpolated(
            vec![[0.0, 8.0, 0.0], [5.0, 8.0, 2.0], [10.0, 8.0, 0.0]],
            false,
        )
        .unwrap();
        let sheet = Body::lofted(&[low, high], false).unwrap();
        let solid = sheet.extruded_straight([0.0, 0.0, 1.0], 3.0).unwrap();
        // Every vertical line through the base meets 3 of material: 10 × 8 × 3.
        let volume = solid.volume().unwrap();
        assert!((volume - 240.0).abs() < 0.5, "{volume}");
    }

    #[test]
    fn faces_can_be_taken_out_and_open_edges_found() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        assert!(cube.naked_edges().unwrap().is_empty());
        assert_eq!(cube.face_count().unwrap(), 6);
        let top = cube.faces_only(&[0]).unwrap();
        assert_eq!(top.face_count().unwrap(), 1);
        assert!(!top.is_solid());
        assert_eq!(top.naked_edges().unwrap().len(), 4);
        let rest = cube.faces_only(&[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(rest.naked_edges().unwrap().len(), 4);
        // And put back together, they close again.
        let again = Body::assembled(&[top, rest]).unwrap();
        assert!(again.is_solid());
        assert!((again.volume().unwrap() - 1.0).abs() < 1e-6);
    }
}
