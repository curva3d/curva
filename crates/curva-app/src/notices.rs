//! What happens once Curva opens, the notices above the command line, updates and recovery.

use crate::*;

/// A model left behind by a crash, and the update check and download.
#[derive(Default)]
pub(crate) struct Startup {
    /// A recovery file left by a Curva that did not close normally, waiting for the user to
    /// restore or discard it.
    pub(crate) left_behind: Option<std::path::PathBuf>,
    /// The answer of the check for updates, while it is under way.
    pub(crate) update_check: Option<std::sync::mpsc::Receiver<Option<updates::Release>>>,
    /// The update being downloaded.
    pub(crate) update_download:
        Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>>,
}

/// What clicking one of the command bar's own lines does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoticeAction {
    /// Nothing; it is only said.
    None,
    /// Download and install the update on offer.
    Update,
    /// Open a page in the browser.
    Open(String),
    /// Add Curva to the Linux desktop's applications (spec `feedback-0.4.2.md` §2).
    AddToApplications,
}

/// One of the command bar's own lines, above the command history.
#[derive(Clone, Debug)]
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) action: NoticeAction,
    /// When it was first shown, for its glow.
    pub(crate) born: Option<f64>,
}

impl Notice {
    pub(crate) fn new(text: String, action: NoticeAction) -> Self {
        Self {
            text,
            action,
            born: None,
        }
    }
}

/// `from` moved towards `to` by `amount` (0 to 1).
pub(crate) fn lerp_colour(from: egui::Color32, to: egui::Color32, amount: f32) -> egui::Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    egui::Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

/// Opens a page in the system's browser.
pub(crate) fn open_link(url: &str) {
    let result = if cfg!(target_os = "windows") {
        quiet_command("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    let _ = result;
}

impl Curva {
    /// What happens once Curva has opened: the model given on the command line opens, the check
    /// for updates starts in the background, and the command bar says what an update brought and
    /// welcomes the user.
    pub(crate) fn opened(mut self, ctx: &egui::Context, file: Option<&std::path::Path>) -> Self {
        if let Some(path) = file
            && let Ok(doc) = Document::open(path)
        {
            self.doc = doc;
        }
        if self.benchmark.is_some() {
            return self;
        }
        if let Some(version) = updates::just_updated() {
            let (a, b, c) = version;
            let text = format!("{a}.{b}.{c}");
            let line = self
                .text
                .render(&Message::with("ui.update.done", vec![text.clone()]));
            self.notices.push(Notice::new(
                line,
                NoticeAction::Open(format!("{}{text}", updates::NOTES)),
            ));
        }
        let welcome = self.text.render(&Message::new("ui.welcome"));
        self.notices.push(Notice::new(welcome, NoticeAction::None));
        // The first time Curva runs from an AppImage, offer it a place in the applications.
        if !self.settings.desktop_offered
            && desktop::appimage().is_some()
            && desktop::data_home().is_some_and(|data| !desktop::added(&data))
        {
            let line = self.text.render(&Message::new("ui.desktop.offer"));
            self.notices
                .push(Notice::new(line, NoticeAction::AddToApplications));
            self.settings.desktop_offered = true;
            self.save_settings();
        }
        if self.settings.check_updates {
            let (sender, receiver) = std::sync::mpsc::channel();
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(updates::check());
                repaint.request_repaint();
            });
            self.startup.update_check = Some(receiver);
        }
        self
    }

    /// Adds Curva to the desktop's applications, or removes it; returns whether that worked.
    pub(crate) fn set_in_applications(&self, wanted: bool) -> bool {
        let (Some(data), Some(appimage)) = (desktop::data_home(), desktop::appimage()) else {
            return false;
        };
        if wanted {
            desktop::add(&data, &appimage).is_ok()
        } else {
            desktop::remove(&data).is_ok()
        }
    }

    /// The command bar's own lines above the command history: an update on offer, what an update
    /// brought, and the welcome. They glow softly in the accent for their first two seconds.
    pub(crate) fn notices(&mut self, ui: &mut egui::Ui, skip: usize) {
        self.receive_update(ui.ctx());
        let now = ui.input(|input| input.time);
        let mut clicked = None;
        for (index, notice) in self.notices.iter_mut().enumerate().skip(skip) {
            let born = *notice.born.get_or_insert(now);
            let age = (now - born) as f32;
            let glow = (1.0 - age / NOTICE_GLOW).clamp(0.0, 1.0);
            if glow > 0.0 {
                ui.ctx().request_repaint();
            }
            let colour = lerp_colour(theme::current().text, theme::ACCENT, glow);
            let text = egui::RichText::new(&notice.text).color(colour);
            match notice.action {
                NoticeAction::None => {
                    ui.label(text);
                }
                NoticeAction::Update | NoticeAction::Open(_) | NoticeAction::AddToApplications => {
                    if ui.link(text).clicked() {
                        clicked = Some(index);
                    }
                }
            }
        }
        if let Some(index) = clicked {
            match self.notices[index].action.clone() {
                NoticeAction::Update => self.start_update(ui.ctx()),
                NoticeAction::Open(url) => open_link(&url),
                NoticeAction::AddToApplications => {
                    let line = self
                        .text
                        .render(&Message::new(if self.set_in_applications(true) {
                            "ui.desktop.added"
                        } else {
                            "ui.desktop.failed"
                        }));
                    self.notices
                        .insert(0, Notice::new(line, NoticeAction::None));
                }
                NoticeAction::None => {}
            }
        }
    }

    /// Takes the answer of the check for updates, and the download, when they come.
    pub(crate) fn receive_update(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.startup.update_check
            && let Ok(found) = receiver.try_recv()
        {
            self.startup.update_check = None;
            if let Some(release) = found {
                let line = self.text.render(&Message::with(
                    "ui.update.available",
                    vec![release.version_text()],
                ));
                // The update line comes first, before the welcome.
                self.notices
                    .insert(0, Notice::new(line, NoticeAction::Update));
                self.update = Some(release);
            }
        }
        if let Some(receiver) = &self.startup.update_download
            && let Ok(result) = receiver.try_recv()
        {
            self.startup.update_download = None;
            let version = self
                .update
                .as_ref()
                .map(updates::Release::version_text)
                .unwrap_or_default();
            match result
                .and_then(|file| updates::install(&file, &version, self.doc.path()).map(|()| file))
            {
                Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => {
                    let line = self
                        .text
                        .render(&Message::with("ui.update.failed", vec![error]));
                    self.notices
                        .insert(0, Notice::new(line, NoticeAction::None));
                }
            }
        }
    }

    /// Downloads the update in the background, saving the model first if it has a file.
    pub(crate) fn start_update(&mut self, ctx: &egui::Context) {
        let Some(release) = self.update.clone() else {
            return;
        };
        if self.startup.update_download.is_some() {
            return;
        }
        if let Some(path) = self.doc.path().map(std::path::Path::to_path_buf) {
            let _ = self.doc.save(&path);
        }
        let line = self.text.render(&Message::with(
            "ui.update.downloading",
            vec![release.version_text()],
        ));
        self.notices
            .insert(0, Notice::new(line, NoticeAction::None));
        let (sender, receiver) = std::sync::mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(updates::download(&release));
            repaint.request_repaint();
        });
        self.startup.update_download = Some(receiver);
    }

    /// Offers back the model a Curva that did not close normally left behind.
    pub(crate) fn recovery_window(&mut self, ui: &mut egui::Ui) {
        let Some(file) = self.startup.left_behind.clone() else {
            return;
        };
        let (mut restore, mut discard) = (false, false);
        egui::Window::new(self.text.render(&Message::new("ui.recovery")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(self.text.render(&Message::new("ui.recovery.question")));
                ui.horizontal(|ui| {
                    restore = ui
                        .button(self.text.render(&Message::new("ui.recovery.restore")))
                        .clicked();
                    discard = ui
                        .button(self.text.render(&Message::new("ui.recovery.discard")))
                        .clicked();
                });
            });
        if restore {
            self.startup.left_behind = None;
            match recovery::restore(&file) {
                Ok(doc) => {
                    self.doc = doc;
                    self.drawn = None;
                    let line = self.text.render(&Message::new("done.recovered"));
                    self.processor.note(line);
                }
                Err(error) => {
                    let line = self
                        .text
                        .render(&Message::with("error.recovery", vec![error]));
                    self.processor.note(line);
                }
            }
        } else if discard {
            self.startup.left_behind = None;
            recovery::discard(&file);
        }
    }
}
