//! Showing and reading lengths in a model's unit (spec `feedback-0.4.2.md` §3): millimetres and
//! metres as decimals; feet as architectural feet-and-inches (`5'-3 1/2"`) and inches as inches
//! and fractions (`63 1/2"`), or either as decimals. Typing accepts any of these forms in any
//! unit, and a typed unit is converted to the model's.

use crate::Unit;
use serde::{Deserialize, Serialize};

/// How a model shows feet and inches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LengthDisplay {
    /// Decimals (`5.29'`, `63.5"`) instead of fractions.
    pub decimals: bool,
    /// The finest fraction of an inch shown: 2, 4, 8, 16, 32 or 64.
    pub denominator: u32,
}

impl Default for LengthDisplay {
    fn default() -> Self {
        Self {
            decimals: false,
            denominator: 16,
        }
    }
}

impl LengthDisplay {
    /// The fractions on offer, from coarsest to finest.
    pub const DENOMINATORS: [u32; 6] = [2, 4, 8, 16, 32, 64];

    /// Whether it is the usual display, which files leave out.
    #[must_use]
    pub fn is_standard(&self) -> bool {
        *self == Self::default()
    }
}

/// A length in `unit`, as the model shows it.
#[must_use]
pub fn format_length(value: f64, unit: Unit, display: LengthDisplay) -> String {
    match unit {
        Unit::Millimetres | Unit::Metres => format!("{value:.3} {}", unit.symbol()),
        Unit::Feet if display.decimals => format!("{}'", decimals(value)),
        Unit::Inches if display.decimals => format!("{}\"", decimals(value)),
        Unit::Feet => feet_and_inches(value * 12.0, display.denominator),
        Unit::Inches => inches(value, display.denominator),
    }
}

/// An area in `unit` squared.
#[must_use]
pub fn format_area(value: f64, unit: Unit) -> String {
    format!("{value:.3} {}²", unit.symbol())
}

/// A volume in `unit` cubed.
#[must_use]
pub fn format_volume(value: f64, unit: Unit) -> String {
    format!("{value:.3} {}³", unit.symbol())
}

/// Up to three decimals, without trailing zeros.
fn decimals(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// Whole units and a reduced fraction of `1/denominator`, rounded to the nearest; the sign, the
/// whole part and the fraction's numerator and denominator.
fn split(value: f64, denominator: u32) -> (bool, u64, u64, u64) {
    let denominator = u64::from(denominator.max(1));
    // Lengths are far below 2^53 sixty-fourths, so the rounding is exact enough.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let steps = (value.abs() * denominator as f64).round() as u64;
    let negative = value < 0.0 && steps > 0;
    let (whole, mut top) = (steps / denominator, steps % denominator);
    let mut bottom = denominator;
    while top > 0 && top % 2 == 0 && bottom % 2 == 0 {
        top /= 2;
        bottom /= 2;
    }
    (negative, whole, top, bottom)
}

fn fraction(whole: u64, top: u64, bottom: u64, leave_zero: bool) -> String {
    match (whole, top) {
        (0, 0) if leave_zero => String::new(),
        (w, 0) => w.to_string(),
        (0, t) => format!("{t}/{bottom}"),
        (w, t) => format!("{w} {t}/{bottom}"),
    }
}

/// `63 1/2"`.
fn inches(value: f64, denominator: u32) -> String {
    let (negative, whole, top, bottom) = split(value, denominator);
    let sign = if negative { "-" } else { "" };
    format!("{sign}{}\"", fraction(whole, top, bottom, false))
}

/// `5'-3 1/2"` from a length in inches.
fn feet_and_inches(total_inches: f64, denominator: u32) -> String {
    let (negative, whole, top, bottom) = split(total_inches, denominator);
    let sign = if negative { "-" } else { "" };
    let (feet, inches) = (whole / 12, whole % 12);
    format!("{sign}{feet}'-{}\"", fraction(inches, top, bottom, false))
}

/// A typed length in `unit`: a plain number is already in it; a number with a unit (`20mm`,
/// `1.6m`, `5ft`, `63in`, `5'`, `3/8"`), or feet and inches together (`5'3"`, `5'-3 1/2"`,
/// `5ft 3in`), is converted to it. Fractions (`1/2`, `3 1/2`) work anywhere a number does.
#[must_use]
pub fn parse_length(text: &str, unit: Unit) -> Option<f64> {
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = text.parse::<f64>() {
        return value.is_finite().then_some(value);
    }
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(&text)),
    };
    let mut parts = Parser { rest: rest.trim() };
    let mut millimetres = 0.0;
    let mut any_unit = false;
    let mut after_feet = false;
    let mut plain = None;
    while !parts.rest.is_empty() {
        let amount = parts.amount()?;
        match parts.unit() {
            Some(found) => {
                millimetres += amount * found;
                any_unit = true;
                after_feet = found == FOOT;
                // A dash may join feet to the inches after them: `5'-3"`.
                if after_feet {
                    parts.rest = parts.rest.trim_start();
                    if let Some(rest) = parts.rest.strip_prefix('-') {
                        parts.rest = rest;
                    }
                }
            }
            // A bare number after feet is inches (`5' 3`); otherwise it must stand alone.
            None if after_feet => {
                millimetres += amount * INCH;
                after_feet = false;
            }
            None if !any_unit && plain.is_none() => plain = Some(amount),
            None => return None,
        }
        parts.rest = parts.rest.trim_start();
    }
    let value = match (plain, any_unit) {
        (Some(_), true) => return None,
        (Some(amount), false) => amount,
        (None, true) => millimetres / unit.millimetres(),
        (None, false) => return None,
    };
    let value = if negative { -value } else { value };
    value.is_finite().then_some(value)
}

const INCH: f64 = 25.4;
const FOOT: f64 = 304.8;

/// What is left of a typed length.
struct Parser<'a> {
    rest: &'a str,
}

impl Parser<'_> {
    /// A number, a fraction (`3/8`) or a whole and a fraction (`3 1/2`).
    fn amount(&mut self) -> Option<f64> {
        let first = self.number()?;
        if let Some(rest) = self.rest.strip_prefix('/') {
            self.rest = rest;
            let bottom = self.number()?;
            return (bottom != 0.0).then(|| first / bottom);
        }
        // `3 1/2`: a whole number, a space, then a fraction.
        let saved = self.rest;
        let spaced = self.rest.trim_start();
        if spaced.len() < self.rest.len() {
            self.rest = spaced;
            if let Some(top) = self.number()
                && let Some(rest) = self.rest.strip_prefix('/')
            {
                self.rest = rest;
                if let Some(bottom) = self.number().filter(|b| *b != 0.0) {
                    return Some(first + top / bottom);
                }
            }
            self.rest = saved;
        }
        Some(first)
    }

    fn number(&mut self) -> Option<f64> {
        let end = self
            .rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(self.rest.len());
        let (digits, rest) = self.rest.split_at(end);
        let value = digits.parse::<f64>().ok()?;
        self.rest = rest;
        Some(value)
    }

    /// A unit right after a number, perhaps after a space: its size in millimetres.
    fn unit(&mut self) -> Option<f64> {
        let trimmed = self.rest.trim_start();
        for (names, size) in UNITS {
            for name in names {
                if let Some(rest) = trimmed.strip_prefix(name) {
                    // A unit word must end there, so `m` is not read from `mm`.
                    let ends = !rest.starts_with(|c: char| c.is_ascii_alphabetic());
                    if ends {
                        self.rest = rest;
                        return Some(size);
                    }
                }
            }
        }
        None
    }
}

/// Units that can be typed, longest names first.
const UNITS: [(&[&str], f64); 6] = [
    (&["millimetres", "millimeters", "mm"], 1.0),
    (&["centimetres", "centimeters", "cm"], 10.0),
    (&["metres", "meters", "m"], 1000.0),
    (&["feet", "foot", "ft", "'", "′"], FOOT),
    (&["inches", "inch", "in", "\"", "″"], INCH),
    (&["kilometres", "kilometers", "km"], 1_000_000.0),
];

#[cfg(test)]
mod tests {
    use super::*;

    const FRACTIONS: LengthDisplay = LengthDisplay {
        decimals: false,
        denominator: 16,
    };

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn feet_show_as_feet_and_inches_with_fractions() {
        let show = |feet: f64| format_length(feet, Unit::Feet, FRACTIONS);
        assert_eq!(show(5.0 + 3.5 / 12.0), "5'-3 1/2\"");
        assert_eq!(show(5.0), "5'-0\"");
        assert_eq!(show(0.5 / 12.0), "0'-1/2\"");
        assert_eq!(show(-(1.0 + 1.0 / 12.0)), "-1'-1\"");
        // Rounding up to a whole foot carries over.
        assert_eq!(show(1.0 - 0.001 / 12.0), "1'-0\"");
    }

    #[test]
    fn inches_show_with_fractions_reduced() {
        let show = |inches: f64, d: u32| {
            format_length(
                inches,
                Unit::Inches,
                LengthDisplay {
                    decimals: false,
                    denominator: d,
                },
            )
        };
        assert_eq!(show(63.5, 16), "63 1/2\"");
        assert_eq!(show(0.375, 64), "3/8\"");
        assert_eq!(show(0.3, 2), "1/2\"");
        assert_eq!(show(10.0, 16), "10\"");
    }

    #[test]
    fn decimals_can_be_chosen_instead() {
        let decimals = LengthDisplay {
            decimals: true,
            denominator: 16,
        };
        assert_eq!(format_length(5.29, Unit::Feet, decimals), "5.29'");
        assert_eq!(format_length(63.5, Unit::Inches, decimals), "63.5\"");
        assert_eq!(
            format_length(12.5, Unit::Millimetres, FRACTIONS),
            "12.500 mm"
        );
    }

    #[test]
    fn every_typed_form_is_read_in_any_unit() {
        let feet = |text: &str| parse_length(text, Unit::Feet);
        assert!(close(feet("5'3\""), 5.25));
        assert!(close(feet("5'-3 1/2\""), 5.0 + 3.5 / 12.0));
        assert!(close(feet("5ft"), 5.0));
        assert!(close(feet("5ft 3in"), 5.25));
        assert!(close(feet("5' 3"), 5.25));
        assert!(close(feet("63in"), 5.25));
        assert!(close(feet("63.5"), 63.5));
        assert!(close(feet("-2'"), -2.0));
        let millimetres = |text: &str| parse_length(text, Unit::Millimetres);
        assert!(close(millimetres("1.6m"), 1600.0));
        assert!(close(millimetres("20mm"), 20.0));
        assert!(close(millimetres("2 cm"), 20.0));
        assert!(close(millimetres("3/8\""), 9.525));
        assert!(close(millimetres("1/2"), 0.5));
        assert!(close(millimetres("3 1/2"), 3.5));
        assert!(close(parse_length("1ft", Unit::Inches), 12.0));
    }

    #[test]
    fn nonsense_is_not_a_length() {
        for text in ["", "abc", "5'x", "3/0", "5 6", "2m 3", "--1", "1.2.3"] {
            assert_eq!(parse_length(text, Unit::Millimetres), None, "{text}");
        }
    }
}
