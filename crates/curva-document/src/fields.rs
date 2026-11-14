//! Rhino's text fields (spec `annotation.md` §2; Rhino 8's `information/text_fields.htm`): text
//! such as `%<PageName()>%` or `%<CurveLength("12")>%` inside a note is worked out when the note is
//! shown. A field that cannot be worked out shows `####`, as in Rhino. Objects are named by their
//! number (Rhino uses its object ids).

use crate::{State, layouts};
use std::path::Path;

/// What a field may need besides the model: the file it is saved in, the sheet the note is on,
/// and the time now.
#[derive(Clone, Debug, Default)]
pub struct FieldContext {
    /// The model's file, if it has been saved.
    pub file: Option<std::path::PathBuf>,
    /// When the file was last saved.
    pub saved: Option<jiff::Zoned>,
    /// The time now, in the computer's time zone; `None` leaves date fields unworked.
    pub now: Option<jiff::Zoned>,
}

impl FieldContext {
    /// The context for a model saved at `file`, now.
    #[must_use]
    pub fn now(file: Option<&Path>) -> Self {
        let saved = file
            .and_then(|f| std::fs::metadata(f).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| jiff::Timestamp::try_from(t).ok())
            .map(|t| t.to_zoned(jiff::tz::TimeZone::system()));
        Self {
            file: file.map(Path::to_path_buf),
            saved,
            now: Some(jiff::Zoned::now()),
        }
    }
}

/// `text` with every `%<...>%` worked out, for a note in `space` (a layout's id, or `None` for
/// the model).
#[must_use]
pub fn resolve(text: &str, state: &State, space: Option<u64>, context: &FieldContext) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("%<") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find(">%") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let expression = &after[..end];
        let worked = Evaluator {
            text: expression,
            at: 0,
            state,
            space,
            context,
        }
        .run();
        out.push_str(&worked.map_or_else(|| "####".to_owned(), |v| v.shown()));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// A field's value.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Number(f64),
    Text(String),
}

impl Value {
    fn shown(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(n) if n.fract().abs() < 1e-9 && n.abs() < 1e15 => format!("{n:.0}"),
            Self::Number(n) => {
                let text = format!("{n:.2}");
                text.trim_end_matches('0').trim_end_matches('.').to_owned()
            }
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(t) => t.trim().parse().ok(),
        }
    }
}

/// Reads and works out one field's expression: numbers, quoted text, function calls, `+ - * /`
/// and brackets.
struct Evaluator<'a> {
    text: &'a str,
    at: usize,
    state: &'a State,
    space: Option<u64>,
    context: &'a FieldContext,
}

impl Evaluator<'_> {
    fn run(mut self) -> Option<Value> {
        let value = self.sum()?;
        self.skip_space();
        (self.at == self.text.len()).then_some(value)
    }

    fn skip_space(&mut self) {
        while self.text[self.at..].starts_with(char::is_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_space();
        if self.text[self.at..].starts_with(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn sum(&mut self) -> Option<Value> {
        let mut left = self.product()?;
        loop {
            if self.eat('+') {
                let right = self.product()?;
                // Numbers add; anything else is joined as text.
                left = match (left.number(), right.number()) {
                    (Some(a), Some(b)) => Value::Number(a + b),
                    _ => Value::Text(format!("{}{}", left.shown(), right.shown())),
                };
            } else if self.eat('-') {
                left = Value::Number(left.number()? - self.product()?.number()?);
            } else {
                return Some(left);
            }
        }
    }

    fn product(&mut self) -> Option<Value> {
        let mut left = self.unit()?;
        loop {
            if self.eat('*') {
                left = Value::Number(left.number()? * self.unit()?.number()?);
            } else if self.eat('/') {
                let right = self.unit()?.number()?;
                if right == 0.0 {
                    return None;
                }
                left = Value::Number(left.number()? / right);
            } else {
                return Some(left);
            }
        }
    }

    fn unit(&mut self) -> Option<Value> {
        self.skip_space();
        let rest = &self.text[self.at..];
        if self.eat('(') {
            let value = self.sum()?;
            return self.eat(')').then_some(value);
        }
        if self.eat('-') {
            return Some(Value::Number(-self.unit()?.number()?));
        }
        if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"')?;
            self.at += end + 2;
            return Some(Value::Text(quoted[..end].to_owned()));
        }
        let digits = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        if digits > 0 {
            self.at += digits;
            return rest[..digits].parse().ok().map(Value::Number);
        }
        let name_end = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        if name_end == 0 {
            return None;
        }
        let name = &rest[..name_end];
        self.at += name_end;
        let mut args = Vec::new();
        if self.eat('(') && !self.eat(')') {
            loop {
                args.push(self.sum()?);
                if self.eat(')') {
                    break;
                }
                if !self.eat(',') {
                    return None;
                }
            }
        }
        self.call(name, &args)
    }

    /// The object numbered by a field's first argument.
    fn object(&self, args: &[Value]) -> Option<&crate::Object> {
        let id = args.first()?.number()?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let id = id as u64;
        self.state.objects.iter().find(|o| o.id == id).map(|o| &**o)
    }

    /// The layout a page field refers to: the one named by number, or the note's own.
    fn page(&self, args: &[Value]) -> Option<(usize, &layouts::Layout)> {
        let id = match args.first() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(arg) => arg.number()? as u64,
            None => self.space?,
        };
        self.state
            .layouts
            .iter()
            .enumerate()
            .find(|(_, l)| l.id == id)
    }

    fn call(&self, name: &str, args: &[Value]) -> Option<Value> {
        let text = |s: &str| Some(Value::Text(s.to_owned()));
        #[allow(clippy::cast_precision_loss)]
        let count = |n: usize| Some(Value::Number(n as f64));
        match name {
            "PageName" => {
                let (index, layout) = self.page(args)?;
                if layout.name.is_empty() {
                    Some(Value::Text(format!("{}", index + 1)))
                } else {
                    text(&layout.name)
                }
            }
            "PageNumber" => Some(Value::Number(self.page(&[]).map_or(0.0, |(index, _)| {
                #[allow(clippy::cast_precision_loss)]
                let number = (index + 1) as f64;
                number
            }))),
            "NumPages" => count(self.state.layouts.len()),
            "PageWidth" => Some(Value::Number(self.page(&[])?.1.size[0])),
            "PageHeight" => Some(Value::Number(self.page(&[])?.1.size[1])),
            "PaperName" => text(self.page(&[])?.1.paper_name()?),
            "ModelUnits" => text(match self.state.unit {
                crate::Unit::Millimetres => "Millimeters",
                crate::Unit::Metres => "Meters",
                crate::Unit::Feet => "Feet",
                crate::Unit::Inches => "Inches",
            }),
            "FileName" => {
                let file = self.context.file.as_ref()?;
                let mask = args.first().and_then(Value::number).unwrap_or(0.0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let mask = mask as u8;
                let short = mask & 1 == 1;
                let bare = mask & 2 == 2;
                let path = if bare {
                    file.with_extension("")
                } else {
                    file.clone()
                };
                let shown = if short {
                    path.file_name()?.to_string_lossy().into_owned()
                } else {
                    path.display().to_string()
                };
                Some(Value::Text(shown))
            }
            "Date" => {
                let format = match args.first() {
                    Some(Value::Text(f)) => f.clone(),
                    _ => "M/d/yyyy".to_owned(),
                };
                Some(Value::Text(net_date(self.context.now.as_ref()?, &format)))
            }
            "DateModified" => {
                let format = match args.first() {
                    Some(Value::Text(f)) => f.clone(),
                    _ => "M/d/yyyy".to_owned(),
                };
                Some(Value::Text(net_date(self.context.saved.as_ref()?, &format)))
            }
            "CurveLength" => Some(Value::Number(self.object(args)?.curve()?.length().ok()?)),
            "Area" => {
                let object = self.object(args)?;
                match &object.shape {
                    crate::Shape::Body(body) => body.area().ok().map(Value::Number),
                    crate::Shape::Curve(curve) => {
                        curva_kernel::Body::planar(std::slice::from_ref(curve))
                            .ok()?
                            .area()
                            .ok()
                            .map(Value::Number)
                    }
                    _ => None,
                }
            }
            "Volume" => Some(Value::Number(
                self.object(args)?.shape.body()?.volume().ok()?,
            )),
            "UserText" => {
                let key = match args.get(1) {
                    Some(Value::Text(k)) => k,
                    _ => return None,
                };
                text(self.object(args)?.user_text.get(key)?)
            }
            "ObjectName" => text(&self.object(args)?.name),
            "ObjectLayer" => {
                let layer = self.object(args)?.layer;
                text(&self.state.layers.iter().find(|l| l.id == layer)?.name)
            }
            "LayerName" => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let id = args.first()?.number()? as u64;
                text(&self.state.layers.iter().find(|l| l.id == id)?.name)
            }
            "ObjectPageName" => {
                let space = self.object(args)?.space?;
                let (index, layout) = self
                    .state
                    .layouts
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.id == space)?;
                if layout.name.is_empty() {
                    Some(Value::Text(format!("{}", index + 1)))
                } else {
                    text(&layout.name)
                }
            }
            "ObjectPageNumber" => {
                let space = self.object(args)?.space?;
                let index = self.state.layouts.iter().position(|l| l.id == space)?;
                count(index + 1)
            }
            "DetailScale" => {
                let frame = args.first()?.number()?;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let frame = frame as u64;
                let scale = self
                    .state
                    .layouts
                    .iter()
                    .flat_map(|l| &l.details)
                    .find(|d| d.frame == frame)?
                    .scale?;
                let format = match args.get(1) {
                    Some(Value::Text(f)) => f.as_str(),
                    _ => "",
                };
                Some(Value::Text(scale_in_format(scale, format)))
            }
            "BlockName" => {
                let object = self.object(args)?;
                let crate::Shape::Instance(instance) = &object.shape else {
                    return None;
                };
                text(
                    &self
                        .state
                        .blocks
                        .iter()
                        .find(|b| b.id == instance.definition)?
                        .name,
                )
            }
            "BlockInstanceCount" => {
                let wanted = match args.first() {
                    Some(Value::Text(name)) => name,
                    _ => return None,
                };
                let block = self.state.blocks.iter().find(|b| &b.name == wanted)?;
                count(
                    self.state
                        .objects
                        .iter()
                        .filter(|o| matches!(&o.shape, crate::Shape::Instance(i) if i.definition == block.id))
                        .count(),
                )
            }
            _ => None,
        }
    }
}

/// A detail scale in one of Rhino's formats: none (`0.1`), `#:1` (`0.1:1`), `1:#` (`1:10`),
/// `1=#` (paper inch = model feet, `1" = 10'`), `#=1` (`1/8" = 1'`), `#=1-0` (`1/8" = 1'-0"`).
fn scale_in_format(scale: f64, format: &str) -> String {
    let tidy = |v: f64| {
        let text = format!("{v:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    match format {
        "#:1" => format!("{}:1", tidy(scale)),
        "1:#" => format!("1:{}", tidy(1.0 / scale)),
        "1=#" => format!("1\" = {}'", tidy(1.0 / scale / 12.0)),
        "#=1" | "#=1-0" => {
            let inches = scale * 12.0;
            let fraction = inch_fraction(inches);
            if format == "#=1" {
                format!("{fraction}\" = 1'")
            } else {
                format!("{fraction}\" = 1'-0\"")
            }
        }
        _ => tidy(scale),
    }
}

/// Inches as a fraction to the nearest sixty-fourth: `1/8`, `3/16`, `1 1/2`.
fn inch_fraction(inches: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sixty_fourths = (inches * 64.0).round().max(0.0) as u64;
    let (whole, mut top) = (sixty_fourths / 64, sixty_fourths % 64);
    let mut bottom = 64;
    while top > 0 && top % 2 == 0 {
        top /= 2;
        bottom /= 2;
    }
    match (whole, top) {
        (w, 0) => w.to_string(),
        (0, t) => format!("{t}/{bottom}"),
        (w, t) => format!("{w} {t}/{bottom}"),
    }
}

/// A date in a .NET date format, as Rhino's `Date` field takes it: `yyyy`, `yy`, `MMMM`, `MMM`,
/// `MM`, `M`, `dddd`, `ddd`, `dd`, `d`, `HH`, `H`, `hh`, `h`, `mm`, `ss`, `tt`; text in single
/// quotes as it is. Names are English.
#[must_use]
pub fn net_date(when: &jiff::Zoned, format: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let month = MONTHS[usize::try_from(when.month() - 1).unwrap_or(0)];
    let day = DAYS[usize::try_from(when.weekday().to_monday_zero_offset()).unwrap_or(0)];
    let hour = when.hour();
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    let chars: Vec<char> = format.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let piece = match (c, run) {
            ('y', 1 | 2) => format!("{:02}", when.year() % 100),
            ('y', _) => format!("{:04}", when.year()),
            ('M', 1) => when.month().to_string(),
            ('M', 2) => format!("{:02}", when.month()),
            ('M', 3) => month[..3].to_owned(),
            ('M', _) => month.to_owned(),
            ('d', 1) => when.day().to_string(),
            ('d', 2) => format!("{:02}", when.day()),
            ('d', 3) => day[..3].to_owned(),
            ('d', _) => day.to_owned(),
            ('H', 1) => hour.to_string(),
            ('H', _) => format!("{hour:02}"),
            ('h', 1) => twelve.to_string(),
            ('h', _) => format!("{twelve:02}"),
            ('m', 1) => when.minute().to_string(),
            ('m', _) => format!("{:02}", when.minute()),
            ('s', 1) => when.second().to_string(),
            ('s', _) => format!("{:02}", when.second()),
            ('t', _) => if hour < 12 { "AM" } else { "PM" }.to_owned(),
            _ => std::iter::repeat_n(c, run).collect(),
        };
        out.push_str(&piece);
        i += run;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use curva_kernel::Curve;

    fn context() -> FieldContext {
        // 17 December 2020, 4:48 pm, in UTC, which needs no zone database.
        let when = jiff::Timestamp::from_second(1_608_223_680)
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::UTC);
        FieldContext {
            file: Some(std::path::PathBuf::from("/projects/tower.curva")),
            saved: Some(when.clone()),
            now: Some(when),
        }
    }

    #[test]
    fn dates_take_rhinos_formats() {
        let when = context().now.unwrap();
        assert_eq!(net_date(&when, "M/d/yyyy"), "12/17/2020");
        assert_eq!(net_date(&when, "M/d/yyyy h:mm tt"), "12/17/2020 4:48 PM");
        assert_eq!(
            net_date(&when, "dddd, MMMM dd, yyyy"),
            "Thursday, December 17, 2020"
        );
    }

    #[test]
    fn fields_read_the_model_its_sheets_and_its_file() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap());
        let paper = crate::layouts::paper_size("A3").unwrap();
        let sheet = doc.add_layout("Plans".to_owned(), paper, true);
        doc.add_layout(String::new(), paper, true);
        let state = doc.state();
        let show = |text: &str, space| resolve(text, state, space, &context());
        assert_eq!(
            show("Sheet %<PageName()>% of %<NumPages()>%", Some(sheet)),
            "Sheet Plans of 2"
        );
        assert_eq!(show("%<PageNumber()>%", Some(sheet)), "1");
        assert_eq!(show("%<PageNumber()>%", None), "0");
        assert_eq!(show("%<PaperName()>%", Some(sheet)), "A3");
        assert_eq!(show(&format!("%<CurveLength(\"{line}\")>%"), None), "5");
        assert_eq!(show(&format!("%<CurveLength(\"{line}\")*2>%"), None), "10");
        assert_eq!(show("%<NumPages()-1>%", None), "1");
        assert_eq!(show("%<FileName(\"3\")>%", None), "tower");
        assert_eq!(show("%<ModelUnits()>%", None), "Millimeters");
        assert_eq!(show("%<Date(\"yyyy\")>%", None), "2020");
        assert_eq!(
            show("%<Nonsense()>% and %<PageName()>%", None),
            "#### and ####"
        );
        assert_eq!(show("no fields", None), "no fields");
    }

    #[test]
    fn detail_scales_show_in_rhinos_formats() {
        assert_eq!(scale_in_format(0.1, ""), "0.1");
        assert_eq!(scale_in_format(0.1, "#:1"), "0.1:1");
        assert_eq!(scale_in_format(0.1, "1:#"), "1:10");
        assert_eq!(scale_in_format(1.0 / 120.0, "1=#"), "1\" = 10'");
        assert_eq!(scale_in_format(1.0 / 96.0, "#=1"), "1/8\" = 1'");
        assert_eq!(scale_in_format(1.0 / 96.0, "#=1-0"), "1/8\" = 1'-0\"");
    }
}
