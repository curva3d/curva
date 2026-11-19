//! The size limits in `AGENTS.md` ("Keeping the code tidy"), measured so a test can check them
//! (code health review of 19 November 2026, issue #400): a source file stays under 800 lines not
//! counting its tests, and a function under 80.
//!
//! The measure reads Rust as text, which is enough here: comments, strings and character
//! literals are blanked first so their braces do not count, then test items (`#[cfg(test)]` and
//! `#[test]`) are left out, and each `fn` is measured from its line to its closing brace.

/// The most lines a source file may have, not counting its tests.
pub const FILE_LIMIT: usize = 800;

/// The most lines a function may have, from its `fn` line to its closing brace.
pub const FUNCTION_LIMIT: usize = 80;

/// One source file's sizes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sizes {
    /// Its lines outside test items.
    pub lines: usize,
    /// Each function outside test items: its name and its length in lines, in file order.
    pub functions: Vec<(String, usize)>,
}

/// The sizes of one Rust source file.
#[must_use]
pub fn measure(source: &str) -> Sizes {
    let code: Vec<char> = blanked(source).chars().collect();
    let line_of = line_starts(&code);
    let tests = test_spans(&code);
    let in_tests = |at: usize| tests.iter().any(|&(a, b)| a <= at && at <= b);
    let total = source.lines().count();
    let test_lines: usize = tests
        .iter()
        .map(|&(a, b)| line_of(b) - line_of(a) + 1)
        .sum();
    let mut functions = Vec::new();
    let mut at = 0;
    while let Some(found) = find_word(&code, "fn", at) {
        at = found + 2;
        if in_tests(found) {
            continue;
        }
        let Some((name, open)) = signature(&code, at) else {
            continue;
        };
        if let Some(close) = matching(&code, open) {
            functions.push((name, line_of(close) - line_of(found) + 1));
        }
    }
    Sizes {
        lines: total.saturating_sub(test_lines),
        functions,
    }
}

/// The source with comments, strings and character literals turned into spaces, newlines kept,
/// so only code is left to count braces in.
fn blanked(source: &str) -> String {
    let s: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < s.len() {
        let rest = |k: usize| s.get(i + k).copied();
        let end = if s[i] == '/' && rest(1) == Some('/') {
            s[i..]
                .iter()
                .position(|&c| c == '\n')
                .map_or(s.len(), |p| i + p)
        } else if s[i] == '/' && rest(1) == Some('*') {
            block_comment_end(&s, i)
        } else if s[i] == '"' || (s[i] == 'b' && rest(1) == Some('"')) {
            string_end(&s, i + usize::from(s[i] == 'b'))
        } else if let Some(hashes) = raw_string_start(&s, i) {
            raw_string_end(&s, i, hashes)
        } else if s[i] == '\'' {
            char_literal_end(&s, i).unwrap_or(i + 1)
        } else {
            out.push(s[i]);
            i += 1;
            continue;
        };
        let end = end.min(s.len());
        let start = i;
        if s[i] == '\'' && end == i + 1 {
            // A lifetime, not a character: kept as code.
            out.push('\'');
        } else {
            out.extend(s[start..end].iter().map(|&c| blank(c)));
        }
        i = end.max(i + 1);
    }
    out
}

fn block_comment_end(s: &[char], from: usize) -> usize {
    let (mut depth, mut i) = (0usize, from);
    while i + 1 < s.len() {
        if s[i] == '/' && s[i + 1] == '*' {
            depth += 1;
            i += 2;
        } else if s[i] == '*' && s[i + 1] == '/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    s.len()
}

/// The end of a string starting at the quote `from`, past its closing quote.
fn string_end(s: &[char], from: usize) -> usize {
    let mut i = from + 1;
    while i < s.len() {
        match s[i] {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    s.len()
}

/// The number of `#`s of a raw string (`r"`, `r#"`, `br#"`) starting at `at`, if one does.
fn raw_string_start(s: &[char], at: usize) -> Option<usize> {
    let after_prefix = match (s.get(at), s.get(at + 1)) {
        (Some('r'), _) => at + 1,
        (Some('b'), Some('r')) => at + 2,
        _ => return None,
    };
    // Not the end of a longer name, such as `bar"`.
    if at > 0
        && s.get(at - 1)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    {
        return None;
    }
    let hashes = s[after_prefix..].iter().take_while(|&&c| c == '#').count();
    (s.get(after_prefix + hashes) == Some(&'"')).then_some(hashes)
}

fn raw_string_end(s: &[char], from: usize, hashes: usize) -> usize {
    let open = s[from..]
        .iter()
        .position(|&c| c == '"')
        .map_or(s.len(), |p| from + p);
    let mut i = open + 1;
    while i < s.len() {
        if s[i] == '"'
            && s[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&c| c == '#')
                .count()
                == hashes
        {
            return i + 1 + hashes;
        }
        i += 1;
    }
    s.len()
}

/// The end of a character literal at `from` (`'a'`, `'\n'`, `'\u{7b}'`), or `None` for a
/// lifetime (`'a`).
fn char_literal_end(s: &[char], from: usize) -> Option<usize> {
    if s.get(from + 1) == Some(&'\\') {
        // Past the escaped character, which may itself be a quote (`'\''`).
        let close = s.get(from + 3..)?.iter().position(|&c| c == '\'')?;
        return Some(from + 3 + close + 1);
    }
    (s.get(from + 2) == Some(&'\'')).then_some(from + 3)
}

/// A function giving the line (from 1) of a position.
fn line_starts(code: &[char]) -> impl Fn(usize) -> usize + use<> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(
            code.iter()
                .enumerate()
                .filter(|(_, c)| **c == '\n')
                .map(|(i, _)| i + 1),
        )
        .collect();
    move |at| starts.partition_point(|&s| s <= at)
}

/// Each test item, `#[cfg(test)]` or `#[test]` up to the end of the item it marks.
fn test_spans(code: &[char]) -> Vec<(usize, usize)> {
    let text: String = code.iter().collect();
    let mut spans = Vec::new();
    for mark in ["#[cfg(test)]", "#[test]"] {
        let mut from = 0;
        while let Some(found) = text[from..].find(mark) {
            let start = from + found;
            from = start + mark.len();
            // The item ends at its first `;` or at the brace closing its first `{`.
            let after = &code[from..];
            let end = match after.iter().position(|&c| c == '{' || c == ';') {
                Some(p) if after[p] == '{' => matching(code, from + p),
                Some(p) => Some(from + p),
                None => None,
            };
            if let Some(end) = end {
                spans.push((start, end));
            }
        }
    }
    // A `#[test]` inside a `#[cfg(test)]` module is counted once.
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (a, b) in spans {
        match merged.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    merged
}

/// The next `word` standing alone at or after `from`.
fn find_word(code: &[char], word: &str, from: usize) -> Option<usize> {
    let w: Vec<char> = word.chars().collect();
    let part = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric() || *c == '_');
    (from..code.len().saturating_sub(w.len() - 1)).find(|&i| {
        code[i..i + w.len()] == w[..]
            && !part(i.checked_sub(1).and_then(|p| code.get(p)))
            && !part(code.get(i + w.len()))
    })
}

/// After `fn`: the function's name and the position of its body's `{`, or `None` for a
/// declaration without a body (`fn f();` in a trait) or a `fn` type.
fn signature(code: &[char], from: usize) -> Option<(String, usize)> {
    let start = from
        + code[from..]
            .iter()
            .take_while(|c| c.is_whitespace())
            .count();
    let name: String = code[start..]
        .iter()
        .take_while(|c| c.is_alphanumeric() || **c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let mut depth = 0i32;
    for (i, &c) in code.iter().enumerate().skip(start + name.len()) {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' => depth -= 1,
            // `->` is not a closing angle bracket.
            '>' if code.get(i.wrapping_sub(1)) != Some(&'-') => depth -= 1,
            '{' if depth <= 0 => return Some((name, i)),
            ';' if depth <= 0 => return None,
            _ => {}
        }
    }
    None
}

/// The brace closing the one at `open`.
fn matching(code: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, &c) in code.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_are_measured_from_their_line_to_their_closing_brace() {
        let source = "fn one() {\n    let a = 1;\n}\n\npub(crate) fn two<T: Into<f64>>(\n    x: T,\n) -> f64 {\n    x.into()\n}\n";
        let sizes = measure(source);
        assert_eq!(
            sizes.functions,
            vec![("one".to_owned(), 3), ("two".to_owned(), 5)]
        );
        assert_eq!(sizes.lines, 9);
    }

    #[test]
    fn braces_in_strings_comments_and_characters_do_not_count() {
        let source = "fn f<'a>(s: &'a str) -> char {\n    // {\n    let t = \"}}\";\n    let r = r#\"{\"#;\n    /* { */\n    if s.is_empty() { '{' } else { '}' }\n}\n";
        assert_eq!(measure(source).functions, vec![("f".to_owned(), 7)]);
    }

    #[test]
    fn tests_and_bodiless_functions_are_left_out() {
        let source = "trait T {\n    fn declared(&self);\n}\nfn kept() {}\n#[cfg(test)]\nmod tests {\n    fn helper() {\n    }\n    #[test]\n    fn t() {}\n}\n";
        let sizes = measure(source);
        assert_eq!(sizes.functions, vec![("kept".to_owned(), 1)]);
        assert_eq!(sizes.lines, 11 - 7);
    }
}
