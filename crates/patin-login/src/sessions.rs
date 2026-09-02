//! Which session to start after signing in.
//!
//! Desktop environments advertise themselves as freedesktop desktop entries
//! in `wayland-sessions` directories; a greeter is expected to offer those as
//! choices rather than hard-coding one. Only the two fields a greeter needs
//! are read — the display name and the command — and entries that ask not to
//! be shown are skipped.

use std::fs;
use std::path::Path;

const SESSION_DIRS: [&str; 2] = [
    "/usr/local/share/wayland-sessions",
    "/usr/share/wayland-sessions",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub name: String,
    pub command: Vec<String>,
}

/// Every advertised session, plus `fallback` if nothing was found — a greeter
/// with an empty list could never log anyone in.
pub fn discover(fallback: &[String]) -> Vec<Session> {
    let mut sessions = Vec::new();
    for directory in SESSION_DIRS {
        collect_from(Path::new(directory), &mut sessions);
    }
    sessions.sort_by(|left, right| left.name.cmp(&right.name));
    if sessions.is_empty() && !fallback.is_empty() {
        sessions.push(Session {
            name: fallback
                .first()
                .and_then(|program| program.rsplit('/').next())
                .unwrap_or("Session")
                .to_string(),
            command: fallback.to_vec(),
        });
    }
    sessions
}

fn collect_from(directory: &Path, sessions: &mut Vec<Session>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
            && let Ok(text) = fs::read_to_string(&path)
            && let Some(session) = parse_desktop_entry(&text)
            && !sessions.iter().any(|known| known.name == session.name)
        {
            sessions.push(session);
        }
    }
}

/// The `Name` and `Exec` of a desktop entry, or `None` when it is hidden or
/// has no runnable command.
fn parse_desktop_entry(text: &str) -> Option<Session> {
    let mut name = None;
    let mut exec = None;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Only the main group describes the session; later groups are
            // desktop actions with their own Name/Exec pairs.
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            // Localised variants (`Name[nl]`) are ignored: an exact match
            // keeps the greeter's list stable regardless of its locale.
            "Name" => name = Some(value.trim().to_string()),
            "Exec" => exec = Some(value.trim().to_string()),
            "Hidden" | "NoDisplay" if value.trim() == "true" => return None,
            _ => {}
        }
    }
    let command = strip_field_codes(&exec?);
    (!command.is_empty()).then(|| Session {
        name: name.unwrap_or_else(|| command[0].clone()),
        command,
    })
}

/// Desktop `Exec` lines may carry `%`-prefixed field codes (`%U`, `%i`, …)
/// that only make sense when launching a file handler; a session command must
/// not receive them as arguments.
fn strip_field_codes(exec: &str) -> Vec<String> {
    exec.split_whitespace()
        .filter(|word| !(word.len() == 2 && word.starts_with('%')))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Session, parse_desktop_entry, strip_field_codes};

    #[test]
    fn reads_the_name_and_command() {
        let entry = "\
[Desktop Entry]
Name=0xin Touch Test
Comment=A test session
Exec=/home/sn3rt/proj/0xin/profiles/fp5/run-touch-test.sh
Type=Application
";
        assert_eq!(
            parse_desktop_entry(entry),
            Some(Session {
                name: "0xin Touch Test".into(),
                command: vec!["/home/sn3rt/proj/0xin/profiles/fp5/run-touch-test.sh".into()],
            })
        );
    }

    #[test]
    fn hidden_entries_are_not_offered() {
        let entry = "[Desktop Entry]\nName=Gone\nExec=gone\nHidden=true\n";
        assert_eq!(parse_desktop_entry(entry), None);
        let no_display = "[Desktop Entry]\nName=Gone\nExec=gone\nNoDisplay=true\n";
        assert_eq!(parse_desktop_entry(no_display), None);
    }

    #[test]
    fn later_groups_do_not_override_the_session() {
        let entry = "\
[Desktop Entry]
Name=Phosh
Exec=phosh-session

[Desktop Action Debug]
Name=Debug
Exec=phosh-session --debug
";
        let session = parse_desktop_entry(entry).unwrap();
        assert_eq!(session.name, "Phosh");
        assert_eq!(session.command, vec!["phosh-session".to_string()]);
    }

    #[test]
    fn an_entry_without_a_command_is_not_a_session() {
        assert_eq!(parse_desktop_entry("[Desktop Entry]\nName=Nothing\n"), None);
    }

    #[test]
    fn field_codes_are_dropped_but_real_arguments_kept() {
        assert_eq!(
            strip_field_codes("gnome-session --session=phrog %U"),
            vec!["gnome-session".to_string(), "--session=phrog".to_string()]
        );
    }
}
