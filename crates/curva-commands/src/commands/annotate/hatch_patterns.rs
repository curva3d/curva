//! `HatchPatterns`: adds the patterns of a `.pat` file to the model's, for Hatch to use (spec
//! `annotation.md` §5). A pattern with the name of one already there takes its place.

use crate::commands::unexpected;
use crate::{Answer, Command, Message, PAT_FILES, Request, Step};
use curva_document::Document;
use curva_document::hatch::{pat_file_patterns, pat_units};

#[derive(Default)]
pub struct HatchPatterns;

impl Command for HatchPatterns {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::file_to_open(
                "prompt.hatch_patterns.path",
                [PAT_FILES],
            )),
            Some(Answer::Path(text)) => {
                let path = std::path::PathBuf::from(text.trim().trim_matches('"'));
                let text = match std::fs::read(&path) {
                    // Pattern files are plain text, often in a Windows code page: anything not
                    // UTF-8 is only in names and comments.
                    Ok(data) => String::from_utf8_lossy(&data).into_owned(),
                    Err(e) => {
                        return Step::Failed(Message::with("error.file", vec![e.to_string()]));
                    }
                };
                let patterns = match pat_file_patterns(&text) {
                    Ok(patterns) if !patterns.is_empty() => patterns,
                    Ok(_) => return Step::Failed(Message::new("error.hatch_patterns.none")),
                    Err(e) => {
                        return Step::Failed(Message::with("error.hatch_patterns.bad", vec![e]));
                    }
                };
                let count = patterns.len();
                let names = patterns
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                for pattern in patterns {
                    doc.add_hatch_pattern(pattern);
                }
                let key = if pat_units(&text) == Some(curva_document::Unit::Inches) {
                    "done.hatch_patterns.inches"
                } else {
                    "done.hatch_patterns"
                };
                Step::Done(Some(Message::with(key, vec![count.to_string(), names])))
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn patterns_from_a_file_join_the_models() {
        let dir = std::env::temp_dir().join(format!("curva-pat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("extra.pat");
        std::fs::write(
            &path,
            ";%UNITS=INCH\n*BRICKS, Brick courses\n0, 0,0, 0,.25\n90, 0,0, .25,.25, .25,-.25\n\
             *Hatch1, Wider lines at 45 degrees\n45, 0,0, 0,.5\n",
        )
        .unwrap();
        let mut doc = Document::default();
        let processor = run(&mut doc, &["HatchPatterns", path.to_str().unwrap()]);
        let said = processor.log().join("\n");
        assert!(said.contains("BRICKS, Hatch1"), "{said}");
        let patterns = doc.state().patterns();
        let bricks = patterns.iter().find(|p| p.name == "BRICKS").unwrap();
        assert!(
            (bricks.lines[0].spacing - 6.35).abs() < 1e-12,
            "in millimetres"
        );
        let hatch1 = patterns.iter().find(|p| p.name == "Hatch1").unwrap();
        assert!(
            (hatch1.lines[0].spacing - 12.7).abs() < 1e-12,
            "in place of the built-in"
        );
        assert_eq!(
            patterns.iter().filter(|p| p.name == "Hatch1").count(),
            1,
            "not listed twice"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
