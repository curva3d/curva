//! The list of commands, and the steps several of them share. One command per file.

mod circle;
mod copy;
mod delete;
mod interpcrv;
mod line;
mod move_objects;
mod new;
mod open;
mod polyline;
mod redo;
mod save;
mod undo;

use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_kernel::{KernelError, Point};

/// How a command is listed.
pub struct CommandDef {
    /// Its Rhino name.
    pub name: &'static str,
    /// Its short names.
    pub aliases: &'static [&'static str],
    /// Whether Enter on an empty line runs it again.
    pub repeatable: bool,
    /// Whether what it changes becomes one undo step.
    pub records_undo: bool,
    /// Makes a fresh run of it.
    pub make: fn() -> Box<dyn Command>,
}

macro_rules! listed {
    ($name:literal, [$($alias:literal),*], $repeat:literal, $undo:literal, $kind:ty) => {
        CommandDef {
            name: $name,
            aliases: &[$($alias),*],
            repeatable: $repeat,
            records_undo: $undo,
            make: || Box::<$kind>::default(),
        }
    };
}

/// Every command Curva has.
pub static COMMANDS: [CommandDef; 12] = [
    listed!("Line", ["l"], true, true, line::Line),
    listed!("Polyline", ["pl", "pline"], true, true, polyline::Polyline),
    listed!(
        "InterpCrv",
        ["interpcurve"],
        true,
        true,
        interpcrv::InterpCrv
    ),
    listed!("Circle", ["c", "ci"], true, true, circle::Circle),
    listed!("Move", ["m"], true, true, move_objects::MoveObjects),
    listed!("Copy", ["co", "cp"], true, true, copy::CopyObjects),
    listed!("Delete", ["del", "erase"], false, true, delete::Delete),
    listed!("Undo", [], false, false, undo::Undo),
    listed!("Redo", [], false, false, redo::Redo),
    listed!("Save", [], false, false, save::Save),
    listed!("Open", [], false, false, open::Open),
    listed!("New", [], false, false, new::New),
];

/// Finds a command by name or alias, ignoring case.
#[must_use]
pub fn resolve(typed: &str) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|def| {
        def.name.eq_ignore_ascii_case(typed)
            || def.aliases.iter().any(|a| a.eq_ignore_ascii_case(typed))
    })
}

/// Commands whose name or alias starts with the typed text, ignoring case.
///
/// An exact name or alias comes first, so `c` offers `Circle` ahead of `Copy`.
#[must_use]
pub fn completions(prefix: &str) -> Vec<&'static str> {
    let lowered = prefix.to_lowercase();
    if lowered.is_empty() {
        return Vec::new();
    }
    let exact = resolve(prefix).map(|def| def.name);
    let others = COMMANDS
        .iter()
        .filter(|def| {
            Some(def.name) != exact
                && std::iter::once(&def.name)
                    .chain(def.aliases)
                    .any(|name| name.to_lowercase().starts_with(&lowered))
        })
        .map(|def| def.name);
    exact.into_iter().chain(others).collect()
}

/// What Enter or Space should run for the typed text and the highlighted completion.
///
/// `New` discards the model and cannot be undone, so a partial name never reaches it.
#[must_use]
pub fn autofilled(typed: &str, highlighted: usize) -> String {
    match completions(typed).get(highlighted) {
        Some(&name) if name != "New" || name.eq_ignore_ascii_case(typed.trim()) => name.to_owned(),
        _ => typed.to_owned(),
    }
}

fn unexpected() -> Step {
    Step::Failed(Message::new("error.unexpected"))
}

fn geometry_failed(error: &KernelError) -> Step {
    Step::Failed(Message::with("error.geometry", vec![error.to_string()]))
}

fn ask_point(key: &'static str, preview: Vec<Point>) -> Step {
    Step::Ask(Request {
        preview,
        ..Request::new(key, Kind::Point)
    })
}

fn ask_objects(key: &'static str) -> Step {
    Step::Ask(Request::new(key, Kind::Objects))
}

/// Collects points until Enter or `Close`, for commands that draw through many points.
#[derive(Default)]
struct PointRun {
    points: Vec<Point>,
}

/// What [`PointRun`] wants next.
enum RunState {
    Asking(Step),
    Finished { closed: bool },
}

impl PointRun {
    fn feed(
        &mut self,
        answer: Option<Answer>,
        start_key: &'static str,
        next_key: &'static str,
    ) -> RunState {
        match answer {
            None => {}
            Some(Answer::Point(point)) => self.points.push(point),
            Some(Answer::Empty) => return RunState::Finished { closed: false },
            Some(Answer::Option(_)) => return RunState::Finished { closed: true },
            Some(_) => return RunState::Asking(unexpected()),
        }
        let count = self.points.len();
        RunState::Asking(Step::Ask(Request {
            allow_empty: count >= 2,
            options: if count >= 3 {
                vec!["Close"]
            } else {
                Vec::new()
            },
            preview: self.points.clone(),
            ..Request::new(if count == 0 { start_key } else { next_key }, Kind::Point)
        }))
    }
}

/// Runs typed lines through a fresh processor, for the commands' tests.
#[cfg(test)]
fn run(doc: &mut curva_document::Document, lines: &[&str]) -> crate::Processor {
    let mut processor = crate::Processor::default();
    for line in lines {
        processor.submit(doc, line);
    }
    processor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_aliases_ignore_case() {
        assert_eq!(resolve("LINE").map(|d| d.name), Some("Line"));
        assert_eq!(resolve("pl").map(|d| d.name), Some("Polyline"));
        assert!(resolve("nothing").is_none());
    }

    #[test]
    fn no_two_commands_share_a_name_or_alias() {
        let mut seen = std::collections::BTreeSet::new();
        for def in &COMMANDS {
            for name in std::iter::once(&def.name).chain(def.aliases) {
                assert!(seen.insert(name.to_lowercase()), "{name} is used twice");
            }
        }
    }

    #[test]
    fn completion_offers_matching_names() {
        assert_eq!(completions("po"), ["Polyline"]);
        assert!(completions("").is_empty());
    }

    #[test]
    fn completion_finds_aliases_and_puts_an_exact_match_first() {
        assert_eq!(completions("pl"), ["Polyline"]);
        assert_eq!(completions("c"), ["Circle", "Copy"]);
        assert_eq!(completions("co"), ["Copy"]);
    }

    #[test]
    fn autofill_runs_the_highlighted_command() {
        assert_eq!(autofilled("pol", 0), "Polyline");
        assert_eq!(autofilled("c", 0), "Circle");
        assert_eq!(autofilled("c", 1), "Copy");
    }

    #[test]
    fn autofill_leaves_text_with_no_match_alone() {
        assert_eq!(autofilled("zzz", 0), "zzz");
        assert_eq!(autofilled("pol", 5), "pol");
    }

    #[test]
    fn a_partial_name_never_runs_new() {
        assert_eq!(autofilled("n", 0), "n");
        assert_eq!(autofilled("ne", 0), "ne");
        assert_eq!(autofilled("NEW", 0), "New");
    }
}
