//! A tree of boxes over the scene's objects (a bounding volume hierarchy), so a question about
//! one small place, such as what lies within reach of the cursor, visits only the objects near
//! it instead of every object (spec `display-pipeline.md` step 12; issue #130).
//!
//! Built top down by splitting at the middle item along the box's longest side, as in Ericson,
//! *Real-Time Collision Detection* (2005), chapter 6, "Bounding Volume Hierarchies", §6.2.1
//! (top-down construction, median split).

use curva_kernel::Point;

/// Items per leaf: few enough that a leaf's items are cheap to test one by one.
const LEAF_ITEMS: usize = 8;

/// A box as its lowest and highest corners.
pub(crate) type Bounds = (Point, Point);

/// The tree. Items are numbered as given; a query answers with their numbers.
#[derive(Clone, Debug, Default)]
pub(crate) struct BoxTree {
    nodes: Vec<Node>,
    /// Item numbers, ordered so each leaf's items sit together.
    order: Vec<usize>,
    /// Each item's own box, by item number.
    items: Vec<Bounds>,
}

#[derive(Clone, Debug)]
struct Node {
    bounds: Bounds,
    /// A leaf holds `order[first..first + count]`; an inner node (`count` 0) has its children at
    /// `first` and `first + 1`.
    first: usize,
    count: usize,
}

fn around(boxes: impl IntoIterator<Item = Bounds>) -> Bounds {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for (l, h) in boxes {
        for axis in 0..3 {
            low[axis] = low[axis].min(l[axis]);
            high[axis] = high[axis].max(h[axis]);
        }
    }
    (low, high)
}

impl BoxTree {
    /// The tree over `items`, each item's box in order.
    pub(crate) fn new(items: Vec<Bounds>) -> Self {
        let mut tree = Self {
            nodes: Vec::new(),
            order: (0..items.len()).collect(),
            items,
        };
        if !tree.items.is_empty() {
            tree.nodes.push(Node {
                bounds: around(tree.items.iter().copied()),
                first: 0,
                count: tree.items.len(),
            });
            tree.split(0, 0, tree.items.len());
        }
        tree
    }

    /// Splits node `at`, holding `order[start..end]`, until its leaves are small.
    fn split(&mut self, at: usize, start: usize, end: usize) {
        if end - start <= LEAF_ITEMS {
            return;
        }
        let (low, high) = self.nodes[at].bounds;
        let axis = (0..3)
            .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
            .unwrap_or(0);
        let middle = start + (end - start) / 2;
        let items = &self.items;
        self.order[start..end].select_nth_unstable_by(middle - start, |&a, &b| {
            let (la, ha) = items[a];
            let (lb, hb) = items[b];
            (la[axis] + ha[axis]).total_cmp(&(lb[axis] + hb[axis]))
        });
        let first = self.nodes.len();
        for (from, to) in [(start, middle), (middle, end)] {
            let bounds = around(self.order[from..to].iter().map(|&item| self.items[item]));
            self.nodes.push(Node {
                bounds,
                first: from,
                count: to - from,
            });
        }
        self.nodes[at].first = first;
        self.nodes[at].count = 0;
        self.split(first, start, middle);
        self.split(first + 1, middle, end);
    }

    /// The numbers of the items whose own box `keep` accepts, in increasing order. A node whose
    /// box `keep` refuses is not looked into, so `keep` must refuse a box only when it would
    /// refuse every box inside it.
    pub(crate) fn query(&self, keep: impl Fn(Bounds) -> bool) -> Vec<usize> {
        let mut found = Vec::new();
        let mut waiting = Vec::new();
        if !self.nodes.is_empty() {
            waiting.push(0);
        }
        while let Some(at) = waiting.pop() {
            let node = &self.nodes[at];
            if !keep(node.bounds) {
                continue;
            }
            if node.count == 0 {
                waiting.push(node.first);
                waiting.push(node.first + 1);
            } else {
                found.extend(
                    self.order[node.first..node.first + node.count]
                        .iter()
                        .copied()
                        .filter(|&item| keep(self.items[item])),
                );
            }
        }
        // In the order the items were given, so answers do not depend on the tree's shape.
        found.sort_unstable();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scattered set of boxes, the same every run.
    fn boxes(count: usize) -> Vec<Bounds> {
        let mut seed = 12345_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / f64::from(1u32 << 31) * 100.0
        };
        (0..count)
            .map(|_| {
                let low = [next(), next(), next() / 10.0];
                let size = [next() / 20.0, next() / 20.0, next() / 50.0];
                (low, [low[0] + size[0], low[1] + size[1], low[2] + size[2]])
            })
            .collect()
    }

    fn overlaps(a: Bounds, b: Bounds) -> bool {
        (0..3).all(|axis| a.0[axis] <= b.1[axis] && b.0[axis] <= a.1[axis])
    }

    #[test]
    fn a_query_finds_exactly_what_testing_every_item_finds() {
        let items = boxes(1000);
        let tree = BoxTree::new(items.clone());
        for probe in [
            ([10.0, 10.0, 0.0], [20.0, 25.0, 10.0]),
            ([50.0, 0.0, 0.0], [50.5, 100.0, 10.0]),
            ([-10.0, -10.0, -10.0], [-5.0, -5.0, -5.0]),
            ([0.0, 0.0, 0.0], [100.0, 100.0, 100.0]),
        ] {
            let every: Vec<usize> = (0..items.len())
                .filter(|&item| overlaps(items[item], probe))
                .collect();
            assert_eq!(tree.query(|bounds| overlaps(bounds, probe)), every);
        }
    }

    #[test]
    fn every_item_is_in_exactly_one_leaf_and_inside_its_parents() {
        let tree = BoxTree::new(boxes(333));
        let mut sorted = tree.order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..333).collect::<Vec<_>>());
        for node in tree.nodes.iter().filter(|node| node.count == 0) {
            for child in [node.first, node.first + 1] {
                let (low, high) = tree.nodes[child].bounds;
                for axis in 0..3 {
                    assert!(node.bounds.0[axis] <= low[axis]);
                    assert!(high[axis] <= node.bounds.1[axis]);
                }
            }
        }
        assert!(BoxTree::new(Vec::new()).query(|_| true).is_empty());
    }
}
