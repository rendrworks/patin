//! Optional network status provider for Patin shells.
//!
//! NetworkManager reports active wifi and wired transports while
//! ModemManager reports independently registered cellular modems. Reading
//! both services lets a shell represent simultaneous wifi and SIM service
//! rather than collapsing everything into one "primary" connection.

use patin::service::Provider;
use zbus::zvariant::OwnedObjectPath;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NetworkSnapshot {
    pub wifi: Option<u8>,
    pub cellular: Option<u8>,
    pub wired: bool,
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
        let mut snapshot = network_manager_snapshot(connection).unwrap_or_default();
        if let Some(percentage) = cellular_strength(connection) {
            snapshot.cellular = Some(percentage);
        }
        Some(snapshot)
    }
}

fn network_manager_snapshot(connection: &zbus::blocking::Connection) -> Option<NetworkSnapshot> {
    let network_manager = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .ok()?;
    let active_connections: Vec<OwnedObjectPath> =
        network_manager.get_property("ActiveConnections").ok()?;
    let mut snapshot = NetworkSnapshot::default();
    for path in active_connections {
        let active = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.NetworkManager",
            path.as_str(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .ok()?;
        let connection_type: String = active.get_property("Type").ok()?;
        match connection_type.as_str() {
            "802-11-wireless" => {
                snapshot.wifi = Some(wifi_strength(connection, &path).unwrap_or(0));
            }
            "802-3-ethernet" => snapshot.wired = true,
            _ => {}
        }
    }
    Some(snapshot)
}

fn wifi_strength(
    connection: &zbus::blocking::Connection,
    active_connection: &OwnedObjectPath,
) -> Option<u8> {
    let active = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        active_connection.as_str(),
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .ok()?;
    let devices: Vec<OwnedObjectPath> = active.get_property("Devices").ok()?;
    let device_path = devices.first()?;
    let device = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        device_path.as_str(),
        "org.freedesktop.NetworkManager.Device.Wireless",
    )
    .ok()?;
    let access_point: OwnedObjectPath = device.get_property("ActiveAccessPoint").ok()?;
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

fn cellular_strength(connection: &zbus::blocking::Connection) -> Option<u8> {
    const MODEM_INTERFACE: &str = "org.freedesktop.ModemManager1.Modem";
    const REGISTERED: i32 = 8;
    let manager = zbus::blocking::fdo::ObjectManagerProxy::builder(connection)
        .destination("org.freedesktop.ModemManager1")
        .ok()?
        .path("/org/freedesktop/ModemManager1")
        .ok()?
        .build()
        .ok()?;
    let objects = manager.get_managed_objects().ok()?;
    objects.into_iter().find_map(|(path, interfaces)| {
        interfaces
            .keys()
            .any(|interface| interface.as_str() == MODEM_INTERFACE)
            .then(|| {
                let modem = zbus::blocking::Proxy::new(
                    connection,
                    "org.freedesktop.ModemManager1",
                    path.as_str(),
                    MODEM_INTERFACE,
                )
                .ok()?;
                let state: i32 = modem.get_property("State").ok()?;
                if state < REGISTERED {
                    return None;
                }
                let (percentage, _recent): (u32, bool) =
                    modem.get_property("SignalQuality").ok()?;
                Some(percentage.min(100) as u8)
            })
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::{NetworkProvider, NetworkSnapshot, Provider};

    #[test]
    fn disconnected_snapshot_has_no_active_transport() {
        assert_eq!(
            NetworkSnapshot::default(),
            NetworkSnapshot {
                wifi: None,
                cellular: None,
                wired: false,
            }
        );
    }

    #[test]
    fn poll_without_a_system_bus_returns_none() {
        let mut provider = NetworkProvider { connection: None };
        assert_eq!(provider.poll(), None);
    }
}
