//! Hatches.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A hatch (Rhino's Hatch, spec `annotation.md` §5): closed flat outlines filled with a pattern
/// or solid. Inside is even–odd, so an outline inside another is a hole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hatch {
    /// Its closed outlines, all in its plane.
    pub loops: Vec<curva_kernel::Curve>,
    /// The pattern's name (see [`crate::State::patterns`]).
    pub pattern: String,
    /// What the pattern's sizes are multiplied by.
    #[serde(default = "Hatch::usual_scale")]
    pub scale: f64,
    /// How far the pattern is turned from the plane's X, in radians.
    #[serde(default)]
    pub rotation: f64,
    /// Where the pattern starts.
    pub base: Point,
    /// The plane's X direction.
    pub x: Point,
    /// The plane's Y direction.
    pub y: Point,
}

impl Hatch {
    /// A pattern at its own size.
    #[must_use]
    pub fn usual_scale() -> f64 {
        1.0
    }

    pub(super) fn with_loops(
        &self,
        loops: Vec<curva_kernel::Curve>,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        Self {
            loops,
            base: point(self.base),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its outlines as polygons in its plane's own coordinates, from its base point.
    #[must_use]
    pub fn rings(&self) -> Vec<Vec<[f64; 2]>> {
        self.loops
            .iter()
            .filter_map(|c| c.sample().ok())
            .map(|points| {
                points
                    .iter()
                    .map(|p| {
                        let d: Point = std::array::from_fn(|i| p[i] - self.base[i]);
                        [dot(d, self.x), dot(d, self.y)]
                    })
                    .collect()
            })
            .collect()
    }

    /// The pattern's line pieces in the model, or nothing for a solid fill. The pattern's sizes
    /// are millimetres; `per_millimetre` is how many of the space's units one millimetre is.
    #[must_use]
    pub fn pieces(
        &self,
        pattern: &crate::hatch::Pattern,
        per_millimetre: f64,
    ) -> Vec<(Point, Point)> {
        let rings = self.rings();
        let flat = crate::hatch::pattern_pieces(
            pattern,
            &rings,
            self.scale * per_millimetre,
            self.rotation,
            [0.0, 0.0],
        );
        let at = |p: [f64; 2]| -> Point {
            std::array::from_fn(|i| self.base[i] + self.x[i] * p[0] + self.y[i] * p[1])
        };
        flat.into_iter().map(|(a, b)| (at(a), at(b))).collect()
    }
}
