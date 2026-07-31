use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use freedesktop_desktop_entry::{
    DesktopEntry, Iter, current_desktop, default_paths, get_languages_from_env,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Application {
    pub name: String,
    command: Vec<String>,
    working_directory: Option<PathBuf>,
}

impl Application {
    fn from_entry(entry: DesktopEntry, locales: &[String], desktops: &[String]) -> Option<Self> {
        if entry.type_() != Some("Application")
            || entry.hidden()
            || entry.no_display()
            || !shown_on_desktop(&entry, desktops)
        {
            return None;
        }
        if let Some(executable) = entry.try_exec()
            && !executable_exists(executable)
        {
            return None;
        }
        let name = entry.name(locales)?.trim().to_owned();
        let command = entry.parse_exec().ok()?;
        if name.is_empty() || command.is_empty() {
            return None;
        }
        Some(Self {
            name,
            command,
            working_directory: entry.path().map(PathBuf::from),
        })
    }

    pub fn launch(&self) -> Result<(), String> {
        let (program, arguments) = self
            .command
            .split_first()
            .ok_or_else(|| "application has no command".to_owned())?;
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(directory) = &self.working_directory {
            command.current_dir(directory);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open {}: {error}", self.name))
    }

    #[cfg(test)]
    pub fn fixture(name: &str) -> Self {
        Self {
            name: name.into(),
            command: vec!["false".into()],
            working_directory: None,
        }
    }
}

pub fn discover() -> Vec<Application> {
    let locales = get_languages_from_env();
    let desktops = current_desktop().unwrap_or_default();
    let mut seen = HashSet::new();
    let mut applications = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| seen.insert(entry.id().to_owned()))
        .filter_map(|entry| Application::from_entry(entry, &locales, &desktops))
        .collect::<Vec<_>>();
    applications.sort_by_cached_key(|application| application.name.to_lowercase());
    applications
}

fn shown_on_desktop(entry: &DesktopEntry, desktops: &[String]) -> bool {
    if entry.only_show_in().is_some_and(|allowed| {
        !allowed
            .iter()
            .any(|name| desktops.iter().any(|d| d == name))
    }) {
        return false;
    }
    !entry.not_show_in().is_some_and(|blocked| {
        blocked
            .iter()
            .any(|name| desktops.iter().any(|d| d == name))
    })
}

fn executable_exists(executable: &str) -> bool {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        return path.is_file();
    }
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(path).is_file()))
}

#[cfg(test)]
mod tests {
    use freedesktop_desktop_entry::DesktopEntry;

    use super::Application;

    fn entry(name: &str, exec: &str) -> DesktopEntry {
        let mut entry = DesktopEntry::from_appid(format!("org.patin.{name}"));
        entry.add_desktop_entry("Type".into(), "Application".into());
        entry.add_desktop_entry("Name".into(), name.into());
        entry.add_desktop_entry("Exec".into(), exec.into());
        entry
    }

    #[test]
    fn accepts_visible_application_and_parses_exec() {
        let application =
            Application::from_entry(entry("Calculator", "calculator --new-window"), &[], &[])
                .unwrap();
        assert_eq!(application.name, "Calculator");
        assert_eq!(application.command, ["calculator", "--new-window"]);
    }

    #[test]
    fn rejects_hidden_and_non_application_entries() {
        let mut hidden = entry("Hidden", "hidden");
        hidden.add_desktop_entry("NoDisplay".into(), "true".into());
        assert!(Application::from_entry(hidden, &[], &[]).is_none());

        let mut link = entry("Website", "browser");
        link.add_desktop_entry("Type".into(), "Link".into());
        assert!(Application::from_entry(link, &[], &[]).is_none());
    }
}
