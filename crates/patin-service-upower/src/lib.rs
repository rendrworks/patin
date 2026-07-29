//! Optional UPower battery provider for Patin shells.
//!
//! Implements [`patin::service::Provider`] over `zbus`'s blocking D-Bus API
//! against UPower's synthetic `DisplayDevice`, the aggregate device UPower
//! maintains specifically for status bars and shells. A missing system bus
//! or UPower service degrades to `None`, matching a shell's existing
//! optional-status-component behavior rather than treating either as an
//! error.

use patin::service::Provider;

const UPOWER_DESTINATION: &str = "org.freedesktop.UPower";
const DISPLAY_DEVICE_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const DEVICE_INTERFACE: &str = "org.freedesktop.UPower.Device";

/// UPower device states relevant to a charging indicator.
/// <https://upower.freedesktop.org/docs/Device.html#Device:State>
const STATE_CHARGING: u32 = 1;
const STATE_FULLY_CHARGED: u32 = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatterySnapshot {
    pub percentage: u8,
    pub charging: bool,
}

pub struct BatteryProvider {
    connection: Option<zbus::blocking::Connection>,
}

impl BatteryProvider {
    pub fn new() -> Self {
        Self {
            connection: zbus::blocking::Connection::system().ok(),
        }
    }
}

impl Default for BatteryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BatteryProvider {
    type Snapshot = Option<BatterySnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        let connection = self.connection.as_ref()?;
        let proxy = zbus::blocking::Proxy::new(
            connection,
            UPOWER_DESTINATION,
            DISPLAY_DEVICE_PATH,
            DEVICE_INTERFACE,
        )
        .ok()?;
        let percentage: f64 = proxy.get_property("Percentage").ok()?;
        let state: u32 = proxy.get_property("State").ok()?;
        Some(BatterySnapshot {
            percentage: percentage.round().clamp(0.0, 100.0) as u8,
            charging: matches!(state, STATE_CHARGING | STATE_FULLY_CHARGED),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BatteryProvider, Provider};

    #[test]
    fn poll_without_a_system_bus_returns_none() {
        let mut provider = BatteryProvider { connection: None };
        assert_eq!(provider.poll(), None);
    }
}
