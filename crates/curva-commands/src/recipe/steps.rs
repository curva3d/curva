//! Steps of a recipe: what each asks for.

#[allow(clippy::wildcard_imports)]
use super::*;

/// One step of a recipe: what it asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct StepDef {
    /// The prompt's key in the translation file.
    pub prompt: &'static str,
    /// The sort of answer.
    pub kind: Kind,
    /// Where it measures from: typed distances and angles, and the cursor tag, start here.
    pub from: Option<Point>,
    /// For a size along a direction: where it starts and the direction, of length 1.
    pub along: Option<(Point, Point)>,
    /// Whether Enter with nothing typed answers it.
    pub optional: bool,
    /// Which of the recipe's options this step offers: all of them unless a list is given.
    pub options: Option<Vec<&'static str>>,
    /// The number option a number typed at this step sets, as Rhino's Offset takes a distance at
    /// its side prompt; the step is then asked again.
    pub sets: Option<&'static str>,
    /// A number step whose answer is remembered under this name: Enter then gives the last one
    /// (or the starting value), shown in the prompt, as Rhino offers its defaults.
    pub remembered: Option<(&'static str, f64)>,
    /// Whether it picks a rectangle's far corner, so the tag shows width × depth.
    pub corner: bool,
    /// For objects: which sort it takes.
    pub only: Only,
    /// For objects: how many it needs; with fewer it asks again.
    pub least: usize,
    /// For objects: how many it takes at most; with more it asks again.
    pub most: usize,
    /// For objects: whether objects chosen at an earlier step are left out.
    pub apart: bool,
}

/// Which sort of objects a step takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Only {
    /// Any object.
    #[default]
    Any,
    /// Curves.
    Curves,
    /// Surfaces and solids.
    Bodies,
}

impl StepDef {
    fn new(prompt: &'static str, kind: Kind) -> Self {
        Self {
            prompt,
            kind,
            from: None,
            along: None,
            optional: false,
            options: None,
            sets: None,
            remembered: None,
            corner: false,
            only: Only::Any,
            least: 1,
            most: usize::MAX,
            apart: false,
        }
    }

    /// Asks for a spot on a curve, clicked; the answer is the exact point on it.
    #[must_use]
    pub fn pick(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::PickPart)
    }

    /// Asks for a position.
    #[must_use]
    pub fn point(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Point)
    }

    /// Asks for a position, or a typed number instead, such as Arc's end point or angle.
    #[must_use]
    pub fn point_or_number(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::PointOrNumber)
    }

    /// Asks for a number, or a click whose distance from [`Self::from`] gives it.
    #[must_use]
    pub fn number(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Number)
    }

    /// Asks for a size along `direction` from `origin`, following the snapped mouse.
    #[must_use]
    pub fn along(prompt: &'static str, origin: Point, direction: Point) -> Self {
        Self {
            from: Some(origin),
            along: curva_kernel::unit(direction).map(|d| (origin, d)),
            ..Self::new(prompt, Kind::Number)
        }
    }

    /// Asks for edges of surfaces and solids, picked in the viewport.
    #[must_use]
    pub fn edges(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Edges)
    }

    /// Asks for faces of surfaces and solids, picked in the viewport.
    #[must_use]
    pub fn faces(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Faces)
    }

    /// Asks for objects; a selection made beforehand answers it.
    #[must_use]
    pub fn objects(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Objects)
    }

    /// Measures from `base`.
    #[must_use]
    pub fn from(mut self, base: Point) -> Self {
        self.from = Some(base);
        self
    }

    /// Enter with nothing typed answers it.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Offers only these of the recipe's options at this step.
    #[must_use]
    pub fn offering(mut self, options: &[&'static str]) -> Self {
        self.options = Some(options.to_vec());
        self
    }

    /// Remembers the number given under `name`; Enter gives it again, `starting` at first.
    #[must_use]
    pub fn remembered(mut self, name: &'static str, starting: f64) -> Self {
        self.remembered = Some((name, starting));
        self.optional = true;
        self
    }

    /// Picks a rectangle's far corner: the pointer's tag shows width × depth.
    #[must_use]
    pub fn corner(mut self) -> Self {
        self.corner = true;
        self
    }

    /// Takes only curves.
    #[must_use]
    pub fn curves(mut self) -> Self {
        self.only = Only::Curves;
        self
    }

    /// Takes only surfaces and solids.
    #[must_use]
    pub fn bodies(mut self) -> Self {
        self.only = Only::Bodies;
        self
    }

    /// Needs at least `count` objects.
    #[must_use]
    pub fn at_least(mut self, count: usize) -> Self {
        self.least = count;
        self
    }

    /// Takes exactly one object, as Sweep1's rail.
    #[must_use]
    pub fn one(mut self) -> Self {
        self.least = 1;
        self.most = 1;
        self
    }

    /// Leaves out objects chosen at an earlier step, as Rhino's Boolean commands do.
    #[must_use]
    pub fn apart(mut self) -> Self {
        self.apart = true;
        self
    }

    /// A number typed at this step sets the number option `option`, and the step is asked again.
    #[must_use]
    pub fn number_sets(mut self, option: &'static str) -> Self {
        self.sets = Some(option);
        self
    }
}
