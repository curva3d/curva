//! Writing Rhino files: what Curva writes reads back the same (spec `docs/specs/3dm-files.md` §4).

use curva_3dm::model::{Geometry, Model, Surface};

fn read(name: &str) -> Model {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap()
}

/// Whether the writer takes this surface as it is (the export turns the others into NURBS).
fn written_as_is(s: &Surface) -> bool {
    matches!(s, Surface::Nurbs(_) | Surface::Plane { .. })
}

/// The objects the writer takes as they are: no extrusion objects, no revolved or summed surfaces.
fn writable(model: &Model) -> Model {
    let mut out = model.clone();
    out.objects.retain(|o| match &o.geometry {
        Geometry::Extrusion(_) => false,
        Geometry::Surface(s) => written_as_is(s),
        Geometry::Brep(b) => b.surfaces.iter().flatten().all(written_as_is),
        _ => true,
    });
    out
}

fn assert_round_trip(name: &str) {
    let before = writable(&read(name));
    let bytes = curva_3dm::write::write(&before);
    let after = curva_3dm::read::read(&bytes)
        .unwrap_or_else(|e| panic!("{name}: Curva's own file does not read back: {e}"));
    assert_eq!(after.version, 70, "{name}");
    assert!(after.skipped.is_empty(), "{name}: {:?}", after.skipped);
    assert_eq!(after.unit, before.unit, "{name}");
    assert_eq!(after.tolerance, before.tolerance, "{name}");
    assert_eq!(after.layers, before.layers, "{name}");
    assert_eq!(after.groups, before.groups, "{name}");
    assert_eq!(after.blocks, before.blocks, "{name}");
    assert_eq!(after.objects.len(), before.objects.len(), "{name}");
    for (a, b) in after.objects.iter().zip(&before.objects) {
        assert_eq!(a, b, "{name}");
    }
}

#[test]
fn every_example_file_comes_back_the_same() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert_round_trip(&name);
    }
}

#[test]
fn the_file_ends_with_its_own_size() {
    let bytes = curva_3dm::write::write(&read("v7_my_curves.3dm"));
    let size = i64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap());
    assert_eq!(size, bytes.len() as i64);
}
