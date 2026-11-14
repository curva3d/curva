//! The command line: typing, completions, the log above it and the drawing aids bar.

use crate::*;

/// The rows of the log to show, newest last. Always the same number, one line each, so that a
/// new message never changes the height of the panel and moves the viewport.
#[cfg(test)]
pub(crate) fn log_rows(log: &[String]) -> Vec<&str> {
    let (blanks, _, shown) = history_rows(log, 0);
    std::iter::repeat_n(" ", blanks)
        .chain(shown.iter().map(String::as_str))
        .collect()
}

/// How the command bar's rows are shared between its own lines (`notices` of them) and the
/// history: blank rows first, how many of its own lines to leave out (the oldest), and the history
/// lines shown. Together they fill [`LOG_LINES`] rows, the oldest going as new ones come.
pub(crate) fn history_rows(log: &[String], notices: usize) -> (usize, usize, &[String]) {
    let total = notices + log.len();
    let skip = total.saturating_sub(LOG_LINES);
    let blanks = LOG_LINES.saturating_sub(total);
    let log_skip = skip.saturating_sub(notices);
    (blanks, skip.min(notices), &log[log_skip.min(log.len())..])
}

/// A command button: its icon, with its name on hover, or its name where it has no icon yet.
pub(crate) fn command_button(ui: &mut egui::Ui, name: &str) -> egui::Response {
    if !icons::has(name) {
        return ui.button(name).on_hover_text(name);
    }
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(COMMAND_BUTTON), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme::current().hover);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), name);
    response.on_hover_text(name)
}

impl Curva {
    /// The commands to offer for what has been typed. None while a command is asking for input.
    pub(crate) fn listed_matches(&self) -> Vec<&'static str> {
        if self.processor.is_running() {
            Vec::new()
        } else {
            self.processor.completions(&self.typed)
        }
    }

    /// Moves the highlight with the arrow keys and fills in with Tab. Returns whether it filled.
    pub(crate) fn completion_keys(&mut self, ui: &egui::Ui) -> bool {
        let matches = self.listed_matches();
        let listed = !matches.is_empty();
        let (tab, down, up) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                listed && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                listed && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            )
        });
        if down {
            self.highlight = (self.highlight + 1) % matches.len();
        }
        if up {
            self.highlight = (self.highlight + matches.len() - 1) % matches.len();
        }
        if tab && let Some(name) = matches.get(self.highlight) {
            (*name).clone_into(&mut self.typed);
            self.highlight = 0;
            return true;
        }
        false
    }

    /// Greys in the rest of the highlighted command after what has been typed.
    pub(crate) fn paint_remainder(&self, ui: &egui::Ui, output: &egui::text_edit::TextEditOutput) {
        let Some(name) = self.listed_matches().get(self.highlight).copied() else {
            return;
        };
        let typed = self.typed.len();
        let continues = name
            .get(..typed)
            .is_some_and(|start| start.eq_ignore_ascii_case(&self.typed));
        if continues {
            ui.painter().text(
                output.galley_pos + egui::vec2(output.galley.rect.right(), 0.0),
                egui::Align2::LEFT_TOP,
                &name[typed..],
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
    }

    /// The list of possible commands, floating above the command line. Returns any clicked.
    pub(crate) fn match_list(&self, ui: &egui::Ui, above: egui::Pos2) -> Option<&'static str> {
        let matches = self.listed_matches();
        if matches.is_empty() {
            return None;
        }
        let mut clicked = None;
        egui::Area::new(ui.scope_id().with("command matches"))
            .order(egui::Order::Foreground)
            .fixed_pos(above)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                // As wide as the longest name, one per row, never cut; ten rows, then it scrolls.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let widest = matches
                        .iter()
                        .map(|name| {
                            ui.painter()
                                .layout_no_wrap(
                                    (*name).to_owned(),
                                    font.clone(),
                                    egui::Color32::WHITE,
                                )
                                .size()
                                .x
                        })
                        .fold(0.0, f32::max);
                    ui.set_min_width(widest + 2.0 * ui.spacing().button_padding.x);
                    let row = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                    egui::ScrollArea::vertical()
                        .max_height(row * MOST_MATCH_ROWS)
                        .show(ui, |ui| {
                            for (index, name) in matches.iter().enumerate() {
                                let shown = ui.selectable_label(index == self.highlight, *name);
                                if index == self.highlight {
                                    shown.scroll_to_me(None);
                                }
                                if shown.clicked() {
                                    clicked = Some(*name);
                                }
                            }
                        });
                });
            });
        clicked
    }

    /// The row of snap checkboxes, with the keys for ortho and grid snap.
    pub(crate) fn drawing_aids_bar(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F8)) {
            self.ortho = !self.ortho;
        }
        if ui.input(|input| input.key_pressed(egui::Key::F9)) {
            self.grid_snap = !self.grid_snap;
        }
        let text = &self.text;
        let tick = |ui: &mut egui::Ui, value: &mut bool, key: &'static str| {
            ui.checkbox(value, text.render(&Message::new(key)));
        };
        let snaps = &mut self.snaps;
        let (ortho, grid_snap) = (&mut self.ortho, &mut self.grid_snap);
        let gumball_on = &mut self.gumball_on;
        let (filter, filter_row) = (&mut self.pick_filter, self.filter_row);
        ui.horizontal(|ui| {
            tick(ui, &mut snaps.end, snap_label(SnapKind::End));
            tick(ui, &mut snaps.mid, snap_label(SnapKind::Mid));
            tick(ui, &mut snaps.center, snap_label(SnapKind::Center));
            tick(
                ui,
                &mut snaps.intersection,
                snap_label(SnapKind::Intersection),
            );
            tick(
                ui,
                &mut snaps.perpendicular,
                snap_label(SnapKind::Perpendicular),
            );
            tick(ui, &mut snaps.near, snap_label(SnapKind::Near));
            ui.separator();
            tick(ui, ortho, "ui.ortho");
            tick(ui, grid_snap, "ui.grid_snap");
            tick(ui, gumball_on, "ui.gumball");
            if filter_row {
                ui.separator();
                ui.label(text.render(&Message::new("ui.filter")));
                tick(ui, &mut filter.points, "ui.filter.points");
                tick(ui, &mut filter.curves, "ui.filter.curves");
                tick(ui, &mut filter.surfaces, "ui.filter.surfaces");
                tick(ui, &mut filter.solids, "ui.filter.solids");
                let mut disabled = !filter.on;
                tick(ui, &mut disabled, "ui.filter.disable");
                filter.on = !disabled;
            }
        });
    }

    pub(crate) fn command_line(&mut self, ui: &mut egui::Ui) {
        // The command bar's own lines come first, then the history; together they fill the rows,
        // the oldest going as new ones come.
        let log = self.processor.log().to_vec();
        let (blanks, skip, shown) = history_rows(&log, self.notices.len());
        for _ in 0..blanks {
            ui.label(" ");
        }
        self.notices(ui, skip);
        for row in shown {
            ui.add(egui::Label::new(row.as_str()).truncate());
        }
        let prompt = self
            .processor
            .prompt()
            .unwrap_or_else(|| self.text.render(&Message::new("ui.command")));
        // These keys must be taken before the text box, which would otherwise act on them.
        let filled = self.completion_keys(ui);
        let mut clicked_option = None;
        let mut submit = false;
        let mut clicked = None;
        let mut changed = false;
        ui.horizontal(|ui| {
            // The question, then its options as buttons, which answer as typing them would.
            if let Some(question) = self.processor.question() {
                ui.label(question);
                for (name, label) in self.processor.options() {
                    let chip = egui::Button::new(egui::RichText::new(label).color(theme::ACCENT))
                        .small()
                        .frame(true);
                    if ui.add(chip).on_hover_text(name).clicked() {
                        clicked_option = Some(name);
                    }
                }
                ui.label(":");
            } else {
                ui.label(format!("{prompt}:"));
            }
            let output = egui::TextEdit::singleline(&mut self.typed)
                .desired_width(f32::INFINITY)
                .show(ui);
            self.command_field = Some(output.response.id);
            if !self.naming_layer {
                output.response.request_focus();
            }
            changed = output.response.changed();
            if filled {
                // Without this the cursor stays mid-word and the next key lands there.
                let mut state = output.state.clone();
                let end = egui::text::CCursor::new(self.typed.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), output.response.id);
            }
            self.paint_remainder(ui, &output);
            clicked = self.match_list(ui, output.response.rect.left_top());
            submit = !self.naming_layer && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
        if let Some(name) = clicked_option {
            self.typed.clear();
            self.processor.submit(&mut self.doc, name);
        }
        self.drawing_aids_bar(ui);
        let wants_path = self
            .processor
            .current_request()
            .is_some_and(|request| request.kind == Kind::Path);
        // Space runs the command as in Rhino, except where a file name may contain one.
        if !wants_path && self.typed.ends_with(' ') {
            self.typed.pop();
            submit = true;
        }
        if changed && !submit {
            self.highlight = 0;
        }
        if let Some(name) = clicked {
            self.typed.clear();
            self.highlight = 0;
            self.processor.submit(&mut self.doc, name);
        } else if submit {
            let line = self.processor.autofilled(&self.typed, self.highlight);
            self.typed.clear();
            self.highlight = 0;
            // With a distance or angle held, Enter places the point where the pointer aims.
            match self.last_aim {
                Some(at) if line.is_empty() && self.processor.lock().is_set() => {
                    self.processor.click(&mut self.doc, at);
                }
                _ => self.processor.submit(&mut self.doc, &line),
            }
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            // With nothing running and nothing typed, Escape clears the selection.
            if !self.processor.is_running() && self.typed.is_empty() {
                self.doc.select(&[]);
                self.doc.select_grips(&[]);
                self.doc.select_edges(&[]);
                self.doc.select_faces(&[]);
            }
            self.typed.clear();
            self.highlight = 0;
            self.processor.cancel(&mut self.doc);
        }
        let delete = !self.naming_layer && ui.input(|input| input.key_pressed(egui::Key::Delete));
        let idle = self.typed.is_empty() && !self.processor.is_running();
        // F10 and F11 show and hide control points, as in Rhino.
        let (points_on, points_off) = ui.input(|input| {
            (
                input.key_pressed(egui::Key::F10),
                input.key_pressed(egui::Key::F11),
            )
        });
        if idle && points_on {
            self.processor.submit(&mut self.doc, "pointson");
        } else if idle && points_off {
            self.processor.submit(&mut self.doc, "pointsoff");
        }
        // With control points picked, Delete takes them away rather than whole curves.
        if delete && idle && !self.doc.grip_selection().is_empty() {
            self.processor.submit(&mut self.doc, "removecontrolpoint");
        } else if delete && idle {
            self.processor.submit(&mut self.doc, "delete");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("message {index}")).collect()
    }

    #[test]
    fn the_log_takes_the_same_space_however_long_it_is() {
        for count in [0, 1, LOG_LINES, LOG_LINES + 7] {
            assert_eq!(log_rows(&messages(count)).len(), LOG_LINES);
        }
    }

    #[test]
    fn the_log_shows_the_newest_messages_last() {
        let log = messages(LOG_LINES + 2);
        let rows = log_rows(&log);
        assert_eq!(rows[LOG_LINES - 1], log[LOG_LINES + 1]);
        assert_eq!(rows[0], log[2]);
    }

    #[test]
    fn a_short_log_is_padded_above_its_messages() {
        let log = messages(1);
        let rows = log_rows(&log);
        assert_eq!(rows[LOG_LINES - 1], "message 0");
        assert_eq!(rows[0], " ");
    }
}
