//! Blocks to and from DXF and DWG (spec `drawing-output.md` §3): a definition becomes a block
//! record holding its objects, and an instance an INSERT with a point, scales, a turn and a
//! normal. A placement that INSERT cannot hold (one that shears) is left out and reported.

use crate::{P, cross, dot, from_object, object_axes, p3, to_object, unit, v3};
use acadrust::CadDocument;
use acadrust::entities::{Block, EntityType, Insert};
use acadrust::tables::BlockRecord;
use acadrust::types::Vector3;
use curva_document::blocks::{Instance, Placement};

/// Writes a block record named `name` holding `contents`, its base point at the origin, as
/// AutoCAD keeps block definitions.
pub(crate) fn write_block(
    cad: &mut CadDocument,
    name: &str,
    contents: Vec<EntityType>,
) -> Result<(), String> {
    let mut record = BlockRecord::new(name);
    record.handle = cad.allocate_handle();
    let owner = record.handle;
    cad.block_records.add(record).map_err(|e| e.to_string())?;
    let mut block = Block::new(name, Vector3::ZERO);
    block.common.owner_handle = owner;
    let handle = cad
        .add_entity(EntityType::Block(block))
        .map_err(|e| e.to_string())?;
    if let Some(record) = cad.block_records.get_mut(name) {
        record.block_entity_handle = handle;
    }
    for mut entity in contents {
        entity.common_mut().owner_handle = owner;
        cad.add_entity(entity).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn length(v: P) -> f64 {
    dot(v, v).sqrt()
}

/// An instance as an INSERT of the block `name`, if its placement keeps right angles.
pub(crate) fn insert_out(instance: &Instance, name: &str) -> Option<Insert> {
    let m = instance.placement;
    let row = |i: usize| -> P { [m[i][0], m[i][1], m[i][2]] };
    let (x, y, z) = (row(0), row(1), row(2));
    let (sx, sy, sz) = (length(x), length(y), length(z));
    if sx < 1e-12 || sy < 1e-12 || sz < 1e-12 {
        return None;
    }
    let square = |a: P, b: P, la: f64, lb: f64| dot(a, b).abs() <= 1e-9 * la * lb;
    if !(square(x, y, sx, sy) && square(y, z, sy, sz) && square(x, z, sx, sz)) {
        return None;
    }
    let normal = unit(z)?;
    // A mirrored placement is a negative X scale, the X axis turned half round to match.
    let mirrored = dot(cross(x, y), z) < 0.0;
    let along = if mirrored { x.map(|c| -c) } else { x };
    let (ox, oy) = object_axes(normal);
    let rotation = dot(along, oy).atan2(dot(along, ox));
    let at = to_object([m[3][0], m[3][1], m[3][2]], normal);
    let mut insert = Insert::new(name, v3(at)).with_scale(if mirrored { -sx } else { sx }, sy, sz);
    insert.rotation = rotation;
    insert.normal = v3(normal);
    Some(insert)
}

/// The placement an INSERT puts its block at.
pub(crate) fn placement_in(insert: &Insert) -> Placement {
    let normal = unit(p3(&insert.normal)).unwrap_or([0.0, 0.0, 1.0]);
    let (ox, oy) = object_axes(normal);
    let (s, c) = insert.rotation.sin_cos();
    let along: P = std::array::from_fn(|i| ox[i] * c + oy[i] * s);
    let up = cross(normal, along);
    let (sx, sy, sz) = (insert.x_scale(), insert.y_scale(), insert.z_scale());
    let at = from_object(p3(&insert.insert_point), normal);
    [
        [along[0] * sx, along[1] * sx, along[2] * sx, 0.0],
        [up[0] * sy, up[1] * sy, up[2] * sy, 0.0],
        [normal[0] * sz, normal[1] * sz, normal[2] * sz, 0.0],
        [at[0], at[1], at[2], 1.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::blocks::PLACED_AS_IS;

    fn same(a: &Placement, b: &Placement) -> bool {
        (0..4).all(|i| (0..4).all(|j| (a[i][j] - b[i][j]).abs() < 1e-9))
    }

    #[test]
    fn placements_go_out_and_come_back() {
        let (s, c) = 0.7_f64.sin_cos();
        let turned_scaled = [
            [2.0 * c, 2.0 * s, 0.0, 0.0],
            [-3.0 * s, 3.0 * c, 0.0, 0.0],
            [0.0, 0.0, 1.5, 0.0],
            [10.0, 20.0, 5.0, 1.0],
        ];
        let mirrored = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [4.0, 0.0, 0.0, 1.0],
        ];
        let standing = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        ];
        for placement in [PLACED_AS_IS, turned_scaled, mirrored, standing] {
            let instance = Instance {
                definition: 0,
                placement,
            };
            let insert = insert_out(&instance, "B").unwrap();
            assert!(same(&placement_in(&insert), &placement), "{placement:?}");
        }
    }

    #[test]
    fn blocks_and_their_instances_go_out_and_come_back() {
        use crate::{Format, export, import, read};
        use curva_document::{Document, Shape};
        use curva_kernel::Curve;
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let circle = doc.add(Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap());
        let (definition, first) =
            doc.define_block("Door/leaf".to_owned(), [0.0; 3], &[line, circle]);
        let (s, c) = 0.5_f64.sin_cos();
        let turned = [
            [c, s, 0.0, 0.0],
            [-s, c, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [100.0, 0.0, 0.0, 1.0],
        ];
        doc.insert_block(definition, turned);
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            assert_eq!(report.brought, vec![("block instances".to_owned(), 2)]);
            let mut back = Document::default();
            let report = import(&read(&bytes, format).unwrap(), &mut back);
            assert_eq!(
                report.brought,
                vec![("block instances".to_owned(), 2)],
                "{format:?}"
            );
            let definitions = back.definitions();
            assert_eq!(definitions.len(), 1, "{format:?}: one definition");
            assert_eq!(definitions[0].name, "Door_leaf");
            assert_eq!(definitions[0].objects.len(), 2);
            let placements: Vec<Placement> = back
                .state()
                .objects
                .iter()
                .filter_map(|o| match &o.shape {
                    Shape::Instance(i) => Some(i.placement),
                    _ => None,
                })
                .collect();
            assert!(same(&placements[1], &turned), "{format:?}: {placements:?}");
        }
        let _ = first;
    }

    #[test]
    fn a_sheared_placement_cannot_be_an_insert() {
        let mut sheared = PLACED_AS_IS;
        sheared[1][0] = 0.5;
        let instance = Instance {
            definition: 0,
            placement: sheared,
        };
        assert!(insert_out(&instance, "B").is_none());
    }
}
