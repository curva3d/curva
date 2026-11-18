//! Bodies written as a STEP file (ISO 10303-21) by solidcore's own writer, `step-export`, so
//! surfaces and solids stay exact (spec `docs/specs/export-formats.md` §1, §4). STEP files are
//! always in millimetres.

use crate::{Body, KernelError, Similarity};
use solidcore::kernel::Kernel;
use step_export::{ExportMetadata, Schema};

/// Which STEP application protocol to write (spec §3). AP214 is Curva's default: it carries
/// colours and layers; Rhino's default is AP203.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepSchema {
    /// AP203, configuration controlled design.
    Ap203,
    /// AP214, automotive design.
    #[default]
    Ap214,
    /// AP242, managed model based 3D engineering.
    Ap242,
}

/// One body to write, with the name it carries in the file.
#[derive(Clone, Copy, Debug)]
pub struct StepPart<'a> {
    /// The surface, polysurface or solid.
    pub body: &'a Body,
    /// Its name, when it has one.
    pub name: Option<&'a str>,
}

/// What was written: the file's text, and the parts left out with the reason.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepFile {
    /// The STEP file's text.
    pub text: String,
    /// How many parts were written.
    pub written: usize,
    /// The parts left out (their position in the list given) and why: solidcore refuses geometry
    /// it cannot write exactly rather than approximating it.
    pub left_out: Vec<(usize, String)>,
}

/// The parts written as one STEP file. `millimetres` is how many millimetres one model unit is
/// (1 for a model in millimetres), as STEP files are in millimetres. `file_name` goes in the
/// file's header.
///
/// # Errors
/// When no part could be written, or the writer refuses the whole file.
pub fn write_step(
    parts: &[StepPart<'_>],
    schema: StepSchema,
    millimetres: f64,
    file_name: Option<&str>,
) -> Result<StepFile, KernelError> {
    let to_mm = (millimetres != 1.0).then(|| Similarity::scaling([0.0; 3], millimetres));
    let mut model: Option<solidcore_record::step::Model> = None;
    let mut file = StepFile::default();
    for (n, part) in parts.iter().enumerate() {
        let scaled;
        let body = match &to_mm {
            Some(change) => {
                scaled = part.body.mapped(change);
                &scaled
            }
            None => part.body,
        };
        let mut one = match model_of(body) {
            Ok(one) => one,
            Err(reason) => {
                file.left_out.push((n, reason));
                continue;
            }
        };
        if let Some(name) = part.name {
            for b in &mut one.bodies {
                b.name = Some(name.to_owned());
            }
        }
        file.written += 1;
        match &mut model {
            Some(all) => all.bodies.append(&mut one.bodies),
            None => model = Some(one),
        }
    }
    let Some(model) = model else {
        let why = file
            .left_out
            .first()
            .map_or_else(|| "nothing to write".to_owned(), |(_, w)| w.clone());
        return Err(KernelError(why));
    };
    let schema = match schema {
        StepSchema::Ap203 => Schema::Ap203,
        StepSchema::Ap214 => Schema::Ap214,
        StepSchema::Ap242 => Schema::Ap242,
    };
    let mut metadata = ExportMetadata::new().with_schema(schema);
    if let Some(name) = file_name {
        metadata = metadata.with_file_name(name);
    }
    file.text = step_export::write_with_metadata(&model, &metadata)
        .map_err(|e| KernelError(e.to_string()))?;
    Ok(file)
}

/// One body as the writer's model, through the engine's own B-rep.
fn model_of(body: &Body) -> Result<solidcore_record::step::Model, String> {
    let record = body.record().map_err(|e| e.0)?;
    let mut kernel = Kernel::default();
    let brep = solidcore_convert::brep::to_kernel_body(&mut kernel, &record, true)
        .map_err(|e| e.to_string())?;
    step_export::brep_to_model(&brep).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_body(origin: crate::Point, size: f64) -> Body {
        Body::cuboid(origin, [size; 3]).unwrap()
    }

    #[test]
    fn a_box_and_a_sphere_are_written_exactly() {
        let cube = box_body([0.0; 3], 10.0);
        let ball = Body::sphere([30.0, 0.0, 0.0], 5.0).unwrap();
        let parts = [
            StepPart {
                body: &cube,
                name: Some("Cube"),
            },
            StepPart {
                body: &ball,
                name: None,
            },
        ];
        let file = write_step(&parts, StepSchema::default(), 1.0, Some("test.stp")).unwrap();
        assert_eq!((file.written, file.left_out.len()), (2, 0));
        assert!(file.text.starts_with("ISO-10303-21;"));
        assert!(file.text.contains("AUTOMOTIVE_DESIGN"));
        assert!(file.text.contains("MANIFOLD_SOLID_BREP"));
        assert!(file.text.contains("'Cube'"));
        // Read back by the same library: two solids.
        let back =
            step_export::import(file.text.as_bytes(), &step_export::Limits::default()).unwrap();
        assert_eq!(back.bodies.len(), 2);
    }

    #[test]
    fn a_model_in_metres_is_written_in_millimetres() {
        let cube = box_body([0.0; 3], 2.0);
        let parts = [StepPart {
            body: &cube,
            name: None,
        }];
        let file = write_step(&parts, StepSchema::Ap203, 1000.0, None).unwrap();
        assert!(file.text.contains("CONFIG_CONTROL_DESIGN"));
        assert!(file.text.contains("SI_UNIT(.MILLI.,.METRE.)"));
        assert!(
            file.text.contains("2000."),
            "a 2 m side is 2000 mm in the file"
        );
    }
}
