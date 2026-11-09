//! The connection between an MCP agent and the Curva window that is open.
//!
//! The window listens on this computer only, and only while the user has switched it on. A client
//! proves it may connect with a secret kept in a file only this user's programs can normally read.

use serde_json::Value;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// How often the listener looks for a new client or a request to stop.
const POLL: Duration = Duration::from_millis(50);

/// One message from an agent, and where to send the answer.
pub struct AgentRequest {
    /// The JSON-RPC message.
    pub message: Value,
    /// Takes the answer, or `None` for a notification.
    pub answer: Sender<Option<Value>>,
}

/// The window's end of the connection. Dropping it stops listening.
pub struct Listener {
    /// Messages waiting for the window to answer them.
    pub requests: Receiver<AgentRequest>,
    stop: Arc<AtomicBool>,
    file: Option<PathBuf>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(file) = &self.file {
            // If this fails the next connection simply replaces the file.
            let _ = std::fs::remove_file(file);
        }
    }
}

/// Where the window leaves the port and secret for clients.
#[must_use]
pub fn connection_file() -> PathBuf {
    std::env::temp_dir().join("curva-mcp-connection")
}

/// A secret that cannot be guessed, from the random keys the standard library gets from the system.
fn new_secret() -> String {
    let part = || RandomState::new().build_hasher().finish();
    format!("{:016x}{:016x}", part(), part())
}

fn serve_client(
    stream: TcpStream,
    secret: &str,
    requests: &Sender<AgentRequest>,
    wake: &(impl Fn() + Send + Sync),
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut writer = stream.try_clone()?;
    let mut lines = BufReader::new(stream).lines();
    match lines.next() {
        Some(Ok(first)) if first.trim() == secret => {}
        _ => return Ok(()),
    }
    for line in lines {
        let Ok(message) = serde_json::from_str::<Value>(&line?) else {
            continue;
        };
        let (answer, wait) = channel();
        if requests.send(AgentRequest { message, answer }).is_err() {
            return Ok(());
        }
        wake();
        match wait.recv() {
            Ok(Some(reply)) => {
                writeln!(writer, "{reply}")?;
                writer.flush()?;
            }
            Ok(None) => {}
            Err(_) => return Ok(()),
        }
    }
    Ok(())
}

fn listen_with(
    wake: impl Fn() + Send + Sync + 'static,
    file: Option<PathBuf>,
) -> std::io::Result<(Listener, u16, String)> {
    let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    socket.set_nonblocking(true)?;
    let port = socket.local_addr()?.port();
    let secret = new_secret();
    if let Some(path) = &file {
        std::fs::write(path, format!("{port} {secret}"))?;
    }
    let (sender, requests) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let (thread_stop, thread_secret) = (Arc::clone(&stop), secret.clone());
    let wake = Arc::new(wake);
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            match socket.accept() {
                Ok((stream, _)) => {
                    let (sender, secret, wake) =
                        (sender.clone(), thread_secret.clone(), Arc::clone(&wake));
                    std::thread::spawn(move || {
                        // A client that goes away ends its own connection and nothing else.
                        let _ = serve_client(stream, &secret, &sender, &*wake);
                    });
                }
                Err(_) => std::thread::sleep(POLL),
            }
        }
    });
    Ok((
        Listener {
            requests,
            stop,
            file,
        },
        port,
        secret,
    ))
}

/// Starts listening for agents. `wake` is called when a message arrives, to redraw the window.
pub fn listen(wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Listener> {
    listen_with(wake, Some(connection_file())).map(|(listener, ..)| listener)
}

/// Connects standard input and output to the open Curva window, for MCP clients.
pub fn bridge_to_window() -> Result<(), String> {
    let text = std::fs::read_to_string(connection_file())
        .map_err(|_| "Curva is not listening. Tick Agent access in the Curva window.".to_owned())?;
    let mut parts = text.split_whitespace();
    let (Some(port), Some(secret)) = (parts.next(), parts.next()) else {
        return Err("the connection file is damaged".to_owned());
    };
    let port: u16 = port
        .parse()
        .map_err(|_| "the connection file is damaged".to_owned())?;
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).map_err(|e| e.to_string())?;
    let mut to_window = stream.try_clone().map_err(|e| e.to_string())?;
    writeln!(to_window, "{secret}").map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut out = std::io::stdout();
        // The copy ends when either side closes, which is the normal way to finish.
        let _ = std::io::copy(&mut reader, &mut out);
        std::process::exit(0);
    });
    let mut input = std::io::stdin().lock();
    std::io::copy(&mut input, &mut to_window).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn start() -> (Listener, u16, String) {
        listen_with(|| {}, None).unwrap()
    }

    fn connect(port: u16) -> TcpStream {
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
    }

    #[test]
    fn a_client_with_the_secret_reaches_the_window_and_gets_its_answer() {
        let (listener, port, secret) = start();
        let mut client = connect(port);
        writeln!(client, "{secret}").unwrap();
        writeln!(client, "{{\"id\":1,\"method\":\"ping\"}}").unwrap();
        let request = listener
            .requests
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(request.message["method"], "ping");
        request.answer.send(Some(json!({ "id": 1 }))).unwrap();
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).unwrap();
        assert!(line.contains("\"id\":1"));
    }

    #[test]
    fn a_client_without_the_secret_is_turned_away() {
        let (listener, port, _) = start();
        let mut client = connect(port);
        writeln!(client, "wrong").unwrap();
        writeln!(client, "{{\"id\":1,\"method\":\"ping\"}}").unwrap();
        let waited = listener.requests.recv_timeout(Duration::from_millis(400));
        assert!(waited.is_err());
    }

    #[test]
    fn secrets_are_different_each_time() {
        assert_ne!(new_secret(), new_secret());
        assert_eq!(new_secret().len(), 32);
    }

    #[test]
    fn it_only_listens_on_this_computer() {
        let (_listener, port, _) = start();
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok());
    }
}
