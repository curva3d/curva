//! Picked SubD parts, as the editing commands read them: by object, and as runs of points.

use crate::recipe::{Context, Value};
use curva_document::MeshPart;
use std::collections::BTreeMap;

/// The parts picked at step `step`, by the object they belong to.
pub(super) fn by_object(cx: &Context<'_>, step: usize) -> BTreeMap<u64, Vec<MeshPart>> {
    let mut picked: BTreeMap<u64, Vec<MeshPart>> = BTreeMap::new();
    if let Some(Value::MeshParts(parts)) = cx.values.get(step) {
        for &(id, part) in parts {
            picked.entry(id).or_default().push(part);
        }
    }
    picked
}

/// The picked edges among `parts`.
pub(super) fn edges(parts: &[MeshPart]) -> Vec<[u32; 2]> {
    parts
        .iter()
        .filter_map(|p| match *p {
            MeshPart::Edge(a, b) => Some([a, b]),
            _ => None,
        })
        .collect()
}

/// The picked parts as one run of points, as Rhino's Stitch and Bridge read a set: a picked
/// point alone, or edges joined end to end into one open run (a closed loop starts at its
/// lowest point and repeats it at the end). `None` when the parts are not one run.
pub(super) fn run(parts: &[MeshPart]) -> Option<Vec<u32>> {
    if let [MeshPart::Point(p)] = parts {
        return Some(vec![*p]);
    }
    let edges = edges(parts);
    if edges.is_empty() || edges.len() != parts.len() {
        return None;
    }
    let mut next: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &[a, b] in &edges {
        next.entry(a).or_default().push(b);
        next.entry(b).or_default().push(a);
    }
    if next.values().any(|n| n.len() > 2) {
        return None;
    }
    // An open run starts at the lower of its two ends; a loop at its lowest point.
    let start = next
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(&p, _)| p)
        .min()
        .or_else(|| next.keys().next().copied())?;
    let mut way = vec![start];
    let mut previous = None;
    let mut here = start;
    while let Some(&to) = next[&here].iter().find(|&&n| Some(n) != previous) {
        way.push(to);
        if to == start || way.len() > edges.len() + 1 {
            break;
        }
        previous = Some(here);
        here = to;
    }
    (way.len() == edges.len() + 1).then_some(way)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_join_into_one_run() {
        let parts = [
            MeshPart::edge(5, 2),
            MeshPart::edge(2, 7),
            MeshPart::edge(9, 5),
        ];
        assert_eq!(run(&parts), Some(vec![7, 2, 5, 9]));
        let square = [
            MeshPart::edge(1, 2),
            MeshPart::edge(2, 3),
            MeshPart::edge(3, 4),
            MeshPart::edge(4, 1),
        ];
        assert_eq!(run(&square), Some(vec![1, 2, 3, 4, 1]));
        assert_eq!(run(&[MeshPart::Point(4)]), Some(vec![4]));
        let apart = [MeshPart::edge(1, 2), MeshPart::edge(5, 6)];
        assert_eq!(run(&apart), None);
    }
}
