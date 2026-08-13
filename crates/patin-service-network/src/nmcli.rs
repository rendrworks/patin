//! Running `nmcli` and reading its escaped, colon-separated output.

use std::process::Command;

use crate::NetworkError;

pub(crate) fn nmcli(arguments: &[&str]) -> Result<String, NetworkError> {
    let output = Command::new("nmcli")
        .args(arguments)
        .output()
        .map_err(|error| NetworkError(format!("could not run nmcli: {error}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(NetworkError(if detail.is_empty() {
        "NetworkManager operation failed".into()
    } else {
        detail
    }))
}

pub(crate) fn split_escaped(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            fields.last_mut().unwrap().push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' {
            fields.push(String::new());
        } else {
            fields.last_mut().unwrap().push(character);
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::split_escaped;

    #[test]
    fn parses_escaped_nmcli_fields() {
        assert_eq!(
            split_escaped("*:Cafe\\: upstairs:77:WPA2"),
            ["*", "Cafe: upstairs", "77", "WPA2"]
        );
    }
}
