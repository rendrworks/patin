//! Optional network status provider for Patin shells.
//!
//! NetworkManager reports active wifi and wired transports while
//! ModemManager reports independently registered cellular modems. Reading
//! both services lets a shell represent simultaneous wifi and SIM service
//! rather than collapsing everything into one "primary" connection.

use patin::service::Provider;
use std::{collections::HashSet, fmt, process::Command};
use zbus::zvariant::OwnedObjectPath;

const HOTSPOT_PROFILE: &str = "Patin Hotspot";

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
        let known_ssids = wifi_profiles()?.into_iter().map(|(_, ssid)| ssid).collect();
        Ok(filter_known_wifi_networks(networks, &known_ssids))
    }

    pub fn scan_wifi_networks(&self) -> Result<Vec<WifiNetwork>, NetworkError> {
        self.visible_wifi_networks(true)
    }

    fn visible_wifi_networks(&self, rescan: bool) -> Result<Vec<WifiNetwork>, NetworkError> {
        let output = nmcli(&[
            "--terse",
            "--escape",
            "yes",
            "--fields",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            if rescan { "yes" } else { "no" },
        ])?;
        let mut networks = Vec::<WifiNetwork>::new();
        for line in output.lines() {
            let fields = split_escaped(line);
            if fields.len() != 4 || fields[1].is_empty() {
                continue;
            }
            let security = if fields[3].is_empty() || fields[3] == "--" {
                WifiSecurity::Open
            } else if fields[3].contains("WPA") || fields[3].contains("SAE") {
                WifiSecurity::Personal
            } else {
                WifiSecurity::Unsupported
            };
            let candidate = WifiNetwork {
                ssid: fields[1].clone(),
                strength: fields[2].parse().unwrap_or(0),
                security,
                active: fields[0] == "*" || fields[0] == "yes",
            };
            if let Some(existing) = networks
                .iter_mut()
                .find(|network| network.ssid == candidate.ssid)
            {
                if candidate.strength > existing.strength {
                    *existing = candidate;
                }
            } else {
                networks.push(candidate);
            }
        }
        networks.sort_by_key(|network| {
            (
                std::cmp::Reverse(network.active),
                std::cmp::Reverse(network.strength),
            )
        });
        Ok(networks)
    }

    pub fn set_wifi_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        nmcli(&["radio", "wifi", if enabled { "on" } else { "off" }]).map(|_| ())
    }

    pub fn set_cellular_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        nmcli(&["radio", "wwan", if enabled { "on" } else { "off" }]).map(|_| ())
    }

    pub fn connect_wifi(&self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
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

fn filter_known_wifi_networks(
    networks: Vec<WifiNetwork>,
    known_ssids: &HashSet<String>,
) -> Vec<WifiNetwork> {
    networks
        .into_iter()
        .filter(|network| network.active || known_ssids.contains(&network.ssid))
        .collect()
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

fn wifi_profiles() -> Result<Vec<(String, String)>, NetworkError> {
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
            nmcli(&[
                "--get-values",
                "802-11-wireless.ssid",
                "connection",
                "show",
                "uuid",
                &uuid,
            ])
            .ok()
            .map(|ssid| (uuid, ssid.trim().to_owned()))
        })
        .filter(|(_, ssid)| !ssid.is_empty())
        .collect())
}

fn wifi_profile_uuid<'a>(profiles: &'a [(String, String)], ssid: &str) -> Option<&'a str> {
    profiles
        .iter()
        .find(|(_, profile_ssid)| profile_ssid == ssid)
        .map(|(uuid, _)| uuid.as_str())
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
    use std::collections::HashSet;

    use super::{
        NetworkProvider, NetworkSnapshot, Provider, WifiNetwork, WifiSecurity,
        filter_known_wifi_networks, split_escaped, validate_hotspot, wifi_profile_uuid,
        wifi_profile_uuids,
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
            ("delta-uuid".into(), "DELTA-6c60c4".into()),
            ("ziggo-uuid".into(), "Ziggo7827342".into()),
        ];
        assert_eq!(
            wifi_profile_uuid(&profiles, "Ziggo7827342"),
            Some("ziggo-uuid")
        );
        assert_eq!(wifi_profile_uuid(&profiles, "Unknown"), None);
    }

    #[test]
    fn known_list_keeps_connected_and_saved_visible_networks() {
        let network = |ssid: &str, active| WifiNetwork {
            ssid: ssid.into(),
            strength: 50,
            security: WifiSecurity::Personal,
            active,
        };
        let known_ssids = HashSet::from(["Home".to_owned()]);

        assert_eq!(
            filter_known_wifi_networks(
                vec![
                    network("Connected", true),
                    network("Home", false),
                    network("New cafe", false),
                ],
                &known_ssids,
            )
            .iter()
            .map(|network| network.ssid.as_str())
            .collect::<Vec<_>>(),
            ["Connected", "Home"]
        );
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
