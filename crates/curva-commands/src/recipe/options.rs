//! Options of a recipe: what each holds, as Rhino offers them.

#[allow(clippy::wildcard_imports)]
use super::*;

/// What an option holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionKind {
    /// Yes or no, flipped by choosing it; remembered between runs.
    Toggle(bool),
    /// A number, typed after choosing it; remembered between runs.
    Number(f64),
    /// A whole number from `low` to `high`, typed after choosing it, such as a curve's degree;
    /// remembered between runs.
    Whole {
        /// Its value until another is typed.
        default: u32,
        /// The least it may be.
        low: u32,
        /// The most it may be.
        high: u32,
    },
    /// A length, typed after choosing it: more than zero, or zero too when `zero` is set;
    /// remembered between runs.
    Distance {
        /// Its value until another is typed.
        default: f64,
        /// Whether zero is allowed, as for a fillet's radius.
        zero: bool,
    },
    /// A way of making the result, such as Arc's `StartPoint`: choosing it switches to it for this
    /// run, and the steps can change with it.
    Mode,
    /// Finishes the command at once in this way, such as `Close` on a polyline; the recipe sees
    /// it as the mode.
    Finish,
    /// Takes back the last answer, as Rhino's `Undo` option does while drawing.
    Undo,
    /// A name, typed after choosing it, such as a hatch pattern's; one of the recipe's
    /// [`Recipe::choices`](super::Recipe::choices) when it gives them. Remembered between runs.
    Name(&'static str),
}

/// One option of a command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionDef {
    /// Its name, as typed and shown.
    pub name: &'static str,
    /// What it holds.
    pub kind: OptionKind,
    /// For a number remembered for each value of a name option, as Rhino's MeshSphere keeps
    /// one `Subdivisions` for each style: that option, and the starting value for each of its
    /// values (the kind's own default for any other).
    pub per: Option<Per>,
}

/// A number option remembered for each value of a name option (see [`OptionDef::per`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Per {
    /// The name option whose value it is remembered for.
    pub by: &'static str,
    /// The starting value for each of that option's values.
    pub starting: &'static [(&'static str, f64)],
}

impl OptionDef {
    /// The same number option, remembered for each value of the name option `by`, starting at
    /// the value `starting` gives for it, as Rhino's MeshSphere keeps `Subdivisions` per style
    /// (Quad 2, Triangle 1).
    #[must_use]
    pub const fn per(self, by: &'static str, starting: &'static [(&'static str, f64)]) -> Self {
        Self {
            per: Some(Per { by, starting }),
            ..self
        }
    }

    /// A yes-or-no option.
    #[must_use]
    pub const fn toggle(name: &'static str, default: bool) -> Self {
        Self {
            name,
            kind: OptionKind::Toggle(default),
            per: None,
        }
    }

    /// A number option.
    #[must_use]
    pub const fn number(name: &'static str, default: f64) -> Self {
        Self {
            name,
            kind: OptionKind::Number(default),
            per: None,
        }
    }

    /// A length option: more than zero, or zero too when `zero` is set.
    #[must_use]
    pub const fn distance(name: &'static str, default: f64, zero: bool) -> Self {
        Self {
            name,
            kind: OptionKind::Distance { default, zero },
            per: None,
        }
    }

    /// Why `value` cannot be this option's value, if it cannot.
    pub(super) fn refuses(&self, value: f64) -> Option<Message> {
        let name = self.name.to_owned();
        match self.kind {
            OptionKind::Whole { low, high, .. }
                if value.fract() != 0.0 || value < f64::from(low) || value > f64::from(high) =>
            {
                Some(Message::with(
                    "error.option_whole",
                    vec![name, low.to_string(), high.to_string()],
                ))
            }
            OptionKind::Distance { zero: true, .. } if value < 0.0 => {
                Some(Message::with("error.option_negative", vec![name]))
            }
            OptionKind::Distance { zero: false, .. } if value <= 0.0 => {
                Some(Message::with("error.option_positive", vec![name]))
            }
            _ if !value.is_finite() => Some(Message::with("error.bad_number", vec![name])),
            _ => None,
        }
    }

    /// A whole-number option, from `low` to `high`.
    #[must_use]
    pub const fn whole(name: &'static str, default: u32, low: u32, high: u32) -> Self {
        Self {
            name,
            kind: OptionKind::Whole { default, low, high },
            per: None,
        }
    }

    /// A way of making the result.
    #[must_use]
    pub const fn mode(name: &'static str) -> Self {
        Self {
            name,
            kind: OptionKind::Mode,
            per: None,
        }
    }

    /// An option that finishes the command in its own way.
    #[must_use]
    pub const fn finish(name: &'static str) -> Self {
        Self {
            name,
            kind: OptionKind::Finish,
            per: None,
        }
    }

    /// A name option, `default` until another is typed.
    #[must_use]
    pub const fn name(name: &'static str, default: &'static str) -> Self {
        Self {
            name,
            kind: OptionKind::Name(default),
            per: None,
        }
    }

    /// Rhino's `Undo` while drawing: takes back the last answer.
    #[must_use]
    pub const fn undo() -> Self {
        Self {
            name: "Undo",
            kind: OptionKind::Undo,
            per: None,
        }
    }
}
