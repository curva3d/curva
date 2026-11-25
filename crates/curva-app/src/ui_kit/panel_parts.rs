//! The parts of a panel's own chrome: the title chip, title tabs, the strip a hidden panel
//! leaves at its edge, and the focus tab. The panel code decides what they do; these draw them.

use super::{CONTROL_RADIUS, MARK, TITLE_ROW, colours};
use eframe::egui;

/// A title chip on the middle of the top edge of `rect`, half above it, wide enough for a name
/// `name_width` wide with `side` room either side for buttons. Returns the row inside it.
pub(crate) fn title_chip(
    ui: &egui::Ui,
    rect: egui::Rect,
    name_width: f32,
    side: f32,
) -> egui::Rect {
    let chip = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.min.y),
        egui::vec2(name_width + 2.0 * side, TITLE_ROW),
    );
    ui.painter().rect(
        chip,
        egui::CornerRadius::same((TITLE_ROW / 2.0) as u8),
        colours::panel(),
        egui::Stroke::new(super::LINE, colours::edge()),
        egui::StrokeKind::Inside,
    );
    chip.shrink2(egui::vec2(super::PAD, 0.0))
}

/// A panel's title or one of its tabs: `galley` at `tab`, middle of `row` height, marked as
/// chosen when it is the one `showing`, else quiet.
pub(crate) fn title_tab(
    ui: &egui::Ui,
    row: egui::Rect,
    tab: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
    showing: bool,
) {
    if showing {
        super::chosen_mark(ui.painter(), tab.expand2(egui::vec2(6.0, -1.0)));
    }
    let colour = if showing {
        colours::text()
    } else {
        colours::quiet()
    };
    ui.painter().galley(
        egui::pos2(tab.min.x, row.center().y - galley.size().y / 2.0),
        galley,
        colour,
    );
}

/// A panel's tab shown as its icon at `tab` (drawn by `draw` in the given rectangle and colour),
/// marked as chosen when it is the one `showing`, else quiet, where the names do not fit.
pub(crate) fn title_icon_tab(
    ui: &egui::Ui,
    tab: egui::Rect,
    showing: bool,
    draw: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32),
) {
    if showing {
        super::chosen_mark(ui.painter(), tab.expand2(egui::vec2(4.0, -1.0)));
    }
    let colour = if showing {
        colours::text()
    } else {
        colours::quiet()
    };
    draw(ui.painter(), tab.shrink(2.0), colour);
}

/// The strip a hidden panel leaves along its edge, at `rect`, with its pieces' `names`, running
/// upwards when `upright`.
pub(crate) fn edge_strip(
    ui: &egui::Ui,
    rect: egui::Rect,
    hovered: bool,
    names: String,
    upright: bool,
) {
    let fill = if hovered {
        colours::edge()
    } else {
        colours::panel()
    };
    ui.painter().rect_filled(rect, CONTROL_RADIUS, fill);
    let galley = ui
        .painter()
        .layout_no_wrap(names, super::small_font(), colours::quiet());
    let size = galley.size();
    let (position, angle) = if upright {
        (
            egui::pos2(
                rect.center().x - size.y / 2.0,
                rect.center().y + size.x / 2.0,
            ),
            -std::f32::consts::FRAC_PI_2,
        )
    } else {
        (rect.center() - size / 2.0, 0.0)
    };
    ui.painter()
        .add(egui::epaint::TextShape::new(position, galley, colours::quiet()).with_angle(angle));
}

/// The tab hanging from the top of the window at `rect` that hides and brings back the panels:
/// a chevron pointing down when `down`, else up.
pub(crate) fn hanging_tab(ui: &egui::Ui, rect: egui::Rect, hovered: bool, down: bool) {
    let fill = if hovered {
        colours::edge()
    } else {
        colours::panel()
    };
    let round = CONTROL_RADIUS + 1;
    let radius = egui::CornerRadius {
        nw: 0,
        ne: 0,
        sw: round,
        se: round,
    };
    ui.painter().rect_filled(rect, radius, fill);
    let towards = if down { super::DOWN } else { super::UP };
    super::chevron(
        ui.painter(),
        rect.center(),
        towards,
        colours::mark(MARK, false),
    );
}

/// A filled tile `size` big with a drawing above its name, marked as chosen when `chosen`: one
/// choice among several shown as pictures, as the view picker's views. `picture` draws in the
/// rectangle it is given, in the colour it is given.
pub(crate) fn picture_tile(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    chosen: bool,
    name: &str,
    picture: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let ink = colours::text();
    let painter = ui.painter();
    if chosen {
        super::chosen_mark(painter, rect);
    } else {
        let fill = if response.hovered() {
            colours::hover()
        } else {
            colours::control()
        };
        painter.rect_filled(rect, CONTROL_RADIUS, fill);
    }
    let room = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.min.y + 15.0),
        egui::vec2(32.0, 20.0),
    );
    picture(painter, room, ink);
    let at = egui::pos2(rect.center().x, rect.max.y - 4.0);
    painter.text(
        at,
        egui::Align2::CENTER_BOTTOM,
        name,
        super::caption_font(),
        ink,
    );
    response
}

/// A thin accent line along the top of `along`, filled from `from` to `to` (fractions of its
/// width), for a job's progress.
pub(crate) fn progress_line(ui: &egui::Ui, along: egui::Rect, from: f32, to: f32) {
    let line = egui::Rect::from_min_size(along.left_top(), egui::vec2(along.width(), 3.0));
    let x = |f: f32| line.left() + line.width() * f.clamp(0.0, 1.0);
    let filled = egui::Rect::from_x_y_ranges(x(from)..=x(to), line.y_range());
    ui.painter().rect_filled(filled, 0.0, colours::accent());
}

/// A round chip with a spinner and `label`, its top right corner at `corner`, washed and
/// outlined in the accent: something is busy, and a click on it says more.
pub(crate) fn busy_chip(ui: &egui::Ui, corner: egui::Pos2, label: String) -> egui::Response {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label, super::small_font(), colours::text());
    let size = egui::vec2(galley.size().x + 34.0, galley.size().y + 8.0);
    let chip = egui::Rect::from_min_size(corner - egui::vec2(size.x, 0.0), size);
    let round = size.y / 2.0;
    painter.rect_filled(chip, round, colours::chosen_fill());
    painter.rect_stroke(
        chip,
        round,
        colours::accent_line(super::LINE),
        egui::StrokeKind::Inside,
    );
    let spinner = egui::Rect::from_center_size(
        egui::pos2(chip.left() + 14.0, chip.center().y),
        egui::vec2(10.0, 10.0),
    );
    egui::Spinner::new().size(10.0).paint_at(ui, spinner);
    let at = egui::pos2(chip.left() + 24.0, chip.center().y - galley.size().y / 2.0);
    painter.galley(at, galley, colours::text());
    ui.interact(chip, ui.scope_id().with("busy chip"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small picture of a sheet `picture` big, centred in a square `side` across, outlined in the
/// accent when it is the one `open`.
pub(crate) fn sheet_thumbnail(ui: &mut egui::Ui, side: f32, picture: egui::Vec2, open: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
    let sheet = egui::Rect::from_center_size(rect.center(), picture);
    ui.painter().rect_filled(sheet, 1.0, colours::sheet());
    let edge = if open {
        colours::accent_line(super::UNDERLINE)
    } else {
        egui::Stroke::new(super::LINE, colours::edge())
    };
    ui.painter()
        .rect_stroke(sheet, 1.0, edge, egui::StrokeKind::Outside);
}
