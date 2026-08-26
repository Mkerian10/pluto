//! Hot-reload supervisor for the MCP server (#213).
//!
//! The default `pluto-mcp` process is a thin supervisor: it spawns the real
//! server as a child (`pluto-mcp --serve`), proxies newline-delimited
//! JSON-RPC between the client and the child, and caches the MCP handshake
//! (the `initialize` request and `notifications/initialized` notification).
//! When the binary on disk changes and no request is in flight, it restarts
//! the child and replays the handshake — so a rebuilt server is picked up
//! mid-session without the client noticing.
//!
//! The child's stderr is inherited, so logs still reach the terminal.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// How often to poll the binary's mtime for changes.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

struct ChildHandle {
    process: Child,
    stdin: ChildStdin,
    stdout: tokio::io::Lines<BufReader<ChildStdout>>,
    spawned_mtime: Option<SystemTime>,
}

fn binary_mtime(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

async fn spawn_child(exe: &PathBuf) -> anyhow::Result<ChildHandle> {
    let mut process = Command::new(exe)
        .arg("--serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let stdin = process.stdin.take().expect("piped stdin");
    let stdout = BufReader::new(process.stdout.take().expect("piped stdout")).lines();
    Ok(ChildHandle {
        process,
        stdin,
        stdout,
        spawned_mtime: binary_mtime(exe),
    })
}

/// Extract the JSON-RPC `id` of a message, if any (requests and responses
/// have one; notifications don't).
fn message_id(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .filter(|id| !id.is_null())
}

/// Extract the JSON-RPC `method` of a message, if any.
fn message_method(line: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("method").and_then(|m| m.as_str().map(String::from)))
}

/// Replay the cached handshake into a freshly spawned child: send the
/// `initialize` request, swallow the child's response to it, then send the
/// `notifications/initialized` notification.
async fn replay_handshake(
    child: &mut ChildHandle,
    initialize_line: &str,
    initialized_line: &str,
) -> anyhow::Result<()> {
    child.stdin.write_all(initialize_line.as_bytes()).await?;
    child.stdin.write_all(b"\n").await?;
    child.stdin.flush().await?;
    let init_id = message_id(initialize_line);
    // Swallow lines until the response to the initialize request appears
    // (the child shouldn't emit anything else first, but be tolerant).
    while let Some(line) = child.stdout.next_line().await? {
        if message_id(&line) == init_id {
            break;
        }
    }
    child.stdin.write_all(initialized_line.as_bytes()).await?;
    child.stdin.write_all(b"\n").await?;
    child.stdin.flush().await?;
    Ok(())
}

async fn restart_child(
    child: &mut ChildHandle,
    exe: &PathBuf,
    initialize_line: &str,
    initialized_line: &str,
) -> anyhow::Result<()> {
    let _ = child.process.kill().await;
    *child = spawn_child(exe).await?;
    replay_handshake(child, initialize_line, initialized_line).await?;
    tracing::info!("pluto-mcp: reloaded server child after binary change");
    Ok(())
}

pub async fn run() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut child = spawn_child(&exe).await?;

    let stdin = BufReader::new(tokio::io::stdin());
    let mut parent_lines = stdin.lines();
    let mut stdout = tokio::io::stdout();

    // Cached handshake for replay on restart.
    let mut initialize_line: Option<String> = None;
    let mut initialized_line: Option<String> = None;

    // Request ids currently awaiting a response from the child.
    let mut in_flight: HashSet<String> = HashSet::new();
    // Set when the binary changed while requests were in flight; the restart
    // happens as soon as the stream goes idle.
    let mut reload_pending = false;

    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            line = parent_lines.next_line() => {
                let Some(line) = line? else {
                    // Client closed stdin: shut down.
                    let _ = child.process.kill().await;
                    return Ok(());
                };
                if line.trim().is_empty() {
                    continue;
                }
                match message_method(&line).as_deref() {
                    Some("initialize") if initialize_line.is_none() => {
                        initialize_line = Some(line.clone());
                    }
                    Some("notifications/initialized") if initialized_line.is_none() => {
                        initialized_line = Some(line.clone());
                    }
                    _ => {}
                }
                if let Some(id) = message_id(&line) {
                    in_flight.insert(id.to_string());
                }
                child.stdin.write_all(line.as_bytes()).await?;
                child.stdin.write_all(b"\n").await?;
                child.stdin.flush().await?;
            }
            line = child.stdout.next_line() => {
                match line? {
                    Some(line) => {
                        if let Some(id) = message_id(&line) {
                            in_flight.remove(&id.to_string());
                        }
                        stdout.write_all(line.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                    None => {
                        // Child died. Respawn and replay the handshake if we
                        // have one; requests that were in flight are lost, but
                        // the session survives.
                        tracing::warn!("pluto-mcp: server child exited; respawning");
                        in_flight.clear();
                        if let (Some(init), Some(inited)) = (&initialize_line, &initialized_line) {
                            let (init, inited) = (init.clone(), inited.clone());
                            restart_child(&mut child, &exe, &init, &inited).await?;
                        } else {
                            child = spawn_child(&exe).await?;
                        }
                    }
                }
            }
            _ = poll.tick() => {
                let changed = binary_mtime(&exe) != child.spawned_mtime;
                if changed {
                    reload_pending = true;
                }
                if reload_pending && in_flight.is_empty() {
                    if let (Some(init), Some(inited)) = (&initialize_line, &initialized_line) {
                        let (init, inited) = (init.clone(), inited.clone());
                        restart_child(&mut child, &exe, &init, &inited).await?;
                        reload_pending = false;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_id_extracts_request_ids() {
        assert_eq!(
            message_id(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#),
            Some(serde_json::json!(1))
        );
        assert_eq!(
            message_id(r#"{"jsonrpc":"2.0","id":"abc","result":{}}"#),
            Some(serde_json::json!("abc"))
        );
    }

    #[test]
    fn message_id_ignores_notifications_and_null() {
        assert_eq!(
            message_id(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None
        );
        assert_eq!(message_id(r#"{"jsonrpc":"2.0","id":null,"method":"x"}"#), None);
        assert_eq!(message_id("not json"), None);
    }

    #[test]
    fn message_method_extracts() {
        assert_eq!(
            message_method(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#),
            Some("initialize".to_string())
        );
        assert_eq!(message_method(r#"{"jsonrpc":"2.0","id":0,"result":{}}"#), None);
    }
}
