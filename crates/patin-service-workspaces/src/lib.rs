//! Optional 0xin workspace-state provider for Patin shells.
//!
//! Polls 0xin's documented control socket (`0xin-control-<display>.sock`,
//! plain-text `workspaces` query) for the currently focused workspace and
//! each workspace's occupied/empty state. Patin cannot depend on the 0xin
//! binary crate, so the socket-path convention and protocol are reproduced
//! here from 0xin's own `src/control.rs` and `src/bin/0xinctl.rs`. Socket
//! absence (0xin not running, a different compositor, or a slow/stuck
//! server) degrades to `None`, matching `patin-service-volume`'s
//! `Option<Snapshot>` convention.

use patin::service::Provider;
use std::env;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const SOCKET_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspacesSnapshot {
    /// 1-indexed, from the first output's `output NAME N` line.
    pub focused: usize,
    /// `occupied[i]` is workspace `i + 1`'s state.
    pub occupied: Vec<bool>,
}

pub struct WorkspacesProvider;

impl WorkspacesProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WorkspacesProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for WorkspacesProvider {
    type Snapshot = Option<WorkspacesSnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        query_workspaces()
    }
}

fn socket_path() -> Option<PathBuf> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")?;
    let display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "default".into());
    let safe_display = display.replace('/', "_");
    Some(PathBuf::from(runtime).join(format!("0xin-control-{safe_display}.sock")))
}

fn query_workspaces() -> Option<WorkspacesSnapshot> {
    let path = socket_path()?;
    let mut stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(SOCKET_TIMEOUT)).ok();
    stream.set_write_timeout(Some(SOCKET_TIMEOUT)).ok();
    stream.write_all(b"workspaces\n").ok()?;
    stream.shutdown(Shutdown::Write).ok();
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    parse_workspaces(&response)
}

/// Parses 0xin's `workspaces` query response:
/// ```text
/// ok
/// output DSI-1 1
/// workspace 1 occupied
/// workspace 2 empty
/// ```
fn parse_workspaces(response: &str) -> Option<WorkspacesSnapshot> {
    let mut lines = response.lines();
    if lines.next()? != "ok" {
        return None;
    }
    let mut focused = None;
    let mut occupied = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix("output ") {
            if focused.is_none() {
                focused = rest.rsplit(' ').next()?.parse::<usize>().ok();
            }
        } else if let Some(rest) = line.strip_prefix("workspace ") {
            let mut parts = rest.split(' ');
            let index = parts.next()?.parse::<usize>().ok()?;
            let state = parts.next()?;
            if index == 0 {
                return None;
            }
            if index > occupied.len() {
                occupied.resize(index, false);
            }
            occupied[index - 1] = state == "occupied";
        }
    }
    Some(WorkspacesSnapshot {
        focused: focused.unwrap_or(1),
        occupied,
    })
}

#[cfg(test)]
mod tests {
    use super::{WorkspacesSnapshot, parse_workspaces};

    #[test]
    fn parses_a_typical_response() {
        let response = "ok\noutput DSI-1 1\nworkspace 1 occupied\nworkspace 2 empty\n";
        assert_eq!(
            parse_workspaces(response),
            Some(WorkspacesSnapshot {
                focused: 1,
                occupied: vec![true, false],
            })
        );
    }

    #[test]
    fn uses_only_the_first_output_line() {
        let response =
            "ok\noutput DSI-1 2\noutput HDMI-1 1\nworkspace 1 empty\nworkspace 2 occupied\n";
        assert_eq!(
            parse_workspaces(response),
            Some(WorkspacesSnapshot {
                focused: 2,
                occupied: vec![false, true],
            })
        );
    }

    #[test]
    fn missing_output_line_defaults_focused_to_one() {
        let response = "ok\nworkspace 1 occupied\n";
        assert_eq!(
            parse_workspaces(response),
            Some(WorkspacesSnapshot {
                focused: 1,
                occupied: vec![true],
            })
        );
    }

    #[test]
    fn error_responses_are_rejected() {
        assert_eq!(parse_workspaces("error session is locked\n"), None);
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(parse_workspaces(""), None);
    }

    #[test]
    fn malformed_workspace_lines_are_rejected() {
        assert_eq!(
            parse_workspaces("ok\nworkspace not-a-number occupied\n"),
            None
        );
    }
}
