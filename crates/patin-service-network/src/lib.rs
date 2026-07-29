//! Optional network status provider for Patin shells.
//!
//! NetworkManager is a near-universal D-Bus service, so this adapter reads
//! it directly rather than shelling out. `Provider::poll` returning `None`
//! means only "NetworkManager is unreachable over D-Bus" — being reachable
//! but disconnected is a real reading (`Some(NetworkSnapshot::Disconnected)`),
//! matching how `patin-service-upower`'s `BatterySnapshot` always carries a
//! real `charging` reading rather than conflating "no data" with "off".
//!
//! Cellular signal detail (a separate D-Bus service, `ModemManager`) is out
//! of scope here; a primary connection type other than wifi/wired reports
//! as generically connected (`NetworkSnapshot::Other`).

use patin::service::Provider;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkSnapshot {
    Disconnected,
    Wired,
    Wifi { percentage: u8 },
    Other,
}

pub struct NetworkProvider {
    connection: Option<zbus::blocking::Connection>,
}

impl NetworkProvider {
    pub fn new() -> Self {
        Self {
            connection: zbus::blocking::Connection::system().ok(),
        }
    }
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
        let network_manager = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .ok()?;
        let primary: zbus::zvariant::OwnedObjectPath =
            network_manager.get_property("PrimaryConnection").ok()?;
        if primary.as_str() == "/" {
            return Some(NetworkSnapshot::Disconnected);
        }
        let connection_type: String = network_manager.get_property("PrimaryConnectionType").ok()?;
        Some(match connection_type.as_str() {
            "802-3-ethernet" => NetworkSnapshot::Wired,
            "802-11-wireless" => wifi_strength(connection, &primary)
                .map(|percentage| NetworkSnapshot::Wifi { percentage })
                .unwrap_or(NetworkSnapshot::Other),
            _ => NetworkSnapshot::Other,
        })
    }
}

fn wifi_strength(
    connection: &zbus::blocking::Connection,
    active_connection: &zbus::zvariant::OwnedObjectPath,
) -> Option<u8> {
    let active = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        active_connection.as_str(),
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .ok()?;
    let devices: Vec<zbus::zvariant::OwnedObjectPath> = active.get_property("Devices").ok()?;
    let device_path = devices.first()?;
    let device = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        device_path.as_str(),
        "org.freedesktop.NetworkManager.Device.Wireless",
    )
    .ok()?;
    let access_point: zbus::zvariant::OwnedObjectPath =
        device.get_property("ActiveAccessPoint").ok()?;
    if access_point.as_str() == "/" {
        return None;
    }
    let access_point = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        access_point.as_str(),
        "org.freedesktop.NetworkManager.AccessPoint",
    )
    .ok()?;
    access_point.get_property("Strength").ok()
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
