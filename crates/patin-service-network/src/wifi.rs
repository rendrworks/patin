//! Wi-Fi: listing what is visible or saved, and the connect/disconnect/
//! forget/radio-toggle actions a settings UI drives.

use std::collections::HashMap;

use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::nmcli::{nmcli, split_escaped};
use crate::profiles::{
    merge_wifi_network, merge_wifi_profiles, system_uptime_seconds, wifi_last_seen_is_recent,
    wifi_profile_uuid, wifi_profiles,
};
use crate::{NetworkError, NetworkProvider, WifiNetwork, WifiProfile, WifiSecurity};

impl NetworkProvider {
    pub fn new() -> Self {
        Self {
            connection: zbus::blocking::Connection::system().ok(),
        }
    }

    pub fn wifi_networks(&self) -> Result<Vec<WifiNetwork>, NetworkError> {
        self.scan_wifi_networks()
    }

    pub fn known_wifi_networks(&self) -> Result<Vec<WifiNetwork>, NetworkError> {
        let networks = self.visible_wifi_networks(false)?;
        Ok(merge_wifi_profiles(networks, &wifi_profiles()?, false))
    }

    pub fn scan_wifi_networks(&self) -> Result<Vec<WifiNetwork>, NetworkError> {
        let networks = self.visible_wifi_networks(true)?;
        Ok(merge_wifi_profiles(networks, &wifi_profiles()?, true))
    }

    pub fn request_wifi_scan(&self) -> Result<(), NetworkError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| NetworkError("NetworkManager D-Bus is unavailable".into()))?;
        let network_manager = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .map_err(|error| NetworkError(format!("could not access NetworkManager: {error}")))?;
        let devices: Vec<OwnedObjectPath> = network_manager
            .call("GetDevices", &())
            .map_err(|error| NetworkError(format!("could not list network devices: {error}")))?;
        for path in devices {
            let device = zbus::blocking::Proxy::new(
                connection,
                "org.freedesktop.NetworkManager",
                path.as_str(),
                "org.freedesktop.NetworkManager.Device",
            )
            .map_err(|error| NetworkError(format!("could not inspect network device: {error}")))?;
            if device.get_property::<u32>("DeviceType").unwrap_or(0) != 2 {
                continue;
            }
            let wireless = zbus::blocking::Proxy::new(
                connection,
                "org.freedesktop.NetworkManager",
                path.as_str(),
                "org.freedesktop.NetworkManager.Device.Wireless",
            )
            .map_err(|error| NetworkError(format!("could not access Wi-Fi device: {error}")))?;
            let options = HashMap::<String, OwnedValue>::new();
            return wireless
                .call("RequestScan", &options)
                .map_err(|error| NetworkError(format!("could not request Wi-Fi scan: {error}")));
        }
        Err(NetworkError("no Wi-Fi device is available".into()))
    }

    pub fn refresh_wifi_networks(
        &self,
        current: &[WifiNetwork],
        include_unknown: bool,
    ) -> Result<Vec<WifiNetwork>, NetworkError> {
        let profiles = current
            .iter()
            .filter(|network| network.known)
            .map(|network| WifiProfile {
                uuid: String::new(),
                ssid: network.ssid.clone(),
            })
            .collect::<Vec<_>>();
        let networks = self.visible_wifi_networks(false)?;
        Ok(merge_wifi_profiles(networks, &profiles, include_unknown))
    }

    fn visible_wifi_networks(&self, rescan: bool) -> Result<Vec<WifiNetwork>, NetworkError> {
        let output = nmcli(&[
            "--terse",
            "--escape",
            "yes",
            "--fields",
            "IN-USE,SSID,SIGNAL,SECURITY,DBUS-PATH",
            "device",
            "wifi",
            "list",
            "--rescan",
            if rescan { "yes" } else { "no" },
        ])?;
        let mut networks = Vec::<WifiNetwork>::new();
        for line in output.lines() {
            let fields = split_escaped(line);
            if fields.len() != 5 || fields[1].is_empty() {
                continue;
            }
            let security = if fields[3].is_empty() || fields[3] == "--" {
                WifiSecurity::Open
            } else if fields[3].contains("WPA") || fields[3].contains("SAE") {
                WifiSecurity::Personal
            } else {
                WifiSecurity::Unsupported
            };
            let active = fields[0] == "*" || fields[0] == "yes";
            if !active && !self.access_point_is_recent(&fields[4]) {
                continue;
            }
            let candidate = WifiNetwork {
                ssid: fields[1].clone(),
                strength: fields[2].parse().unwrap_or(0),
                security,
                active,
                available: true,
                known: false,
            };
            merge_wifi_network(&mut networks, candidate);
        }
        networks.sort_by_key(|network| {
            (
                std::cmp::Reverse(network.active),
                std::cmp::Reverse(network.strength),
            )
        });
        Ok(networks)
    }

    fn access_point_is_recent(&self, path: &str) -> bool {
        let Some(connection) = &self.connection else {
            return true;
        };
        let Some(uptime) = system_uptime_seconds() else {
            return true;
        };
        let Ok(access_point) = zbus::blocking::Proxy::new(
            connection,
            "org.freedesktop.NetworkManager",
            path,
            "org.freedesktop.NetworkManager.AccessPoint",
        ) else {
            return false;
        };
        let Ok(last_seen) = access_point.get_property::<i32>("LastSeen") else {
            return false;
        };
        wifi_last_seen_is_recent(last_seen, uptime)
    }

    pub fn set_wifi_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        nmcli(&["radio", "wifi", if enabled { "on" } else { "off" }]).map(|_| ())
    }

    pub fn set_cellular_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        nmcli(&["radio", "wwan", if enabled { "on" } else { "off" }]).map(|_| ())
    }

    pub fn connect_wifi(&self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
        if password.is_none()
            && let Ok(profiles) = wifi_profiles()
            && let Some(uuid) = wifi_profile_uuid(&profiles, ssid)
        {
            return nmcli(&["connection", "up", "uuid", uuid]).map(|_| ());
        }
        let mut arguments = vec!["device", "wifi", "connect", ssid];
        if let Some(password) = password {
            arguments.extend(["password", password]);
        }
        nmcli(&arguments).map(|_| ())
    }

    pub fn disconnect_wifi(&self) -> Result<(), NetworkError> {
        let device = nmcli(&[
            "--terse",
            "--fields",
            "DEVICE,TYPE,STATE",
            "device",
            "status",
        ])?
        .lines()
        .filter_map(|line| {
            let fields = split_escaped(line);
            (fields.len() == 3).then_some(fields)
        })
        .find(|fields| fields[1] == "wifi" && fields[2].starts_with("connected"))
        .map(|fields| fields[0].clone())
        .ok_or_else(|| NetworkError("no active network device".into()))?;
        nmcli(&["device", "disconnect", &device]).map(|_| ())
    }

    pub fn forget_wifi(&self, ssid: &str) -> Result<(), NetworkError> {
        let profiles = wifi_profiles()?;
        let uuid = wifi_profile_uuid(&profiles, ssid)
            .ok_or_else(|| NetworkError(format!("no saved profile for {ssid}")))?;
        nmcli(&["connection", "delete", "uuid", uuid]).map(|_| ())
    }
}
