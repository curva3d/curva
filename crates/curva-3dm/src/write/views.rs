//! Views, construction planes, details, clipping planes, pages and document text as Rhino reads them.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn write_plane(w: &mut Writer, p: &Plane) {
    w.f64s(&p.origin);
    w.f64s(&p.x);
    w.f64s(&p.y);
    w.f64s(&p.z);
    w.f64s(&p.equation());
}

/// A construction plane as openNURBS writes one, with Rhino's default grid.
pub(super) fn write_cplane(w: &mut Writer, plane: &Plane, name: &str) {
    w.chunk_version(1, 1);
    write_plane(w, plane);
    w.f64(1.0); // grid spacing
    w.f64(1.0); // snap spacing
    w.i32(70); // grid lines
    w.i32(5); // every fifth line thick
    w.wstring(name);
    w.bool(false); // not drawn into the depth buffer
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: P3) -> P3 {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
    a.map(|v| v / length)
}

/// What a view's attributes say: its kind (0 a model view, 1 a page, 2 a view nested in a
/// page), its paper size in millimetres, its display mode's id (nil for Rhino's default) and
/// whether its projection is locked.
#[derive(Clone, Copy)]
pub(super) struct ViewKind {
    kind: i32,
    size: [f64; 2],
    display: Uuid,
    locked: bool,
}

impl ViewKind {
    fn plain(kind: i32, size: [f64; 2]) -> Self {
        Self {
            kind,
            size,
            display: Uuid::NIL,
            locked: false,
        }
    }
}

/// A named view: its viewport (camera and frustum), the construction plane its kind of view
/// has, its target and its name.
/// A view: its camera, construction plane, target and name, and for a layout or a detail its
/// kind (1 a page, 2 a view nested in a page) and paper size in millimetres.
pub(super) fn write_view(
    w: &mut Writer,
    v: &View,
    id: Uuid,
    kind: Option<ViewKind>,
    user_text: &[(String, String)],
) {
    let direction = unit(v.direction);
    let x = unit(cross(direction, v.up));
    let y = cross(x, direction);
    w.begin(tcode::VIEW_VIEWPORT);
    w.chunk_version(1, 5);
    w.i32(1); // camera valid
    w.i32(1); // frustum valid
    w.i32(1); // port valid
    w.i32(if v.parallel { 1 } else { 2 });
    w.f64s(&v.location);
    w.f64s(&direction);
    w.f64s(&y);
    w.f64s(&x);
    w.f64s(&y);
    w.f64s(&direction.map(|c| -c));
    w.f64s(&v.frustum);
    // A square port, as the frustum is: left, right, bottom, top (pixels run down), near, far.
    for p in [0, 1000, 1000, 0, 0, 255] {
        w.i32(p);
    }
    w.uuid(id);
    for locked in [false, false, false, true, true] {
        w.bool(locked);
    }
    w.f64s(&v.target);
    w.bool(true); // the camera frame is valid
    w.f64s(&[1.0, 1.0, 1.0]);
    w.end();
    // A layout's user text, in its viewport's user data.
    if !user_text.is_empty() {
        w.begin(tcode::VIEW_VIEWPORT_USERDATA);
        write_user_text(w, user_text);
        w.short(tcode::OPENNURBS_CLASS_END, 0);
        w.end();
    }
    // A flat view looking along an axis draws on that axis' plane, as Rhino's standard views do.
    let along = |axis: usize, sign: f64| v.parallel && direction[axis] * sign > 1.0 - 1e-9;
    let (px, py) = if along(1, 1.0) {
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
    } else if along(0, -1.0) {
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
    } else {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    };
    let plane = Plane {
        origin: [0.0; 3],
        x: px,
        y: py,
        z: cross(px, py),
    };
    w.begin(tcode::VIEW_CPLANE);
    write_cplane(w, &plane, "");
    w.end();
    w.begin(tcode::VIEW_TARGET);
    w.f64s(&v.target);
    w.end();
    w.begin(tcode::VIEW_NAME);
    w.wstring(&v.name);
    w.end();
    if let Some(kind) = kind {
        // Version 1.3 of the view's attributes: kind, paper size, an unused parent id and
        // nested position, the display mode, page settings (version 1.0: page number, size,
        // margins, printer), and whether the projection is locked.
        w.begin(tcode::VIEW_ATTRIBUTES);
        w.chunk_version(1, 3);
        w.i32(kind.kind);
        w.f64s(&kind.size);
        w.uuid(Uuid::NIL);
        w.f64s(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
        w.uuid(kind.display);
        w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
        w.i32(0);
        w.f64s(&[kind.size[0], kind.size[1], 0.0, 0.0, 0.0, 0.0]);
        w.wstring("");
        w.end();
        w.bool(kind.locked);
        w.end();
    }
    w.short(tcode::ENDOFTABLE, 0);
}

/// A detail view (`ON_DetailView::Write`, version 1.1): its camera as a view nested in its
/// layout, its outline, and its scale.
pub(super) fn write_detail(w: &mut Writer, d: &Detail) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 1);
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    write_view(
        w,
        &d.view,
        d.id,
        Some(ViewKind {
            display: d.display,
            locked: d.locked,
            ..ViewKind::plain(2, [0.0, 0.0])
        }),
        &[],
    );
    w.end();
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    write_nurbs_curve(w, &d.boundary);
    w.end();
    w.f64(d.ratio);
    w.end();
}

/// A clipping plane as openNURBS writes one (`ON_ClippingPlaneSurface::Write`): its plane
/// surface in a plain chunk, then the clipping plane (`ON_ClippingPlane::Write`, version 1.4):
/// the first viewport, its id, the plane, whether it is on, all its viewports (an
/// `ON_UuidList`, version 1.0: count and ids), and no clipping depth.
pub(super) fn write_clipping_plane(w: &mut Writer, c: &ClippingPlane) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 1);
    write_plane(w, &c.plane);
    w.f64s(&c.extents[0]);
    w.f64s(&c.extents[1]);
    w.f64s(&c.extents[0]);
    w.f64s(&c.extents[1]);
    w.end();
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 4);
    w.uuid(c.viewports.first().copied().unwrap_or(Uuid::NIL));
    w.uuid(c.id);
    write_plane(w, &c.plane);
    w.bool(c.enabled);
    let mut sorted = c.viewports.clone();
    sorted.sort();
    sorted.dedup();
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.count(sorted.len());
    for id in &sorted {
        w.uuid(*id);
    }
    w.end();
    w.f64(-1.0); // depth: none
    w.bool(false); // depth off
    w.end();
    w.end();
}

/// The settings' view list: Rhino's four standard model views, then a page view for each
/// layout, at its paper size, looking down on it.
pub(super) fn write_view_list(w: &mut Writer, model: &Model) {
    let span = 1000.0;
    let standard = |name: &str, direction: P3, up: P3, parallel: bool| View {
        name: name.to_owned(),
        location: direction.map(|c| -c * span * 10.0),
        direction,
        up,
        target: [0.0; 3],
        parallel,
        frustum: if parallel {
            [-span, span, -span, span, 1.0, span * 100.0]
        } else {
            [-0.36, 0.36, -0.36, 0.36, 1.0, span * 100.0]
        },
    };
    let s = 1.0 / 3f64.sqrt();
    let views = [
        standard("Top", [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], true),
        standard("Front", [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], true),
        standard("Right", [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], true),
        standard("Perspective", [-s, s, -s], [-s, s, 2.0 * s], false),
    ];
    w.begin(tcode::SETTINGS_VIEW_LIST);
    w.count(views.len() + model.pages.len());
    for (i, v) in views.iter().enumerate() {
        w.begin(tcode::VIEW_RECORD);
        write_view(
            w,
            v,
            model
                .model_views
                .iter()
                .find(|(name, _)| *name == v.name)
                .map_or_else(|| Uuid::fresh(0x5354_4456 + i as u64), |(_, id)| *id),
            Some(ViewKind::plain(0, [0.0, 0.0])),
            &[],
        );
        w.end();
    }
    for page in &model.pages {
        let [width, height] = page.size_mm;
        let centre = [width / 2.0, height / 2.0, 0.0];
        let view = View {
            name: page.name.clone(),
            location: [centre[0], centre[1], 100.0],
            direction: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            target: centre,
            parallel: true,
            frustum: [
                -width / 2.0,
                width / 2.0,
                -height / 2.0,
                height / 2.0,
                1.0,
                200.0,
            ],
        };
        w.begin(tcode::VIEW_RECORD);
        write_view(
            w,
            &view,
            page.id,
            Some(ViewKind::plain(1, page.size_mm)),
            &page.user_text,
        );
        w.end();
    }
    w.end();
}

/// The settings' attributes, to version 1.1: the usual linetype scale, plot colour and current
/// linetype, then the unit layouts are drawn in.
pub(super) fn write_page_unit(w: &mut Writer, model: &Model) {
    let unit = model.page_unit.unwrap_or(model.unit);
    w.begin(tcode::SETTINGS_ATTRIBUTES);
    w.chunk_version(1, 1);
    w.f64(1.0); // linetype display scale
    w.color([0, 0, 0, 0]); // plot colour
    w.i32(0); // plot colour from the layer
    w.i32(-1); // no current linetype
    w.i32(0); // linetype from the layer
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.i32(102);
    w.u32(unit);
    w.f64(0.001);
    w.f64(1f64.to_radians());
    w.f64(0.01);
    w.i32(0);
    w.i32(3);
    w.f64(metres_per(unit));
    w.wstring("");
    w.end();
    w.end();
}

/// The document user text as openNURBS keeps it (`ONX_Model`'s step 17): a user table for the
/// plug-in `ON_DocumentUserStringList`, whose record is that object, with the keys and values as
/// its user data.
pub(super) fn write_document_text(w: &mut Writer, pairs: &[(String, String)]) {
    w.begin(tcode::USER_TABLE);
    w.begin(tcode::USER_TABLE_UUID);
    w.uuid(class::DOCUMENT_USER_STRINGS);
    w.begin_versioned(tcode::USER_TABLE_RECORD_HEADER, 1, 0);
    w.bool(false); // not kept for a plug-in that is missing
    w.i32(70);
    w.u32(OPENNURBS_VERSION as u32);
    w.end();
    w.end();
    w.begin(tcode::USER_RECORD);
    w.begin(tcode::OPENNURBS_CLASS);
    w.begin(tcode::OPENNURBS_CLASS_UUID);
    w.uuid(class::DOCUMENT_USER_STRINGS);
    w.end();
    w.begin(tcode::OPENNURBS_CLASS_DATA);
    w.u8(1);
    w.end();
    write_user_text(w, pairs);
    w.short(tcode::OPENNURBS_CLASS_END, 0);
    w.end();
    w.end();
    w.short(tcode::ENDOFTABLE, 0);
    w.end();
}
