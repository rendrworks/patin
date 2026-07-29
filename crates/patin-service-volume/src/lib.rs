//! Optional audio volume provider for Patin shells.
//!
//! Linux audio systems do not expose one universal standard D-Bus volume
//! interface, so this adapter shells out instead: it tries `wpctl` (native
//! PipeWire) first, then falls back to `pactl` (the PulseAudio-compatible
//! service most WirePlumber/PipeWire setups also provide). Neither command
//! being available degrades to `None`.

use std::process::Command;

use patin::service::Provider;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VolumeSnapshot {
    pub percentage: u8,
    pub muted: bool,
}

pub struct VolumeProvider;

impl VolumeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VolumeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for VolumeProvider {
    type Snapshot = Option<VolumeSnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        read_wpctl_volume().or_else(read_pactl_volume)
    }
}

fn read_wpctl_volume() -> Option<VolumeSnapshot> {
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

fn read_pactl_volume() -> Option<VolumeSnapshot> {
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
    let muted = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).contains("yes"));
    Some(VolumeSnapshot { percentage, muted })
}

fn parse_wpctl_volume(output: &str) -> Option<VolumeSnapshot> {
    let muted = output.contains("[MUTED]");
    let value = output.split_whitespace().find_map(|word| {
        word.trim_matches(|character: char| !character.is_ascii_digit() && character != '.')
            .parse::<f32>()
            .ok()
    })?;
    Some(VolumeSnapshot {
        percentage: (value * 100.0).round().clamp(0.0, 100.0) as u8,
        muted,
    })
}

#[cfg(test)]
mod tests {
    use super::{VolumeSnapshot, parse_wpctl_volume};

    #[test]
    fn parses_wpctl_volume_and_mute_state() {
        assert_eq!(
            parse_wpctl_volume("Volume: 0.55\n"),
            Some(VolumeSnapshot {
                percentage: 55,
                muted: false
            })
        );
        assert_eq!(
            parse_wpctl_volume("Volume: 1.00 [MUTED]\n"),
            Some(VolumeSnapshot {
                percentage: 100,
                muted: true
            })
        );
        assert_eq!(parse_wpctl_volume("no sink"), None);
    }
}
