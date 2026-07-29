use pam_client2::{Context, Flag, conv_mock::Conversation};
use std::{ffi::CStr, sync::mpsc::Sender};
use zeroize::Zeroizing;

#[derive(Debug)]
pub enum AuthResult {
    Success,
    Failure(String),
}

pub fn effective_username() -> Result<String, String> {
    unsafe {
        let entry = libc::getpwuid(libc::geteuid());
        if entry.is_null() || (*entry).pw_name.is_null() {
            return Err("cannot resolve effective user".into());
        }
        CStr::from_ptr((*entry).pw_name)
            .to_str()
            .map(str::to_owned)
            .map_err(|_| "effective username is not UTF-8".into())
    }
}

pub fn authenticate(username: String, password: Zeroizing<String>, sender: Sender<AuthResult>) {
    std::thread::spawn(move || {
        let result = (|| {
            let conversation = Conversation::with_credentials(&username, password.as_str());
            let mut context = Context::new("patin-lock", Some(&username), conversation)
                .map_err(|error| format!("PAM initialization failed: {error}"))?;
            context
                .authenticate(Flag::DISALLOW_NULL_AUTHTOK)
                .map_err(|_| "Authentication failed".to_string())?;
            context
                .acct_mgmt(Flag::NONE)
                .map_err(|_| "Account is not permitted".to_string())?;
            Ok(())
        })();
        let _ = sender.send(match result {
            Ok(()) => AuthResult::Success,
            Err(message) => AuthResult::Failure(message),
        });
    });
}
