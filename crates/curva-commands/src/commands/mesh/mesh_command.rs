//! Rhino's `Mesh` command: surfaces, polysurfaces, solids and SubDs as new mesh objects, the
//! originals kept, with the shared mesh settings (spec `export-formats.md` §2) as its options.
//! The mesh panel shows the same settings; until it is hooked up the options are typed.

use super::meshing::mesh_objects;
use crate::Message;
use crate::commands::arguments::{number, objects};
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::{DEFAULT_SUBD_LEVEL, MOST_SUBD_LEVEL, MeshSettings};

/// `Mesh`: meshes of the chosen surfaces, solids and SubDs, each on its source's layer.
#[derive(Default)]
pub struct MeshCommand;

impl Recipe for MeshCommand {
    const NAME: &'static str = "mesh";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("MaxAngle", 0.0),
        OptionDef::distance("MaxDistance", 0.0, true),
        OptionDef::whole(
            "SubDLevel",
            DEFAULT_SUBD_LEVEL as u32,
            1,
            MOST_SUBD_LEVEL as u32,
        ),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        // As Rhino's `-Mesh`: the objects, then the density, Enter keeping the last one; the
        // preview shows the mesh while it is asked.
        vec![
            StepDef::objects("prompt.mesh.objects"),
            StepDef::number("prompt.mesh.density").remembered("Density", 0.5),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let limit = |name| Some(cx.number(name)).filter(|v| *v > 0.0);
        let settings = MeshSettings {
            density: number(cx, 1)
                .or_else(|| cx.doc.remembered("mesh.density"))
                .unwrap_or(0.5),
            max_angle: limit("MaxAngle"),
            max_distance: limit("MaxDistance"),
            subd_level: cx.number("SubDLevel") as u8,
        }
        .checked();
        let ids: Vec<u64> = objects(cx, 0)
            .iter()
            .copied()
            .filter(|id| {
                cx.doc
                    .state()
                    .objects
                    .iter()
                    .any(|o| o.id == *id && matches!(o.shape, Shape::Body(_) | Shape::SubD(_)))
            })
            .collect();
        let (meshes, numbers) = mesh_objects(cx.doc, &ids, &settings);
        if meshes.is_empty() {
            return Err(Message::new("error.mesh_nothing"));
        }
        // History keeps only the density; meshes made with the advanced limits do not follow.
        let follows = settings.max_angle.is_none() && settings.max_distance.is_none();
        let mut made = Made::default();
        for (from, _, mesh) in meshes {
            let is_body = cx
                .doc
                .state()
                .objects
                .iter()
                .any(|o| o.id == from && o.shape.body().is_some());
            made.derived.push(Derived {
                shape: Shape::Mesh(mesh),
                from,
                history: (follows && is_body).then_some(curva_document::Recipe::MeshOf {
                    source: from,
                    density: settings.density,
                }),
            });
        }
        made.message = Some(Message::with(
            "done.mesh",
            vec![made.derived.len().to_string(), numbers.faces.to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::Body;

    fn ball() -> Document {
        let mut doc = Document::default();
        doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        doc
    }

    fn faces(doc: &Document) -> usize {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| o.shape.mesh())
            .map(curva_mesh::Mesh::face_count)
            .sum()
    }

    #[test]
    fn a_solid_gets_a_mesh_beside_it() {
        let mut doc = ball();
        run(&mut doc, &["selall", "mesh", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        assert!(objects[0].shape.body().is_some());
        let mesh = objects[1].shape.mesh().unwrap();
        let volume = 4.0 / 3.0 * std::f64::consts::PI * 125.0;
        assert!((mesh.volume().unwrap() - volume).abs() / volume < 0.02);
    }

    #[test]
    fn density_gives_more_faces_and_is_remembered() {
        let mut low = ball();
        run(&mut low, &["selall", "mesh", "0.1"]);
        let mut high = ball();
        run(&mut high, &["selall", "mesh", "0.9"]);
        assert!(faces(&low) < faces(&high));
        assert_eq!(low.remembered("mesh.density"), Some(0.1));
        assert_eq!(
            super::super::meshing::remembered_settings(&low).density,
            0.1
        );
    }

    #[test]
    fn max_angle_and_max_distance_tighten_the_mesh() {
        let mut plain = ball();
        run(&mut plain, &["selall", "mesh", ""]);
        let mut angle = ball();
        run(&mut angle, &["selall", "mesh", "MaxAngle", "3", ""]);
        let mut distance = ball();
        run(
            &mut distance,
            &["selall", "mesh", "MaxDistance", "0.0005", ""],
        );
        assert!(faces(&plain) < faces(&angle));
        assert!(faces(&plain) < faces(&distance));
    }

    #[test]
    fn a_subd_is_meshed_at_its_level() {
        let cube = || curva_mesh::SubD::subd_box(&curva_mesh::Frame::world(), [10.0; 3], [1, 1, 1]);
        let mut low = Document::default();
        low.add_shape(Shape::SubD(cube()));
        run(&mut low, &["selall", "mesh", "SubDLevel", "1", ""]);
        let mut high = Document::default();
        high.add_shape(Shape::SubD(cube()));
        run(&mut high, &["selall", "mesh", "SubDLevel", "3", ""]);
        assert_eq!(faces(&low), 24);
        assert_eq!(faces(&high), 24 * 16);
    }

    #[test]
    fn the_preview_shows_the_mesh_and_follows_a_typed_density() {
        let mut doc = ball();
        let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&ids);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "mesh");
        let coarse = processor.preview(&doc, None, "0.1").len();
        let fine = processor.preview(&doc, None, "0.9").len();
        assert!(coarse > 0 && coarse < fine, "{coarse} {fine}");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn curves_alone_give_nothing_to_mesh() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "1,0", "selall", "mesh", ""]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
