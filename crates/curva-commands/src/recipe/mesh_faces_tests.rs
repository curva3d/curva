//! A step that takes faces of meshes only (`StepDef::mesh_faces`), as Rhino's "Select mesh
//! faces" (#819).

use super::*;

/// Faces of meshes, then nothing; it makes nothing.
#[derive(Default)]
struct Faces;

impl Recipe for Faces {
    const NAME: &'static str = "facestest";
    const OPTIONS: &'static [OptionDef] = &[];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_faces("prompt.extractmeshfaces.faces")]
    }

    fn make(&self, _cx: &Context<'_>) -> Result<Made, Message> {
        Ok(Made::default())
    }
}

#[test]
fn a_mesh_faces_step_asks_for_mesh_parts_taking_faces_only_as_rhino() {
    let mut doc = Document::default();
    let mut command = RecipeCommand::<Faces>::default();
    let Step::Ask(request) = command.step(&mut doc, None) else {
        panic!("expected a question");
    };
    assert_eq!(request.kind, Kind::MeshParts);
    assert!(request.faces_only);
    let plain = StepDef::mesh_parts("prompt.extractmeshfaces.faces");
    assert!(!plain.faces_only);
}
