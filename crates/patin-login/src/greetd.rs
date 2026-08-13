//! The greetd client protocol.
//!
//! Unlike `patin-lock`, which authenticates against PAM itself, a greeter
//! never touches PAM: greetd owns the conversation and the privileges, and
//! the greeter only relays answers over `$GREETD_SOCK`. Messages are a
//! native-endian `u32` length followed by that many bytes of JSON.
//!
//! The exchange is a small state machine — open a session for a user, answer
//! each prompt greetd forwards from PAM, then ask greetd to start the real
//! session command. Because PAM deliberately delays a failed attempt, all of
//! it runs on a worker thread and reports back over a channel, the same shape
//! `patin-lock`'s `auth` module uses.

use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// What a completed sign-in attempt reports back to the UI.
#[derive(Debug)]
pub enum LoginResult {
    /// greetd accepted the credentials and is starting the session; the
    /// greeter is about to be torn down.
    Success,
    Failure(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    CreateSession { username: String },
    PostAuthMessageResponse { response: Option<String> },
    StartSession { cmd: Vec<String>, env: Vec<String> },
    CancelSession,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Success,
    Error {
        error_type: String,
        description: String,
    },
    AuthMessage {
        auth_message_type: String,
        auth_message: String,
    },
}

/// Whether this greeter is talking to a real greetd.
///
/// Running without `$GREETD_SOCK` is not an error: it renders the UI for
/// visual checks in a nested session, and reports any sign-in attempt as
/// unavailable rather than pretending to authenticate.
pub enum Backend {
    Greetd,
    Preview,
}

impl Backend {
    pub fn detect() -> Self {
        if env::var_os("GREETD_SOCK").is_some() {
            Self::Greetd
        } else {
            Self::Preview
        }
    }

    pub fn is_preview(&self) -> bool {
        matches!(self, Self::Preview)
    }
}

/// Run one full sign-in attempt on a worker thread, reporting the outcome to
/// `sender`. `command` is the session greetd should exec on success.
pub fn sign_in(
    backend: &Backend,
    username: String,
    password: Zeroizing<String>,
    command: Vec<String>,
    sender: Sender<LoginResult>,
) {
    if backend.is_preview() {
        std::thread::spawn(move || {
            let _ = sender.send(LoginResult::Failure(
                "No greetd session (preview mode)".into(),
            ));
        });
        return;
    }
    std::thread::spawn(move || {
        let _ = sender.send(match attempt(&username, &password, &command) {
            Ok(()) => LoginResult::Success,
            Err(message) => LoginResult::Failure(message),
        });
    });
}

fn attempt(username: &str, password: &str, command: &[String]) -> Result<(), String> {
    let mut session = Connection::open()?;
    let mut response = session.request(&Request::CreateSession {
        username: username.to_string(),
    })?;

    loop {
        match response {
            Response::Success => break,
            Response::Error {
                error_type,
                description,
            } => {
                // An auth failure leaves greetd holding a dead session; it has
                // to be cancelled before the next attempt can be created.
                session.request(&Request::CancelSession).ok();
                return Err(friendly_error(&error_type, &description));
            }
            Response::AuthMessage {
                auth_message_type,
                auth_message,
            } => {
                // Only secret prompts get the password. Visible prompts are
                // for things like a one-time token, which this greeter has no
                // field for, so they are answered empty rather than leaking
                // the password into a non-secret prompt.
                let answer = match auth_message_type.as_str() {
                    "secret" => Some(password.to_string()),
                    "visible" => Some(String::new()),
                    _ => None,
                };
                if auth_message_type == "error" {
                    session.request(&Request::CancelSession).ok();
                    return Err(auth_message.trim().to_string());
                }
                response = session.request(&Request::PostAuthMessageResponse { response: answer })?;
            }
        }
    }

    match session.request(&Request::StartSession {
        cmd: command.to_vec(),
        env: Vec::new(),
    })? {
        Response::Success => Ok(()),
        Response::Error { description, .. } => Err(description),
        Response::AuthMessage { auth_message, .. } => Err(auth_message),
    }
}

/// greetd's own wording is aimed at logs; these are the two cases a person
/// standing at the screen can act on.
fn friendly_error(error_type: &str, description: &str) -> String {
    if error_type == "auth_error" {
        "Authentication failed".into()
    } else {
        description.to_string()
    }
}

struct Connection {
    stream: UnixStream,
}

impl Connection {
    fn open() -> Result<Self, String> {
        let path = env::var_os("GREETD_SOCK").ok_or("GREETD_SOCK is not set")?;
        let stream = UnixStream::connect(&path)
            .map_err(|error| format!("cannot reach greetd: {error}"))?;
        Ok(Self { stream })
    }

    fn request(&mut self, request: &Request) -> Result<Response, String> {
        let payload =
            serde_json::to_vec(request).map_err(|error| format!("cannot encode request: {error}"))?;
        let length = u32::try_from(payload.len()).map_err(|_| "request is too large".to_string())?;
        self.stream
            .write_all(&length.to_ne_bytes())
            .and_then(|()| self.stream.write_all(&payload))
            .map_err(|error| format!("cannot send request: {error}"))?;

        let mut header = [0u8; 4];
        self.stream
            .read_exact(&mut header)
            .map_err(|error| format!("cannot read response: {error}"))?;
        let mut body = vec![0u8; u32::from_ne_bytes(header) as usize];
        self.stream
            .read_exact(&mut body)
            .map_err(|error| format!("cannot read response: {error}"))?;
        serde_json::from_slice(&body).map_err(|error| format!("cannot decode response: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, friendly_error};

    #[test]
    fn requests_serialize_to_greetds_tagged_shape() {
        let created = serde_json::to_string(&Request::CreateSession {
            username: "sn3rt".into(),
        })
        .unwrap();
        assert_eq!(created, r#"{"type":"create_session","username":"sn3rt"}"#);

        let answered = serde_json::to_string(&Request::PostAuthMessageResponse {
            response: Some("hunter2".into()),
        })
        .unwrap();
        assert_eq!(
            answered,
            r#"{"type":"post_auth_message_response","response":"hunter2"}"#
        );

        let started = serde_json::to_string(&Request::StartSession {
            cmd: vec!["0xin".into()],
            env: Vec::new(),
        })
        .unwrap();
        assert_eq!(started, r#"{"type":"start_session","cmd":["0xin"],"env":[]}"#);
    }

    #[test]
    fn responses_deserialize_from_greetds_tagged_shape() {
        let message: Response =
            serde_json::from_str(r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password: "}"#)
                .unwrap();
        assert!(matches!(
            message,
            Response::AuthMessage {
                ref auth_message_type,
                ..
            } if auth_message_type == "secret"
        ));

        let success: Response = serde_json::from_str(r#"{"type":"success"}"#).unwrap();
        assert!(matches!(success, Response::Success));

        let error: Response = serde_json::from_str(
            r#"{"type":"error","error_type":"auth_error","description":"failed"}"#,
        )
        .unwrap();
        assert!(matches!(error, Response::Error { .. }));
    }

    #[test]
    fn auth_errors_are_reported_in_human_terms() {
        assert_eq!(
            friendly_error("auth_error", "pam_authenticate: AUTH_ERR"),
            "Authentication failed"
        );
        assert_eq!(friendly_error("error", "no such user"), "no such user");
    }
}
