//! Optional network status provider for Patin shells.
//!
//! NetworkManager reports active wifi and wired transports while
//! ModemManager reports independently registered cellular modems. Reading
//! both services lets a shell represent simultaneous wifi and SIM service
//! rather than collapsing everything into one "primary" connection.
//!
//! This root holds the shared vocabulary and the [`Provider`] entry point.
//! The work is split by concern: [`dbus`] reads live state, [`nmcli`]
//! shells out for everything D-Bus does not cover, and [`wifi`],
//! [`profiles`], and [`hotspot`] implement the actions built on top.

mod dbus;
mod hotspot;
mod nmcli;
mod profiles;
mod wifi;

use patin::service::Provider;
use std::fmt;

use dbus::{cellular_strength, network_manager_snapshot};

const HOTSPOT_PROFILE: &str = "Patin Hotspot";
const WIFI_FRESHNESS_SECONDS: i32 = 30;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NetworkSnapshot {
    pub wifi: Option<u8>,
    pub cellular: Option<u8>,
    pub wired: bool,
    pub wifi_available: bool,
    pub wifi_enabled: bool,
    pub cellular_available: bool,
    pub cellular_enabled: bool,
    pub hotspot_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WifiSecurity {
    Open,
    Personal,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub security: WifiSecurity,
    pub active: bool,
    pub available: bool,
    pub known: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WifiProfile {
    uuid: String,
    ssid: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HotspotConfig {
    pub ssid: String,
    pub password_configured: bool,
    pub security: HotspotSecurity,
    pub band: HotspotBand,
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            ssid: "Patin".into(),
            password_configured: false,
            security: HotspotSecurity::Open,
            band: HotspotBand::Automatic,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotspotSecurity {
    Open,
    WpaPersonal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotspotBand {
    Automatic,
    Ghz2_4,
    Ghz5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkError(String);

impl NetworkError {
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NetworkError {}

pub struct NetworkProvider {
    connection: Option<zbus::blocking::Connection>,
}

impl Default for NetworkProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for NetworkProvider {
    type Snapshot = Option<NetworkSnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        let connection = self.connection.as_ref()?;
        let mut snapshot = network_manager_snapshot(connection).unwrap_or_default();
        if let Some(percentage) = cellular_strength(connection) {
            snapshot.cellular = Some(percentage);
            snapshot.cellular_available = true;
        }
        Some(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::{NetworkProvider, Provider};

    #[test]
    fn poll_without_a_system_bus_returns_none() {
        let mut provider = NetworkProvider { connection: None };
        assert_eq!(provider.poll(), None);
    }
}
