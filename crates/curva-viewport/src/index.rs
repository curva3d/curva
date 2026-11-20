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

    /// The first item, nearest first, that `accept` takes: items are tried in increasing order of
    /// their `distance`, and of their number between equal distances; `None` for an item means it
    /// is not offered. `bound` gives, for a box, no more than the distance of any item inside it;
    /// a box whose bound is beyond `reach` is not looked into, nor an item beyond it. The answer is
    /// the same as trying every item in that order, but only the boxes near enough are opened.
    pub(crate) fn nearest(
        &self,
        reach: f64,
        bound: impl Fn(Bounds) -> f64,
        mut distance: impl FnMut(usize) -> Option<f64>,
        mut accept: impl FnMut(usize) -> bool,
    ) -> Option<(usize, f64)> {
        use std::cmp::Reverse;
        let mut boxes = std::collections::BinaryHeap::new();
        let mut items = std::collections::BinaryHeap::new();
        if let Some(root) = self.nodes.first() {
            boxes.push(Reverse((Nearness(bound(root.bounds)), 0)));
        }
        loop {
            let next_box = boxes.peek().map(|Reverse((Nearness(d), _))| *d);
            let next_item = items.peek().map(|Reverse((Nearness(d), _))| *d);
            match (next_item, next_box) {
                // An item nearer than any box still shut is the nearest left.
                (Some(item), next) if next.is_none_or(|b| item < b) => {
                    let Reverse((Nearness(d), at)) = items.pop()?;
                    if accept(at) {
                        return Some((at, d));
                    }
                }
                (_, Some(_)) => {
                    let Reverse((_, at)) = boxes.pop()?;
                    let node = &self.nodes[at];
                    if node.count == 0 {
                        for child in [node.first, node.first + 1] {
                            let d = bound(self.nodes[child].bounds);
                            if d <= reach {
                                boxes.push(Reverse((Nearness(d), child)));
                            }
                        }
                    } else {
                        for &item in &self.order[node.first..node.first + node.count] {
                            if let Some(d) = distance(item).filter(|d| *d <= reach) {
                                items.push(Reverse((Nearness(d), item)));
                            }
                        }
                    }
                }
                (_, None) => return None,
            }
        }
    }
}

/// A distance that sorts, for the queue of [`BoxTree::nearest`]; distances are never NaN.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Nearness(f64);

impl Eq for Nearness {}

impl Ord for Nearness {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for Nearness {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
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

    /// The nearest-first search takes the same item as trying every item in order of distance,
    /// then number, including between items equally near, with some items not offered and some
    /// not taken.
    #[test]
    fn the_nearest_search_takes_what_trying_every_item_in_order_takes() {
        let mut items: Vec<Bounds> = boxes(600).into_iter().map(|(low, _)| (low, low)).collect();
        // Copies of earlier points, for ties.
        items.extend(items[..60].to_vec());
        let tree = BoxTree::new(items.clone());
        let flat = |p: Point, at: [f64; 2]| (p[0] - at[0]).hypot(p[1] - at[1]);
        let to_box = |(low, high): Bounds, at: [f64; 2]| {
            let across = (low[0] - at[0]).max(at[0] - high[0]).max(0.0);
            let down = (low[1] - at[1]).max(at[1] - high[1]).max(0.0);
            across.hypot(down)
        };
        let offered = |item: usize| item % 5 != 1;
        let taken = |item: usize| !item.is_multiple_of(3);
        for step in 0..400 {
            let at = [f64::from(step % 20) * 5.3, f64::from(step / 20) * 5.3];
            let reach = 2.0 + f64::from(step % 7);
            let mut every: Vec<(f64, usize)> = (0..items.len())
                .filter(|&item| offered(item))
                .map(|item| (flat(items[item].0, at), item))
                .filter(|(d, _)| *d <= reach)
                .collect();
            every.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let expected = every.into_iter().find(|(_, item)| taken(*item));
            let found = tree.nearest(
                reach,
                |bounds| to_box(bounds, at),
                |item| offered(item).then(|| flat(items[item].0, at)),
                taken,
            );
            assert_eq!(found.map(|(item, _)| item), expected.map(|(_, item)| item));
        }
    }
}
