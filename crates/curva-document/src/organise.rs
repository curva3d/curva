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
