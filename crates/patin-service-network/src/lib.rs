//! Optional network status provider for Patin shells.
//!
//! NetworkManager reports active wifi and wired transports while
//! ModemManager reports independently registered cellular modems. Reading
//! both services lets a shell represent simultaneous wifi and SIM service
//! rather than collapsing everything into one "primary" connection.

use patin::service::Provider;
use std::{collections::HashMap, fmt, fs, process::Command};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

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

    pub fn hotspot_config(&self) -> HotspotConfig {
        let ssid = nmcli(&[
            "--get-values",
            "802-11-wireless.ssid",
            "connection",
            "show",
            HOTSPOT_PROFILE,
        ])
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Patin".into());
        let password_configured = nmcli(&[
            "--show-secrets",
            "--get-values",
            "802-11-wireless-security.psk",
            "connection",
            "show",
            HOTSPOT_PROFILE,
        ])
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
        let key_mgmt = nmcli(&[
            "--get-values",
            "802-11-wireless-security.key-mgmt",
            "connection",
            "show",
            HOTSPOT_PROFILE,
        ])
        .unwrap_or_default();
        let band = nmcli(&[
            "--get-values",
            "802-11-wireless.band",
            "connection",
            "show",
            HOTSPOT_PROFILE,
        ])
        .unwrap_or_default();
        HotspotConfig {
            ssid,
            password_configured,
            security: if key_mgmt.trim().is_empty() {
                HotspotSecurity::Open
            } else {
                HotspotSecurity::WpaPersonal
            },
            band: match band.trim() {
                "bg" => HotspotBand::Ghz2_4,
                "a" => HotspotBand::Ghz5,
                _ => HotspotBand::Automatic,
            },
        }
    }

    pub fn save_hotspot(
        &self,
        config: &HotspotConfig,
        password: Option<&str>,
    ) -> Result<(), NetworkError> {
        validate_hotspot(&config.ssid, config.security, password)?;
        if config.security == HotspotSecurity::WpaPersonal
            && password.is_none()
            && !config.password_configured
        {
            return Err(NetworkError("set a hotspot password before saving".into()));
        }
        if nmcli(&["connection", "show", HOTSPOT_PROFILE]).is_err() {
            nmcli(&[
                "connection",
                "add",
                "type",
                "wifi",
                "ifname",
                "*",
                "con-name",
                HOTSPOT_PROFILE,
                "autoconnect",
                "no",
                "ssid",
                &config.ssid,
            ])?;
        }
        let band = match config.band {
            HotspotBand::Automatic => "",
            HotspotBand::Ghz2_4 => "bg",
            HotspotBand::Ghz5 => "a",
        };
        nmcli(&[
            "connection",
            "modify",
            HOTSPOT_PROFILE,
            "802-11-wireless.mode",
            "ap",
            "802-11-wireless.ssid",
            &config.ssid,
            "802-11-wireless.band",
            band,
            "ipv4.method",
            "shared",
            "ipv6.method",
            "disabled",
        ])?;
        match config.security {
            HotspotSecurity::Open => {
                nmcli(&[
                    "connection",
                    "modify",
                    HOTSPOT_PROFILE,
                    "remove",
                    "802-11-wireless-security",
                ])?;
            }
            HotspotSecurity::WpaPersonal => {
                if let Some(password) = password {
                    nmcli(&[
                        "connection",
                        "modify",
                        HOTSPOT_PROFILE,
                        "802-11-wireless-security.key-mgmt",
                        "wpa-psk",
                        "802-11-wireless-security.psk",
                        password,
                    ])?;
                }
            }
        }
        Ok(())
    }

    pub fn set_hotspot_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        if enabled {
            nmcli(&["connection", "up", HOTSPOT_PROFILE])?;
        } else {
            nmcli(&["connection", "down", HOTSPOT_PROFILE])?;
        }
        Ok(())
    }
}

fn merge_wifi_profiles(
    networks: Vec<WifiNetwork>,
    profiles: &[WifiProfile],
    include_unknown: bool,
) -> Vec<WifiNetwork> {
    let mut result = if include_unknown {
        networks
    } else {
        networks
            .into_iter()
            .filter(|network| {
                network.active || profiles.iter().any(|profile| profile.ssid == network.ssid)
            })
            .collect()
    };
    for network in &mut result {
        network.known = profiles.iter().any(|profile| profile.ssid == network.ssid);
    }
    for profile in profiles {
        if !result.iter().any(|network| network.ssid == profile.ssid) {
            result.push(WifiNetwork {
                ssid: profile.ssid.clone(),
                strength: 0,
                security: WifiSecurity::Unsupported,
                active: false,
                available: false,
                known: true,
            });
        }
    }
    result.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.available.cmp(&left.available))
            .then_with(|| right.strength.cmp(&left.strength))
            .then_with(|| left.ssid.to_lowercase().cmp(&right.ssid.to_lowercase()))
    });
    result
}

fn merge_wifi_network(networks: &mut Vec<WifiNetwork>, candidate: WifiNetwork) {
    if let Some(existing) = networks
        .iter_mut()
        .find(|network| network.ssid == candidate.ssid)
    {
        if candidate.active || (!existing.active && candidate.strength > existing.strength) {
            *existing = candidate;
        }
    } else {
        networks.push(candidate);
    }
}

fn wifi_profile_uuids(profiles: &str) -> Vec<String> {
    profiles
        .lines()
        .filter_map(|line| {
            let fields = split_escaped(line);
            (fields.len() == 2 && fields[1] == "802-11-wireless" && !fields[0].is_empty())
                .then(|| fields[0].clone())
        })
        .collect()
}

fn wifi_profiles() -> Result<Vec<WifiProfile>, NetworkError> {
    let overview = nmcli(&[
        "--terse",
        "--escape",
        "yes",
        "--fields",
        "UUID,TYPE",
        "connection",
        "show",
    ])?;
    Ok(wifi_profile_uuids(&overview)
        .into_iter()
        .filter_map(|uuid| {
            let values = nmcli(&[
                "--terse",
                "--escape",
                "yes",
                "--get-values",
                "802-11-wireless.ssid,802-11-wireless.mode",
                "connection",
                "show",
                "uuid",
                &uuid,
            ])
            .ok()?;
            parse_wifi_profile(&uuid, &values)
        })
        .collect())
}

fn parse_wifi_profile(uuid: &str, values: &str) -> Option<WifiProfile> {
    let mut lines = values.lines();
    let ssid = unescape_field(lines.next()?).trim().to_owned();
    let mode = lines.next().unwrap_or_default().trim();
    (!ssid.is_empty() && mode != "ap").then(|| WifiProfile {
        uuid: uuid.into(),
        ssid,
    })
}

fn unescape_field(value: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            result.push(character);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

fn system_uptime_seconds() -> Option<i32> {
    fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

fn wifi_last_seen_is_recent(last_seen: i32, uptime: i32) -> bool {
    last_seen >= 0 && uptime.saturating_sub(last_seen) <= WIFI_FRESHNESS_SECONDS
}

fn wifi_profile_uuid<'a>(profiles: &'a [WifiProfile], ssid: &str) -> Option<&'a str> {
    profiles
        .iter()
        .find(|profile| profile.ssid == ssid)
        .map(|profile| profile.uuid.as_str())
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

fn nmcli(arguments: &[&str]) -> Result<String, NetworkError> {
    let output = Command::new("nmcli")
        .args(arguments)
        .output()
        .map_err(|error| NetworkError(format!("could not run nmcli: {error}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(NetworkError(if detail.is_empty() {
        "NetworkManager operation failed".into()
    } else {
        detail
    }))
}

fn split_escaped(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            fields.last_mut().unwrap().push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' {
            fields.push(String::new());
        } else {
            fields.last_mut().unwrap().push(character);
        }
    }
    fields
}

fn validate_hotspot(
    ssid: &str,
    security: HotspotSecurity,
    password: Option<&str>,
) -> Result<(), NetworkError> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(NetworkError(
            "hotspot SSID must contain 1 to 32 bytes".into(),
        ));
    }
    if security == HotspotSecurity::WpaPersonal && password.is_none() {
        return Ok(());
    }
    if let Some(password) = password
        && (!(8..=63).contains(&password.len()) || !password.is_ascii())
    {
        return Err(NetworkError(
            "hotspot password must contain 8 to 63 ASCII characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        NetworkProvider, NetworkSnapshot, Provider, WifiNetwork, WifiProfile, WifiSecurity,
        merge_wifi_network, merge_wifi_profiles, parse_wifi_profile, split_escaped,
        validate_hotspot, wifi_last_seen_is_recent, wifi_profile_uuid, wifi_profile_uuids,
    };

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

    #[test]
    fn poll_without_a_system_bus_returns_none() {
        let mut provider = NetworkProvider { connection: None };
        assert_eq!(provider.poll(), None);
    }

    #[test]
    fn parses_escaped_nmcli_fields() {
        assert_eq!(
            split_escaped("*:Cafe\\: upstairs:77:WPA2"),
            ["*", "Cafe: upstairs", "77", "WPA2"]
        );
    }

    #[test]
    fn selects_wifi_profile_uuids_from_valid_overview_fields() {
        assert_eq!(
            wifi_profile_uuids(
                "wifi-uuid:802-11-wireless\nethernet-uuid:802-3-ethernet\n:802-11-wireless"
            ),
            ["wifi-uuid"]
        );
    }

    #[test]
    fn selects_saved_profile_uuid_by_actual_ssid() {
        let profiles = vec![
            WifiProfile {
                uuid: "delta-uuid".into(),
                ssid: "DELTA-6c60c4".into(),
            },
            WifiProfile {
                uuid: "ziggo-uuid".into(),
                ssid: "Ziggo7827342".into(),
            },
        ];
        assert_eq!(
            wifi_profile_uuid(&profiles, "Ziggo7827342"),
            Some("ziggo-uuid")
        );
        assert_eq!(wifi_profile_uuid(&profiles, "Unknown"), None);
    }

    #[test]
    fn known_list_includes_unavailable_profiles_and_omits_unknown_networks() {
        let network = |ssid: &str, active| WifiNetwork {
            ssid: ssid.into(),
            strength: 50,
            security: WifiSecurity::Personal,
            active,
            available: true,
            known: false,
        };
        let profiles = [
            WifiProfile {
                uuid: "home".into(),
                ssid: "Home".into(),
            },
            WifiProfile {
                uuid: "away".into(),
                ssid: "Away".into(),
            },
        ];

        let result = merge_wifi_profiles(
            vec![
                network("Connected", true),
                network("Home", false),
                network("New cafe", false),
            ],
            &profiles,
            false,
        );
        assert_eq!(
            result
                .iter()
                .map(|network| network.ssid.as_str())
                .collect::<Vec<_>>(),
            ["Connected", "Home", "Away"]
        );
        assert!(result[1].known && result[1].available);
        assert!(result[2].known && !result[2].available);
    }

    #[test]
    fn profile_parser_excludes_access_point_mode() {
        assert_eq!(
            parse_wifi_profile("home", "Cafe\\: upstairs\ninfrastructure\n"),
            Some(WifiProfile {
                uuid: "home".into(),
                ssid: "Cafe: upstairs".into(),
            })
        );
        assert_eq!(parse_wifi_profile("hotspot", "Patin\nap\n"), None);
    }

    #[test]
    fn access_point_availability_expires_after_thirty_seconds() {
        assert!(wifi_last_seen_is_recent(980, 1_000));
        assert!(wifi_last_seen_is_recent(970, 1_000));
        assert!(!wifi_last_seen_is_recent(969, 1_000));
        assert!(!wifi_last_seen_is_recent(-1, 1_000));
    }

    #[test]
    fn connected_access_point_wins_over_stronger_duplicate_ssid() {
        let mut networks = vec![WifiNetwork {
            ssid: "DELTA".into(),
            strength: 90,
            security: WifiSecurity::Personal,
            active: false,
            available: true,
            known: true,
        }];
        merge_wifi_network(
            &mut networks,
            WifiNetwork {
                ssid: "DELTA".into(),
                strength: 69,
                security: WifiSecurity::Personal,
                active: true,
                available: true,
                known: true,
            },
        );

        assert_eq!(networks.len(), 1);
        assert!(networks[0].active);
        assert_eq!(networks[0].strength, 69);
    }

    #[test]
    fn validates_hotspot_credentials_before_network_manager() {
        assert!(
            validate_hotspot(
                "Patin",
                super::HotspotSecurity::WpaPersonal,
                Some("eight888")
            )
            .is_ok()
        );
        assert!(validate_hotspot("", super::HotspotSecurity::Open, None).is_err());
        assert!(
            validate_hotspot("Patin", super::HotspotSecurity::WpaPersonal, Some("short")).is_err()
        );
    }
}
