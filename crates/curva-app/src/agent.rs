//! Agents connected over MCP: keeping the link, answering requests, asking the user to approve
//! actions, and the pictures of the model an agent can ask for.

use crate::*;

/// The agents' connection and what they asked for.
#[derive(Default)]
pub(crate) struct Agents {
    pub(crate) agent: Option<curva_mcp::Listener>,
    pub(crate) waiting_approval: Option<(curva_mcp::AgentRequest, String)>,
    /// When an agent last asked something, to show whether one is connected.
    pub(crate) agent_seen: Option<std::time::Instant>,
}

/// How long after an agent's last request it still counts as connected.
pub(crate) const AGENT_RECENT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) const AGENT_ACTIVE: egui::Color32 = egui::Color32::from_rgb(110, 200, 120);

pub(crate) const PICTURE_SIZE: [u32; 2] = [1200, 800];

/// What a picture of the model shows: everything, shaded or `ghosted`, seen by `camera`.
pub(crate) fn picture_of(
    doc: &Document,
    outlines: &Outlines,
    camera: &Camera,
    aspect: f64,
    ghosted: bool,
) -> Picture {
    let selection = doc.selection();
    let faces = face_vertices(outlines, &selection);
    Picture {
        lines: if faces.is_empty() || ghosted {
            scene_lines(outlines, &selection)
        } else {
            shaded_scene_lines(outlines, &selection)
        },
        faces,
        matrix: camera.matrix(aspect),
        light: camera.forward().map(|value| value as f32),
        ghosted,
    }
}

/// A picture of the model for an MCP agent, as the bytes of a PNG file.
pub(crate) fn mcp_picture(doc: &Document) -> Result<Vec<u8>, String> {
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    picture_from(doc, &camera, false)
}

/// A picture of the model seen by `camera`, as the bytes of a PNG file.
pub(crate) fn picture_from(
    doc: &Document,
    camera: &Camera,
    ghosted: bool,
) -> Result<Vec<u8>, String> {
    let outlines = Outlines::of(doc.state());
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    let path = std::env::temp_dir().join(format!("curva-mcp-{}.png", std::process::id()));
    save_picture(
        picture_of(doc, &outlines, camera, aspect, ghosted),
        PICTURE_SIZE,
        &path,
    )?;
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    // The picture has been read; a leftover temporary file would do no harm.
    let _ = std::fs::remove_file(&path);
    Ok(bytes)
}

/// Runs semicolon-separated command lines and saves a picture of the result.
pub(crate) fn picture(path: &str, script: &str) -> Result<(), String> {
    let mut doc = Document::default();
    let mut processor = Processor::default();
    // A `SetDisplayMode Ghosted` in the script makes the picture ghosted.
    let mut ghosted = false;
    for line in script.split(';') {
        processor.submit(&mut doc, line);
        while let Some(request) = processor.take_view_request() {
            if let ViewRequest::Display(mode) = request {
                ghosted = mode == DisplayMode::Ghosted;
            }
        }
    }
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    save_picture(
        picture_of(&doc, &outlines, &camera, aspect, ghosted),
        PICTURE_SIZE,
        std::path::Path::new(path),
    )?;
    for line in processor.log() {
        println!("{line}");
    }
    Ok(())
}

impl Curva {
    /// Starts or stops listening to match the checkbox.
    pub(crate) fn keep_agent_link(&mut self, ctx: &egui::Context) {
        if self.agent_on && self.agents.agent.is_none() {
            let waker = ctx.clone();
            match curva_mcp::listen(move || waker.request_repaint()) {
                Ok(listener) => self.agents.agent = Some(listener),
                Err(error) => {
                    self.agent_on = false;
                    self.processor.note(error.to_string());
                }
            }
        } else if !self.agent_on {
            self.agents.agent = None;
            self.agents.waiting_approval = None;
        }
    }

    pub(crate) fn answer_agent(&mut self, request: &curva_mcp::AgentRequest) {
        // Earlier requests may have changed the view, and a picture shows what the window does.
        self.apply_view_request();
        let camera = self.panes[self.active].camera;
        let ghosted = self.panes[self.active].display == DisplayMode::Ghosted;
        let draw = move |doc: &Document| picture_from(doc, &camera, ghosted);
        let answer =
            Session::new(&mut self.doc, &mut self.processor, &draw).handle(&request.message);
        self.apply_view_request();
        // A client that has gone away no longer needs its answer.
        let _ = request.answer.send(answer);
    }

    /// Answers what agents have asked, stopping at anything the user must approve first.
    pub(crate) fn serve_agents(&mut self) {
        while self.agents.waiting_approval.is_none() {
            let Some(request) = self
                .agents
                .agent
                .as_ref()
                .and_then(|agent| agent.requests.try_recv().ok())
            else {
                return;
            };
            self.agents.agent_seen = Some(std::time::Instant::now());
            if let Some(line) = curva_mcp::needs_approval(&request.message) {
                self.agents.waiting_approval = Some((request, line));
            } else {
                self.answer_agent(&request);
            }
        }
    }

    /// Asks the user about an action an agent wants to take.
    pub(crate) fn approval_window(&mut self, ui: &egui::Ui) {
        let Some((_, line)) = &self.agents.waiting_approval else {
            return;
        };
        let question = self
            .text
            .render(&Message::with("ui.agent.approve", vec![line.clone()]));
        let (allow_text, decline_text) = (
            self.text.render(&Message::new("ui.agent.allow")),
            self.text.render(&Message::new("ui.agent.decline")),
        );
        let (mut allow, mut decline) = (false, false);
        egui::Window::new(self.text.render(&Message::new("ui.agent")))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(question);
                ui.horizontal(|ui| {
                    allow = ui.button(allow_text).clicked();
                    decline = ui.button(decline_text).clicked();
                });
            });
        if (allow || decline)
            && let Some((request, _)) = self.agents.waiting_approval.take()
        {
            if allow {
                self.answer_agent(&request);
            } else {
                // A client that has gone away no longer needs its answer.
                let _ = request
                    .answer
                    .send(Some(curva_mcp::declined(&request.message)));
            }
        }
    }

    /// Agent access, and whether an agent is talking to Curva.
    pub(crate) fn agent_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.agent_on,
                self.text.render(&Message::new("ui.agent")),
            );
            let recent = self
                .agents
                .agent_seen
                .is_some_and(|seen| seen.elapsed() < AGENT_RECENT);
            let (key, colour) = match (self.agents.agent.is_some(), recent) {
                (false, _) => ("ui.agent.off", theme::current().quiet),
                (true, false) => ("ui.agent.waiting", theme::current().quiet),
                (true, true) => ("ui.agent.active", AGENT_ACTIVE),
            };
            ui.colored_label(colour, self.text.render(&Message::new(key)));
        });
    }
}
