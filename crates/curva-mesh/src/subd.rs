//! The SubD object: the cage the user edits, from which the smooth surface is worked out (spec
//! `docs/specs/mesh-and-subd.md` §2.2).
//!
//! The cage is a polygon mesh at double precision with faces of any number of corners. Its edges
//! are smooth or creased, each with a sharpness (0 smooth, infinite for Rhino 8's creases, so
//! softer creases can come later without changing files), and its points are tagged as Rhino
//! tags them: smooth, crease, corner or dart. Texture coordinates ride on the cage's corners;
//! colours, when given, are one per face (a face may have none, as in Rhino). The smooth surface
//! itself is never stored: it is worked out by subdiv-kernels (`smooth.rs`).

use crate::{Mesh, Point};
use curva_kernel::Similarity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How sharp an edge is: 0 smooth, infinite a crease (Rhino 8's two kinds). Stored as `None` for
/// infinite, since text formats cannot write infinity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sharpness(Option<f64>);

impl Sharpness {
    /// A smooth edge.
    pub const SMOOTH: Self = Self(Some(0.0));
    /// A crease: infinitely sharp.
    pub const CREASE: Self = Self(None);

    /// A sharpness of `value`: 0 or less is smooth, infinity a crease; numbers between are the
    /// partly sharp edges Rhino 8 can make.
    #[must_use]
    pub fn new(value: f64) -> Self {
        if value.is_nan() || value <= 0.0 {
            Self::SMOOTH
        } else if value.is_infinite() {
            Self::CREASE
        } else {
            Self(Some(value))
        }
    }

    /// The sharpness as a number, infinity for a crease.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0.unwrap_or(f64::INFINITY)
    }

    /// Whether the edge is a crease.
    #[must_use]
    pub fn is_crease(self) -> bool {
        self.0.is_none()
    }
}

/// How a cage point behaves, as Rhino 8 tags it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VertexTag {
    /// The surface passes smoothly near it.
    #[default]
    Smooth,
    /// On a crease (two creased edges, or an open border): the surface follows the crease.
    Crease,
    /// Kept sharp: the surface passes through it, as a SubD box's corners.
    Corner,
    /// Where a crease ends inside smooth surface (one creased edge).
    Dart,
}

/// Whether a SubD is drawn flat (its cage) or smooth (its limit surface), as `SubDDisplayToggle`
/// switches it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubDDisplay {
    /// The smooth surface, as Rhino draws a SubD at first.
    #[default]
    Smooth,
    /// The cage's flat faces.
    Flat,
}

/// A SubD: its cage, the sharpness of its edges, its points' tags, and how it is drawn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubD {
    cage: Mesh,
    /// Edges that are not smooth, by their two points, lower number first, sorted.
    sharp: Vec<([u32; 2], Sharpness)>,
    /// Points tagged as corners by the user (other tags follow from the edges).
    corners: Vec<u32>,
    /// One colour per face, when coloured.
    colours: Option<Vec<Option<[f32; 3]>>>,
    display: SubDDisplay,
}

impl SubD {
    /// A SubD with this cage, smooth everywhere except its open borders, which are creases, as
    /// Rhino 8 makes one from a mesh (`SubD.CreateFromMesh` with its smooth options): no corners, so
    /// an open grid's outer corners round off. Points at exactly the same place are joined first,
    /// as a SubD's cage is one piece of surface.
    #[must_use]
    pub fn new(cage: Mesh) -> Self {
        let (cage, _) = cage.welded(0.0);
        let mut subd = Self {
            cage,
            ..Self::default()
        };
        let uses = subd.edge_uses();
        for (&edge, faces) in &uses {
            if faces.len() == 1 {
                subd.set_sharpness(edge, Sharpness::CREASE);
            }
        }
        subd
    }

    /// The same SubD with these edges (by their two points, either order) creased.
    #[must_use]
    pub fn with_creases(mut self, edges: &[[u32; 2]]) -> Self {
        let uses = self.edge_uses();
        for &[a, b] in edges {
            let key = (a.min(b), a.max(b));
            if uses.contains_key(&key) {
                self.set_sharpness(key, Sharpness::CREASE);
            }
        }
        self
    }

    /// The same SubD with the edge between two points given this sharpness (when they share an
    /// edge).
    #[must_use]
    pub fn with_sharpness(mut self, [a, b]: [u32; 2], sharpness: Sharpness) -> Self {
        if self.edge_uses().contains_key(&(a.min(b), a.max(b))) {
            self.set_sharpness((a, b), sharpness);
        }
        self
    }

    /// The same SubD with these points kept sharp as corners.
    #[must_use]
    pub fn with_corners(mut self, points: &[u32]) -> Self {
        for &v in points {
            if (v as usize) < self.cage.vertex_count() && !self.corners.contains(&v) {
                self.corners.push(v);
            }
        }
        self.corners.sort_unstable();
        self
    }

    /// The same SubD with a colour, or none, for each face; `None` when the count does not match.
    #[must_use]
    pub fn with_face_colours(mut self, colours: Vec<Option<[f32; 3]>>) -> Option<Self> {
        (colours.len() == self.cage.face_count()).then(|| {
            self.colours = Some(colours);
            self
        })
    }

    /// The same SubD drawn flat or smooth.
    #[must_use]
    pub fn with_display(mut self, display: SubDDisplay) -> Self {
        self.display = display;
        self
    }

    /// The cage.
    #[must_use]
    pub fn cage(&self) -> &Mesh {
        &self.cage
    }

    /// How sharp the edge between two points is (smooth when they share no edge).
    #[must_use]
    pub fn sharpness(&self, a: u32, b: u32) -> Sharpness {
        self.sharp
            .binary_search_by_key(&[a.min(b), a.max(b)], |e| e.0)
            .map_or(Sharpness::SMOOTH, |i| self.sharp[i].1)
    }

    /// The creased edges, lower point first.
    pub fn creases(&self) -> impl Iterator<Item = [u32; 2]> + '_ {
        self.sharp
            .iter()
            .filter(|(_, s)| s.is_crease())
            .map(|(e, _)| *e)
    }

    /// A point's tag, as Rhino tags it: a corner where set or where three or more creases meet,
    /// a crease on two, a dart at the end of one, smooth otherwise.
    #[must_use]
    pub fn vertex_tag(&self, v: u32) -> VertexTag {
        if self.corners.binary_search(&v).is_ok() {
            return VertexTag::Corner;
        }
        let creased = self.creases().filter(|e| e.contains(&v)).count();
        match creased {
            0 => VertexTag::Smooth,
            1 => VertexTag::Dart,
            2 => VertexTag::Crease,
            _ => VertexTag::Corner,
        }
    }

    /// The face colours, when it has them.
    #[must_use]
    pub fn face_colours(&self) -> Option<&[Option<[f32; 3]>]> {
        self.colours.as_deref()
    }

    /// Whether it is drawn flat or smooth.
    #[must_use]
    pub fn display(&self) -> SubDDisplay {
        self.display
    }

    /// The lowest and highest corners around its cage, which contains the smooth surface.
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        self.cage.bounds()
    }

    /// The SubD moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        Self {
            cage: self.cage.translated(by),
            ..self.clone()
        }
    }

    /// The SubD turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        Self {
            cage: self.cage.mapped(change),
            ..self.clone()
        }
    }

    /// Sets an edge's sharpness, keeping the list sorted (a smooth edge is left out).
    fn set_sharpness(&mut self, (a, b): (u32, u32), s: Sharpness) {
        let key = [a.min(b), a.max(b)];
        match self.sharp.binary_search_by_key(&key, |e| e.0) {
            Ok(i) if s == Sharpness::SMOOTH => {
                self.sharp.remove(i);
            }
            Ok(i) => self.sharp[i].1 = s,
            Err(_) if s == Sharpness::SMOOTH => {}
            Err(i) => self.sharp.insert(i, (key, s)),
        }
    }

    /// A SubD on a changed cage, keeping what it can: `points[v]` is old point `v`'s number in
    /// the new cage (if it is still there), and `faces[f]` new face `f`'s old number (if it had
    /// one). Edges keep their sharpness and points their corner tags where both ends are still
    /// there; faces keep their colours; the new cage's open border is creased as always.
    pub(crate) fn carried_to(
        &self,
        cage: Mesh,
        points: &[Option<u32>],
        faces: &[Option<usize>],
    ) -> Self {
        let mut subd = Self::new(cage);
        subd.display = self.display;
        let uses = subd.edge_uses();
        let old_uses = self.edge_uses();
        for &([a, b], s) in &self.sharp {
            // A border's crease comes from being a border, so only inner edges' carry over.
            if old_uses.get(&(a, b)).is_none_or(|f| f.len() != 2) {
                continue;
            }
            if let (Some(Some(na)), Some(Some(nb))) =
                (points.get(a as usize), points.get(b as usize))
            {
                let key = ((*na).min(*nb), (*na).max(*nb));
                if uses.get(&key).is_some_and(|f| f.len() == 2) {
                    subd.set_sharpness(key, s);
                }
            }
        }
        let mut corners: Vec<u32> = self
            .corners
            .iter()
            .filter_map(|&v| points.get(v as usize).copied().flatten())
            .collect();
        corners.sort_unstable();
        corners.dedup();
        subd.corners = corners;
        if let Some(old) = &self.colours {
            let colours: Vec<Option<[f32; 3]>> = faces
                .iter()
                .map(|f| f.and_then(|f| old.get(f).copied().flatten()))
                .collect();
            if colours.len() == subd.cage.face_count() && colours.iter().any(Option::is_some) {
                subd.colours = Some(colours);
            }
        }
        subd
    }

    /// The same SubD with these points no longer corners.
    #[must_use]
    pub fn without_corners(mut self, points: &[u32]) -> Self {
        self.corners.retain(|v| !points.contains(v));
        self
    }

    /// Each edge with the faces that use it, keyed lower point first.
    pub(crate) fn edge_uses(&self) -> BTreeMap<(u32, u32), Vec<usize>> {
        let mut uses: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
        for (f, face) in self.cage.faces().enumerate() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                uses.entry((a.min(b), a.max(b))).or_default().push(f);
            }
        }
        uses
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    #[test]
    fn a_subd_moves_with_its_cage() {
        let subd = SubD::new(cube())
            .with_creases(&[[0, 1]])
            .translated([0.0, 0.0, 5.0]);
        assert_eq!(subd.bounds(), Some(([0.0, 0.0, 5.0], [1.0, 1.0, 6.0])));
        assert_eq!(subd.cage().face_count(), 6);
        assert!(subd.sharpness(1, 0).is_crease());
    }

    #[test]
    fn tags_follow_the_creases_as_in_rhino() {
        let subd = SubD::new(cube());
        assert!((0..8).all(|v| subd.vertex_tag(v) == VertexTag::Smooth));
        assert_eq!(
            subd.creases().count(),
            0,
            "a closed cage has no border creases"
        );
        let creased = subd.with_creases(&[[0, 1], [1, 2]]);
        assert_eq!(creased.vertex_tag(0), VertexTag::Dart);
        assert_eq!(creased.vertex_tag(1), VertexTag::Crease);
        let three = creased.with_creases(&[[1, 5]]);
        assert_eq!(three.vertex_tag(1), VertexTag::Corner);
        let cornered = SubD::new(cube()).with_corners(&[6]);
        assert_eq!(cornered.vertex_tag(6), VertexTag::Corner);
        assert_eq!(cornered.sharpness(0, 1), Sharpness::SMOOTH);
    }

    #[test]
    fn an_open_cage_has_creased_borders_and_no_corners_as_in_rhino() {
        // A 2 × 2 grid of squares.
        let points: Vec<Point> = (0..3)
            .flat_map(|j| (0..3).map(move |i| [f64::from(i), f64::from(j), 0.0]))
            .collect();
        let faces = [[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]];
        let subd = SubD::new(Mesh::new(points, &faces).unwrap());
        assert_eq!(subd.creases().count(), 8);
        assert_eq!(subd.vertex_tag(0), VertexTag::Crease);
        assert_eq!(
            subd.clone().with_corners(&[0]).vertex_tag(0),
            VertexTag::Corner
        );
        assert_eq!(subd.vertex_tag(1), VertexTag::Crease);
        assert_eq!(subd.vertex_tag(4), VertexTag::Smooth);
    }

    #[test]
    fn a_crease_is_written_without_infinity() {
        let subd = SubD::new(cube()).with_creases(&[[0, 1]]);
        let text = serde_json::to_string(&subd).unwrap();
        let back: SubD = serde_json::from_str(&text).unwrap();
        assert_eq!(back, subd);
        assert_eq!(back.sharpness(0, 1).value(), f64::INFINITY);
    }

    #[test]
    fn face_colours_need_one_per_face() {
        assert!(
            SubD::new(cube())
                .with_face_colours(vec![Some([1.0, 0.0, 0.0]); 5])
                .is_none()
        );
        let coloured = SubD::new(cube())
            .with_face_colours(vec![Some([1.0, 0.0, 0.0]); 6])
            .unwrap();
        assert_eq!(coloured.face_colours().map(<[_]>::len), Some(6));
        assert_eq!(
            coloured.with_display(SubDDisplay::Flat).display(),
            SubDDisplay::Flat
        );
    }
}
