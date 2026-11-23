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

/// What a picture of the model shows: everything, drawn in `style`, seen by `camera`; with
/// the camera measured as the picture is, from the middle of the model, so a model far from the
/// origin pictures as exactly as one at it (#225).
pub(crate) fn picture_of(
    doc: &Document,
    outlines: &Outlines,
    camera: &Camera,
    aspect: f64,
    style: curva_viewport::Style,
) -> (Picture, Camera) {
    let selection = doc.selection();
    let origin = outlines.drawing_origin();
    let camera = camera.measured_from(origin);
    let faces = curva_viewport::face_vertices_from(outlines, &selection, origin);
    let picture = Picture {
        lines: if faces.is_empty() || !style.solid() {
            curva_viewport::scene_lines_from(outlines, &selection, origin)
        } else {
            curva_viewport::shaded_scene_lines_from(outlines, &selection, origin)
        },
        faces,
        matrix: camera.matrix(aspect),
        light: camera.forward().map(|value| value as f32),
        style,
    };
    (picture, camera)
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
    let style = if ghosted {
        curva_viewport::Style::GHOSTED
    } else {
        curva_viewport::Style::SHADED
    };
    let (picture, camera) = picture_of(doc, &outlines, camera, aspect, style);
    save_bevy_view(picture, &camera, &path)?;
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    // The picture has been read; a leftover temporary file would do no harm.
    let _ = std::fs::remove_file(&path);
    Ok(bytes)
}

/// Behind pictures for agents: the dark grey they had before Bevy drew them.
const PICTURE_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(28, 31, 36);

/// Saves `picture`, seen by `camera`, as a PNG file at `path`, drawn as the window draws a view:
/// Bevy's surfaces with Curva's lines on them. The selection is in the picture's colours.
fn save_bevy_view(picture: Picture, camera: &Camera, path: &std::path::Path) -> Result<(), String> {
    // The Hidden and Technical display modes draw on white paper, as their faces are (#164).
    let background = if picture.style.light() == Some(curva_viewport::Light::Flat) {
        (egui::Color32::WHITE, egui::Color32::WHITE)
    } else if picture.style.light() == Some(curva_viewport::Light::Sun) {
        crate::sheets::MASSING_SKY
    } else {
        (PICTURE_BACKGROUND, PICTURE_BACKGROUND)
    };
    let pixels = curva_viewport::bevy_picture(
        picture,
        Vec::new(),
        *camera,
        curva_viewport::Look::default(),
        background,
        PICTURE_SIZE,
    )?;
    curva_viewport::save_png(path, PICTURE_SIZE, &pixels)
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
    let mut doc = curva_formats::new_document();
    if let Some(scene) = scene {
        benchmark_scene(&mut doc, scene)?;
    }
    let mut processor = Processor::default();
    // A `SetDisplayMode Ghosted` (or `Hidden`) in the script makes the picture ghosted (or hidden).
    let mut mode = DisplayMode::Shaded;
    for line in script.split(';') {
        processor.submit(&mut doc, line);
        while let Some(request) = processor.take_view_request() {
            if let ViewRequest::Display(asked) = request {
                mode = asked;
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
        let camera = camera.measured_from(parts.origin);
        // The cut, measured from the same point: its height less the model's middle.
        let cut = cut.map(|[x, y, z, d]| [x, y, z, d + (f64::from(z) * parts.origin[2]) as f32]);
        let numbered = Picture {
            lines: parts.shaded_lines,
            faces: parts.faces,
            matrix: camera.matrix(aspect),
            light: camera.forward().map(|value| value as f32),
            ..Picture::default()
        };
        curva_viewport::save_styled_picture(
            numbered,
            style,
            cut,
            PICTURE_SIZE,
            std::path::Path::new(path),
        )?;
    } else {
        let style = crate::styles::mode_style(mode);
        let (picture, camera) = picture_of(&doc, &outlines, &camera, aspect, style);
        save_bevy_view(picture, &camera, std::path::Path::new(path))?;
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
            ui_kit::toggle(
                ui,
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

/// Rhino's Shaded viewport grey, behind the look pictures for comparing with it.
const RHINO_GREY: egui::Color32 = egui::Color32::from_rgb(158, 163, 171);

/// Pictures of Bevy's looks with Curva's lines, beside Curva's own Shaded, with what each costs
/// per frame, for the owner to choose from (`curva --look-pictures <folder>`): the test building,
/// and a box, a sphere and a flat four-sided surface as in Rhino's example.
pub(crate) fn look_pictures(out: &std::path::Path) -> Result<(), String> {
    use curva_viewport::{Lighting, Look, Style};
    let text = |error: curva_kernel::KernelError| error.to_string();
    std::fs::create_dir_all(out).map_err(|error| error.to_string())?;
    let mut building = Document::default();
    benchmark_scene(
        &mut building,
        BenchmarkScene::named("building").ok_or("no building scene")?,
    )?;
    let mut shapes = Document::default();
    shapes.add_body(
        curva_kernel::Body::cuboid([3000.0, 2500.0, 0.0], [1600.0, 1600.0, 2000.0])
            .map_err(text)?,
    );
    shapes.add_body(curva_kernel::Body::sphere([4200.0, -800.0, 900.0], 900.0).map_err(text)?);
    let outline = curva_kernel::Curve::polyline(
        vec![
            [-3000.0, -2500.0, 0.0],
            [1200.0, -3200.0, 0.0],
            [1500.0, 1800.0, 0.0],
            [-2600.0, 2600.0, 0.0],
        ],
        true,
    )
    .map_err(text)?;
    shapes.add_body(curva_kernel::Body::planar(&[outline]).map_err(text)?);
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
        let (low, high) = outlines.bounds().ok_or("an empty scene")?;
        camera.frame(low, high);
        camera.zoom(zoom);
        let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
        let (curva, _) = picture_of(doc, &outlines, &camera, aspect, Style::SHADED);
        save_picture(
            curva,
            PICTURE_SIZE,
            &out.join(format!("{scene}-0-curva.png")),
        )?;
        // Bevy's surfaces with Curva's lines, numbered as the window numbers them.
        let parts = curva_viewport::SceneParts::of(&outlines);
        let camera = camera.measured_from(parts.origin);
        for (index, (name, look)) in looks.iter().enumerate() {
            let picture = Picture {
                lines: parts.shaded_lines.clone(),
                faces: parts.faces.clone(),
                matrix: camera.matrix(aspect),
                light: camera.forward().map(|value| value as f32),
                ..Picture::default()
            };
            let ms = curva_viewport::save_bevy_picture(
                picture,
                parts.objects.clone(),
                camera,
                *look,
                RHINO_GREY,
                PICTURE_SIZE,
                &out.join(format!("{scene}-{}-{name}.png", index + 1)),
            )?;
            println!("{scene} {name}: {ms:.2} ms per frame");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixels of a PNG file's bytes, four a pixel; `None` if they are not one.
    fn pixels_of(png_bytes: &[u8]) -> Option<Vec<u8>> {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        let mut reader = decoder.read_info().ok()?;
        let mut pixels = vec![0; reader.output_buffer_size()?];
        reader.next_frame(&mut pixels).ok()?;
        Some(pixels)
    }

    /// The *Far* check on pictures for agents (#225): a circle and a box 500 km from the origin
    /// picture as they do near it, as the window draws them (2 m out, clear of the grid round the
    /// origin, which a far model's picture does not show).
    #[test]
    fn a_far_model_pictures_as_at_the_origin() {
        let picture = |offset: Point| {
            let at = |p: Point| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
            let mut doc = Document::default();
            doc.add(Curve::circle(at([0.0, 0.0, 0.0]), 5.0).unwrap());
            doc.add_body(
                curva_kernel::Body::cuboid(at([-3.0, -3.0, 0.0]), [6.0, 6.0, 4.0]).unwrap(),
            );
            mcp_picture(&doc).ok().and_then(|bytes| pixels_of(&bytes))
        };
        let (Some(near), Some(far)) =
            (picture([2000.0, 2000.0, 0.0]), picture([5.0e8, 5.0e8, 0.0]))
        else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        let differing = near
            .chunks(4)
            .zip(far.chunks(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 6))
            .count();
        assert!(differing <= 50, "{differing} pixels differ");
    }
}
