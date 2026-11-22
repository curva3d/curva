//! The viewport's colours for the light and dark themes, and the grid drawn in them.

use crate::{CPlane, Outlines, Vertex, grid_segment};
use curva_kernel::Point;

/// The viewport's colours for a theme: its background, the grid and axes, and the colour that
/// objects on a layer still in the standard colour take, so they read on either background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The 3D background.
    pub background: [f32; 3],
    /// A minor grid line, every unit, already blended over the background.
    pub minor: [f32; 3],
    /// A major grid line, every 10 units, already blended over the background.
    pub major: [f32; 3],
    /// The X axis, softened.
    pub x_axis: [f32; 3],
    /// The Y axis, softened.
    pub y_axis: [f32; 3],
    /// What the standard layer colour becomes.
    pub ink: [f32; 3],
}

/// `line` laid over `background` at `amount`, as a see-through line would look.
pub(crate) const fn over(background: [f32; 3], line: [f32; 3], amount: f32) -> [f32; 3] {
    [
        background[0] + (line[0] - background[0]) * amount,
        background[1] + (line[1] - background[1]) * amount,
        background[2] + (line[2] - background[2]) * amount,
    ]
}

/// `0xRRGGBB` as a colour.
pub(crate) const fn hex(rgb: u32) -> [f32; 3] {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    ]
}

impl Palette {
    /// The dark theme: faint light lines over `#262624`, as Serpentine3D draws its grid.
    pub const DARK: Self = Self {
        background: hex(0x26_2624),
        minor: over(hex(0x26_2624), [1.0; 3], 0.05),
        major: over(hex(0x26_2624), [1.0; 3], 0.11),
        x_axis: over(hex(0x26_2624), [0.86, 0.33, 0.30], 0.6),
        y_axis: over(hex(0x26_2624), [0.38, 0.78, 0.42], 0.6),
        ink: crate::STANDARD_INK,
    };

    /// The light theme: faint dark lines over `#f5f4ef`, and dark curves.
    pub const LIGHT: Self = Self {
        background: hex(0xf5_f4ef),
        minor: over(hex(0xf5_f4ef), [0.0; 3], 0.07),
        major: over(hex(0xf5_f4ef), [0.0; 3], 0.15),
        x_axis: over(hex(0xf5_f4ef), [0.80, 0.25, 0.22], 0.6),
        y_axis: over(hex(0xf5_f4ef), [0.25, 0.62, 0.30], 0.6),
        ink: hex(0x1f_1e1d),
    };
}

/// How far the grid reaches each way from the plane's origin, in lines.
pub(crate) const GRID_REACH: i32 = 100;

/// How a grid is drawn: in which colours, and how far apart its lines are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLook {
    /// Its colours and the axes'.
    pub palette: Palette,
    /// Units between two lines.
    pub spacing: f64,
    /// Every how many lines a major one comes.
    pub major_every: i32,
}

impl GridLook {
    /// A line every unit and a major one every 10, in `palette`.
    #[must_use]
    pub const fn new(palette: Palette) -> Self {
        Self {
            palette,
            spacing: 1.0,
            major_every: 10,
        }
    }

    /// The same grid with its lines `spacing` units apart and a major one every `major_every`.
    #[must_use]
    pub const fn spaced(self, spacing: f64, major_every: i32) -> Self {
        Self {
            spacing,
            major_every,
            ..self
        }
    }
}

impl Palette {
    /// The light theme's grid and axes laid over `background` instead of its own, for a light
    /// background of a display mode's own: white paper, or the Massing sky.
    #[must_use]
    pub const fn light_over(background: [f32; 3]) -> Self {
        Self {
            background,
            minor: over(background, [0.0; 3], 0.07),
            major: over(background, [0.0; 3], 0.15),
            x_axis: over(background, [0.80, 0.25, 0.22], 0.6),
            y_axis: over(background, [0.25, 0.62, 0.30], 0.6),
            ink: Self::LIGHT.ink,
        }
    }

    /// The same palette with its major grid lines in `colour` and its minor ones halfway between
    /// that and `background`, for a grid colour the user chose.
    #[must_use]
    pub const fn with_grid(self, colour: [f32; 3], background: [f32; 3]) -> Self {
        Self {
            major: colour,
            minor: over(background, colour, 0.5),
            ..self
        }
    }
}

/// The grid and axes of a plane as line segments, as Serpentine3D draws them: lines
/// `look.spacing` apart with a major one every `look.major_every`, 100 lines out each way, with
/// softened axes; measured from `origin`, as the scene they are drawn with
/// ([`crate::SceneParts::origin`]).
#[must_use]
pub fn grid_lines_in(plane: &CPlane, look: &GridLook, origin: Point) -> Vec<Vertex> {
    let on_plane = |u: f64, v: f64| {
        let along = |axis: usize| plane.origin[axis] + plane.x[axis] * u + plane.y[axis] * v;
        crate::narrow_from([along(0), along(1), along(2)], origin)
    };
    let palette = &look.palette;
    let every = look.major_every.max(1);
    let mut out = Vec::new();
    let reach = f64::from(GRID_REACH) * look.spacing;
    // Minor lines first, so the major ones draw over them where they meet.
    for major in [false, true] {
        for step in -GRID_REACH..=GRID_REACH {
            if step == 0 || (step % every == 0) != major {
                continue;
            }
            let colour = if major { palette.major } else { palette.minor };
            let at = f64::from(step) * look.spacing;
            grid_segment(&mut out, on_plane(at, -reach), on_plane(at, reach), colour);
            grid_segment(&mut out, on_plane(-reach, at), on_plane(reach, at), colour);
        }
    }
    grid_segment(
        &mut out,
        on_plane(-reach, 0.0),
        on_plane(reach, 0.0),
        palette.x_axis,
    );
    grid_segment(
        &mut out,
        on_plane(0.0, -reach),
        on_plane(0.0, reach),
        palette.y_axis,
    );
    out
}

impl Outlines {
    /// The same outlines with objects in the standard layer colour drawn in `ink` instead, so
    /// they read on a light background.
    #[must_use]
    pub fn inked(mut self, ink: [f32; 3]) -> Self {
        if ink == STANDARD_INK {
            return self;
        }
        for colour in &mut self.colours {
            if *colour == STANDARD_INK {
                *colour = ink;
            }
        }
        for faces in &mut self.faces {
            if faces.colour == STANDARD_INK {
                faces.colour = ink.map(|c| 0.35 + c * 0.5);
            }
        }
        self
    }
}

/// The standard layer colour, which objects take until their layer is given another.
pub const STANDARD_INK: [f32; 3] = curva_document::STANDARD_COLOUR;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spaced_grid_puts_its_lines_on_the_spacing_and_its_majors_where_asked() {
        let look = GridLook::new(Palette::DARK).spaced(10.0, 10);
        let lines = grid_lines_in(&CPlane::WORLD_XY, &look, [0.0; 3]);
        assert!(!lines.is_empty());
        for vertex in &lines {
            let [x, y, _] = vertex.position;
            assert!(x.abs() <= 1000.0 + 1e-3 && y.abs() <= 1000.0 + 1e-3);
            let on_line = |v: f32| (v / 10.0 - (v / 10.0).round()).abs() < 1e-4;
            assert!(on_line(x) || on_line(y), "{x}, {y} off the spacing");
            if vertex.colour == Palette::DARK.major {
                let on_major = |v: f32| (v / 100.0 - (v / 100.0).round()).abs() < 1e-4;
                assert!(on_major(x) || on_major(y));
            }
        }
    }

    #[test]
    fn a_chosen_grid_colour_makes_the_major_lines_and_softer_minor_ones() {
        let palette = Palette::DARK.with_grid([1.0, 0.0, 0.0], [0.0; 3]);
        assert_eq!(palette.major, [1.0, 0.0, 0.0]);
        assert_eq!(palette.minor, [0.5, 0.0, 0.0]);
        assert_eq!(palette.x_axis, Palette::DARK.x_axis);
    }
}
