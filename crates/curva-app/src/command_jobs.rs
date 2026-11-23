//! Long jobs in the command bar (#480; the owner's choice of 21 November 2026, options 1 and 2
//! mixed): a thin progress line along the top of the bar, and a chip at its right with a spinner,
//! the job's name and how far along it is. Clicking the chip shows the step and the counts with a
//! Stop button; Escape stops it too. The command line stays free for typing.

use crate::*;
use curva_document::progress::JobView;

/// The chip's words for `job`: its name, and how far along it is when counted.
pub(crate) fn chip_text(text: &Translations, job: &JobView) -> String {
    let name = text.render(&Message::with(job.name.0, job.name.1.clone()));
    match job.fraction() {
        Some(part) => format!("{name} · {:.0}%", part * 100.0),
        None => format!("{name}…"),
    }
}

/// The details under the chip: the step, its count when counted, and how to stop.
pub(crate) fn detail_text(text: &Translations, job: &JobView) -> String {
    let step = job.step.as_ref().map_or_else(String::new, |(key, args)| {
        text.render(&Message::with(key, args.clone()))
    });
    if job.total > 0 {
        format!("{step}: {} / {}", job.done.min(job.total), job.total)
    } else {
        step
    }
}

impl Curva {
    /// The running jobs' progress line along the top of `bar` and the chip at its right end;
    /// returns whether the chip asked to stop them.
    pub(crate) fn command_jobs(&mut self, ui: &egui::Ui, bar: egui::Rect) -> bool {
        let jobs = curva_document::progress::running();
        let Some(job) = jobs.first() else {
            return false;
        };
        // The line: its counted part, or a short piece sliding along while not counted.
        let (from, to) = match job.fraction() {
            Some(part) => (0.0, part),
            None => {
                let t = (ui.input(|i| i.time) * 0.6).fract() as f32;
                (t * 1.3 - 0.3, t * 1.3)
            }
        };
        ui_kit::progress_line(ui, bar, from, to);
        // The chip, at the right end of the bar's first row.
        let mut label = chip_text(&self.text, job);
        if jobs.len() > 1 {
            label = format!("{label} (+{})", jobs.len() - 1);
        }
        let corner = egui::pos2(bar.right() - ui_kit::PAD, bar.top() + 6.0);
        let response = ui_kit::busy_chip(ui, corner, label);
        let mut stop = false;
        egui::Popup::from_toggle_button_response(&response).show(|ui| {
            for job in &jobs {
                ui.strong(
                    self.text
                        .render(&Message::with(job.name.0, job.name.1.clone())),
                );
                ui.label(detail_text(&self.text, job));
            }
            ui.horizontal(|ui| {
                stop = ui
                    .button(self.text.render(&Message::new("ui.job.stop")))
                    .clicked();
                ui.weak(self.text.render(&Message::new("ui.job.esc")));
            });
        });
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chip_names_the_job_and_how_far_along_it_is() {
        let text = Translations::english();
        let mut job = JobView {
            name: ("job.open", vec!["house.3dm".to_owned()]),
            step: Some(("job.step.solids", Vec::new())),
            done: 912,
            total: 1536,
        };
        assert_eq!(chip_text(&text, &job), "Opening house.3dm · 59%");
        assert_eq!(detail_text(&text, &job), "Building solids: 912 / 1536");
        job.total = 0;
        job.step = Some(("job.step.reading", Vec::new()));
        assert_eq!(chip_text(&text, &job), "Opening house.3dm…");
        assert_eq!(detail_text(&text, &job), "Reading the file");
    }
}
