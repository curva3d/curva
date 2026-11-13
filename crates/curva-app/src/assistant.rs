//! The AI Assistant (spec `docs/specs/ai-assistant.md`): a chat in its panel that works on the
//! model, with a model from the cloud or this machine. It reaches Curva through the same MCP tools
//! an external assistant uses, by running Curva's own MCP bridge, so every change goes through
//! Agent access and its approvals. Model services are reached through the system's `curl`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

/// Where LM Studio's local server listens by default.
pub const LM_STUDIO: &str = "http://localhost:1234/v1";
/// OpenAI's chat endpoint.
const OPENAI: &str = "https://api.openai.com/v1/chat/completions";
/// Anthropic's messages endpoint.
const CLAUDE: &str = "https://api.anthropic.com/v1/messages";
/// How many rounds of tool calls one request may take before it stops.
const MOST_ROUNDS: usize = 12;
/// What the assistant is told it is.
const SYSTEM: &str = "You are the AI Assistant inside Curva, a NURBS 3D modelling program like Rhino. \
Use the tools to look at and change the model; run Curva commands as a user would type them. \
Lengths are in the model's unit. Keep replies short.";

/// Which model service the assistant uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    /// A ChatGPT account, through OpenAI's Codex command-line tool.
    ChatGpt,
    /// A model on this machine, through LM Studio's local server.
    #[default]
    LmStudio,
    /// OpenAI, with an API key.
    OpenAi,
    /// Claude, with an API key.
    Claude,
}

impl Provider {
    /// How it shows in the panel's header.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ChatGpt => "ChatGPT · account",
            Self::LmStudio => "LM Studio · local",
            Self::OpenAi => "OpenAI · API key",
            Self::Claude => "Claude · API key",
        }
    }

    /// The model it starts with.
    #[must_use]
    pub fn standard_model(self) -> &'static str {
        match self {
            Self::ChatGpt => "gpt-5",
            Self::LmStudio => "",
            Self::OpenAi => "gpt-5",
            Self::Claude => "claude-sonnet-5-5",
        }
    }
}

/// The assistant's connection, kept in the settings. Keys are not kept here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    /// The service.
    pub provider: Provider,
    /// The model.
    pub model: String,
    /// Whether a connection has been made.
    pub connected: bool,
}

/// One line of the conversation as shown.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// What the user asked.
    User(String),
    /// What the assistant answered.
    Assistant(String),
    /// A tool the assistant used, by name.
    Tool(String),
    /// Something that went wrong.
    Error(String),
}

/// Where the API keys are kept: a file of their own in Curva's folder, readable only by the user.
fn keys_file() -> Option<std::path::PathBuf> {
    Some(crate::recovery::app_folder()?.join("assistant-keys.json"))
}

/// The key kept for a service, if any.
#[must_use]
pub fn key(provider: Provider) -> Option<String> {
    let text = std::fs::read_to_string(keys_file()?).ok()?;
    let keys: Value = serde_json::from_str(&text).ok()?;
    keys[format!("{provider:?}")].as_str().map(str::to_owned)
}

/// Keeps a service's key.
pub fn set_key(provider: Provider, value: &str) -> Result<(), String> {
    let file = keys_file().ok_or_else(|| "no folder for Curva's settings".to_owned())?;
    let mut keys: Value = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    keys[format!("{provider:?}")] = json!(value);
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, keys.to_string()).map_err(|e| e.to_string())?;
    restrict(&file);
    Ok(())
}

#[cfg(unix)]
fn restrict(file: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_file: &std::path::Path) {}

/// Sends a JSON body to a web service with the system's curl, and reads the JSON answer.
fn post(url: &str, headers: &[String], body: &Value) -> Result<Value, String> {
    let mut command = crate::quiet_command("curl");
    command.args([
        "-sS",
        "--max-time",
        "180",
        "-X",
        "POST",
        url,
        "-H",
        "Content-Type: application/json",
    ]);
    for header in headers {
        command.args(["-H", header]);
    }
    command.args(["--data-binary", "@-"]);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl could not run: {e}"))?;
    if let Some(mut input) = child.stdin.take() {
        input
            .write_all(body.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let answer: Value = serde_json::from_slice(&output.stdout).map_err(|_| {
        String::from_utf8_lossy(&output.stdout)
            .chars()
            .take(300)
            .collect::<String>()
    })?;
    if let Some(error) = answer.get("error") {
        return Err(error["message"]
            .as_str()
            .unwrap_or("the service refused the request")
            .to_owned());
    }
    Ok(answer)
}

/// The models LM Studio has, from its local server.
pub fn local_models() -> Result<Vec<String>, String> {
    let output = crate::quiet_command("curl")
        .args(["-sS", "--max-time", "5", &format!("{LM_STUDIO}/models")])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(
            "LM Studio's server is not running. Start it in LM Studio, then try again.".to_owned(),
        );
    }
    let list: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(list["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// Curva's MCP tools, reached through the running Curva's bridge.
struct Tools {
    child: std::process::Child,
    reader: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Tools {
    fn open() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = crate::quiet_command(exe)
            .args(["--mcp", "--connect"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().ok_or("no output from the bridge")?;
        let mut tools = Self {
            child,
            reader: BufReader::new(stdout),
            next: 1,
        };
        tools.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "Curva AI Assistant", "version": env!("CARGO_PKG_VERSION")}}))?;
        Ok(tools)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let input = self.child.stdin.as_mut().ok_or("the bridge closed")?;
        writeln!(input, "{message}").map_err(|e| e.to_string())?;
        input.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        loop {
            line.clear();
            if self
                .reader
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                return Err("Curva did not answer. Turn on Agent access in this panel.".to_owned());
            }
            let answer: Value = serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;
            if answer["id"] == json!(id) {
                if let Some(error) = answer.get("error") {
                    return Err(error["message"].as_str().unwrap_or("tool error").to_owned());
                }
                return Ok(answer["result"].clone());
            }
        }
    }

    fn list(&mut self) -> Result<Vec<Value>, String> {
        Ok(self.request("tools/list", json!({}))?["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Runs a tool and gives its result as text for the model.
    fn call(&mut self, name: &str, arguments: Value) -> String {
        match self.request("tools/call", json!({"name": name, "arguments": arguments})) {
            Ok(result) => result["content"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| result.to_string()),
            Err(error) => format!("Error: {error}"),
        }
    }
}

impl Drop for Tools {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Answers one request: asks the model, runs the tools it calls, and returns the lines to show.
/// `history` holds the conversation so far as (user, assistant) turns.
pub fn answer(connection: &Connection, history: &[(String, String)], request: &str) -> Vec<Line> {
    let result = match connection.provider {
        Provider::ChatGpt => answer_through_codex(connection, request),
        Provider::Claude => answer_claude(connection, history, request),
        Provider::LmStudio | Provider::OpenAi => answer_openai(connection, history, request),
    };
    result.unwrap_or_else(|error| vec![Line::Error(error)])
}

fn answer_openai(
    connection: &Connection,
    history: &[(String, String)],
    request: &str,
) -> Result<Vec<Line>, String> {
    let mut tools = Tools::open()?;
    let schema: Vec<Value> = tools
        .list()?
        .iter()
        .map(|tool| json!({"type": "function", "function": {"name": tool["name"], "description": tool["description"], "parameters": tool["inputSchema"]}}))
        .collect();
    let (url, headers) = if connection.provider == Provider::LmStudio {
        (format!("{LM_STUDIO}/chat/completions"), Vec::new())
    } else {
        let key = key(Provider::OpenAi).ok_or("Add an OpenAI API key first.")?;
        (
            OPENAI.to_owned(),
            vec![format!("Authorization: Bearer {key}")],
        )
    };
    let mut messages = vec![json!({"role": "system", "content": SYSTEM})];
    for (user, assistant) in history {
        messages.push(json!({"role": "user", "content": user}));
        messages.push(json!({"role": "assistant", "content": assistant}));
    }
    messages.push(json!({"role": "user", "content": request}));
    let mut shown = Vec::new();
    for _ in 0..MOST_ROUNDS {
        let mut body = json!({"model": connection.model, "messages": messages, "tools": schema});
        if schema.is_empty() {
            body.as_object_mut().map(|b| b.remove("tools"));
        }
        let reply = post(&url, &headers, &body)?;
        let message = reply["choices"][0]["message"].clone();
        let calls = message["tool_calls"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        messages.push(message.clone());
        if calls.is_empty() {
            shown.push(Line::Assistant(
                message["content"].as_str().unwrap_or("").trim().to_owned(),
            ));
            return Ok(shown);
        }
        for call in calls {
            let name = call["function"]["name"].as_str().unwrap_or("").to_owned();
            let arguments: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or_else(|| json!({}));
            shown.push(Line::Tool(name.clone()));
            let result = tools.call(&name, arguments);
            messages.push(json!({"role": "tool", "tool_call_id": call["id"], "content": result}));
        }
    }
    shown.push(Line::Error(
        "The assistant took too many steps and stopped.".to_owned(),
    ));
    Ok(shown)
}

fn answer_claude(
    connection: &Connection,
    history: &[(String, String)],
    request: &str,
) -> Result<Vec<Line>, String> {
    let key = key(Provider::Claude).ok_or("Add a Claude API key first.")?;
    let mut tools = Tools::open()?;
    let schema: Vec<Value> = tools
        .list()?
        .iter()
        .map(|tool| json!({"name": tool["name"], "description": tool["description"], "input_schema": tool["inputSchema"]}))
        .collect();
    let headers = vec![
        format!("x-api-key: {key}"),
        "anthropic-version: 2023-06-01".to_owned(),
    ];
    let mut messages = Vec::new();
    for (user, assistant) in history {
        messages.push(json!({"role": "user", "content": user}));
        messages.push(json!({"role": "assistant", "content": assistant}));
    }
    messages.push(json!({"role": "user", "content": request}));
    let mut shown = Vec::new();
    for _ in 0..MOST_ROUNDS {
        let body = json!({"model": connection.model, "max_tokens": 4096, "system": SYSTEM, "tools": schema, "messages": messages});
        let reply = post(CLAUDE, &headers, &body)?;
        let content = reply["content"].as_array().cloned().unwrap_or_default();
        messages.push(json!({"role": "assistant", "content": content}));
        let uses: Vec<&Value> = content
            .iter()
            .filter(|block| block["type"] == "tool_use")
            .collect();
        if uses.is_empty() {
            let text: Vec<&str> = content.iter().filter_map(|b| b["text"].as_str()).collect();
            shown.push(Line::Assistant(text.join("\n").trim().to_owned()));
            return Ok(shown);
        }
        let mut results = Vec::new();
        for block in uses {
            let name = block["name"].as_str().unwrap_or("").to_owned();
            shown.push(Line::Tool(name.clone()));
            let result = tools.call(&name, block["input"].clone());
            results.push(
                json!({"type": "tool_result", "tool_use_id": block["id"], "content": result}),
            );
        }
        messages.push(json!({"role": "user", "content": results}));
    }
    shown.push(Line::Error(
        "The assistant took too many steps and stopped.".to_owned(),
    ));
    Ok(shown)
}

/// A ChatGPT account, through OpenAI's Codex command-line tool, which signs in with the account
/// and reaches Curva through the same MCP bridge.
fn answer_through_codex(connection: &Connection, request: &str) -> Result<Vec<Line>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.display().to_string().replace('\\', "\\\\");
    let output = crate::quiet_command(codex())
        .args([
            "exec",
            "--json",
            "--skip-git-repo-check",
            "-m",
            &connection.model,
            "-c",
            &format!("mcp_servers.curva.command=\"{exe}\""),
            "-c",
            "mcp_servers.curva.args=[\"--mcp\",\"--connect\"]",
            &format!("{SYSTEM}\n\n{request}"),
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "Codex is not installed. Install it to use a ChatGPT account.".to_owned())?;
    let mut shown = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let item = &event["item"];
        match item["type"].as_str() {
            Some("mcp_tool_call") => shown.push(Line::Tool(
                item["tool"].as_str().unwrap_or("tool").to_owned(),
            )),
            Some("agent_message") if event["type"] == "item.completed" => {
                shown.push(Line::Assistant(
                    item["text"].as_str().unwrap_or("").to_owned(),
                ));
            }
            _ => {}
        }
    }
    if shown.is_empty() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if error.is_empty() {
            "Codex gave no answer. Sign in first.".to_owned()
        } else {
            error
        });
    }
    Ok(shown)
}

/// The Codex command.
fn codex() -> &'static str {
    if cfg!(target_os = "windows") {
        "codex.cmd"
    } else {
        "codex"
    }
}

/// Starts Codex's ChatGPT sign-in, which opens the browser.
pub fn sign_in() -> Result<(), String> {
    crate::quiet_command(codex())
        .arg("login")
        .spawn()
        .map(|_| ())
        .map_err(|_| {
            "Codex is not installed. Install it from developers.openai.com/codex/cli.".to_owned()
        })
}

/// What an external assistant's MCP settings need, to connect to this Curva.
#[must_use]
pub fn mcp_settings() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "curva".to_owned());
    serde_json::to_string_pretty(&json!({
        "mcpServers": {"curva": {"command": exe, "args": ["--mcp", "--connect"]}}
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mcp_settings_name_this_curva_and_the_bridge() {
        let settings: Value = serde_json::from_str(&mcp_settings()).unwrap();
        assert_eq!(
            settings["mcpServers"]["curva"]["args"],
            json!(["--mcp", "--connect"])
        );
    }

    #[test]
    fn every_service_starts_with_a_model_but_local_ones_are_chosen() {
        assert!(Provider::LmStudio.standard_model().is_empty());
        assert!(!Provider::Claude.standard_model().is_empty());
    }
}
