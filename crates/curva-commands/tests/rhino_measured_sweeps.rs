//! Sweep1 and Sweep2 against Rhino 8 itself (the testing stream, 25 November 2026). Each case was
//! built in Rhino 8 from the same curves and swept with its own command line (`_-Sweep1` or
//! `_-Sweep2` by the Rhino MCP, every option at its default), then measured with RhinoCommon: the
//! surface's area, extent, and whether it is closed. Curva sweeps the same curves here and must
//! make the same surface.
//!
//! Only the cases Curva already matches are here. Those it does not (several sections, a section
//! away from the rail's start, a rail with corners, two sections on two rails) are issues #665,
//! #666 and #667, each with Rhino's numbers; add them here as they are fixed. A case that fails is
//! a difference from Rhino: fix the command, never the numbers.

use curva_commands::Processor;
use curva_document::Document;
use curva_kernel::Curve;

/// What Rhino 8 measured: the area, the extent's lowest and highest corners, and whether the
/// surface is a closed solid.
struct Rhino {
    area: f64,
    min: [f64; 3],
    max: [f64; 3],
    solid: bool,
}

/// Rhino's own S-shaped rail: `InterpCrv` through (0,0,0), (5,3,0), (10,−3,0), (15,0,0), as the
/// exact cubic it made (length 20.3627).
fn s_rail() -> Curve {
    Curve::nurbs(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0],
        vec![
            [0.0, 0.0, 0.0],
            [1.295_607_260_655_442_5, 1.448_854_583_426_051_6, 0.0],
            [5.222_635_643_606_733, 6.330_687_249_944_37, 0.0],
            [9.777_364_356_393_269, -6.330_687_249_944_371, 0.0],
            [13.704_392_739_344_557, -1.448_854_583_426_051_1, 0.0],
            [15.0, 0.0, 0.0],
        ],
        vec![],
    )
    .unwrap()
}

/// Runs a sweep command with each step's curves picked in turn and returns the one object it
/// made, measured.
fn swept(command: &str, steps: &[Vec<Curve>]) -> (f64, [f64; 3], [f64; 3], bool) {
    let mut doc = Document::default();
    let ids: Vec<Vec<u64>> = steps
        .iter()
        .map(|curves| curves.iter().map(|c| doc.add(c.clone())).collect())
        .collect();
    let before = doc.state().objects.len();
    let mut processor = Processor::default();
    processor.submit(&mut doc, command);
    for chosen in &ids {
        doc.select(chosen);
        processor.submit(&mut doc, "");
    }
    assert!(!processor.is_running(), "{command} is still asking");
    let objects = &doc.state().objects;
    assert_eq!(objects.len(), before + 1, "{command} made one object");
    let body = objects[before].shape.body().expect("a surface");
    let (min, max) = body.bounds().unwrap();
    (body.area().unwrap(), min, max, body.is_solid())
}

fn check(name: &str, curva: (f64, [f64; 3], [f64; 3], bool), rhino: &Rhino) {
    let (area, min, max, solid) = curva;
    assert!(
        (area - rhino.area).abs() <= 1e-3 * rhino.area,
        "{name}: area {area}, Rhino 8 {}",
        rhino.area
    );
    for i in 0..3 {
        assert!(
            (min[i] - rhino.min[i]).abs() <= 1e-2 && (max[i] - rhino.max[i]).abs() <= 1e-2,
            "{name}: extent {min:?}–{max:?}, Rhino 8 {:?}–{:?}",
            rhino.min,
            rhino.max
        );
    }
    assert_eq!(solid, rhino.solid, "{name}: closed");
}

#[test]
fn sweep1_one_circle_square_to_the_rail_at_its_start() {
    // Rhino's circle of radius 1 on the plane square to the rail's start tangent.
    let t: [f64; 3] = [1.295_607_260_655_442_5, 1.448_854_583_426_051_6, 0.0];
    let length = t[0].hypot(t[1]);
    let x = [-t[1] / length, t[0] / length, 0.0];
    let circle = Curve::circle_on([0.0; 3], 1.0, x, [0.0, 0.0, 1.0]).unwrap();
    let rhino = Rhino {
        area: 127.9428,
        min: [-0.745, -4.19, -1.0],
        max: [15.745, 4.19, 1.0],
        solid: false,
    };
    check(
        "Sweep1, circle square to the rail",
        swept("sweep1", &[vec![s_rail()], vec![circle]]),
        &rhino,
    );
}

#[test]
fn sweep1_one_circle_drawn_in_the_right_view_at_the_rail_start() {
    // The circle lies in world YZ, not square to the rail: Rhino keeps its angle to the rail.
    let circle = Curve::circle_on([0.0; 3], 1.0, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rhino = Rhino {
        area: 108.8123,
        min: [0.0, -3.956, -1.0],
        max: [15.0, 3.955, 1.0],
        solid: false,
    };
    check(
        "Sweep1, circle in world YZ",
        swept("sweep1", &[vec![s_rail()], vec![circle]]),
        &rhino,
    );
}

#[test]
fn sweep1_along_a_closed_rail_makes_a_closed_torus() {
    let rail = Curve::circle([0.0; 3], 10.0).unwrap();
    let circle = Curve::circle_on([10.0, 0.0, 0.0], 1.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rhino = Rhino {
        area: 394.783,
        min: [-11.0, -11.0, -1.0],
        max: [11.0, 11.0, 1.0],
        solid: true,
    };
    check(
        "Sweep1, closed rail",
        swept("sweep1", &[vec![rail], vec![circle]]),
        &rhino,
    );
}

#[test]
fn sweep2_an_arc_between_two_diverging_lines_is_scaled_up() {
    // Rhino's default MaintainHeight=No scales the arc as the rails part: 2 high at 5 apart,
    // 3.2 high at 8 apart.
    let a = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
    let b = Curve::line([0.0, 5.0, 0.0], [10.0, 8.0, 0.0]).unwrap();
    // The arc from (0,0,0) over (0,2.5,2) to (0,5,0): centre (0,2.5,−0.5625).
    let arc = Curve::arc(
        [0.0, 2.5, -0.5625],
        [0.0, 5.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let rhino = Rhino {
        area: 91.0738,
        min: [0.0, 0.0, 0.0],
        max: [10.0, 8.0, 3.2],
        solid: false,
    };
    check(
        "Sweep2, diverging lines",
        swept("sweep2", &[vec![a], vec![b], vec![arc]]),
        &rhino,
    );
}
