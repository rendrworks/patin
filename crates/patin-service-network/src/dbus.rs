//! Reading live transport state over D-Bus: NetworkManager for wifi and
//! wired, ModemManager for independently registered cellular modems.

use zbus::zvariant::OwnedObjectPath;

use crate::{HOTSPOT_PROFILE, NetworkSnapshot};

pub(crate) fn network_manager_snapshot(connection: &zbus::blocking::Connection) -> Option<NetworkSnapshot> {
    let network_manager = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .ok()?;
    let active_connections: Vec<OwnedObjectPath> =
        network_manager.get_property("ActiveConnections").ok()?;
    let mut snapshot = NetworkSnapshot {
        wifi_enabled: network_manager
            .get_property("WirelessEnabled")
            .unwrap_or(false),
        cellular_enabled: network_manager.get_property("WwanEnabled").unwrap_or(false),
        ..Default::default()
    };
    let devices: Vec<OwnedObjectPath> = network_manager.call("GetDevices", &()).unwrap_or_default();
    for path in devices {
        if let Ok(device) = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.NetworkManager",
            path.as_str(),
            "org.freedesktop.NetworkManager.Device",
        ) {
            let device_type: u32 = device.get_property("DeviceType").unwrap_or(0);
            snapshot.wifi_available |= device_type == 2;
            snapshot.cellular_available |= device_type == 8;
        }
    }
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
                let strength = wifi_strength(connection, &path).unwrap_or(0);
                let id: String = active.get_property("Id").unwrap_or_default();
                if id == HOTSPOT_PROFILE {
                    snapshot.hotspot_active = true;
                } else {
                    snapshot.wifi = Some(strength);
                }
            }
            "802-3-ethernet" => snapshot.wired = true,
            _ => {}
        }
    }
    Some(snapshot)
}

pub(crate) fn wifi_strength(
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

pub(crate) fn cellular_strength(connection: &zbus::blocking::Connection) -> Option<u8> {
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
    use crate::NetworkSnapshot;

    #[test]
    fn disconnected_snapshot_has_no_active_transport() {
        assert_eq!(
            NetworkSnapshot::default(),
            NetworkSnapshot {
                wifi: None,
                cellular: None,
                wired: false,
                wifi_available: false,
                wifi_enabled: false,
                cellular_available: false,
                cellular_enabled: false,
                hotspot_active: false,
            }
        );
    }
}
