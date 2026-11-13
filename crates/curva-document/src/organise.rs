//! Locking and grouping objects, as Rhino's Lock, Unlock, Group and Ungroup.

use crate::Document;

impl Document {
    /// Locks objects: they stay in sight but cannot be picked or changed until unlocked.
    pub fn lock(&mut self, ids: &[u64]) -> usize {
        let mut count = 0;
        for object in &mut self.state.objects {
            if ids.contains(&object.id) && !object.locked {
                object.locked = true;
                count += 1;
            }
        }
        self.selection.retain(|id| !ids.contains(id));
        count
    }

    /// Unlocks every locked object and returns how many there were.
    pub fn unlock_all(&mut self) -> usize {
        let mut count = 0;
        for object in &mut self.state.objects {
            if object.locked {
                object.locked = false;
                count += 1;
            }
        }
        count
    }

    /// Puts objects in one new group, out of any group they were in, and returns its id.
    pub fn group(&mut self, ids: &[u64]) -> u64 {
        let group = self.state.next_group;
        self.state.next_group += 1;
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                object.group = Some(group);
            }
        }
        group
    }

    /// Takes objects out of their groups and returns how many groups were broken up.
    pub fn ungroup(&mut self, ids: &[u64]) -> usize {
        let groups: std::collections::BTreeSet<u64> = self
            .state
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .filter_map(|object| object.group)
            .collect();
        for object in &mut self.state.objects {
            if object.group.is_some_and(|group| groups.contains(&group)) {
                object.group = None;
            }
        }
        groups.len()
    }

    /// Sets objects' linetype.
    pub fn set_linetype(&mut self, ids: &[u64], linetype: crate::Linetype) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                object.linetype = linetype;
            }
        }
    }

    /// Sets a layer's linetype (`ByLayer` means continuous for a layer).
    pub fn set_layer_linetype(&mut self, id: u64, linetype: crate::Linetype) {
        let linetype = if linetype.is_by_layer() {
            crate::Linetype::Continuous
        } else {
            linetype
        };
        if let Some(layer) = self.state.layers.iter_mut().find(|l| l.id == id) {
            layer.linetype = linetype;
        }
    }

    /// Sets (or, with an empty value, removes) a key of user text on objects.
    pub fn set_user_text(&mut self, ids: &[u64], key: &str, value: &str) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                if value.is_empty() {
                    object.user_text.remove(key);
                } else {
                    object.user_text.insert(key.to_owned(), value.to_owned());
                }
            }
        }
    }

    /// Copies layer, print width, linetype and name from one object to others.
    pub fn match_properties(&mut self, from: u64, to: &[u64]) -> usize {
        let Some(source) = self.state.objects.iter().find(|o| o.id == from).cloned() else {
            return 0;
        };
        let mut count = 0;
        for object in &mut self.state.objects {
            if to.contains(&object.id) && object.id != from {
                object.layer = source.layer;
                object.print_width = source.print_width;
                object.linetype = source.linetype;
                object.name.clone_from(&source.name);
                count += 1;
            }
        }
        count
    }

    /// Locks every unlocked object in sight and unlocks every locked one.
    pub fn lock_swap(&mut self) -> usize {
        let mut count = 0;
        for object in &mut self.state.objects {
            if !object.hidden && object.input_of.is_none() {
                object.locked = !object.locked;
                count += 1;
            }
        }
        self.selection
            .retain(|id| self.state.objects.iter().any(|o| o.id == *id && !o.locked));
        count
    }

    /// Unlocks some objects.
    pub fn unlock(&mut self, ids: &[u64]) -> usize {
        let mut count = 0;
        for object in &mut self.state.objects {
            if ids.contains(&object.id) && object.locked {
                object.locked = false;
                count += 1;
            }
        }
        count
    }

    /// Moves objects in the drawing order: to the front (drawn last) or back, or one step.
    pub fn reorder(&mut self, ids: &[u64], to_front: bool, all_the_way: bool) {
        let objects = &mut self.state.objects;
        if all_the_way {
            let (moving, mut rest): (Vec<_>, Vec<_>) =
                objects.drain(..).partition(|o| ids.contains(&o.id));
            if to_front {
                rest.extend(moving);
                *objects = rest;
            } else {
                let mut out = moving;
                out.extend(rest);
                *objects = out;
            }
            return;
        }
        let order: Vec<usize> = (0..objects.len()).collect();
        let indices: Vec<usize> = order
            .into_iter()
            .filter(|i| ids.contains(&objects[*i].id))
            .collect();
        if to_front {
            for i in indices.into_iter().rev() {
                if i + 1 < objects.len() {
                    objects.swap(i, i + 1);
                }
            }
        } else {
            for i in indices {
                if i > 0 {
                    objects.swap(i, i - 1);
                }
            }
        }
    }

    /// Removes layers nothing is on (but the current one and the first), and block definitions no
    /// instance uses; returns how many of each went.
    pub fn purge(&mut self) -> (usize, usize) {
        let current = self.state.current_layer;
        let used: std::collections::BTreeSet<u64> =
            self.state.objects.iter().map(|o| o.layer).collect();
        let before = self.state.layers.len();
        let first = self.state.layers.first().map(|l| l.id);
        self.state
            .layers
            .retain(|l| used.contains(&l.id) || l.id == current || Some(l.id) == first);
        let layers = before - self.state.layers.len();
        let mut blocks = 0;
        loop {
            let unused: Vec<u64> = self
                .state
                .blocks
                .iter()
                .map(|d| d.id)
                .filter(|id| {
                    !self.state.objects.iter().any(|o| matches!(&o.shape, crate::Shape::Instance(i) if i.definition == *id))
                        && !self.state.blocks.iter().any(|d| d.objects.iter().any(|o| matches!(&o.shape, crate::Shape::Instance(i) if i.definition == *id)))
                })
                .collect();
            if unused.is_empty() {
                break;
            }
            blocks += unused.len();
            self.state.blocks.retain(|d| !unused.contains(&d.id));
        }
        (layers, blocks)
    }

    /// The objects whose geometry the engine finds broken, with why.
    #[must_use]
    pub fn audit(&self) -> Vec<(u64, String)> {
        let mut found = Vec::new();
        for object in &self.state.objects {
            let problem = match &object.shape {
                crate::Shape::Curve(c) => c.sample().err().map(|e| e.to_string()),
                crate::Shape::Body(b) => b.mesh().err().map(|e| e.to_string()),
                crate::Shape::Instance(i) => {
                    (!self.state.blocks.iter().any(|d| d.id == i.definition))
                        .then(|| "a block instance of a missing definition".to_owned())
                }
            };
            if let Some(why) = problem {
                found.push((object.id, why));
            }
        }
        found
    }

    /// The objects with every other member of their groups added, as picking one member of a
    /// group picks it all.
    #[must_use]
    pub fn with_groups(&self, ids: &[u64]) -> Vec<u64> {
        let groups: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .filter_map(|object| object.group)
            .collect();
        let mut out = ids.to_vec();
        for object in &self.state.objects {
            if object.group.is_some_and(|group| groups.contains(&group))
                && !out.contains(&object.id)
            {
                out.push(object.id);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::Document;
    use curva_kernel::Curve;

    fn three() -> (Document, [u64; 3]) {
        let mut doc = Document::default();
        let ids =
            [0.0, 1.0, 2.0].map(|y| doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap()));
        (doc, ids)
    }

    #[test]
    fn locked_objects_cannot_be_picked_until_unlocked() {
        let (mut doc, [a, b, _]) = three();
        doc.select(&[a, b]);
        assert_eq!(doc.lock(&[a]), 1);
        assert_eq!(doc.selection(), vec![b]);
        assert!(!doc.selectable().contains(&a));
        assert_eq!(doc.unlock_all(), 1);
        assert!(doc.selectable().contains(&a));
    }

    #[test]
    fn a_group_is_picked_whole_and_ungroup_breaks_it() {
        let (mut doc, [a, b, c]) = three();
        doc.group(&[a, b]);
        let mut picked = doc.with_groups(&[b]);
        picked.sort_unstable();
        assert_eq!(picked, vec![a, b]);
        assert_eq!(doc.with_groups(&[c]), vec![c]);
        assert_eq!(doc.ungroup(&[a]), 1);
        assert_eq!(doc.with_groups(&[b]), vec![b]);
    }
}
