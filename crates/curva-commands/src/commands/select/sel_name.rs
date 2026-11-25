//! `SelName`, as in Rhino: selects the objects whose name matches, whatever its case, with `*` for
//! any run of letters and `?` for any one (#743).

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// The running command.
/// Whether `name` matches `pattern`, whatever their case: `*` stands for any run of characters,
/// `?` for any one.
fn matches(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (
        pattern.to_lowercase().chars().collect(),
        name.to_lowercase().chars().collect(),
    );
    // `ok[j]`: whether the pattern so far matches the first `j` characters of the name.
    let mut ok = vec![false; n.len() + 1];
    ok[0] = true;
    for c in &p {
        let mut next = vec![false; n.len() + 1];
        if *c == '*' {
            let mut any = false;
            for j in 0..=n.len() {
                any |= ok[j];
                next[j] = any;
            }
        } else {
            for j in 1..=n.len() {
                next[j] = ok[j - 1] && (*c == '?' || *c == n[j - 1]);
            }
        }
        ok = next;
    }
    ok[n.len()]
}

#[derive(Default)]
pub struct SelName;

impl Command for SelName {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.selname.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let open = doc.selectable();
                let found: Vec<u64> = doc
                    .state()
                    .objects
                    .iter()
                    .filter(|o| open.contains(&o.id) && matches(&name, &o.name))
                    .map(|o| o.id)
                    .collect();
                doc.select(&found);
                Step::Done(Some(Message::with(
                    "done.select",
                    vec![found.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn wildcards_match_as_in_rhino() {
        assert!(super::matches("al*", "Alpha"));
        assert!(super::matches("*ph?", "alpha"));
        assert!(super::matches("*", ""));
        assert!(!super::matches("al*", "beta"));
        assert!(!super::matches("a?", "abc"));
    }

    #[test]
    fn it_finds_a_name_whatever_its_case() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.rename(&[a], "Beam");
        run(&mut doc, &["selname", "beam"]);
        assert_eq!(doc.selection(), [a]);
    }
}
