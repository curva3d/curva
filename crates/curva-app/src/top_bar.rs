//! The top bar: the command tabs and their buttons, the specialization dial and the File items.

use crate::*;

/// The top bar's chosen command tab and specialization.
#[derive(Default)]
pub(crate) struct TopBar {
    /// The command tab showing, as an index into [`COMMAND_TABS`].
    pub(crate) command_tab: usize,
    /// The specialization selected in the top bar's dial, as an index into [`SPECIALIZATIONS`].
    pub(crate) specialization: usize,
    /// Curva's mark, made from the window icon the first time the bar is drawn.
    pub(crate) mark: Option<egui::TextureHandle>,
}

/// `path` shortened in the middle until `fits` accepts it: the first part and as many of the
/// last parts as fit, with `…` between (spec `feedback-0.4.2.md` §4).
pub(crate) fn shortened_path(path: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(path) {
        return path.to_owned();
    }
    let separator = if path.contains('\\') { '\\' } else { '/' };
    let parts: Vec<&str> = path.split(separator).collect();
    let Some((first, rest)) = parts.split_first() else {
        return path.to_owned();
    };
    let gap = format!("{separator}…{separator}");
    // Keep the most of the end that fits, always keeping at least the file's own name.
    for kept in (1..rest.len()).rev() {
        let tail = rest[rest.len() - kept..].join(&separator.to_string());
        let candidate = format!("{first}{gap}{tail}");
        if fits(&candidate) {
            return candidate;
        }
    }
    let name = rest.last().copied().unwrap_or(first);
    let short = format!("…{separator}{name}");
    if fits(&short) { short } else { name.to_owned() }
}

/// The command tabs at the top, each with its most used commands (spec
/// `docs/specs/ui-floating-panels.md`).
pub(crate) const COMMAND_TABS: [(&str, &[&str]); 6] = [
    (
        "ui.tab.curves",
        &[
            "Line",
            "Polyline",
            "Curve",
            "InterpCrv",
            "Circle",
            "Arc",
            "Ellipse",
            "Rectangle",
            "Polygon",
            "Slot",
            "Helix",
            "Spiral",
            "Parabola",
            "Conic",
            "TextObject",
            "Hyperbola",
            "HandleCurve",
            "Sketch",
            "Point",
            "Points",
            "Lines",
            "Offset",
            "Fillet",
            "Chamfer",
            "BlendCrv",
            "OffsetMultiple",
            "Extend",
            "Match",
            "Fair",
            "SimplifyCrv",
            "CurveBoolean",
            "MakeUniform",
            "Reparameterize",
            "Weight",
        ],
    ),
    (
        "ui.tab.surfaces",
        &[
            "PlanarSrf",
            "Plane",
            "SrfPt",
            "ExtrudeCrv",
            "Revolve",
            "Loft",
            "Sweep1",
            "OffsetSrf",
            "ExtrudeSrf",
            "ExtrudeCrvTapered",
            "ExtrudeCrvAlongCrv",
            "ExtrudeCrvToPoint",
            "RailRevolve",
            "DupEdge",
            "DupBorder",
            "ExtractSrf",
            "EdgeSrf",
            "NetworkSrf",
            "Patch",
            "ExtendSrf",
            "Untrim",
            "RebuildUV",
            "Project",
            "Pull",
            "DupFaceBorder",
            "Sweep2",
            "BlendSrf",
            "MatchSrf",
            "SplitSrf",
            "ShrinkTrimmedSrf",
            "ExtractIsocurve",
            "InterpCrvOnSrf",
            "OffsetCrvOnSrf",
            "UnrollSrf",
            "FilletSrf",
            "ChamferSrf",
        ],
    ),
    (
        "ui.tab.solids",
        &[
            "Box",
            "Sphere",
            "Cylinder",
            "Cone",
            "TruncatedCone",
            "Torus",
            "Tube",
            "Ellipsoid",
            "Paraboloid",
            "Pyramid",
            "Pipe",
            "FilletEdge",
            "ChamferEdge",
            "Shell",
            "MergeAllFaces",
            "Wirecut",
            "MoveFace",
            "PushPull",
            "Cap",
            "ExtrudeFace",
            "Volume",
        ],
    ),
    (
        "ui.tab.modify",
        &[
            "BooleanUnion",
            "BooleanDifference",
            "BooleanIntersection",
            "BooleanSplit",
            "Trim",
            "Split",
            "Join",
            "Explode",
            "Move",
            "Copy",
            "Rotate",
            "Scale",
            "Mirror",
            "Array",
            "ArrayLinear",
            "Align",
            "Distribute",
            "Shear",
            "Twist",
            "Bend",
            "Taper",
            "Flow",
            "CageEdit",
            "Group",
            "Ungroup",
            "EditBoolean",
            "BreakBoolean",
            "Block",
            "Insert",
            "BlockManager",
            "BlockEdit",
            "ExplodeBlock",
            "ChangeLayer",
            "MatchProperties",
            "Linetype",
            "PrintWidth",
            "SetUserText",
            "GetUserText",
            "Purge",
            "LockSwap",
            "BringToFront",
            "SendToBack",
            "Lock",
            "Unlock",
        ],
    ),
    (
        "ui.tab.analyze",
        &[
            "Distance",
            "Length",
            "Area",
            "Volume",
            "Angle",
            "Radius",
            "EvaluatePt",
            "What",
            "BoundingBox",
            "AreaCentroid",
            "VolumeCentroid",
            "ShowEdges",
            "CurvatureGraph",
            "Section",
            "Contour",
            "Intersect",
            "Silhouette",
            "Zebra",
            "DraftAnalysis",
            "Count",
            "Audit",
            "Plan",
        ],
    ),
    (
        "ui.tab.select",
        &[
            "SelAll",
            "SelNone",
            "Invert",
            "SelCrv",
            "SelLine",
            "SelPolyline",
            "SelCircle",
            "SelArc",
            "SelClosedCrv",
            "SelOpenCrv",
            "SelSrf",
            "SelSolid",
            "SelPt",
            "SelColor",
            "Hide",
            "Show",
            "HideSwap",
        ],
    ),
];

/// The specializations in the top bar's dial; only Freeform has commands yet.
pub(crate) const SPECIALIZATIONS: [&str; 4] = [
    "ui.specialization.freeform",
    "ui.specialization.architecture",
    "ui.specialization.publisher",
    "ui.specialization.analysis",
];

/// The width of the specializations dial in the top bar, in points.
pub(crate) const DIAL_WIDTH: f32 = 440.0;

/// The height of the top bar, in points.
pub(crate) const TOP_BAR_HEIGHT: f32 = 36.0;

/// The size of a command button with an icon, in points.
pub(crate) const COMMAND_BUTTON: f32 = 30.0;

impl Curva {
    /// The command buttons, in tabs. A button runs its command as typing it would, ending any
    /// command already running, as a Rhino toolbar button does.
    pub(crate) fn command_tabs(&mut self, ui: &mut egui::Ui) {
        // Only Freeform has commands yet; the other specializations say when they come.
        if self.bar.specialization != 0 {
            ui.label(self.text.render(&Message::new("ui.specialization.later")));
            return;
        }
        ui.horizontal(|ui| {
            for (index, (key, _)) in COMMAND_TABS.iter().enumerate() {
                let label = self.text.render(&Message::new(key));
                if ui
                    .selectable_label(self.bar.command_tab == index, label)
                    .clicked()
                {
                    self.bar.command_tab = index;
                }
            }
        });
        let mut chosen = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for name in COMMAND_TABS[self.bar.command_tab.min(COMMAND_TABS.len() - 1)].1 {
                    if command_button(ui, name).clicked() {
                        chosen = Some(*name);
                    }
                }
            });
        });
        if let Some(name) = chosen {
            self.run_from_button(name);
        }
    }

    /// The top bar, named Curva: always there and fixed (only focus mode hides it). The file's
    /// name and Curva centred on top; under them File and Settings at the left, and the
    /// specializations as a dial in the middle, the selected one centred.
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let glass = self.settings.glass;
        let frame = panel_frame(false).fill(if glass {
            theme::current().glass
        } else {
            theme::current().panel
        });
        let mut chosen = None;
        egui::Area::new(egui::Id::NULL.with("top bar"))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                if glass {
                    ui.painter().add(egui::Shape::from(
                        eframe::egui_wgpu::Callback::new_paint_callback(
                            rect,
                            curva_viewport::Glass {
                                rect,
                                radius: f32::from(PANEL_RADIUS),
                            },
                        ),
                    ));
                }
                frame.show(ui, |ui| {
                    let inner = rect.shrink(PANEL_MARGIN);
                    ui.set_min_size(inner.size());
                    ui.set_max_size(inner.size());
                    // Left: Curva's mark and name, then File and Settings.
                    let left = ui
                        .horizontal_centered(|ui| {
                            let mark = self.top_mark(ui.ctx());
                            if let Some(mark) = mark {
                                ui.add(
                                    egui::Image::new(&mark)
                                        .fit_to_exact_size(egui::Vec2::splat(18.0)),
                                );
                            }
                            ui.label(egui::RichText::new("Curva").strong().size(14.0));
                            ui.add_space(8.0);
                            ui.menu_button(self.text.render(&Message::new("ui.tab.file")), |ui| {
                                chosen = self.file_items(ui);
                            });
                            if ui
                                .button(self.text.render(&Message::new("ui.file.settings")))
                                .clicked()
                            {
                                self.open_settings(SettingsTab::General);
                            }
                        })
                        .response
                        .rect;
                    // Centre: the specialization dial, centred on the window.
                    let dial = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, inner.center().y),
                        egui::vec2(DIAL_WIDTH, 22.0),
                    );
                    self.specialization_dial(ui, dial);
                    // Right: the file's name over its path, right-aligned.
                    let room = egui::Rect::from_min_max(
                        egui::pos2(dial.max.x.max(left.max.x) + 12.0, rect.min.y),
                        egui::pos2(inner.max.x, rect.max.y),
                    );
                    self.file_label(ui, room);
                });
            });
        if let Some(name) = chosen {
            self.run_from_button(name);
        }
    }

    /// Curva's mark as a texture, made once from the window icon.
    fn top_mark(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.bar.mark.is_none() {
            let icon = window_icon()?;
            let size = [icon.width as usize, icon.height as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
            self.bar.mark =
                Some(ctx.load_texture("curva mark", image, egui::TextureOptions::LINEAR));
        }
        self.bar.mark.clone()
    }

    /// The model's name on top and its path below, smaller and quieter, both right-aligned in
    /// `room`; a long path is shortened in the middle and shown in full on hover. An untitled
    /// model shows its name alone.
    fn file_label(&self, ui: &egui::Ui, room: egui::Rect) {
        if room.width() < 24.0 {
            return;
        }
        let path = self.doc.path();
        let name = path.and_then(|p| p.file_stem()).map_or_else(
            || self.text.render(&Message::new("ui.untitled")),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let painter = ui.painter_at(room);
        let colours = theme::current();
        let right = room.max.x;
        let middle = room.center().y;
        let Some(path) = path else {
            painter.text(
                egui::pos2(right, middle),
                egui::Align2::RIGHT_CENTER,
                name,
                egui::FontId::proportional(13.0),
                colours.text,
            );
            return;
        };
        painter.text(
            egui::pos2(right, middle - 1.0),
            egui::Align2::RIGHT_BOTTOM,
            name,
            egui::FontId::proportional(12.5),
            colours.text,
        );
        let full = path.to_string_lossy().into_owned();
        let font = egui::FontId::proportional(10.0);
        let width = room.width();
        let shown = shortened_path(&full, |text| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(text.to_owned(), font.clone(), colours.quiet)
                    .size()
                    .x
                    <= width
            })
        });
        let drawn = painter.text(
            egui::pos2(right, middle),
            egui::Align2::RIGHT_TOP,
            shown,
            font,
            colours.quiet,
        );
        let hover = ui.interact(
            drawn.union(egui::Rect::from_min_max(
                egui::pos2(drawn.min.x, room.min.y),
                drawn.max,
            )),
            egui::Id::NULL.with("top bar path"),
            egui::Sense::hover(),
        );
        hover.on_hover_text(full);
    }

    /// The specializations as a dial that loops: the selected one in the centre, its neighbours
    /// either side and fading. Clicking a neighbour or scrolling over the dial turns it.
    pub(crate) fn specialization_dial(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let count = SPECIALIZATIONS.len();
        let response = ui.interact(rect, ui.scope_id().with("dial"), egui::Sense::click());
        if response.hovered() {
            let scroll =
                ui.input(|input| input.smooth_scroll_delta.x + input.smooth_scroll_delta.y);
            if scroll.abs() > 20.0 {
                self.bar.specialization =
                    (self.bar.specialization + if scroll < 0.0 { 1 } else { count - 1 }) % count;
            }
        }
        let spacing = rect.width() / 3.0;
        for offset in [-1_i32, 0, 1] {
            let index = (self.bar.specialization as i32 + offset).rem_euclid(count as i32) as usize;
            let centre = egui::pos2(rect.center().x + spacing * offset as f32, rect.center().y);
            let fade = match offset.abs() {
                0 => 1.0,
                1 => 0.55,
                _ => 0.22,
            };
            let colour = if offset == 0 {
                theme::current().text
            } else {
                theme::current().quiet.gamma_multiply(fade)
            };
            let name = self.text.render(&Message::new(SPECIALIZATIONS[index]));
            let galley = ui.painter().layout_no_wrap(
                name,
                egui::FontId::new(14.0, egui::FontFamily::Name("Bold".into())),
                colour,
            );
            let at = centre - galley.size() / 2.0;
            let hit = egui::Rect::from_min_size(at, galley.size()).expand(4.0);
            ui.painter().galley(at, galley, colour);
            if offset != 0
                && ui
                    .interact(
                        hit,
                        ui.scope_id().with(("dial item", offset)),
                        egui::Sense::click(),
                    )
                    .clicked()
            {
                self.bar.specialization = index;
            }
            if offset == 0 {
                ui.painter().line_segment(
                    [
                        egui::pos2(hit.min.x + 4.0, hit.max.y),
                        egui::pos2(hit.max.x - 4.0, hit.max.y),
                    ],
                    egui::Stroke::new(2.0, theme::ACCENT),
                );
            }
        }
    }

    /// Runs a command as typing it would, ending any command already running, as a Rhino
    /// toolbar button does.
    pub(crate) fn run_from_button(&mut self, name: &str) {
        if self.processor.is_running() {
            self.processor.cancel(&mut self.doc);
        }
        self.typed.clear();
        self.highlight = 0;
        self.processor.submit(&mut self.doc, name);
    }

    /// The File menu: New, Open, Save, Save As, Import, Export and Export Selected run their
    /// commands; About opens Settings at its About tab. Returns a command to run.
    pub(crate) fn file_items(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let mut chosen = None;
        for (name, key) in [
            ("New", "ui.file.new"),
            ("Open", "ui.file.open"),
            ("Save", "ui.file.save"),
            ("SaveAs", "ui.file.save_as"),
        ] {
            if ui.button(self.text.render(&Message::new(key))).clicked() {
                chosen = Some(name);
                ui.close();
            }
        }
        ui.separator();
        // Rhino files: Import adds one to the model, Export and ExportSelected write one.
        for (name, key) in [
            ("Import", "ui.file.import"),
            ("Export", "ui.file.export"),
            ("ExportSelected", "ui.file.export_selected"),
        ] {
            if ui.button(self.text.render(&Message::new(key))).clicked() {
                chosen = Some(name);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(self.text.render(&Message::new("ui.file.about")))
            .clicked()
        {
            self.open_settings(SettingsTab::About);
            ui.close();
        }
        chosen
    }

    /// Brings a closed or hidden panel piece out, as Settings is.
    pub(crate) fn open_piece(&mut self, piece: layout::Piece) {
        let window = self.window_rect;
        self.layout = self.layout.restored(piece, window);
        if let Some(panel) = self.layout.panel_of(piece)
            && let Some(index) = self
                .layout
                .place(panel)
                .and_then(|p| p.pieces.iter().position(|p| *p == piece))
        {
            self.layout = self
                .layout
                .with_active(panel, index)
                .with_hidden(panel, false);
        }
        self.save_layout();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_path_is_shortened_in_the_middle_keeping_the_file_name() {
        let fits = |limit: usize| move |text: &str| text.chars().count() <= limit;
        let path = r"V:\projects\client\2026\tower.curva";
        assert_eq!(super::shortened_path(path, fits(100)), path);
        assert_eq!(
            super::shortened_path(path, fits(26)),
            r"V:\…\2026\tower.curva"
        );
        assert_eq!(super::shortened_path(path, fits(20)), r"V:\…\tower.curva");
        assert_eq!(super::shortened_path(path, fits(12)), "tower.curva");
        assert_eq!(
            super::shortened_path("/home/me/models/tower.curva", fits(22)),
            "/…/models/tower.curva"
        );
    }

    use super::*;

    #[test]
    fn every_command_button_names_a_command_and_each_tab_has_some() {
        for (tab, names) in COMMAND_TABS {
            assert!(!names.is_empty(), "{tab} is empty");
            for name in names {
                let found = curva_commands::commands::resolve(name).map(|def| def.name);
                assert_eq!(found, Some(*name), "{name} in {tab}");
            }
        }
    }
}
