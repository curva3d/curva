//! Blocks (spec `docs/specs/blocks.md`): named definitions of objects, and instances that place a
//! definition with a placement. A definition is embedded in the model, or linked to another file
//! that the application reads.

use crate::{Document, Object, Shape};
use curva_kernel::{Point, Similarity};
use serde::{Deserialize, Serialize};

/// A placement: rows are the images of x, y and z, then the move (the engine's convention).
pub type Placement = [[f64; 4]; 4];

/// The placement that changes nothing.
pub const PLACED_AS_IS: Placement = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A placed copy of a block definition.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    /// The definition it draws.
    pub definition: u64,
    /// Where and how it is placed.
    pub placement: Placement,
}

/// A named group of objects placed by instances.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockDefinition {
    /// Identifies it.
    pub id: u64,
    /// Its name.
    pub name: String,
    /// Its objects, with their base point at the origin.
    pub objects: Vec<Object>,
    /// The file it is read from, for a linked definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked: Option<String>,
}

/// A point placed.
#[must_use]
pub fn placed(p: Point, m: &Placement) -> Point {
    std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
}

/// `first` then `then`.
#[must_use]
pub fn followed_by(first: &Placement, then: &Placement) -> Placement {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| first[i][k] * then[k][j]).sum();
        }
    }
    out
}

/// A placement that moves by `by`.
#[must_use]
pub fn moved_by(by: Point) -> Placement {
    let mut m = PLACED_AS_IS;
    m[3] = [by[0], by[1], by[2], 1.0];
    m
}

/// A placement from a turn, mirror or resize.
#[must_use]
pub fn of_similarity(change: &Similarity) -> Placement {
    let origin = change.point([0.0; 3]);
    let image = |axis: Point| -> [f64; 4] {
        let p = change.point(axis);
        [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2], 0.0]
    };
    [
        image([1.0, 0.0, 0.0]),
        image([0.0, 1.0, 0.0]),
        image([0.0, 0.0, 1.0]),
        [origin[0], origin[1], origin[2], 1.0],
    ]
}

/// The inverse of an affine placement, if it has one.
#[must_use]
pub fn inverse(m: &Placement) -> Option<Placement> {
    let a = [
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ];
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-15 {
        return None;
    }
    let c =
        |r1: usize, c1: usize, r2: usize, c2: usize| a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1];
    let inv = [
        [
            c(1, 1, 2, 2) / det,
            -c(0, 1, 2, 2) / det,
            c(0, 1, 1, 2) / det,
        ],
        [
            -c(1, 0, 2, 2) / det,
            c(0, 0, 2, 2) / det,
            -c(0, 0, 1, 2) / det,
        ],
        [
            c(1, 0, 2, 1) / det,
            -c(0, 0, 2, 1) / det,
            c(0, 0, 1, 1) / det,
        ],
    ];
    let t = [m[3][0], m[3][1], m[3][2]];
    let it: [f64; 3] =
        std::array::from_fn(|j| -(t[0] * inv[0][j] + t[1] * inv[1][j] + t[2] * inv[2][j]));
    Some([
        [inv[0][0], inv[0][1], inv[0][2], 0.0],
        [inv[1][0], inv[1][1], inv[1][2], 0.0],
        [inv[2][0], inv[2][1], inv[2][2], 0.0],
        [it[0], it[1], it[2], 1.0],
    ])
}

/// A shape placed by an affine placement.
#[must_use]
pub fn shape_placed(shape: &Shape, m: &Placement) -> Option<Shape> {
    Some(match shape {
        Shape::Curve(c) => Shape::Curve(c.mapped_affine(*m).ok()?),
        Shape::Body(b) => Shape::Body(b.transformed(*m).ok()?),
        Shape::Instance(i) => Shape::Instance(Instance {
            definition: i.definition,
            placement: followed_by(&i.placement, m),
        }),
    })
}

/// How deep instances may nest before drawing stops, in case of a loop.
const DEEPEST: usize = 16;

impl Document {
    /// The block definitions.
    #[must_use]
    pub fn definitions(&self) -> &[BlockDefinition] {
        &self.state.blocks
    }

    /// The definition with this name, ignoring case.
    #[must_use]
    pub fn definition_named(&self, name: &str) -> Option<&BlockDefinition> {
        self.state
            .blocks
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// The instances of a definition, at the top of the model.
    #[must_use]
    pub fn instances_of(&self, definition: u64) -> Vec<u64> {
        self.state
            .objects
            .iter()
            .filter(|o| matches!(&o.shape, Shape::Instance(i) if i.definition == definition))
            .map(|o| o.id)
            .collect()
    }

    /// Makes a definition from objects, with `base` as its base point: the objects leave the
    /// model and one instance takes their place. Returns the definition's and the instance's ids.
    pub fn define_block(&mut self, name: String, base: Point, ids: &[u64]) -> (u64, u64) {
        let to_origin = moved_by([-base[0], -base[1], -base[2]]);
        let mut objects: Vec<Object> = self
            .state
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .cloned()
            .collect();
        for object in &mut objects {
            if let Some(shape) = shape_placed(&object.shape, &to_origin) {
                object.shape = shape;
            }
            object.group = None;
        }
        let layer = objects
            .first()
            .map_or(self.state.current_layer, |o| o.layer);
        self.remove(ids);
        let id = self.state.next_block;
        self.state.next_block += 1;
        self.state.blocks.push(BlockDefinition {
            id,
            name,
            objects,
            linked: None,
        });
        let instance = self.add_shape_on(
            Shape::Instance(Instance {
                definition: id,
                placement: moved_by(base),
            }),
            Some(layer),
        );
        (id, instance)
    }

    /// Adds a definition from objects already at their base (a file read for Insert).
    pub fn add_definition(
        &mut self,
        name: String,
        objects: Vec<Object>,
        linked: Option<String>,
    ) -> u64 {
        let id = self.state.next_block;
        self.state.next_block += 1;
        self.state.blocks.push(BlockDefinition {
            id,
            name,
            objects,
            linked,
        });
        id
    }

    /// Places an instance of a definition.
    pub fn insert_block(&mut self, definition: u64, placement: Placement) -> u64 {
        self.add_shape(Shape::Instance(Instance {
            definition,
            placement,
        }))
    }

    /// Whether `inner` is `outer` or used inside it, at any depth: placing `inner` in `outer`
    /// would then draw it inside itself.
    #[must_use]
    pub fn block_contains(&self, outer: u64, inner: u64) -> bool {
        fn visit(doc: &Document, at: u64, wanted: u64, depth: usize) -> bool {
            if at == wanted {
                return true;
            }
            if depth > DEEPEST {
                return true;
            }
            doc.state
                .blocks
                .iter()
                .find(|d| d.id == at)
                .is_some_and(|d| {
                    d.objects.iter().any(|o| match &o.shape {
                        Shape::Instance(i) => visit(doc, i.definition, wanted, depth + 1),
                        _ => false,
                    })
                })
        }
        visit(self, outer, inner, 0)
    }

    /// The plain shapes an instance draws, placed, nested instances opened, each with the layer
    /// of the definition object it comes from.
    #[must_use]
    pub fn instance_shapes(&self, instance: &Instance) -> Vec<(Shape, u64)> {
        self.state.instance_shapes(instance)
    }
}

impl crate::State {
    /// The plain shapes an instance draws, placed, nested instances opened, each with the layer
    /// of the definition object it comes from.
    #[must_use]
    pub fn instance_shapes(&self, instance: &Instance) -> Vec<(Shape, u64)> {
        fn open(
            state: &crate::State,
            instance: &Instance,
            depth: usize,
            out: &mut Vec<(Shape, u64)>,
        ) {
            if depth > DEEPEST {
                return;
            }
            let Some(definition) = state.blocks.iter().find(|d| d.id == instance.definition) else {
                return;
            };
            for object in &definition.objects {
                match &object.shape {
                    Shape::Instance(inner) => {
                        let placed = Instance {
                            definition: inner.definition,
                            placement: followed_by(&inner.placement, &instance.placement),
                        };
                        open(state, &placed, depth + 1, out);
                    }
                    shape => {
                        if let Some(placed) = shape_placed(shape, &instance.placement) {
                            out.push((placed, object.layer));
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        open(self, instance, 0, &mut out);
        out
    }
}

impl Document {
    /// Replaces instances by the objects of their definitions, placed (one level). Returns the
    /// new objects.
    pub fn explode_instances(&mut self, ids: &[u64]) -> Vec<u64> {
        let found: Vec<(u64, Instance, u64)> = self
            .state
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .filter_map(|o| match &o.shape {
                Shape::Instance(i) => Some((o.id, *i, o.layer)),
                _ => None,
            })
            .collect();
        let mut added = Vec::new();
        for (id, instance, layer) in found {
            let Some(definition) = self
                .state
                .blocks
                .iter()
                .find(|d| d.id == instance.definition)
                .cloned()
            else {
                continue;
            };
            self.remove(&[id]);
            for object in definition.objects {
                if let Some(shape) = shape_placed(&object.shape, &instance.placement) {
                    added.push(self.add_shape_on(shape, Some(object.layer).or(Some(layer))));
                }
            }
        }
        added
    }

    /// Points instances at another definition.
    pub fn replace_definition(&mut self, ids: &[u64], definition: u64) -> usize {
        let mut count = 0;
        for object in &mut self.state.objects {
            if ids.contains(&object.id)
                && let Shape::Instance(i) = &mut object.shape
            {
                i.definition = definition;
                count += 1;
            }
        }
        count
    }

    /// Renames a definition.
    pub fn rename_definition(&mut self, definition: u64, name: String) {
        if let Some(d) = self.state.blocks.iter_mut().find(|d| d.id == definition) {
            d.name = name;
        }
    }

    /// Deletes a definition no instance uses (anywhere); returns whether it went.
    pub fn delete_definition(&mut self, definition: u64) -> bool {
        let used = !self.instances_of(definition).is_empty()
            || self.state.blocks.iter().any(|d| {
                d.objects
                    .iter()
                    .any(|o| matches!(&o.shape, Shape::Instance(i) if i.definition == definition))
            });
        if used {
            return false;
        }
        self.state.blocks.retain(|d| d.id != definition);
        true
    }

    /// A definition's objects replaced, as when its linked file changed.
    pub fn set_definition_objects(&mut self, definition: u64, objects: Vec<Object>) {
        if let Some(d) = self.state.blocks.iter_mut().find(|d| d.id == definition) {
            d.objects = objects;
        }
    }

    /// A linked definition made embedded: it keeps its objects and forgets its file.
    pub fn embed_definition(&mut self, definition: u64) {
        if let Some(d) = self.state.blocks.iter_mut().find(|d| d.id == definition) {
            d.linked = None;
        }
    }

    /// Opens a definition for editing in place: its objects come into the model, placed as
    /// `instance` places them, and its instances hide. Returns the objects to edit.
    pub fn start_block_edit(&mut self, instance: u64) -> Option<Vec<u64>> {
        let (definition, placement) = self
            .state
            .objects
            .iter()
            .find(|o| o.id == instance)
            .and_then(|o| match &o.shape {
                Shape::Instance(i) => Some((i.definition, i.placement)),
                _ => None,
            })?;
        let objects = self
            .state
            .blocks
            .iter()
            .find(|d| d.id == definition)?
            .objects
            .clone();
        let instances = self.instances_of(definition);
        for object in &mut self.state.objects {
            if instances.contains(&object.id) {
                object.hidden = true;
            }
        }
        let mut added = Vec::new();
        for object in objects {
            if let Some(shape) = shape_placed(&object.shape, &placement) {
                let id = self.add_shape_on(shape, Some(object.layer));
                added.push(id);
            }
        }
        self.state.block_edit = Some(BlockEdit {
            definition,
            placement,
            objects: added.clone(),
        });
        Some(added)
    }

    /// Saves the definition being edited from the objects being edited (and any added to the
    /// edit since), and shows its instances again.
    pub fn finish_block_edit(&mut self) -> Option<u64> {
        let edit = self.state.block_edit.take()?;
        let back = inverse(&edit.placement)?;
        let objects: Vec<Object> = self
            .state
            .objects
            .iter()
            .filter(|o| edit.objects.contains(&o.id))
            .filter_map(|o| {
                let mut o = o.clone();
                o.shape = shape_placed(&o.shape, &back)?;
                Some(o)
            })
            .collect();
        self.remove(&edit.objects);
        self.set_definition_objects(edit.definition, objects);
        let instances = self.instances_of(edit.definition);
        for object in &mut self.state.objects {
            if instances.contains(&object.id) {
                object.hidden = false;
            }
        }
        Some(edit.definition)
    }

    /// Whether a block is being edited in place.
    #[must_use]
    pub fn block_editing(&self) -> Option<&BlockEdit> {
        self.state.block_edit.as_ref()
    }
}

/// A definition being edited in place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockEdit {
    /// The definition.
    pub definition: u64,
    /// The placement of the instance it is edited through.
    pub placement: Placement,
    /// The objects standing in for its own while it is edited.
    pub objects: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use super::{PLACED_AS_IS, moved_by};
    use crate::{Document, Shape};
    use curva_kernel::{Body, Curve};

    fn chair() -> (Document, Vec<u64>) {
        let mut doc = Document::default();
        let seat = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 0.2]).unwrap());
        let leg = doc.add(Curve::line([0.0; 3], [0.0, 0.0, -1.0]).unwrap());
        (doc, vec![seat, leg])
    }

    #[test]
    fn a_block_is_made_placed_and_exploded() {
        let (mut doc, ids) = chair();
        let (definition, first) = doc.define_block("Chair".to_owned(), [0.0; 3], &ids);
        assert_eq!(doc.state().objects.len(), 1);
        let second = doc.insert_block(definition, moved_by([5.0, 0.0, 0.0]));
        assert_eq!(doc.instances_of(definition), vec![first, second]);
        let Shape::Instance(i) = &doc.state().objects[1].shape else {
            panic!()
        };
        let shapes = doc.instance_shapes(i);
        assert_eq!(shapes.len(), 2);
        let (low, _) = shapes[0].0.body().unwrap().bounds().unwrap();
        assert!((low[0] - 5.0).abs() < 1e-9);
        let parts = doc.explode_instances(&[second]);
        assert_eq!(parts.len(), 2);
        assert!(
            !doc.delete_definition(definition),
            "still used by the first instance"
        );
    }

    #[test]
    fn editing_a_definition_changes_every_instance() {
        let (mut doc, ids) = chair();
        let (definition, first) = doc.define_block("Chair".to_owned(), [0.0; 3], &ids);
        let second = doc.insert_block(definition, moved_by([5.0, 0.0, 0.0]));
        let editing = doc.start_block_edit(second).unwrap();
        assert_eq!(editing.len(), 2);
        // Take the leg away while editing.
        let leg = editing
            .iter()
            .copied()
            .find(|id| {
                doc.state()
                    .objects
                    .iter()
                    .find(|o| o.id == *id)
                    .unwrap()
                    .curve()
                    .is_some()
            })
            .unwrap();
        doc.remove(&[leg]);
        doc.finish_block_edit();
        let Shape::Instance(i) = &doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == first)
            .unwrap()
            .shape
        else {
            panic!()
        };
        assert_eq!(doc.instance_shapes(i).len(), 1);
    }

    #[test]
    fn a_block_cannot_contain_itself() {
        let (mut doc, ids) = chair();
        let (definition, instance) = doc.define_block("Chair".to_owned(), [0.0; 3], &ids);
        let (outer, _) = doc.define_block("Table".to_owned(), [0.0; 3], &[instance]);
        assert!(doc.block_contains(outer, definition));
        assert!(!doc.block_contains(definition, outer));
        let _ = PLACED_AS_IS;
    }
}
