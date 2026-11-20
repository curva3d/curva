//! SketchUp files (`.skp`) read into a Curva model, through OpenSKP's pure-Rust reader (the
//! owner's choice, 18 November 2026; SketchUp's own toolkit may not ship with Curva).
//!
//! What comes in, as Rhino brings it in:
//! - faces as meshes, one for each tag and material in each component or group, triangulated by
//!   OpenSKP (which handles holes and concave faces);
//! - edges that bound no face as polylines;
//! - components as block definitions and their placements as block instances;
//! - groups at the top level as their objects, placed and grouped; groups inside a component as
//!   objects of its definition;
//! - tags as layers, with their colours and visibility; materials as object colours;
//! - the model's unit (SketchUp keeps every length in inches; they come in in the file's unit).
//!
//! Left out, and listed in the report: hidden faces and edges, texts, dimensions, section planes,
//! construction lines and points, and images.

pub mod format;

use curva_document::blocks::Placement;
use curva_document::{Document, Shape, Unit};

use curva_kernel::{Curve, Point};
/// A SketchUp model as OpenSKP reads it.
pub use openskp::model::SkpModel as Model;
use openskp::model::{Definition, EntityId, SkpModel};
use std::collections::{BTreeMap, HashMap};

/// What came in and what was left out, by kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Objects brought in, by kind.
    pub brought: Vec<(String, usize)>,
    /// Things left out, by kind.
    pub skipped: Vec<(String, usize)>,
    /// Anything else worth saying.
    pub notes: Vec<String>,
}

impl Report {
    fn count(list: &mut Vec<(String, usize)>, kind: &str, n: usize) {
        if n == 0 {
            return;
        }
        match list.iter_mut().find(|(k, _)| k == kind) {
            Some((_, count)) => *count += n,
            None => list.push((kind.to_owned(), n)),
        }
    }

    /// One line for the command bar: what came in, then what was left out.
    #[must_use]
    pub fn summary(&self) -> String {
        let list = |items: &[(String, usize)]| {
            items
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut text = list(&self.brought);
        if !self.skipped.is_empty() {
            text.push_str("; left out ");
            text.push_str(&list(&self.skipped));
        }
        text
    }
}

/// A SketchUp file's model, read from its bytes.
///
/// # Errors
///
/// When the bytes are not a SketchUp file OpenSKP can read, saying why.
pub fn read(bytes: &[u8]) -> Result<SkpModel, String> {
    openskp::SkpFile::from_buffer(bytes.to_vec())
        .parse(&openskp::ParseOptions::default())
        .map_err(|e| e.to_string())
}

/// A new Curva model holding the SketchUp model, in its unit.
#[must_use]
pub fn open(model: &SkpModel) -> (Document, Report) {
    let mut doc = Document::default();
    let unit = unit_of(model);
    doc.set_unit(unit, false);
    let report = import(model, &mut doc);
    (doc, report)
}

/// The SketchUp model added to `doc`, in `doc`'s unit.
#[must_use]
pub fn import(model: &SkpModel, doc: &mut Document) -> Report {
    let mut reader = Reader {
        model,
        inch: 25.4 / doc.state().unit.millimetres(),
        materials: model
            .materials
            .iter()
            .filter_map(|m| m.id.map(|id| (id, m)))
            .collect(),
        layers: HashMap::new(),
        tag_names: tag_names(model),
        outer_tag: String::new(),
        unnamed_face_tags: false,
        blocks: HashMap::new(),
        report: Report::default(),
    };
    reader.layers_in(doc);
    let root = model.root();
    let made = reader.definition_objects(doc, root, &identity());
    // Top-level groups: their objects together as a group.
    for group in made.groups {
        if group.len() > 1 {
            doc.group(&group);
        }
    }
    if reader.unnamed_face_tags {
        reader.report.notes.push(
            "faces' own tags in files from before SketchUp 2021 are not read yet; such faces              take their group's or component's tag"
                .to_owned(),
        );
    }
    reader.report
}

/// The unit the SketchUp file works in, millimetres when it does not say (or works in
/// centimetres, which Curva does not offer).
fn unit_of(model: &SkpModel) -> Unit {
    let named = model.units.as_deref().unwrap_or("").to_lowercase();
    if named.contains("milli") || named == "mm" {
        Unit::Millimetres
    } else if named.contains("feet") || named.contains("foot") || named == "ft" {
        Unit::Feet
    } else if named.contains("inch") || named == "in" {
        Unit::Inches
    } else if named.contains("meter") || named.contains("metre") || named == "m" {
        Unit::Metres
    } else {
        Unit::Millimetres
    }
}

/// A placement that leaves things where they are.
fn identity() -> Placement {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `first` followed by `then`, in Curva's row form (`p' = p·A + t`).
fn followed_by(first: &Placement, then: &Placement) -> Placement {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| first[i][k] * then[k][j]).sum()))
}

/// A point under a placement.
fn placed(p: Point, m: &Placement) -> Point {
    std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
}

/// What one definition's contents became: the objects made, and at the top level each group's
/// objects, to be grouped.
#[derive(Default)]
struct Made {
    objects: Vec<u64>,
    groups: Vec<Vec<u64>>,
}

struct Reader<'a> {
    model: &'a SkpModel,
    /// Curva units in an inch.
    inch: f64,
    materials: HashMap<EntityId, &'a openskp::model::Material>,
    /// Each tag's Curva layer, by name.
    layers: HashMap<String, u64>,
    /// Each tag's name by its number, for faces that carry only the number.
    tag_names: HashMap<EntityId, String>,
    /// The tag of the group being read, which its untagged contents take, as SketchUp shows them.
    outer_tag: String,
    /// Whether some face's own tag could not be named (files from before SketchUp 2021).
    unnamed_face_tags: bool,
    /// Each component's Curva block definition, by its SketchUp id.
    blocks: HashMap<EntityId, u64>,
    report: Report,
}

impl Reader<'_> {
    /// The tags as layers, with their colours and visibility. SketchUp's default tag is Curva's
    /// default layer.
    fn layers_in(&mut self, doc: &mut Document) {
        for tag in &self.model.layers {
            if is_default_tag(&tag.name) {
                continue;
            }
            let id = doc.layer_at_path(&tag.name.replace("::", "-"));
            doc.set_layer_colour(id, tag.color.map(|c| f32::from(c) / 255.0));
            if tag.hidden {
                doc.set_layer_visible(id, false);
            }
            self.layers.insert(tag.name.clone(), id);
        }
    }

    fn layer(&self, doc: &Document, name: &str) -> u64 {
        self.layers
            .get(name)
            .copied()
            .unwrap_or(doc.state().current_layer)
    }

    /// The contents of `definition`, placed by `at`, added to `doc`: its faces, loose edges and
    /// placements. Components become instances of their block; groups are opened in place.
    fn definition_objects(
        &mut self,
        doc: &mut Document,
        definition: &Definition,
        at: &Placement,
    ) -> Made {
        let mut made = Made::default();
        made.objects.extend(self.faces_in(doc, definition, at));
        made.objects.extend(self.edges_in(doc, definition, at));
        let left = [
            ("texts", definition.texts.len()),
            ("dimensions", definition.dimensions.len()),
            ("section planes", definition.section_planes.len()),
            ("construction lines", definition.construction_lines.len()),
            ("construction points", definition.construction_points.len()),
        ];
        for (kind, n) in left {
            Report::count(&mut self.report.skipped, kind, n);
        }
        for instance in &definition.instances {
            let Some(inner) = instance
                .ref_idx
                .and_then(|id| self.model.definitions.get(&id))
            else {
                Report::count(
                    &mut self.report.skipped,
                    "placements of missing components",
                    1,
                );
                continue;
            };
            if instance.hidden {
                Report::count(&mut self.report.skipped, "hidden objects", 1);
                continue;
            }
            if inner.is_image {
                Report::count(&mut self.report.skipped, "images", 1);
                continue;
            }
            let local = placement_of(&instance.matrix, self.inch);
            let tag = if instance.layer.is_empty() {
                self.outer_tag.clone()
            } else {
                instance.layer.clone()
            };
            if inner.is_group {
                let outer = std::mem::replace(&mut self.outer_tag, tag);
                let objects = self
                    .definition_objects(doc, inner, &followed_by(&local, at))
                    .objects;
                self.outer_tag = outer;
                made.objects.extend(&objects);
                made.groups.push(objects);
                Report::count(&mut self.report.brought, "groups", 1);
            } else {
                let block = self.block(doc, inner);
                let id = doc.insert_block(block, followed_by(&local, at));
                let layer = self.layer(doc, &tag);
                doc.move_to_layer(&[id], layer);
                made.objects.push(id);
                Report::count(&mut self.report.brought, "block instances", 1);
            }
        }
        made
    }

    /// The block definition for component `definition`, made the first time it is placed.
    fn block(&mut self, doc: &mut Document, definition: &Definition) -> u64 {
        if let Some(&block) = self.blocks.get(&definition.id) {
            return block;
        }
        // A component's contents keep their own tags; its placements carry theirs.
        let outer = std::mem::take(&mut self.outer_tag);
        let objects = self
            .definition_objects(doc, definition, &identity())
            .objects;
        self.outer_tag = outer;
        let name = if definition.name.is_empty() {
            format!("Component {}", definition.id)
        } else {
            definition.name.clone()
        };
        let (block, instance) = doc.define_block(name, [0.0; 3], &objects);
        doc.remove(&[instance]);
        self.blocks.insert(definition.id, block);
        block
    }

    /// The faces of `definition` as meshes, one for each tag and material, placed by `at`.
    fn faces_in(
        &mut self,
        doc: &mut Document,
        definition: &Definition,
        at: &Placement,
    ) -> Vec<u64> {
        // Triangles by tag and material, with each mesh's points by their SketchUp ids.
        type Part = (Vec<Point>, HashMap<EntityId, u32>, Vec<[u32; 3]>);
        let mut parts: BTreeMap<(String, Option<EntityId>), Part> = BTreeMap::new();
        let mut hidden = 0;
        for face in definition.faces.values() {
            if face.hidden {
                hidden += 1;
                continue;
            }
            let loops: Vec<Vec<EntityId>> = face
                .loops
                .iter()
                .map(|lp| {
                    lp.iter()
                        .filter_map(|c| {
                            let edge = definition.edges.get(&c.edge_id)?;
                            Some(if c.orientation >= 0 {
                                edge.v1_id
                            } else {
                                edge.v2_id
                            })
                        })
                        .collect()
                })
                .filter(|lp: &Vec<EntityId>| lp.len() >= 3)
                .collect();
            if loops.is_empty() {
                continue;
            }
            let Some(normal) = face.normal.or_else(|| loop_normal(definition, &loops[0])) else {
                continue;
            };
            let triangles =
                openskp::triangulator::triangulate_face_3d(&definition.vertices, &loops, normal);
            let tag = if face.layer.is_empty() {
                match face.layer_id.and_then(|id| self.tag_names.get(&id)) {
                    Some(name) => name.clone(),
                    None => {
                        self.unnamed_face_tags |= face.layer_id.is_some_and(|id| id != 0);
                        self.outer_tag.clone()
                    }
                }
            } else {
                face.layer.clone()
            };
            let part = parts.entry((tag, face.material_id)).or_default();
            for triangle in triangles {
                let mut corners = [0u32; 3];
                for (corner, id) in corners.iter_mut().zip(triangle) {
                    *corner = match part.1.get(&id) {
                        Some(&index) => index,
                        None => {
                            let Some(v) = definition.vertices.get(&id) else {
                                continue;
                            };
                            let p = placed([v.x * self.inch, v.y * self.inch, v.z * self.inch], at);
                            part.0.push(p);
                            let index = (part.0.len() - 1) as u32;
                            part.1.insert(id, index);
                            index
                        }
                    };
                }
                if corners[0] != corners[1] && corners[1] != corners[2] && corners[0] != corners[2]
                {
                    part.2.push(corners);
                }
            }
        }
        Report::count(&mut self.report.skipped, "hidden faces", hidden);
        let mut made = Vec::new();
        for ((tag, material), (points, _, triangles)) in parts {
            if triangles.is_empty() {
                continue;
            }
            let Ok(mesh) = curva_mesh::Mesh::new(points, &triangles) else {
                Report::count(
                    &mut self.report.skipped,
                    "unreadable faces",
                    triangles.len(),
                );
                continue;
            };
            let layer = self.layer(doc, &tag);
            let id = doc.add_shape_on(Shape::Mesh(mesh), Some(layer));
            if let Some(colour) = material
                .and_then(|m| self.materials.get(&m))
                .map(|m| m.color)
            {
                doc.set_colour(&[id], Some([0, 1, 2].map(|k| f32::from(colour[k]) / 255.0)));
            }
            made.push(id);
            Report::count(&mut self.report.brought, "meshes", 1);
        }
        made
    }

    /// The edges of `definition` that bound no face, as polylines placed by `at`.
    fn edges_in(
        &mut self,
        doc: &mut Document,
        definition: &Definition,
        at: &Placement,
    ) -> Vec<u64> {
        let mut made = Vec::new();
        for run in definition.loose_edge_runs() {
            let hidden = run
                .edge_ids
                .iter()
                .filter_map(|id| definition.edges.get(id))
                .any(|e| e.hidden);
            if hidden {
                Report::count(&mut self.report.skipped, "hidden edges", 1);
                continue;
            }
            let points: Vec<Point> = run
                .vertex_ids
                .iter()
                .filter_map(|id| definition.vertices.get(id))
                .map(|v| placed([v.x * self.inch, v.y * self.inch, v.z * self.inch], at))
                .collect();
            let curve = match points.len() {
                0 | 1 => continue,
                2 => Curve::line(points[0], points[1]),
                _ => Curve::polyline(points, run.closed),
            };
            let Ok(curve) = curve else {
                continue;
            };
            let layer = run
                .edge_ids
                .first()
                .and_then(|id| definition.edges.get(id))
                .map_or_else(
                    || self.layer(doc, &self.outer_tag),
                    |e| {
                        let tag = if e.layer.is_empty() {
                            &self.outer_tag
                        } else {
                            &e.layer
                        };
                        self.layer(doc, tag)
                    },
                );
            made.push(doc.add_shape_on(Shape::Curve(curve), Some(layer)));
            Report::count(&mut self.report.brought, "curves", 1);
        }
        made
    }
}

/// Each tag's name by its number, as the edges give both: OpenSKP names a face's tag only in
/// SketchUp 2021 files and later, and gives older files' faces the number alone.
fn tag_names(model: &SkpModel) -> HashMap<EntityId, String> {
    std::iter::once(model.root())
        .chain(model.definitions.values())
        .flat_map(|d| d.edges.values())
        .filter_map(|e| Some((e.layer_id?, e.layer.clone())).filter(|(_, n)| !n.is_empty()))
        .collect()
}

/// Whether `name` is SketchUp's default tag ("Layer0", called "Untagged" since 2020).
fn is_default_tag(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case("layer0") || name.eq_ignore_ascii_case("untagged")
}

/// A face's normal from its outer loop, when the file gives none (Newell's method).
fn loop_normal(definition: &Definition, ids: &[EntityId]) -> Option<[f64; 3]> {
    let points: Vec<[f64; 3]> = ids
        .iter()
        .filter_map(|id| definition.vertices.get(id))
        .map(|v| [v.x, v.y, v.z])
        .collect();
    let mut n = [0.0; 3];
    for (k, p) in points.iter().enumerate() {
        let q = points[(k + 1) % points.len()];
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    (length > 0.0).then(|| n.map(|c| c / length))
}

/// A SketchUp placement as Curva's: OpenSKP gives 13 numbers, a row-major 3 × 3 (which acts on a
/// point as a column, `p' = R p + t`), the move (in inches) and a scale term that multiplies both
/// (`1/w`). Curva's rows act on a point as a row (`p' = p·A + t`), so `A` is `R` turned over.
fn placement_of(matrix: &[f64], inch: f64) -> Placement {
    if matrix.len() < 12 {
        return identity();
    }
    let s = matrix
        .get(12)
        .copied()
        .filter(|s| s.is_finite() && *s != 0.0)
        .unwrap_or(1.0);
    let r = |row: usize, column: usize| matrix[3 * row + column] * s;
    [
        [r(0, 0), r(1, 0), r(2, 0), 0.0],
        [r(0, 1), r(1, 1), r(2, 1), 0.0],
        [r(0, 2), r(1, 2), r(2, 2), 0.0],
        [
            matrix[9] * s * inch,
            matrix[10] * s * inch,
            matrix[11] * s * inch,
            1.0,
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sketchup_placement_turns_and_moves_as_curvas() {
        // A quarter turn about z (x to y), then a move of 10 inches along x, scale term 1.
        let matrix = [
            0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0, 1.0,
        ];
        let m = placement_of(&matrix, 25.4);
        let p = placed([1.0, 0.0, 0.0], &m);
        assert!(
            (p[0] - 254.0).abs() < 1e-9 && (p[1] - 1.0).abs() < 1e-9 && p[2].abs() < 1e-9,
            "{p:?}"
        );
        // The scale term doubles both the turn and the move.
        let doubled = [
            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.0,
        ];
        let p = placed([1.0, 1.0, 1.0], &placement_of(&doubled, 1.0));
        assert_eq!(p, [4.0, 2.0, 2.0]);
    }

    #[test]
    fn the_default_tag_is_the_default_layer() {
        assert!(is_default_tag("Layer0") && is_default_tag("Untagged") && !is_default_tag("Walls"));
    }
}
