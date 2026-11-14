//! The AI Assistant panel: its connections and its chat.

use crate::*;

/// The AI Assistant panel's chat and connection choices.
#[derive(Default)]
pub(crate) struct AssistantPanel {
    /// The AI Assistant's conversation as shown.
    pub(crate) assistant_lines: Vec<assistant::Line>,
    /// The conversation so far, as (request, answer) turns, for the model.
    pub(crate) assistant_history: Vec<(String, String)>,
    /// What is being typed to the assistant.
    pub(crate) assistant_input: String,
    /// The answer being worked out.
    pub(crate) assistant_busy: Option<std::sync::mpsc::Receiver<(String, Vec<assistant::Line>)>>,
    /// The way of connecting chosen, before it connects.
    pub(crate) assistant_choice: Option<assistant::Provider>,
    /// An API key being typed.
    pub(crate) assistant_key: String,
    /// The models LM Studio offers.
    pub(crate) assistant_models: Vec<String>,
    /// Why connecting did not work.
    pub(crate) assistant_error: Option<String>,
}

impl Curva {
    /// The AI Assistant panel: the choice of model service until one is connected, then the chat.
    pub(crate) fn assistant_panel(&mut self, ui: &mut egui::Ui) {
        self.receive_assistant();
        if self.settings.assistant.connected {
            self.assistant_chat(ui);
        } else {
            self.assistant_connections(ui);
        }
        ui.separator();
        self.agent_status(ui);
    }

    /// The ways to connect a model, as Serpentine3D offers them, and how to connect an outside
    /// assistant through MCP.
    pub(crate) fn assistant_connections(&mut self, ui: &mut egui::Ui) {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(say("ui.assistant.connect"));
        ui.horizontal_wrapped(|ui| {
            for (provider, key) in [
                (assistant::Provider::ChatGpt, "ui.assistant.chatgpt"),
                (assistant::Provider::LmStudio, "ui.assistant.local"),
                (assistant::Provider::OpenAi, "ui.assistant.key"),
            ] {
                let chosen = self.assistant.assistant_choice == Some(provider)
                    || (provider == assistant::Provider::OpenAi
                        && self.assistant.assistant_choice == Some(assistant::Provider::Claude));
                if ui.selectable_label(chosen, say(key)).clicked() {
                    self.assistant.assistant_choice = Some(provider);
                    self.assistant.assistant_error = None;
                    if provider == assistant::Provider::LmStudio {
                        match assistant::local_models() {
                            Ok(models) => self.assistant.assistant_models = models,
                            Err(error) => self.assistant.assistant_error = Some(error),
                        }
                    }
                }
            }
        });
        let mut connect = None;
        match self.assistant.assistant_choice {
            Some(assistant::Provider::ChatGpt) => {
                ui.label(say("ui.assistant.chatgpt_note"));
                ui.horizontal(|ui| {
                    if ui.button(say("ui.assistant.sign_in")).clicked()
                        && let Err(error) = assistant::sign_in()
                    {
                        self.assistant.assistant_error = Some(error);
                    }
                    if ui.button(say("ui.assistant.use")).clicked() {
                        connect = Some((
                            assistant::Provider::ChatGpt,
                            assistant::Provider::ChatGpt.standard_model().to_owned(),
                        ));
                    }
                });
            }
            Some(assistant::Provider::LmStudio) => {
                if self.assistant.assistant_models.is_empty() {
                    ui.label(say("ui.assistant.no_models"));
                }
                for model in self.assistant.assistant_models.clone() {
                    if ui.button(&model).clicked() {
                        connect = Some((assistant::Provider::LmStudio, model));
                    }
                }
            }
            Some(provider @ (assistant::Provider::OpenAi | assistant::Provider::Claude)) => {
                ui.horizontal(|ui| {
                    let mut choice = provider;
                    ui.radio_value(&mut choice, assistant::Provider::OpenAi, "OpenAI");
                    ui.radio_value(&mut choice, assistant::Provider::Claude, "Claude");
                    self.assistant.assistant_choice = Some(choice);
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.assistant.assistant_key)
                        .password(true)
                        .hint_text(say("ui.assistant.key_hint")),
                );
                if ui.button(say("ui.assistant.use")).clicked() {
                    let provider = self.assistant.assistant_choice.unwrap_or(provider);
                    if !self.assistant.assistant_key.trim().is_empty() {
                        if let Err(error) =
                            assistant::set_key(provider, self.assistant.assistant_key.trim())
                        {
                            self.assistant.assistant_error = Some(error);
                        }
                        self.assistant.assistant_key.clear();
                    }
                    connect = Some((provider, provider.standard_model().to_owned()));
                }
            }
            None => {}
        }
        if let Some(error) = &self.assistant.assistant_error {
            ui.colored_label(theme::ACCENT, error);
        }
        // An outside assistant, through MCP.
        egui::CollapsingHeader::new(say("ui.assistant.mcp")).show(ui, |ui| {
            ui.label(say("ui.assistant.mcp_steps"));
            let mut settings = assistant::mcp_settings();
            ui.add(
                egui::TextEdit::multiline(&mut settings)
                    .code_editor()
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
            if ui.button(say("ui.assistant.copy")).clicked() {
                ui.ctx().copy_text(assistant::mcp_settings());
            }
        });
        if let Some((provider, model)) = connect {
            self.settings.assistant = assistant::Connection {
                provider,
                model,
                connected: true,
            };
            self.save_settings();
        }
    }

    /// The conversation, and the box to ask in.
    pub(crate) fn assistant_chat(&mut self, ui: &mut egui::Ui) {
        let connection = self.settings.assistant.clone();
        ui.horizontal(|ui| {
            ui.colored_label(
                theme::ACCENT,
                format!("{} · {}", connection.provider.label(), connection.model),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("⟲")
                    .on_hover_text(self.text.render(&Message::new("ui.assistant.change")))
                    .clicked()
                {
                    self.settings.assistant.connected = false;
                    self.save_settings();
                }
                if ui
                    .small_button("+")
                    .on_hover_text(self.text.render(&Message::new("ui.assistant.new")))
                    .clicked()
                {
                    self.assistant.assistant_lines.clear();
                    self.assistant.assistant_history.clear();
                }
            });
        });
        let height = (ui.available_height() - 64.0).max(60.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.assistant.assistant_lines {
                    match line {
                        assistant::Line::User(text) => {
                            ui.label(egui::RichText::new(text).strong());
                        }
                        assistant::Line::Assistant(text) => {
                            ui.label(text);
                        }
                        assistant::Line::Tool(name) => {
                            ui.colored_label(theme::current().quiet, format!("· {name}"));
                        }
                        assistant::Line::Error(text) => {
                            ui.colored_label(theme::ACCENT, text);
                        }
                    }
                }
                if self.assistant.assistant_busy.is_some() {
                    ui.colored_label(
                        theme::current().quiet,
                        self.text.render(&Message::new("ui.assistant.thinking")),
                    );
                }
            });
        let mut send = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.assistant.assistant_input)
                    .hint_text(self.text.render(&Message::new("ui.assistant.ask")))
                    .desired_width(ui.available_width() - 60.0),
            );
            send |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            send |= ui
                .add_enabled(
                    self.assistant.assistant_busy.is_none(),
                    egui::Button::new(self.text.render(&Message::new("ui.assistant.send"))),
                )
                .clicked();
        });
        if send
            && self.assistant.assistant_busy.is_none()
            && !self.assistant.assistant_input.trim().is_empty()
        {
            let request = std::mem::take(&mut self.assistant.assistant_input);
            self.assistant
                .assistant_lines
                .push(assistant::Line::User(request.clone()));
            // The assistant works on the model through Agent access.
            self.agent_on = true;
            let history = self.assistant.assistant_history.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let repaint = ui.ctx().clone();
            std::thread::spawn(move || {
                let lines = assistant::answer(&connection, &history, &request);
                let _ = sender.send((request, lines));
                repaint.request_repaint();
            });
            self.assistant.assistant_busy = Some(receiver);
        }
    }

    /// Takes the assistant's answer when it comes.
    pub(crate) fn receive_assistant(&mut self) {
        if let Some(receiver) = &self.assistant.assistant_busy
            && let Ok((request, lines)) = receiver.try_recv()
        {
            self.assistant.assistant_busy = None;
            let reply = lines
                .iter()
                .filter_map(|line| match line {
                    assistant::Line::Assistant(text) => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.assistant.assistant_history.push((request, reply));
            self.assistant.assistant_lines.extend(lines);
        }
    }
}
