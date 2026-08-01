use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub label: String,
    program: PathBuf,
    arguments: Vec<OsString>,
}

impl Action {
    fn new(label: &str, program: impl Into<PathBuf>, arguments: &[&str]) -> Self {
        Self {
            label: label.into(),
            program: program.into(),
            arguments: arguments.iter().map(OsString::from).collect(),
        }
    }

    pub fn launch(&self) -> Result<(), String> {
        Command::new(&self.program)
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not run {}: {error}", self.label))
    }

    #[cfg(test)]
    pub fn fixture(label: &str) -> Self {
        Self::new(label, "/bin/true", &[])
    }
}

pub fn configured() -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(program) = env::var_os("PATIN_SESSION_LOGOUT_PROGRAM") {
        let argument = env::var_os("PATIN_SESSION_LOGOUT_ARGUMENT");
        actions.push(Action {
            label: env::var("PATIN_SESSION_LOGOUT_LABEL").unwrap_or_else(|_| "Log out".into()),
            program: PathBuf::from(program),
            arguments: argument.into_iter().collect(),
        });
    }
    actions.push(Action::new("Reboot", "systemctl", &["reboot"]));
    actions.push(Action::new("Shut down", "systemctl", &["poweroff"]));
    actions
}

#[cfg(test)]
mod tests {
    use super::Action;

    #[test]
    fn action_keeps_program_and_arguments_separate() {
        let action = Action::new("Restart", "systemctl", &["reboot"]);
        assert_eq!(action.program.to_string_lossy(), "systemctl");
        assert_eq!(action.arguments, ["reboot"]);
    }
}
