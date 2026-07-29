use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatusSnapshot {
    pub battery: Option<String>,
    pub volume: Option<String>,
}

pub struct SystemStatus {
    power_supply_root: PathBuf,
}

impl SystemStatus {
    pub fn new() -> Self {
        Self {
            power_supply_root: PathBuf::from("/sys/class/power_supply"),
        }
    }

    pub fn poll(&self) -> StatusSnapshot {
        StatusSnapshot {
            battery: read_battery(&self.power_supply_root),
            volume: read_volume(),
        }
    }
}

fn read_battery(root: &Path) -> Option<String> {
    let mut batteries = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (read_trimmed(path.join("type")).as_deref() == Some("Battery")).then_some(path)
        })
        .collect::<Vec<_>>();
    batteries.sort();
    batteries.sort_by_key(|path| read_trimmed(path.join("scope")).as_deref() != Some("System"));

    batteries.into_iter().find_map(|path| {
        let capacity = read_trimmed(path.join("capacity"))?
            .parse::<u8>()
            .ok()?
            .min(100);
        let charging = matches!(
            read_trimmed(path.join("status")).as_deref(),
            Some("Charging" | "Full")
        );
        Some(if charging {
            format!("BAT {capacity}%+")
        } else {
            format!("BAT {capacity}%")
        })
    })
}

fn read_volume() -> Option<String> {
    read_wpctl_volume().or_else(read_pactl_volume)
}

fn read_wpctl_volume() -> Option<String> {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_wpctl_volume(&String::from_utf8_lossy(&output.stdout)))
        .flatten()
}

fn read_pactl_volume() -> Option<String> {
    let volume = Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    if !volume.status.success() {
        return None;
    }
    let percentage = String::from_utf8_lossy(&volume.stdout)
        .split_whitespace()
        .find(|word| word.ends_with('%'))?
        .trim_end_matches('%')
        .parse::<u8>()
        .ok()?
        .min(100);
    let mute = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).contains("yes"));
    Some(if mute {
        "VOL MUTE".into()
    } else {
        format!("VOL {percentage}%")
    })
}

fn parse_wpctl_volume(output: &str) -> Option<String> {
    let muted = output.contains("[MUTED]");
    let value = output.split_whitespace().find_map(|word| {
        word.trim_matches(|character: char| !character.is_ascii_digit() && character != '.')
            .parse::<f32>()
            .ok()
    })?;
    if muted {
        Some("VOL MUTE".into())
    } else {
        Some(format!(
            "VOL {}%",
            (value * 100.0).round().clamp(0.0, 100.0)
        ))
    }
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_wpctl_volume;

    #[test]
    fn parses_wpctl_volume_and_mute_state() {
        assert_eq!(parse_wpctl_volume("Volume: 0.55\n"), Some("VOL 55%".into()));
        assert_eq!(
            parse_wpctl_volume("Volume: 1.00 [MUTED]\n"),
            Some("VOL MUTE".into())
        );
        assert_eq!(parse_wpctl_volume("no sink"), None);
    }
}
