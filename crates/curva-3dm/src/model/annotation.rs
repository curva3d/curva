//! A file's annotation as it holds it: hatches and their patterns, dimension styles, text,
//! dimensions and leaders.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A hatch pattern (`ON_HatchPattern`).
#[derive(Clone, Debug, PartialEq)]
pub struct HatchPattern {
    /// Identifies it.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// What it shows.
    pub description: String,
    /// Whether it fills solid (openNURBS' fill type 0) rather than with lines (1).
    pub solid: bool,
    /// Its line families.
    pub lines: Vec<HatchLine>,
}

/// One family of a hatch pattern's lines (`ON_HatchLine`), at scale one in model units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HatchLine {
    /// Their direction, radians from the hatch plane's X.
    pub angle: f64,
    /// A point the first line passes through.
    pub base: [f64; 2],
    /// From one line to the next: along the line, then across it.
    pub offset: [f64; 2],
    /// Dash lengths, a gap when negative; solid when empty.
    pub dashes: Vec<f64>,
}

/// A hatch (`ON_Hatch`): outlines on a plane, filled with a pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Hatch {
    /// Its plane; the outlines are 2D curves in it.
    pub plane: Plane,
    /// What the pattern's sizes are multiplied by.
    pub scale: f64,
    /// How far the pattern is turned, radians.
    pub rotation: f64,
    /// The pattern, by its place in the hatch pattern table.
    pub pattern: i32,
    /// The outlines, each with whether it is an outer one (else a hole).
    pub loops: Vec<(bool, Curve)>,
    /// Where the pattern starts, in the plane.
    pub base: [f64; 2],
}

/// An annotation style (`ON_DimStyle`), as much of it as Curva reads.
#[derive(Clone, Debug, PartialEq)]
pub struct DimStyle {
    /// Identifies it; notes name their style by it.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// Its text height, in model units.
    pub text_height: f64,
    /// Notes' horizontal alignment, as Rhino 5 numbered it: 0 left, 1 centre, 2 right.
    pub h_align: u32,
    /// Notes' vertical alignment, as Rhino 5 numbered it: 3 top, 4 middle of the first line,
    /// 5 middle, 6 middle of the last line, 7 bottom, 8 under the bottom.
    pub v_align: u32,
    /// Arrowheads' length, in model units.
    pub arrow_size: f64,
    /// How far extension lines reach past the dimension line, in model units.
    pub extension_extension: f64,
    /// The gap between a measured point and its extension line, in model units.
    pub extension_offset: f64,
    /// The gap between the dimension line and its text, in model units.
    pub text_gap: f64,
    /// Decimal places of lengths.
    pub length_resolution: i32,
    /// Its font's family name; Rhino's default when `None`.
    pub font: Option<String>,
}

/// A note (`ON_Text`).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    /// Its words as rich text (RTF), or plain.
    pub rich: String,
    /// Its plane: the note's point and directions.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style the note sets for itself (its overrides), if it has one.
    pub own: Option<DimStyle>,
    /// A turn of the words within the plane, radians.
    pub rotation: f64,
}

/// A linear dimension (`ON_DimLinear`).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearDimension {
    /// Its plane, at the first point measured; for a rotated dimension its X is the direction
    /// measured along, for an aligned one it runs from the first point to the second.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Whether it measures along its plane's X (rotated) rather than point to point (aligned).
    pub rotated: bool,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The second point measured, in the plane.
    pub second: [f64; 2],
    /// A point its dimension line passes through, in the plane.
    pub line: [f64; 2],
}

/// A radius or diameter dimension (`ON_DimRadial`).
#[derive(Clone, Debug, PartialEq)]
pub struct RadialDimension {
    /// Its plane, at the circle's centre.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Whether it shows the diameter rather than the radius.
    pub diameter: bool,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The point on the circle its arrow touches, in the plane.
    pub radius_point: [f64; 2],
    /// Where its leader ends and its text sits, in the plane.
    pub text_point: [f64; 2],
}

/// An angle dimension (`ON_DimAngular`).
#[derive(Clone, Debug, PartialEq)]
pub struct AngularDimension {
    /// Its plane, at the angle's corner.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The directions of its two sides, in the plane.
    pub sides: [[f64; 2]; 2],
    /// How far along each side its extension lines start.
    pub offsets: [f64; 2],
    /// A point its arc passes through, in the plane.
    pub arc_point: [f64; 2],
}

/// openNURBS' `ON_UNSET_VALUE`: a kink offset left at its default.
pub const UNSET_VALUE: f64 = -1.234_321_012_343_21e308;

/// An ordinate dimension (`ON_DimOrdinate`).
#[derive(Clone, Debug, PartialEq)]
pub struct OrdinateDimension {
    /// Its plane, at the base point the distance is measured from.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// Which coordinate it gives: 0 implied by its leader, 1 X, 2 Y.
    pub direction: u32,
    /// The point measured, in the plane.
    pub point: [f64; 2],
    /// Where its leader ends, in the plane.
    pub leader: [f64; 2],
    /// Its two kink offsets; [`UNSET_VALUE`] for the default.
    pub kinks: [f64; 2],
}

/// A leader (`ON_Leader`).
#[derive(Clone, Debug, PartialEq)]
pub struct Leader {
    /// Its plane.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its words, as rich text or plain.
    pub rich: String,
    /// The arrow's tip, then the points the line passes through, in the plane.
    pub points: Vec<[f64; 2]>,
}
