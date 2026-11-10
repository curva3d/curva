//! Curva's MCP server: lets an AI agent read the model and run the same commands a person types.
//!
//! The protocol is JSON-RPC, one message per line. The code that handles a message does no
//! input or output of its own, so every tool can be checked without a real client.

use curva_commands::Processor;
use curva_commands::commands::resolve;
use curva_document::Document;
use curva_kernel::Curve;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

mod live;

pub use live::{AgentRequest, Listener, bridge_to_window, listen};

const PROTOCOL_VERSION: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Draws the model and returns a PNG file's bytes.
pub type Renderer = fn(&Document) -> Result<Vec<u8>, String>;

/// Draws the model for one request, perhaps as a window currently shows it.
pub type Drawing<'a> = &'a dyn Fn(&Document) -> Result<Vec<u8>, String>;

/// A request working on a model lent to it.
pub struct Session<'a> {
    doc: &'a mut Document,
    processor: &'a mut Processor,
    renderer: Drawing<'a>,
}

/// A model being worked on by an agent.
pub struct Server {
    doc: Document,
    processor: Processor,
    renderer: Renderer,
}

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .fold(0_u32, |all, byte| (all << 8) | u32::from(*byte))
            << (8 * (3 - chunk.len()));
        for place in 0..4 {
            if place <= chunk.len() {
                let index = (value >> (18 - 6 * place)) & 63;
                out.push(char::from(BASE64[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn kind_of(curve: &Curve) -> &'static str {
    match curve {
        Curve::Point { .. } => "point",
        Curve::Line { .. } => "line",
        Curve::Polyline { .. } => "polyline",
        Curve::Circle { .. } => "circle",
        Curve::Arc { .. } => "arc",
        Curve::Interpolated { .. } | Curve::Nurbs { .. } => "smooth curve",
        Curve::Compound { .. } => "joined curve",
    }
}

fn text_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn tool_error(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn tools() -> Value {
    let no_arguments = json!({ "type": "object", "properties": {} });
    json!([
        {
            "name": "run_commands",
            "description": "Runs lines on the Curva command line, exactly as a person would type them, for example [\"line\", \"0,0\", \"10,0\"]. The whole call is one undo step. Returns what Curva reported, and the prompt if a command is still waiting for an answer.",
            "inputSchema": {
                "type": "object",
                "properties": { "lines": { "type": "array", "items": { "type": "string" } } },
                "required": ["lines"]
            }
        },
        {
            "name": "list_objects",
            "description": "Lists every object in the model with its id, kind, layer, length and whether it is hidden or selected.",
            "inputSchema": no_arguments
        },
        {
            "name": "list_layers",
            "description": "Lists the layers of the model.",
            "inputSchema": no_arguments
        },
        {
            "name": "select_objects",
            "description": "Replaces the selection with these object ids, for commands that act on the selection.",
            "inputSchema": {
                "type": "object",
                "properties": { "ids": { "type": "array", "items": { "type": "integer" } } },
                "required": ["ids"]
            }
        },
        {
            "name": "render_view",
            "description": "Returns a picture of the model, so that an agent can check its own work. In a Curva window it shows the last-used viewport as it is; otherwise the whole model is fitted in a perspective view.",
            "inputSchema": no_arguments
        }
    ])
}

impl<'a> Session<'a> {
    /// Lends a model to an agent's request.
    pub fn new(doc: &'a mut Document, processor: &'a mut Processor, renderer: Drawing<'a>) -> Self {
        Self {
            doc,
            processor,
            renderer,
        }
    }

    fn run_commands(&mut self, arguments: &Value) -> Result<Value, String> {
        let lines = arguments
            .get("lines")
            .and_then(Value::as_array)
            .ok_or("lines must be a list of strings")?;
        let lines: Vec<&str> = lines
            .iter()
            .map(|line| line.as_str().ok_or("lines must be a list of strings"))
            .collect::<Result<_, _>>()?;
        let from = self.processor.log().len();
        let mark = self.doc.undo_mark();
        let before = self.doc.state().clone();
        for line in lines {
            self.processor.submit(self.doc, line);
        }
        self.doc.squash_undo(mark, before);
        let mut said = self.processor.log()[from..].join("\n");
        if let Some(prompt) = self.processor.prompt() {
            if !said.is_empty() {
                said.push('\n');
            }
            said.push_str("Waiting for: ");
            said.push_str(&prompt);
        }
        Ok(text_result(&said))
    }

    fn list_objects(&self) -> Value {
        let selected = self.doc.selection();
        let objects: Vec<Value> = self
            .doc
            .state()
            .objects
            .iter()
            .map(|object| {
                json!({
                    "id": object.id,
                    "kind": kind_of(&object.curve),
                    "layer": object.layer,
                    "length": object.curve.length().ok(),
                    "hidden": object.hidden,
                    "selected": selected.contains(&object.id),
                })
            })
            .collect();
        text_result(&Value::Array(objects).to_string())
    }

    fn list_layers(&self) -> Value {
        let layers: Vec<Value> = self
            .doc
            .layers()
            .iter()
            .map(|layer| {
                json!({
                    "id": layer.id,
                    "name": layer.name,
                    "visible": layer.visible,
                    "locked": layer.locked,
                    "current": layer.id == self.doc.current_layer(),
                })
            })
            .collect();
        text_result(&Value::Array(layers).to_string())
    }

    fn select_objects(&mut self, arguments: &Value) -> Result<Value, String> {
        let ids = arguments
            .get("ids")
            .and_then(Value::as_array)
            .ok_or("ids must be a list of object ids")?;
        let ids: Vec<u64> = ids
            .iter()
            .map(|id| id.as_u64().ok_or("ids must be a list of object ids"))
            .collect::<Result<_, _>>()?;
        self.doc.select(&ids);
        Ok(text_result(&format!(
            "Selected {} objects.",
            self.doc.selection().len()
        )))
    }

    fn render_view(&self) -> Result<Value, String> {
        let png = (self.renderer)(self.doc)?;
        Ok(json!({
            "content": [{ "type": "image", "data": base64(&png), "mimeType": "image/png" }]
        }))
    }

    fn call_tool(&mut self, params: &Value) -> Value {
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let outcome = match params.get("name").and_then(Value::as_str) {
            Some("run_commands") => self.run_commands(&arguments),
            Some("list_objects") => Ok(self.list_objects()),
            Some("list_layers") => Ok(self.list_layers()),
            Some("select_objects") => self.select_objects(&arguments),
            Some("render_view") => self.render_view(),
            other => Err(format!("unknown tool: {}", other.unwrap_or("(none)"))),
        };
        outcome.unwrap_or_else(|error| tool_error(&error))
    }

    /// Answers one message. Notifications have no answer.
    pub fn handle(&mut self, message: &Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned()?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => json!({
                "protocolVersion": params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "curva", "version": env!("CARGO_PKG_VERSION") },
            }),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": tools() }),
            "tools/call" => self.call_tool(&params),
            _ => {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": METHOD_NOT_FOUND, "message": "unknown method" },
                }));
            }
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

/// The commands that an agent cannot run on the open window without the user's say-so.
const NEEDS_APPROVAL: [&str; 4] = ["Delete", "New", "Open", "Save"];

/// If this message would do something the user should approve first, the line that would do it.
#[must_use]
pub fn needs_approval(message: &Value) -> Option<String> {
    if message.get("method").and_then(Value::as_str) != Some("tools/call") {
        return None;
    }
    let params = message.get("params")?;
    if params.get("name").and_then(Value::as_str) != Some("run_commands") {
        return None;
    }
    params
        .get("arguments")?
        .get("lines")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|line| resolve(line.trim()).is_some_and(|def| NEEDS_APPROVAL.contains(&def.name)))
        .map(str::to_owned)
}

/// The answer to a message the user declined.
#[must_use]
pub fn declined(message: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": message.get("id").cloned().unwrap_or(Value::Null),
        "result": tool_error("The user declined this action."),
    })
}

impl Server {
    /// A server on an empty model.
    #[must_use]
    pub fn new(renderer: Renderer) -> Self {
        Self {
            doc: Document::default(),
            processor: Processor::default(),
            renderer,
        }
    }

    /// Answers one message. Notifications have no answer.
    pub fn handle(&mut self, message: &Value) -> Option<Value> {
        Session {
            doc: &mut self.doc,
            processor: &mut self.processor,
            renderer: &self.renderer,
        }
        .handle(message)
    }

    /// Answers messages from `input`, one per line, until it ends.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let answer = match serde_json::from_str::<Value>(&line) {
                Ok(message) => self.handle(&message),
                Err(error) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": INVALID_PARAMS, "message": error.to_string() },
                })),
            };
            if let Some(answer) = answer {
                writeln!(output, "{answer}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_picture(_doc: &Document) -> Result<Vec<u8>, String> {
        Ok(vec![1, 2, 3])
    }

    fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
        let message = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        });
        server.handle(&message).unwrap()["result"].clone()
    }

    fn text_of(result: &Value) -> String {
        result["content"][0]["text"].as_str().unwrap().to_owned()
    }

    fn server() -> Server {
        Server::new(fake_picture)
    }

    #[test]
    fn it_introduces_itself_with_the_clients_protocol() {
        let answer = server()
            .handle(&json!({
                "jsonrpc": "2.0", "id": 7, "method": "initialize",
                "params": { "protocolVersion": "2025-03-26" },
            }))
            .unwrap();
        assert_eq!(answer["id"], 7);
        assert_eq!(answer["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(answer["result"]["serverInfo"]["name"], "curva");
    }

    #[test]
    fn notifications_get_no_answer() {
        let message = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(server().handle(&message).is_none());
    }

    #[test]
    fn it_lists_its_tools() {
        let answer = server()
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
            .unwrap();
        let names: Vec<&str> = answer["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"run_commands") && names.contains(&"render_view"));
    }

    #[test]
    fn an_unknown_method_is_an_error() {
        let answer = server()
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "nonsense" }))
            .unwrap();
        assert_eq!(answer["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn commands_change_the_model_and_report_back() {
        let mut server = server();
        let result = call(
            &mut server,
            "run_commands",
            json!({ "lines": ["line", "0,0", "10,0"] }),
        );
        assert!(text_of(&result).contains("Created a line"));
        let listed: Value =
            serde_json::from_str(&text_of(&call(&mut server, "list_objects", json!({})))).unwrap();
        assert_eq!(listed[0]["kind"], "line");
        assert_eq!(listed[0]["length"], 10.0);
    }

    #[test]
    fn a_command_still_waiting_says_what_it_needs() {
        let mut server = server();
        let result = call(&mut server, "run_commands", json!({ "lines": ["line"] }));
        assert!(text_of(&result).contains("Waiting for: Start of line"));
    }

    #[test]
    fn a_whole_call_is_one_undo_step() {
        let mut server = server();
        let lines = json!({ "lines": ["line", "0,0", "1,0", "line", "0,0", "0,1"] });
        call(&mut server, "run_commands", lines);
        call(&mut server, "run_commands", json!({ "lines": ["undo"] }));
        let listed: Value =
            serde_json::from_str(&text_of(&call(&mut server, "list_objects", json!({})))).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 0);
    }

    #[test]
    fn selecting_then_running_a_command_acts_on_the_selection() {
        let mut server = server();
        call(
            &mut server,
            "run_commands",
            json!({ "lines": ["line", "0,0", "1,0"] }),
        );
        call(&mut server, "select_objects", json!({ "ids": [0] }));
        call(&mut server, "run_commands", json!({ "lines": ["delete"] }));
        let listed: Value =
            serde_json::from_str(&text_of(&call(&mut server, "list_objects", json!({})))).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 0);
    }

    #[test]
    fn bad_arguments_are_a_tool_error_not_a_crash() {
        let mut server = server();
        let result = call(&mut server, "run_commands", json!({ "lines": "line" }));
        assert_eq!(result["isError"], true);
        let result = call(&mut server, "no_such_tool", json!({}));
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn a_picture_comes_back_as_an_image() {
        let result = call(&mut server(), "render_view", json!({}));
        assert_eq!(result["content"][0]["type"], "image");
        assert_eq!(result["content"][0]["data"], "AQID");
    }

    #[test]
    fn layers_are_listed() {
        let listed: Value =
            serde_json::from_str(&text_of(&call(&mut server(), "list_layers", json!({})))).unwrap();
        assert_eq!(listed[0]["current"], true);
    }

    fn run_message(lines: Value) -> Value {
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "run_commands", "arguments": { "lines": lines } },
        })
    }

    #[test]
    fn destructive_commands_need_approval() {
        for line in ["delete", "del", "NEW", "open", "save"] {
            assert_eq!(
                needs_approval(&run_message(json!([line]))).as_deref(),
                Some(line)
            );
        }
    }

    #[test]
    fn a_destructive_command_is_found_among_others() {
        let message = run_message(json!(["line", "0,0", "1,0", "delete"]));
        assert_eq!(needs_approval(&message).as_deref(), Some("delete"));
    }

    #[test]
    fn ordinary_commands_and_reading_need_no_approval() {
        assert_eq!(
            needs_approval(&run_message(json!(["line", "0,0", "1,0"]))),
            None
        );
        let listing = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "list_objects", "arguments": {} },
        });
        assert_eq!(needs_approval(&listing), None);
    }

    #[test]
    fn a_declined_action_is_reported_as_a_tool_error_with_its_id() {
        let answer = declined(&run_message(json!(["delete"])));
        assert_eq!(answer["id"], 4);
        assert_eq!(answer["result"]["isError"], true);
    }

    #[test]
    fn a_session_works_on_a_document_it_is_lent() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        let mut session = Session::new(&mut doc, &mut processor, &fake_picture);
        session.handle(&run_message(json!(["line", "0,0", "1,0"])));
        assert_eq!(doc.state().objects.len(), 1);
    }
    #[test]
    fn base64_pads_correctly() {
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }

    #[test]
    fn it_serves_one_answer_per_line_and_survives_bad_input() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nnot json\n";
        let mut output = Vec::new();
        server().serve(&input[..], &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"id\":1"));
        assert!(lines[1].contains("error"));
    }
}
