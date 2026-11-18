//! A tree of boxes around the bodies of a ray scene, so a ray only reaches the bodies whose box
//! it passes through (#317). Whether anything lies on a ray does not depend on the order the
//! bodies are tried in, so leaving out the ones it cannot reach changes no answer.

use crate::Point;

/// Bodies in a leaf at most.
const LEAF: usize = 4;

/// A box of the tree: the bodies `order[start..start + count]` when a leaf, else its two halves.
struct Node {
    low: Point,
    high: Point,
    start: usize,
    count: usize,
    halves: Option<(usize, usize)>,
}

/// The bodies' boxes, nested.
pub(super) struct BoxTree {
    boxes: Vec<(Point, Point)>,
    nodes: Vec<Node>,
    order: Vec<usize>,
}

impl BoxTree {
    /// The tree over `boxes` (low and high corners), each body by its index.
    pub(super) fn new(boxes: &[(Point, Point)]) -> Self {
        let mut tree = Self {
            boxes: boxes.to_vec(),
            nodes: Vec::new(),
            order: (0..boxes.len()).collect(),
        };
        if !boxes.is_empty() {
            tree.build(boxes, 0, boxes.len());
        }
        tree
    }

    /// Makes the node over `order[start..end]`, and below it; returns its index.
    fn build(&mut self, boxes: &[(Point, Point)], start: usize, end: usize) -> usize {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for &i in &self.order[start..end] {
            for k in 0..3 {
                low[k] = low[k].min(boxes[i].0[k]);
                high[k] = high[k].max(boxes[i].1[k]);
            }
        }
        let at = self.nodes.len();
        self.nodes.push(Node {
            low,
            high,
            start,
            count: end - start,
            halves: None,
        });
        if end - start > LEAF {
            // Split at the middle body along the box's longest side.
            let axis = (0..3)
                .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
                .unwrap_or(0);
            let middle = |i: usize| boxes[i].0[axis] + boxes[i].1[axis];
            let half = start + (end - start) / 2;
            self.order[start..end]
                .select_nth_unstable_by(half - start, |&a, &b| middle(a).total_cmp(&middle(b)));
            let first = self.build(boxes, start, half);
            let second = self.build(boxes, half, end);
            self.nodes[at].halves = Some((first, second));
        }
        at
    }

    /// The bodies whose box the ray from `from` along `way` for `far` (which may be infinite)
    /// passes through, in increasing order.
    pub(super) fn along(&self, from: Point, way: Point, far: f64) -> Vec<usize> {
        let mut found = Vec::new();
        let mut stack = if self.nodes.is_empty() {
            vec![]
        } else {
            vec![0]
        };
        while let Some(at) = stack.pop() {
            let node = &self.nodes[at];
            if !meets(node.low, node.high, from, way, far) {
                continue;
            }
            match node.halves {
                Some((first, second)) => {
                    stack.push(first);
                    stack.push(second);
                }
                None => found.extend(
                    self.order[node.start..node.start + node.count]
                        .iter()
                        .copied()
                        .filter(|&i| meets(self.boxes[i].0, self.boxes[i].1, from, way, far)),
                ),
            }
        }
        found.sort_unstable();
        found
    }
}

/// Whether the ray from `from` along `way` for `far` (which may be infinite) passes through the
/// box from `low` to `high`, its sides included.
pub(super) fn meets(low: Point, high: Point, from: Point, way: Point, far: f64) -> bool {
    let (mut enter, mut leave) = (0.0_f64, far);
    for k in 0..3 {
        if way[k] == 0.0 {
            if from[k] < low[k] || from[k] > high[k] {
                return false;
            }
            continue;
        }
        let (a, b) = ((low[k] - from[k]) / way[k], (high[k] - from[k]) / way[k]);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
        if enter > leave {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_reaches_only_the_boxes_it_passes_through() {
        // A row of 100 unit boxes along x, 3 apart.
        let boxes: Vec<(Point, Point)> = (0..100)
            .map(|i| {
                let x = f64::from(i) * 3.0;
                ([x, 0.0, 0.0], [x + 1.0, 1.0, 1.0])
            })
            .collect();
        let tree = BoxTree::new(&boxes);
        let up = [0.0, 0.0, 1.0];
        // Straight up through box 10, and from below it, however far.
        assert_eq!(tree.along([30.5, 0.5, -5.0], up, f64::INFINITY), [10]);
        // Between two boxes: none.
        assert!(tree.along([32.0, 0.5, -5.0], up, f64::INFINITY).is_empty());
        // Along the row: every box from the 20th on, but only as far as it goes.
        let along = tree.along([60.5, 0.5, 0.5], [1.0, 0.0, 0.0], 10.0);
        assert_eq!(along, [20, 21, 22, 23]);
        // Stopping short of a box misses it.
        assert!(tree.along([30.5, 0.5, -5.0], up, 4.0).is_empty());
        // Every box a ray meets is found, as testing each one would.
        for (from, way) in [
            ([-10.0, 0.5, 0.5], [1.0, 0.0, 0.0]),
            ([150.0, -50.0, 0.5], [0.0, 1.0, 0.0]),
            ([0.0, -1.0, -1.0], [0.8, 0.0, 0.6]),
        ] {
            let each: Vec<usize> = (0..boxes.len())
                .filter(|&i| meets(boxes[i].0, boxes[i].1, from, way, f64::INFINITY))
                .collect();
            assert_eq!(tree.along(from, way, f64::INFINITY), each);
        }
    }
}
