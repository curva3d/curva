//! Hatch patterns (spec `annotation.md` §5): families of parallel, maybe dashed, lines in the
//! `.pat` form AutoCAD, Rhino and most CAD programs share, and the lines of a pattern clipped to
//! a region. No Rust crate reads `.pat` files (checked on 14 November 2026: crates.io and
//! acadrust, which reads DXF and DWG only), and solidcore has no pattern clipping, so both are
//! here; the region's inside is decided even–odd, as Rhino fills hatches.
//!
//! A `.pat` line family is `angle, x-origin, y-origin, delta-x, delta-y [, dash, ...]`: lines at
//! `angle` degrees through the origin, each next line shifted `delta-x` along and `delta-y`
//! across, dashes positive for pen down and negative for gaps (zero is a dot).

use serde::{Deserialize, Serialize};

/// One family of parallel lines in a pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternLine {
    /// Their direction, in degrees from the plane's X.
    pub angle: f64,
    /// A point the first line passes through.
    pub origin: [f64; 2],
    /// How far each next line's dashes move along it.
    pub shift: f64,
    /// How far apart the lines are.
    pub spacing: f64,
    /// Dash lengths, pen down when positive, a gap when negative; solid lines when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashes: Vec<f64>,
}

/// A named hatch pattern. Sizes are millimetres, so a pattern looks the same in any unit; a
/// hatch's scale multiplies them (spec `annotation.md` §5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Its name, as listed.
    pub name: String,
    /// What it shows, from the `.pat` header.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Its line families; none for a solid fill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<PatternLine>,
}

impl Pattern {
    /// Whether it fills solid rather than with lines.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.lines.is_empty()
    }
}

/// The patterns every document starts with: Rhino's usual set, then ANSI31 to ANSI38. The ANSI
/// patterns are defined in inches in `acad.pat`; they are given here in millimetres.
const BUILT_IN: &str = "
*Solid, Solid fill
*Hatch1, Lines at 45 degrees
45, 0,0, 0,3
*Hatch2, Close lines at 45 degrees
45, 0,0, 0,1.5
*Hatch3, Crossed lines at 45 degrees
45, 0,0, 0,3
135, 0,0, 0,3
*Dash, Dashed lines at 45 degrees
45, 0,0, 1.5,3, 3,-1.5
*Grid, Square grid
0, 0,0, 0,5
90, 0,0, 0,5
*Grid60, Grid at 60 degrees
0, 0,0, 0,5
60, 0,0, 0,5
*Plus, Plus signs
0, -1,0, 5,5, 2,-8
90, 0,-1, 5,5, 2,-8
*Squares, Small squares
0, 0,0, 0,5, 2.5,-2.5
90, 0,0, 0,5, 2.5,-2.5
*ANSI31, ANSI iron, brick, stone masonry
45, 0,0, 0,3.175
*ANSI32, ANSI steel
45, 0,0, 0,9.525
45, 4.49013,0, 0,9.525
*ANSI33, ANSI bronze, brass, copper
45, 0,0, 0,6.35
45, 4.49013,0, 0,6.35, 3.175,-1.5875
*ANSI34, ANSI plastic, rubber
45, 0,0, 0,19.05
45, 4.49013,0, 0,19.05
45, 8.98026,0, 0,19.05
45, 13.47038,0, 0,19.05
*ANSI35, ANSI fire brick, refractory material
45, 0,0, 0,6.35
45, 4.49013,0, 0,6.35, 7.9375,-1.5875,0,-1.5875
*ANSI36, ANSI marble, slate, glass
45, 0,0, 5.55625,3.175, 7.9375,-1.5875,0,-1.5875
*ANSI37, ANSI lead, zinc, magnesium, sound/heat/electric insulation
45, 0,0, 0,3.175
135, 0,0, 0,3.175
*ANSI38, ANSI aluminum
45, 0,0, 0,3.175
135, 0,0, 6.35,3.175, 7.9375,-4.7625
";

/// The patterns a new document has.
#[must_use]
pub fn built_in_patterns() -> Vec<Pattern> {
    parse_pat(BUILT_IN).unwrap_or_default()
}

/// The patterns in the text of a `.pat` file, in the file's own units. Comments start with `;`;
/// a pattern starts with `*name, description` and its line families follow, one to a line.
///
/// # Errors
///
/// A message naming the line that is not a number list, or a family before any name.
pub fn parse_pat(text: &str) -> Result<Vec<Pattern>, String> {
    let mut patterns: Vec<Pattern> = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('*') {
            let (name, description) = header.split_once(',').unwrap_or((header, ""));
            patterns.push(Pattern {
                name: name.trim().to_owned(),
                description: description.trim().to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        let numbers: Result<Vec<f64>, _> =
            line.split(',').map(|n| n.trim().parse::<f64>()).collect();
        let numbers = match numbers {
            Ok(n) if n.len() >= 5 && n.iter().all(|v| v.is_finite()) => n,
            _ => return Err(format!("line {}: {raw}", number + 1)),
        };
        let Some(pattern) = patterns.last_mut() else {
            return Err(format!(
                "line {}: a line family before any pattern name",
                number + 1
            ));
        };
        pattern.lines.push(PatternLine {
            angle: numbers[0],
            origin: [numbers[1], numbers[2]],
            shift: numbers[3],
            spacing: numbers[4],
            dashes: numbers[5..].to_vec(),
        });
    }
    Ok(patterns)
}

/// The most line pieces one hatch draws; a pattern far too fine for its region stops here
/// rather than freezing the program.
pub const MOST_PIECES: usize = 20_000;

/// The pieces of a pattern's lines inside a region, in the region's plane coordinates. `rings`
/// are closed polygons (outer outline and holes alike; inside is even–odd), `scale` multiplies
/// the pattern's sizes, `rotation` (radians) turns it about `base`.
#[must_use]
// CUSTOM KERNEL OPERATION: hatch-patterns
pub fn pattern_pieces(
    pattern: &Pattern,
    rings: &[Vec<[f64; 2]>],
    scale: f64,
    rotation: f64,
    base: [f64; 2],
) -> Vec<([f64; 2], [f64; 2])> {
    let mut pieces = Vec::new();
    let points: Vec<[f64; 2]> = rings.iter().flatten().copied().collect();
    if points.is_empty() || !scale.is_finite() || scale <= 0.0 {
        return pieces;
    }
    for family in &pattern.lines {
        let spacing = (family.spacing * scale).abs();
        if !spacing.is_finite() || spacing <= 1e-12 {
            continue;
        }
        let angle = family.angle.to_radians() + rotation;
        let along = [angle.cos(), angle.sin()];
        let across = [-along[1], along[0]];
        // The family's origin, turned with the pattern about the base point.
        let (s, c) = rotation.sin_cos();
        let o = [family.origin[0] * scale, family.origin[1] * scale];
        let origin = [base[0] + o[0] * c - o[1] * s, base[1] + o[0] * s + o[1] * c];
        let rel = |p: [f64; 2]| [p[0] - origin[0], p[1] - origin[1]];
        let dot = |a: [f64; 2], b: [f64; 2]| a[0] * b[0] + a[1] * b[1];
        let (low, high) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            let v = dot(rel(*p), across);
            (lo.min(v), hi.max(v))
        });
        // The spacing's sign says which way successive lines go; either way, cover the region.
        let sign = family.spacing.signum() * scale.signum();
        let shift = family.shift * scale * sign;
        let dashes: Vec<f64> = family.dashes.iter().map(|d| d * scale).collect();
        let period: f64 = dashes.iter().map(|d| d.abs()).sum();
        let first = (low / spacing).floor() as i64;
        let last = (high / spacing).ceil() as i64;
        for k in first..=last {
            let offset = k as f64 * spacing;
            // Where this line crosses the rings, as distances along it.
            let mut hits = Vec::new();
            for ring in rings {
                for (i, a) in ring.iter().enumerate() {
                    let b = ring[(i + 1) % ring.len()];
                    let (da, db) = (dot(rel(*a), across) - offset, dot(rel(b), across) - offset);
                    if (da > 0.0) != (db > 0.0) {
                        let t = da / (da - db);
                        let p = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                        hits.push(dot(rel(p), along));
                    }
                }
            }
            hits.sort_by(f64::total_cmp);
            let at = |t: f64| {
                [
                    origin[0] + across[0] * offset + along[0] * t,
                    origin[1] + across[1] * offset + along[1] * t,
                ]
            };
            for &[from, to] in hits.as_chunks::<2>().0 {
                if dashes.is_empty() || period < 1e-12 {
                    pieces.push((at(from), at(to)));
                } else {
                    // Dashes start from where the line's own shift puts them.
                    let phase = k as f64 * shift;
                    let mut t = phase + ((from - phase) / period).floor() * period;
                    'dashes: while t < to {
                        for d in &dashes {
                            let end = t + d.abs();
                            if *d >= 0.0 {
                                let (a, b) = (t.max(from), end.min(to));
                                if *d == 0.0 && t >= from && t <= to {
                                    pieces.push((at(t), at(t)));
                                } else if b > a {
                                    pieces.push((at(a), at(b)));
                                }
                            }
                            t = end;
                            if pieces.len() >= MOST_PIECES || t >= to {
                                break 'dashes;
                            }
                        }
                    }
                }
                if pieces.len() >= MOST_PIECES {
                    return pieces;
                }
            }
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: f64) -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]]
    }

    fn pattern(name: &str) -> Pattern {
        built_in_patterns()
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
    }

    #[test]
    fn the_built_in_patterns_read() {
        let all = built_in_patterns();
        assert_eq!(all.len(), 17);
        assert!(all[0].is_solid());
        assert_eq!(pattern("ANSI37").lines.len(), 2);
        assert_eq!(pattern("ANSI38").lines[1].dashes, vec![7.9375, -4.7625]);
    }

    #[test]
    fn a_bad_line_is_named() {
        assert!(
            parse_pat("*A\n0,0,0,0,x\n")
                .unwrap_err()
                .starts_with("line 2")
        );
        assert!(parse_pat("0,0,0,0,1\n").is_err());
        assert_eq!(parse_pat("; only a comment\n").unwrap(), Vec::new());
    }

    #[test]
    fn level_lines_fill_a_square_at_their_spacing() {
        let level = Pattern {
            name: "Level".into(),
            description: String::new(),
            lines: vec![PatternLine {
                angle: 0.0,
                origin: [0.0, 0.5],
                shift: 0.0,
                spacing: 1.0,
                dashes: Vec::new(),
            }],
        };
        let pieces = pattern_pieces(&level, &[square(10.0)], 1.0, 0.0, [0.0, 0.0]);
        assert_eq!(pieces.len(), 10);
        for (a, b) in &pieces {
            assert!((a[0].min(b[0])).abs() < 1e-9 && (a[0].max(b[0]) - 10.0).abs() < 1e-9);
        }
        // Twice the scale, half the lines.
        assert_eq!(
            pattern_pieces(&level, &[square(10.0)], 2.0, 0.0, [0.0, 0.0]).len(),
            5
        );
    }

    #[test]
    fn a_hole_is_left_empty() {
        let hole = vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]];
        let pieces = pattern_pieces(
            &pattern("Grid"),
            &[square(20.0), hole],
            0.2,
            0.0,
            [0.0, 0.0],
        );
        let middle = [5.0, 5.0];
        for (a, b) in &pieces {
            // No piece passes through the middle of the hole.
            let on = (a[0] - middle[0]).abs() < 1e-9 && (b[0] - middle[0]).abs() < 1e-9;
            let level = (a[1] - middle[1]).abs() < 1e-9 && (b[1] - middle[1]).abs() < 1e-9;
            let spans = |lo: f64, hi: f64, v: f64| lo.min(hi) < v && v < lo.max(hi);
            assert!(!(on && spans(a[1], b[1], 5.0)) && !(level && spans(a[0], b[0], 5.0)));
        }
    }

    #[test]
    fn dashes_break_the_lines() {
        let solid = pattern_pieces(&pattern("Hatch1"), &[square(30.0)], 1.0, 0.0, [0.0, 0.0]);
        let dashed = pattern_pieces(&pattern("Dash"), &[square(30.0)], 1.0, 0.0, [0.0, 0.0]);
        assert!(dashed.len() > solid.len());
        let length = |ps: &[([f64; 2], [f64; 2])]| -> f64 {
            ps.iter()
                .map(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1]))
                .sum()
        };
        // Two thirds pen down.
        let share = length(&dashed) / length(&solid);
        assert!((share - 2.0 / 3.0).abs() < 0.08, "{share}");
    }

    #[test]
    fn a_pattern_far_too_fine_stops() {
        let pieces = pattern_pieces(&pattern("Hatch1"), &[square(1000.0)], 0.001, 0.0, [0.0; 2]);
        assert!(pieces.len() <= MOST_PIECES);
    }
}
