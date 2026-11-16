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

/// How much closer than a framed view pictures of standard scenes are taken.
pub(crate) const SCENE_PICTURE_ZOOM: f64 = 0.74;

/// The height a plan is cut at, in millimetres, as is usual for floor plans.
pub(crate) const PLAN_CUT_HEIGHT: f32 = 1200.0;

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

/// Runs semicolon-separated command lines and saves a picture of the result: in `style` if given
/// (`--style diagram`), starting from a standard scene if given (`--scene building`), and seen
/// from a standard view if given (`--view top`; otherwise in perspective).
pub(crate) fn picture(
    path: &str,
    script: &str,
    style: Option<curva_viewport::PictureStyle>,
    scene: Option<BenchmarkScene>,
    view: Option<StandardView>,
) -> Result<(), String> {
    let mut doc = Document::default();
    if let Some(scene) = scene {
        benchmark_scene(&mut doc, scene)?;
    }
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
    let mut camera = view.map_or_else(Camera::default, StandardView::camera);
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
        // A standard scene fills more of the picture, for judging how it looks.
        if scene.is_some() {
            camera.zoom(SCENE_PICTURE_ZOOM);
        }
    }
    // The Architectural style is a cut drawing: everything above a plan's cut height goes.
    let cut = (style == Some(curva_viewport::PictureStyle::Architectural)).then_some([
        0.0,
        0.0,
        1.0,
        -PLAN_CUT_HEIGHT,
    ]);
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    if let Some(style) = style {
        // Styles find outlines from object numbers, so the faces are numbered.
        let parts = curva_viewport::SceneParts::of(&outlines);
        let numbered = Picture {
            lines: parts.shaded_lines,
            faces: parts.faces,
            matrix: camera.matrix(aspect),
            light: camera.forward().map(|value| value as f32),
            ghosted: false,
        };
        curva_viewport::save_styled_picture(
            numbered,
            style,
            cut,
            PICTURE_SIZE,
            std::path::Path::new(path),
        )?;
    } else {
        save_picture(
            picture_of(&doc, &outlines, &camera, aspect, ghosted),
            PICTURE_SIZE,
            std::path::Path::new(path),
        )?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use curva_viewport::{Lighting, Look};

    /// Rhino's Shaded viewport grey, behind the pictures for comparing with it.
    const RHINO_GREY: egui::Color32 = egui::Color32::from_rgb(158, 163, 171);

    /// Pictures of Bevy's looks with Curva's lines, beside Curva's own Shaded, with what each
    /// costs per frame, for the owner to choose from: the test building, and a box, a sphere and
    /// a flat four-sided surface as in Rhino's example. Run on demand:
    /// `CURVA_LOOKS_OUT=<folder> cargo test -p curva-app bevy_look_options -- --ignored`.
    #[test]
    #[ignore = "makes pictures for the owner; needs a graphics card"]
    fn bevy_look_options() {
        let out = std::path::PathBuf::from(
            std::env::var("CURVA_LOOKS_OUT").unwrap_or_else(|_| "looks".into()),
        );
        std::fs::create_dir_all(&out).unwrap();
        let mut building = Document::default();
        benchmark_scene(&mut building, BenchmarkScene::named("building").unwrap()).unwrap();
        let mut shapes = Document::default();
        shapes.add_body(
            curva_kernel::Body::cuboid([3000.0, 2500.0, 0.0], [1600.0, 1600.0, 2000.0]).unwrap(),
        );
        shapes.add_body(curva_kernel::Body::sphere([4200.0, -800.0, 900.0], 900.0).unwrap());
        let outline = curva_kernel::Curve::polyline(
            vec![
                [-3000.0, -2500.0, 0.0],
                [1200.0, -3200.0, 0.0],
                [1500.0, 1800.0, 0.0],
                [-2600.0, 2600.0, 0.0],
            ],
            true,
        )
        .unwrap();
        shapes.add_body(curva_kernel::Body::planar(&[outline]).unwrap());
        let looks = [
            (
                "studio",
                Look {
                    lighting: Lighting::Studio,
                    shine: 0.6,
                },
            ),
            (
                "sunlit",
                Look {
                    lighting: Lighting::Sunlit,
                    shine: 0.6,
                },
            ),
        ];
        // The shapes closer, as in Rhino's example.
        for (scene, doc, zoom) in [
            ("building", &building, SCENE_PICTURE_ZOOM),
            ("shapes", &shapes, 0.55),
        ] {
            let outlines = Outlines::of(doc.state());
            let mut camera = Camera::default();
            let (low, high) = outlines.bounds().unwrap();
            camera.frame(low, high);
            camera.zoom(zoom);
            let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
            let curva = picture_of(doc, &outlines, &camera, aspect, false);
            save_picture(
                curva,
                PICTURE_SIZE,
                &out.join(format!("{scene}-0-curva.png")),
            )
            .unwrap();
            // Bevy's surfaces with Curva's lines, numbered as the window numbers them.
            let parts = curva_viewport::SceneParts::of(&outlines);
            for (index, (name, look)) in looks.iter().enumerate() {
                let picture = Picture {
                    lines: parts.shaded_lines.clone(),
                    faces: parts.faces.clone(),
                    matrix: camera.matrix(aspect),
                    light: camera.forward().map(|value| value as f32),
                    ghosted: false,
                };
                let ms = curva_viewport::save_bevy_picture(
                    picture,
                    parts.objects.clone(),
                    camera,
                    *look,
                    RHINO_GREY,
                    PICTURE_SIZE,
                    &out.join(format!("{scene}-{}-{name}.png", index + 1)),
                )
                .unwrap();
                println!("{scene} {name}: {ms:.2} ms per frame");
            }
        }
    }
}
