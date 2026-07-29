//! Optional display backlight provider for Patin shells.
//!
//! There is no portable D-Bus property to read the current backlight level
//! (systemd-logind only exposes a `SetBrightness` method, not a readable
//! one), so this adapter reads Linux's documented `/sys/class/backlight`
//! ABI directly. It does not assume a driver or panel name; a missing or
//! invalid backlight entry degrades to `None`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use patin::service::Provider;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BrightnessSnapshot {
    pub percentage: u8,
}

pub struct BacklightProvider {
    root: PathBuf,
}

impl BacklightProvider {
    pub fn new() -> Self {
        Self {
            root: PathBuf::from("/sys/class/backlight"),
        }
    }
}

impl Default for BacklightProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BacklightProvider {
    type Snapshot = Option<BrightnessSnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        read_brightness(&self.root)
    }
}

fn read_brightness(root: &Path) -> Option<BrightnessSnapshot> {
    let mut entries = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    entries.into_iter().find_map(|entry| {
        let current = read_trimmed(entry.join("brightness"))?
            .parse::<u64>()
            .ok()?;
        let maximum = read_trimmed(entry.join("max_brightness"))?
            .parse::<u64>()
            .ok()?;
        brightness_snapshot(current, maximum)
    })
}

fn brightness_snapshot(current: u64, maximum: u64) -> Option<BrightnessSnapshot> {
    (maximum > 0).then(|| BrightnessSnapshot {
        percentage: current.saturating_mul(100).div_ceil(maximum).min(100) as u8,
    })
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{BrightnessSnapshot, brightness_snapshot};

    #[test]
    fn computes_brightness_and_rejects_zero_maximum() {
        assert_eq!(
            brightness_snapshot(2405, 4095),
            Some(BrightnessSnapshot { percentage: 59 })
        );
        assert_eq!(brightness_snapshot(1, 0), None);
    }
}
