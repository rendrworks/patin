//! Composes Patin's optional service-adapter crates into one snapshot for
//! the demo bar's row layout. Owning this composition (which providers to
//! poll and how to present them) is the demo's job, not the toolkit's.

use patin::service::Provider;
use patin_service_brightness::{BacklightProvider, BrightnessSnapshot};
use patin_service_network::{NetworkProvider, NetworkSnapshot};
use patin_service_upower::{BatteryProvider, BatterySnapshot};
use patin_service_volume::{VolumeProvider, VolumeSnapshot};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatusSnapshot {
    pub battery: Option<BatterySnapshot>,
    pub volume: Option<VolumeSnapshot>,
    pub brightness: Option<BrightnessSnapshot>,
    pub network: Option<NetworkSnapshot>,
}

pub struct SystemStatus {
    battery: BatteryProvider,
    volume: VolumeProvider,
    brightness: BacklightProvider,
    network: NetworkProvider,
}

impl SystemStatus {
    pub fn new() -> Self {
        Self {
            battery: BatteryProvider::new(),
            volume: VolumeProvider::new(),
            brightness: BacklightProvider::new(),
            network: NetworkProvider::new(),
        }
    }

    pub fn poll(&mut self) -> StatusSnapshot {
        StatusSnapshot {
            battery: self.battery.poll(),
            volume: self.volume.poll(),
            brightness: self.brightness.poll(),
            network: self.network.poll(),
        }
    }
}
