//! Harness for the kernel evaluation (`docs/specs/m0-kernel-evaluation.md`).
//!
//! Reads the neutral case files, runs each on a [`Kernel`] and records the outcome, measurements,
//! time taken and picture. It lives outside the product code and depends only on the standard
//! library, so adding a kernel never affects Curva itself.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs, panic, thread};

/// One test operation, read from a case file.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Number of the operation in the spec.
    pub id: u32,
    /// Description, as in the spec.
    pub name: String,
    /// Section of the spec the operation belongs to.
    pub group: String,
    /// Kernel-neutral name of the operation.
    pub operation: String,
    /// The Rhino command or commands that give the reference result.
    pub rhino: String,
    /// Inputs, as `label kind parameters`.
    pub inputs: Vec<String>,
    /// Control points of inputs that need them, as `label points`.
    pub input_points: Vec<String>,
    /// Options, as `key=value`.
    pub options: Vec<String>,
    /// Earlier cases whose results this case starts from.
    pub uses: Vec<u32>,
    /// What to measure on the result.
    pub measures: Vec<String>,
}

/// Why a case file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseError(pub String);

impl std::fmt::Display for CaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CaseError {}

/// Reads the text of one case file.
pub fn parse_case(text: &str) -> Result<Case, CaseError> {
    let mut id = None;
    let mut case = Case {
        id: 0,
        name: String::new(),
        group: String::new(),
        operation: String::new(),
        rhino: String::new(),
        inputs: Vec::new(),
        input_points: Vec::new(),
        options: Vec::new(),
        uses: Vec::new(),
        measures: Vec::new(),
    };
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line_number = index + 1;
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| CaseError(format!("line {line_number}: expected `key: value`")))?;
        let value = value.trim().to_owned();
        let number = |value: &str| {
            value
                .parse::<u32>()
                .map_err(|_| CaseError(format!("line {line_number}: not a number")))
        };
        match key.trim() {
            "id" => id = Some(number(&value)?),
            "name" => case.name = value,
            "group" => case.group = value,
            "operation" => case.operation = value,
            "rhino" => case.rhino = value,
            "input" => case.inputs.push(value),
            "input-points" => case.input_points.push(value),
            "option" => case.options.push(value),
            "uses" => case.uses.push(number(&value)?),
            "measure" => case.measures.push(value),
            other => {
                return Err(CaseError(format!(
                    "line {line_number}: unknown key `{other}`"
                )));
            }
        }
    }
    case.id = id.ok_or_else(|| CaseError("missing `id`".to_owned()))?;
    let required = [
        ("name", &case.name),
        ("group", &case.group),
        ("operation", &case.operation),
    ];
    for (key, value) in required.into_iter().chain([("rhino", &case.rhino)]) {
        if value.is_empty() {
            return Err(CaseError(format!("case {}: missing `{key}`", case.id)));
        }
    }
    if case.inputs.is_empty() && case.uses.is_empty() {
        return Err(CaseError(format!(
            "case {}: no inputs and no earlier results",
            case.id
        )));
    }
    if case.measures.is_empty() {
        return Err(CaseError(format!("case {}: nothing to measure", case.id)));
    }
    Ok(case)
}

/// Reads every `.case` file in a folder, sorted by id, and checks the set is consistent.
pub fn load_cases(folder: &Path) -> Result<Vec<Case>, CaseError> {
    let entries = fs::read_dir(folder).map_err(|error| CaseError(error.to_string()))?;
    let mut cases = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| CaseError(error.to_string()))?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "case")
        {
            let text = fs::read_to_string(&path).map_err(|error| CaseError(error.to_string()))?;
            cases.push(
                parse_case(&text)
                    .map_err(|error| CaseError(format!("{}: {error}", path.display())))?,
            );
        }
    }
    cases.sort_by_key(|case| case.id);
    let mut seen = BTreeSet::new();
    for case in &cases {
        if !seen.insert(case.id) {
            return Err(CaseError(format!("case {} appears twice", case.id)));
        }
        // A case may only start from an earlier one, so results exist when it runs.
        if let Some(used) = case
            .uses
            .iter()
            .find(|used| **used >= case.id || !seen.contains(used))
        {
            return Err(CaseError(format!(
                "case {} uses unknown case {used}",
                case.id
            )));
        }
    }
    Ok(cases)
}

/// What a kernel did with a case, using the spec's four outcomes (section 4).
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// It produced a result.
    Works,
    /// It produced a result, but a wrong one. Set by comparison with Rhino.
    Wrong(String),
    /// It said clearly that it cannot do this.
    Refuses(String),
    /// It crashed.
    Crashes(String),
    /// It did not finish in the time allowed.
    Hangs,
}

impl Outcome {
    /// The word used for this outcome in reports.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Works => "works",
            Self::Wrong(_) => "wrong",
            Self::Refuses(_) => "refuses",
            Self::Crashes(_) => "crashes",
            Self::Hangs => "hangs",
        }
    }

    fn detail(&self) -> &str {
        match self {
            Self::Wrong(detail) | Self::Refuses(detail) | Self::Crashes(detail) => detail,
            Self::Works | Self::Hangs => "",
        }
    }
}

/// What a kernel returns for a case it attempted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attempt {
    /// Measurements, as name and value.
    pub measurements: Vec<(String, String)>,
    /// Picture of the result, if one was made.
    pub image: Option<PathBuf>,
}

/// A geometry kernel under test.
pub trait Kernel: Send + Sync {
    /// The kernel's name, for reports.
    fn name(&self) -> &str;

    /// Runs one case. `Err` means the kernel refuses, with its reason.
    fn run(&self, case: &Case) -> Result<Attempt, String>;
}

/// The recorded result of one case on one kernel.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Which case.
    pub case_id: u32,
    /// What happened.
    pub outcome: Outcome,
    /// What the kernel measured.
    pub attempt: Attempt,
    /// How long it took.
    pub time: Duration,
}

/// Runs one case, turning a crash or a hang into a recorded outcome instead of ending the run.
pub fn run_case(kernel: std::sync::Arc<dyn Kernel>, case: Case, limit: Duration) -> Record {
    let case_id = case.id;
    let (sender, receiver) = mpsc::channel();
    let start = Instant::now();
    // Runs on its own thread because a hung kernel can only be abandoned, not interrupted.
    thread::spawn(move || {
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| kernel.run(&case)));
        let _ = sender.send(result);
    });
    let (outcome, attempt) = match receiver.recv_timeout(limit) {
        Ok(Ok(Ok(attempt))) => (Outcome::Works, attempt),
        Ok(Ok(Err(reason))) => (Outcome::Refuses(reason), Attempt::default()),
        Ok(Err(payload)) => (
            Outcome::Crashes(panic_text(payload.as_ref())),
            Attempt::default(),
        ),
        Err(mpsc::RecvTimeoutError::Timeout) => (Outcome::Hangs, Attempt::default()),
        Err(mpsc::RecvTimeoutError::Disconnected) => (
            Outcome::Crashes("the run ended early".to_owned()),
            Attempt::default(),
        ),
    };
    Record {
        case_id,
        outcome,
        attempt,
        time: start.elapsed(),
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "crash without a message".to_owned())
}

/// Writes results as tab-separated text, one case per line.
pub fn report(kernel: &str, records: &[Record]) -> String {
    let mut text = String::from("kernel\tcase\toutcome\tseconds\tmeasurements\timage\tdetail\n");
    for record in records {
        let measurements: Vec<String> = record
            .attempt
            .measurements
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let image = record
            .attempt
            .image
            .as_ref()
            .map(|path| path.display().to_string());
        // Writing to a String cannot fail.
        let _ = writeln!(
            text,
            "{kernel}\t{}\t{}\t{:.3}\t{}\t{}\t{}",
            record.case_id,
            record.outcome.label(),
            record.time.as_secs_f64(),
            measurements.join(";"),
            image.unwrap_or_default(),
            record.outcome.detail().replace(['\t', '\n'], " "),
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const SAMPLE: &str = "id: 3\nname: N\ngroup: G\noperation: O\nrhino: R\n\
        # a comment\ninput: a line (0,0,0) (1,0,0)\noption: d=2\nmeasure: length\n";

    struct Fake(fn(&Case) -> Result<Attempt, String>);

    impl Kernel for Fake {
        fn name(&self) -> &str {
            "fake"
        }
        fn run(&self, case: &Case) -> Result<Attempt, String> {
            (self.0)(case)
        }
    }

    fn run(behaviour: fn(&Case) -> Result<Attempt, String>, limit: Duration) -> Record {
        run_case(
            Arc::new(Fake(behaviour)),
            parse_case(SAMPLE).expect("sample is valid"),
            limit,
        )
    }

    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn reads_a_case() {
        let case = parse_case(SAMPLE).expect("sample is valid");
        assert_eq!(case.id, 3);
        assert_eq!(case.inputs, ["a line (0,0,0) (1,0,0)"]);
        assert_eq!(case.options, ["d=2"]);
        assert_eq!(case.measures, ["length"]);
    }

    #[test]
    fn rejects_unknown_keys_and_missing_parts() {
        assert!(parse_case(&format!("{SAMPLE}colour: red\n")).is_err());
        assert!(parse_case("id: 1\nname: N\n").is_err());
        assert!(parse_case(&SAMPLE.replace("measure: length\n", "")).is_err());
    }

    #[test]
    fn all_31_cases_are_valid_and_numbered_in_order() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cases");
        let cases = load_cases(&folder).expect("cases load");
        let ids: Vec<u32> = cases.iter().map(|case| case.id).collect();
        assert_eq!(ids, (1..=31).collect::<Vec<u32>>());
    }

    #[test]
    fn records_success_with_measurements() {
        let record = run(
            |_| {
                Ok(Attempt {
                    measurements: vec![("volume".to_owned(), "8".to_owned())],
                    image: None,
                })
            },
            LONG,
        );
        assert_eq!(record.outcome, Outcome::Works);
        assert!(report("fake", &[record]).contains("fake\t3\tworks\t"));
    }

    #[test]
    fn records_a_refusal() {
        let record = run(|_| Err("not supported".to_owned()), LONG);
        assert_eq!(record.outcome, Outcome::Refuses("not supported".to_owned()));
    }

    #[test]
    fn a_crash_is_recorded_and_does_not_end_the_run() {
        let record = run(|_| panic!("boom"), LONG);
        assert_eq!(record.outcome, Outcome::Crashes("boom".to_owned()));
    }

    #[test]
    fn a_hang_is_recorded() {
        let record = run(
            |_| {
                thread::sleep(Duration::from_secs(5));
                Ok(Attempt::default())
            },
            Duration::from_millis(50),
        );
        assert_eq!(record.outcome, Outcome::Hangs);
    }
}
