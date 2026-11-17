//! Rhino's notes (`ON_Text`) as Curva's: their words out of rich text, and where their first
//! line's baseline is, from how Rhino aligns them.

use crate::model::{DimStyle, P3, Text};
use curva_document::annotation::{LINE_SPACING, Text as Note, TextAlign};

/// Rich text (RTF, as Rhino keeps a note's words) as plain words: groups that hold no words
/// (the font and colour tables, `\*` destinations) are dropped, `\par` and `\line` end a line,
/// `\'hh` and `\uN` are characters, and other control words are skipped (Microsoft's Rich Text
/// Format specification 1.9.1, "Conventions of an RTF Reader"). Words that are not RTF stay
/// as they are.
#[must_use]
pub fn plain_text(rich: &str) -> String {
    if !rich.trim_start().starts_with("{\\rtf") {
        return rich.replace("\r\n", "\n");
    }
    let mut rtf = Rtf {
        chars: rich.chars().collect(),
        out: String::new(),
        dropped: vec![false],
        stand_ins: 1,
    };
    let mut i = 0;
    while i < rtf.chars.len() {
        let hidden = rtf.hidden();
        match rtf.chars[i] {
            '{' => {
                rtf.dropped.push(hidden);
                i += 1;
            }
            '}' => {
                rtf.dropped.pop();
                i += 1;
            }
            '\\' => match rtf.escape(i) {
                Some(next) => i = next,
                None => break,
            },
            '\r' | '\n' => i += 1,
            other => {
                if !hidden {
                    rtf.out.push(other);
                }
                i += 1;
            }
        }
    }
    rtf.out.trim_end_matches('\n').to_owned()
}

/// Rich text being read: its characters, the words so far, for each open group whether its
/// words are dropped, and how many stand-in characters follow a `\u` character.
struct Rtf {
    chars: Vec<char>,
    out: String,
    dropped: Vec<bool>,
    stand_ins: usize,
}

impl Rtf {
    /// Whether the open group's words are dropped.
    fn hidden(&self) -> bool {
        self.dropped.last().copied().unwrap_or(false)
    }

    /// Drops the open group's words.
    fn drop_group(&mut self) {
        if let Some(last) = self.dropped.last_mut() {
            *last = true;
        }
    }

    /// What follows a backslash at `i`: an escaped character, a `\'hh` byte, a `\*`
    /// destination or a control word; where reading goes on, or `None` at the end.
    fn escape(&mut self, i: usize) -> Option<usize> {
        let hidden = self.hidden();
        let next = *self.chars.get(i + 1)?;
        Some(if matches!(next, '\\' | '{' | '}') {
            if !hidden {
                self.out.push(next);
            }
            i + 2
        } else if next == '\'' {
            let hex: String = self.chars.iter().skip(i + 2).take(2).collect();
            if let (false, Ok(byte)) = (hidden, u8::from_str_radix(&hex, 16)) {
                self.out.push(char::from(byte));
            }
            i + 4
        } else if next == '*' {
            self.drop_group();
            i + 2
        } else if next.is_ascii_alphabetic() {
            self.control_word(i)
        } else {
            i + 2
        })
    }

    /// A control word at `i` (a backslash, letters, perhaps a number, perhaps one space);
    /// where reading goes on.
    fn control_word(&mut self, i: usize) -> usize {
        let hidden = self.hidden();
        let chars = &self.chars;
        let mut j = i + 1;
        while j < chars.len() && chars[j].is_ascii_alphabetic() {
            j += 1;
        }
        let word: String = chars[i + 1..j].iter().collect();
        let digits = j;
        if j < chars.len() && (chars[j] == '-' || chars[j].is_ascii_digit()) {
            j += 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
        }
        let number: Option<i64> = chars[digits..j].iter().collect::<String>().parse().ok();
        // One space after a control word belongs to it.
        if j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        match word.as_str() {
            "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" => self.drop_group(),
            "par" | "line" if !hidden => self.out.push('\n'),
            "tab" if !hidden => self.out.push('\t'),
            "uc" => {
                self.stand_ins = number.and_then(|n| usize::try_from(n).ok()).unwrap_or(1);
            }
            "u" => {
                if let (false, Some(n)) = (hidden, number) {
                    let code = if n < 0 { n + 65_536 } else { n };
                    if let Some(ch) = u32::try_from(code).ok().and_then(char::from_u32) {
                        self.out.push(ch);
                    }
                }
                // The stand-in characters that follow, for older readers.
                let mut left = self.stand_ins;
                while left > 0 && j < self.chars.len() && !matches!(self.chars[j], '\\' | '{' | '}')
                {
                    j += 1;
                    left -= 1;
                }
            }
            _ => {}
        }
        j
    }
}

/// The first font a note's rich text lists, if any: `{\fonttbl{\f0 Arial;}}` gives `Arial`.
fn rich_font(rich: &str) -> Option<String> {
    let table = &rich[rich.find("\\fonttbl")?..];
    let mut rest = &table[table.find("\\f0")? + 3..];
    // Further control words (`\fnil`, `\fcharset0`) come before the name.
    while let Some(after) = rest.trim_start().strip_prefix('\\') {
        let end = after.find([' ', ';', '\\']).unwrap_or(after.len());
        rest = &after[end..];
    }
    let name = rest.trim_start().split(';').next()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// A note from a file, in the file's unit: its words, height and alignment from the style it sets
/// for itself or the one it names, its first line's baseline worked out from where Rhino aligns
/// it (Rhino 5's numbering: 3 top, 4 middle of the first line, 5 middle, 6 middle of the last
/// line, 7 bottom).
#[must_use]
pub fn note_in(t: &Text, styles: &[DimStyle]) -> Note {
    let style = t
        .own
        .clone()
        .or_else(|| styles.iter().find(|s| s.id == t.style).cloned());
    let (height, h_align, v_align) =
        style.map_or((1.0, 0, 3), |s| (s.text_height, s.h_align, s.v_align));
    let content = plain_text(&t.rich);
    let lines = content.lines().count().max(1);
    let z = t.plane.z;
    let (sin, cos) = t.rotation.sin_cos();
    let x: P3 = std::array::from_fn(|k| t.plane.x[k] * cos + t.plane.y[k] * sin);
    let y: P3 = [
        z[1] * x[2] - z[2] * x[1],
        z[2] * x[0] - z[0] * x[2],
        z[0] * x[1] - z[1] * x[0],
    ];
    #[allow(clippy::cast_precision_loss)]
    let between = (lines - 1) as f64 * height * LINE_SPACING;
    // How far up from the plane's origin the first line's baseline is.
    let up = match v_align {
        3 => -height,
        4 => -height / 2.0,
        5 => (height + between) / 2.0 - height,
        6 => between - height / 2.0,
        _ => between,
    };
    Note {
        content,
        origin: std::array::from_fn(|k| t.plane.origin[k] + y[k] * up),
        x,
        y,
        height,
        font: rich_font(&t.rich),
        align: match h_align {
            1 => TextAlign::Centre,
            2 => TextAlign::Right,
            _ => TextAlign::Left,
        },
        style: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_text_becomes_plain_words() {
        let rich =
            r"{\rtf1\deff0{\fonttbl{\f0 Arial;}}\f0\fs20 Ground floor\par Plan \u233?\'e9\}}";
        assert_eq!(plain_text(rich), "Ground floor\nPlan éé}");
        assert_eq!(rich_font(rich).as_deref(), Some("Arial"));
        assert_eq!(plain_text("Hall\r\nKitchen"), "Hall\nKitchen");
        assert_eq!(
            rich_font(r"{\rtf1{\fonttbl{\f0\fnil\fcharset0 Segoe UI;}}x}").as_deref(),
            Some("Segoe UI")
        );
    }
}
