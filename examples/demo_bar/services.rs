//! Composes Patin's optional service-adapter crates into one snapshot for
//! the demo bar's row layout. Owning this composition (which providers to
//! poll and how to format them) is the demo's job, not the toolkit's.

use patin::service::Provider;
use patin_service_brightness::{BacklightProvider, BrightnessSnapshot};
use patin_service_upower::{BatteryProvider, BatterySnapshot};
use patin_service_volume::{VolumeProvider, VolumeSnapshot};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatusSnapshot {
    pub battery: Option<String>,
    pub volume: Option<String>,
    pub brightness: Option<String>,
}

pub struct SystemStatus {
    battery: BatteryProvider,
    volume: VolumeProvider,
    brightness: BacklightProvider,
}

impl SystemStatus {
    pub fn new() -> Self {
        Self {
            battery: BatteryProvider::new(),
            volume: VolumeProvider::new(),
            brightness: BacklightProvider::new(),
        }
    }

    pub fn poll(&mut self) -> StatusSnapshot {
        StatusSnapshot {
            battery: self.battery.poll().map(format_battery),
            volume: self.volume.poll().map(format_volume),
            brightness: self.brightness.poll().map(format_brightness),
        }
    }
}

fn format_battery(snapshot: BatterySnapshot) -> String {
    if snapshot.charging {
        format!("BAT {}%+", snapshot.percentage)
    } else {
        format!("BAT {}%", snapshot.percentage)
    }
}

fn format_volume(snapshot: VolumeSnapshot) -> String {
    if snapshot.muted {
        "VOL MUTE".into()
    } else {
        format!("VOL {}%", snapshot.percentage)
    }
}

fn format_brightness(snapshot: BrightnessSnapshot) -> String {
    format!("BRI {}%", snapshot.percentage)
}
