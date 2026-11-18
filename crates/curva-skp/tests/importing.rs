//! SketchUp files read into Curva: a model written by OpenSKP's own writer (so no binary file is
//! kept here) with a tag, a component placed twice, a group and loose faces, read back.

use curva_document::{Shape, Unit};
use openskp::create::{
    DefinitionOptions, FaceOptions, GroupOptions, InstanceOptions, LayerOptions,
};

/// A 10-inch square on the ground at `x`, `y`.
fn square(x: f64, y: f64) -> Vec<[f64; 3]> {
    vec![
        [x, y, 0.0],
        [x + 10.0, y, 0.0],
        [x + 10.0, y + 10.0, 0.0],
        [x, y + 10.0, 0.0],
    ]
}

fn sample() -> Vec<u8> {
    let mut skp = openskp::create::create();
    let walls = skp
        .add_layer(
            "Walls",
            &LayerOptions {
                color: Some([200, 100, 50, 255]),
                ..LayerOptions::default()
            },
        )
        .unwrap();
    // A component: one square, placed twice.
    let chair = {
        let mut definition = skp
            .add_component_definition("Chair", &DefinitionOptions::default())
            .unwrap();
        definition
            .add_face(&square(0.0, 0.0), &FaceOptions::default())
            .unwrap();
        definition.close().unwrap()
    };
    // A group of two squares on the Walls tag, moved 50 inches up in y.
    {
        let mut group = skp
            .add_group(&GroupOptions {
                translation: [0.0, 50.0, 0.0],
                layer: Some(walls),
                ..GroupOptions::default()
            })
            .unwrap();
        group
            .add_face(&square(0.0, 0.0), &FaceOptions::default())
            .unwrap();
        group
            .add_face(&square(20.0, 0.0), &FaceOptions::default())
            .unwrap();
        group.close().unwrap();
    }
    // The component's two placements (definitions and groups come first in a SketchUp file).
    for x in [100.0, 200.0] {
        skp.add_instance(
            chair,
            &InstanceOptions {
                translation: [x, 0.0, 0.0],
                ..InstanceOptions::default()
            },
        )
        .unwrap();
    }
    // A loose square, untagged.
    skp.add_face(&square(-30.0, 0.0), &FaceOptions::default())
        .unwrap();
    skp.to_bytes().unwrap()
}

#[test]
fn components_groups_tags_and_faces_come_in_where_they_were() {
    let model = curva_skp::read(&sample()).unwrap();
    let mut doc = curva_document::Document::default();
    doc.set_unit(Unit::Millimetres, false);
    let report = curva_skp::import(&model, &mut doc);
    let state = doc.state();
    // The component is one block, placed twice, its square 10 inches wide in millimetres.
    assert_eq!(state.blocks.len(), 1, "{}", report.summary());
    assert_eq!(state.blocks[0].name, "Chair");
    let instances: Vec<_> = state
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Instance(i) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(instances.len(), 2);
    let moves: Vec<f64> = instances.iter().map(|i| i.placement[3][0]).collect();
    assert_eq!(moves, [2540.0, 5080.0]);
    let in_block = state.blocks[0].objects[0].shape.mesh().unwrap();
    let (low, high) = in_block.bounds().unwrap();
    assert!((high[0] - low[0] - 254.0).abs() < 1e-9);
    // The group's two squares, of one tag and material, are one mesh, placed 50 inches up and
    // reaching from 0 to 30 inches across.
    let group_mesh = state
        .objects
        .iter()
        .filter_map(|o| o.shape.mesh())
        .filter_map(|m| m.bounds())
        .find(|(low, _)| (low[1] - 1270.0).abs() < 1e-9);
    let (low, high) = group_mesh.unwrap_or_else(|| panic!("{}", report.summary()));
    assert!(
        low[0].abs() < 1e-9 && (high[0] - 762.0).abs() < 1e-9,
        "{low:?} {high:?}"
    );
    // The group's mesh is on the Walls layer, coloured as the tag; the loose square on the
    // default layer, 30 inches to the left.
    let walls = state.layers.iter().find(|l| l.name == "Walls").unwrap();
    assert!((walls.colour[0] - 200.0 / 255.0).abs() < 1e-6);
    let on_walls: Vec<_> = state
        .objects
        .iter()
        .filter(|o| o.layer == walls.id)
        .collect();
    assert_eq!(on_walls.len(), 1, "{}", report.summary());
    let (low, _) = on_walls[0].shape.mesh().unwrap().bounds().unwrap();
    assert!((low[1] - 1270.0).abs() < 1e-9);
    let loose = state
        .objects
        .iter()
        .filter_map(|o| {
            o.shape
                .mesh()
                .and_then(curva_mesh::Mesh::bounds)
                .map(|b| (o.layer, b))
        })
        .find(|(_, (low, _))| (low[0] + 762.0).abs() < 1e-9)
        .unwrap();
    assert_eq!(loose.0, state.current_layer);
}
