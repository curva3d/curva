//! Curva's views and clipping planes as Rhino's.

#[allow(clippy::wildcard_imports)]
use super::*;

fn unit3(a: P3) -> P3 {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
    a.map(|v| v / length)
}

/// A Curva saved view as a file's: the camera Curva's viewport would have, with a frustum that
/// shows as much at the target.
pub(super) fn view_of(v: &curva_document::SavedView) -> model::View {
    let (forward, up, eye) = match v.axes {
        Some([forward, _, up]) => (
            forward,
            up,
            std::array::from_fn(|k| v.target[k] - forward[k] * v.distance),
        ),
        None => {
            let (cp, sp) = (v.pitch.cos(), v.pitch.sin());
            let eye: P3 = [
                v.target[0] + v.distance * cp * v.yaw.cos(),
                v.target[1] + v.distance * cp * v.yaw.sin(),
                v.target[2] + v.distance * sp,
            ];
            let forward = unit3(std::array::from_fn(|k| v.target[k] - eye[k]));
            let right = unit3(cross(forward, [0.0, 0.0, 1.0]));
            (forward, cross(right, forward), eye)
        }
    };
    let (near, far) = (0.01 * v.distance, 100.0 * v.distance);
    let half = crate::import::HALF_FIELD.tan() * if v.parallel { v.distance } else { near };
    model::View {
        name: v.name.clone(),
        location: eye,
        direction: forward,
        up,
        target: v.target,
        parallel: v.parallel,
        frustum: [-half, half, -half, half, near, far],
    }
}

/// A clipping plane's rectangle as Rhino's clipping plane: its plane at the rectangle's middle,
/// x along its first side and the normal towards the side it keeps (Rhino keeps the side the
/// normal points to), its extents, and the views it cuts by viewport id (Curva's viewports in
/// Rhino's layout: Top, Perspective, Front, Right; details by their own ids).
pub(super) fn clipping_plane(
    o: &Object,
    clip: &curva_document::clipping::ClippingPlane,
    model_views: &[(String, Uuid)],
    details: &BTreeMap<u64, Uuid>,
) -> Result<Geometry, String> {
    use curva_document::clipping::ClipView;
    let curve = o.curve().ok_or("a clipping plane without its rectangle")?;
    let mut points = curve.sample().map_err(|e| e.0)?;
    if points.len() > 3 && points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 {
        return Err("a clipping plane without its rectangle".to_owned());
    }
    // The rectangle's normal, by Newell's method as Curva's clipping takes it.
    let mut normal = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let z = unit(if clip.flipped {
        normal.map(|v| -v)
    } else {
        normal
    });
    #[allow(clippy::cast_precision_loss)]
    let count = points.len() as f64;
    let origin: P3 = std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / count);
    let side: P3 = std::array::from_fn(|k| points[1][k] - points[0][k]);
    let along = (0..3).map(|k| side[k] * z[k]).sum::<f64>();
    let x = unit(std::array::from_fn(|k| side[k] - along * z[k]));
    let y = cross(z, x);
    let local = |p: &P3, axis: &P3| (0..3).map(|k| (p[k] - origin[k]) * axis[k]).sum::<f64>();
    let range = |axis: &P3| {
        points.iter().fold([f64::MAX, f64::MIN], |[lo, hi], p| {
            let t = local(p, axis);
            [lo.min(t), hi.max(t)]
        })
    };
    let viewports = clip
        .views
        .iter()
        .filter_map(|view| match view {
            ClipView::Viewport(slot) => ["Top", "Perspective", "Front", "Right"]
                .get(*slot)
                .and_then(|name| model_views.iter().find(|(n, _)| n == name))
                .map(|(_, id)| *id),
            ClipView::Detail(frame) => details.get(frame).copied(),
        })
        .collect();
    Ok(Geometry::ClippingPlane(model::ClippingPlane {
        plane: Plane { origin, x, y, z },
        extents: [range(&x), range(&y)],
        viewports,
        id: Uuid::fresh(o.id),
        enabled: true,
    }))
}

fn unit(a: P3) -> P3 {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
    a.map(|v| v / length)
}
