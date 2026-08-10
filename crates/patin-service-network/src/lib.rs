//! Optional iwd and ModemManager adapter for Patin shells.
//!
//! iwd owns Wi-Fi station/AP state and, when configured to do so, IP
//! configuration. ModemManager independently reports and controls the modem.

use patin::service::Provider;
use std::{
    collections::HashMap,
    fmt, fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use zbus::zvariant::OwnedObjectPath;

const IWD_SERVICE: &str = "net.connman.iwd";
const IWD_ROOT: &str = "/net/connman/iwd";
const DEVICE_INTERFACE: &str = "net.connman.iwd.Device";
const STATION_INTERFACE: &str = "net.connman.iwd.Station";
const NETWORK_INTERFACE: &str = "net.connman.iwd.Network";
const KNOWN_NETWORK_INTERFACE: &str = "net.connman.iwd.KnownNetwork";
const ACCESS_POINT_INTERFACE: &str = "net.connman.iwd.AccessPoint";
const AGENT_PATH: &str = "/org/patin/IwdAgent";

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

struct CredentialAgent {
    passphrase: Arc<Mutex<Option<String>>>,
}

#[zbus::interface(name = "net.connman.iwd.Agent")]
impl CredentialAgent {
    fn release(&self) {}

    fn request_passphrase(&self, _network: OwnedObjectPath) -> zbus::fdo::Result<String> {
        self.passphrase
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("credential state is unavailable".into()))?
            .clone()
            .ok_or_else(|| zbus::fdo::Error::Failed("no passphrase was supplied".into()))
    }

    fn cancel(&self, _reason: &str) {}
}

pub struct NetworkProvider {
    connection: Option<zbus::blocking::Connection>,
    passphrase: Arc<Mutex<Option<String>>>,
    agent_registered: AtomicBool,
}

impl NetworkProvider {
    pub fn new() -> Self {
        let connection = zbus::blocking::Connection::system().ok();
        let passphrase = Arc::new(Mutex::new(None));
        Self {
            connection,
            passphrase,
            agent_registered: AtomicBool::new(false),
        }
    }

    pub fn wifi_networks(&self) -> Result<Vec<WifiNetwork>, NetworkError> {
        let connection = self.connection()?;
        let (_, station_path) = station_device(connection)?;
        let station = proxy(connection, &station_path, STATION_INTERFACE)?;
        let _: Result<(), zbus::Error> = station.call("Scan", &());
        let ordered: Vec<(OwnedObjectPath, i16)> = station
            .call("GetOrderedNetworks", &())
            .map_err(dbus_error("could not read iwd scan results"))?;
        let mut networks = Vec::new();
        for (path, signal) in ordered {
            let network = proxy(connection, &path, NETWORK_INTERFACE)?;
            let ssid: String = network
                .get_property("Name")
                .map_err(dbus_error("could not read iwd network name"))?;
            let kind: String = network.get_property("Type").unwrap_or_default();
            let connected: bool = network.get_property("Connected").unwrap_or(false);
            networks.push(WifiNetwork {
                ssid,
                strength: signal_percentage(signal),
                security: security_from_iwd(&kind),
                active: connected,
            });
        }
        Ok(networks)
    }

    pub fn set_wifi_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let (device, _) = any_iwd_device(connection)?;
        proxy(connection, &device, DEVICE_INTERFACE)?
            .set_property("Powered", enabled)
            .map_err(dbus_error("could not change iwd radio power"))
    }

    pub fn set_cellular_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let modem = modem_path(connection)
            .ok_or_else(|| NetworkError("ModemManager did not expose a modem".into()))?;
        let modem = proxy(connection, &modem, "org.freedesktop.ModemManager1.Modem")?;
        modem
            .call::<_, _, ()>("Enable", &(enabled))
            .map_err(dbus_error("could not change modem power"))
    }

    pub fn connect_wifi(&self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let network_path = find_network(connection, ssid)?;
        let network = proxy(connection, &network_path, NETWORK_INTERFACE)?;
        if let Some(password) = password {
            self.register_agent();
            if !self.agent_registered.load(Ordering::SeqCst) {
                return Err(NetworkError(
                    "iwd rejected Patin's credential agent; another agent may be active".into(),
                ));
            }
            *self
                .passphrase
                .lock()
                .map_err(|_| NetworkError("credential state is unavailable".into()))? =
                Some(password.into());
        }
        let result = network
            .call::<_, _, ()>("Connect", &())
            .map_err(dbus_error("iwd could not connect to the network"));
        if let Ok(mut secret) = self.passphrase.lock() {
            *secret = None;
        }
        result
    }

    pub fn disconnect_wifi(&self) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let (_, station_path) = station_device(connection)?;
        proxy(connection, &station_path, STATION_INTERFACE)?
            .call::<_, _, ()>("Disconnect", &())
            .map_err(dbus_error("iwd could not disconnect the station"))
    }

    pub fn forget_wifi(&self, ssid: &str) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let objects = iwd_objects(connection)?;
        for (path, interfaces) in objects {
            if !interfaces.contains_key(KNOWN_NETWORK_INTERFACE) {
                continue;
            }
            let known = proxy(connection, &path, KNOWN_NETWORK_INTERFACE)?;
            let name: String = known.get_property("Name").unwrap_or_default();
            if name == ssid {
                return known
                    .call::<_, _, ()>("Forget", &())
                    .map_err(dbus_error("iwd could not forget the network"));
            }
        }
        Err(NetworkError(format!(
            "iwd has no saved network named {ssid}"
        )))
    }

    pub fn hotspot_config(&self) -> HotspotConfig {
        read_hotspot().map_or_else(
            || HotspotConfig {
                ssid: "Patin".into(),
                password_configured: false,
                security: HotspotSecurity::WpaPersonal,
                band: HotspotBand::Automatic,
            },
            |stored| HotspotConfig {
                ssid: stored.ssid,
                password_configured: true,
                security: HotspotSecurity::WpaPersonal,
                band: HotspotBand::Automatic,
            },
        )
    }

    pub fn save_hotspot(
        &self,
        config: &HotspotConfig,
        password: Option<&str>,
    ) -> Result<(), NetworkError> {
        validate_hotspot(config, password)?;
        let existing = read_hotspot();
        let password = password
            .map(str::to_owned)
            .or_else(|| existing.map(|stored| stored.password))
            .ok_or_else(|| NetworkError("set a hotspot password before saving".into()))?;
        write_hotspot(&StoredHotspot {
            ssid: config.ssid.clone(),
            password,
        })
    }

    pub fn set_hotspot_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        let connection = self.connection()?;
        let (device_path, _) = any_iwd_device(connection)?;
        let device = proxy(connection, &device_path, DEVICE_INTERFACE)?;
        if enabled {
            if !iwd_network_configuration_enabled(connection) {
                return Err(NetworkError(
                    "enable iwd network configuration before starting a hotspot".into(),
                ));
            }
            let stored = read_hotspot()
                .ok_or_else(|| NetworkError("save hotspot settings before enabling it".into()))?;
            device
                .set_property("Mode", "ap")
                .map_err(dbus_error("iwd could not switch the device to AP mode"))?;
            let mut last_error = None;
            for _ in 0..10 {
                let access_point = proxy(connection, &device_path, ACCESS_POINT_INTERFACE)?;
                match access_point
                    .call::<_, _, ()>("Start", &(stored.ssid.clone(), stored.password.clone()))
                {
                    Ok(()) => return Ok(()),
                    Err(error) => last_error = Some(error),
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(NetworkError(format!(
                "iwd could not start the hotspot: {}",
                last_error.expect("the AP start loop runs at least once")
            )))
        } else {
            if let Ok(access_point) = proxy(connection, &device_path, ACCESS_POINT_INTERFACE) {
                access_point
                    .call::<_, _, ()>("Stop", &())
                    .map_err(dbus_error("iwd could not stop the hotspot"))?;
            }
            device
                .set_property("Mode", "station")
                .map_err(dbus_error("iwd could not return to station mode"))
        }
    }

    fn connection(&self) -> Result<&zbus::blocking::Connection, NetworkError> {
        self.connection
            .as_ref()
            .ok_or_else(|| NetworkError("the system D-Bus is unavailable".into()))
    }

    fn register_agent(&self) {
        if self.agent_registered.load(Ordering::SeqCst) {
            return;
        }
        let Some(connection) = &self.connection else {
            return;
        };
        if connection
            .object_server()
            .at(
                AGENT_PATH,
                CredentialAgent {
                    passphrase: self.passphrase.clone(),
                },
            )
            .is_err()
        {
            return;
        }
        let Ok(manager) = zbus::blocking::Proxy::new(
            connection,
            IWD_SERVICE,
            IWD_ROOT,
            "net.connman.iwd.AgentManager",
        ) else {
            return;
        };
        let Ok(path) = OwnedObjectPath::try_from(AGENT_PATH) else {
            return;
        };
        self.agent_registered.store(
            manager.call::<_, _, ()>("RegisterAgent", &(path)).is_ok(),
            Ordering::SeqCst,
        );
    }
}

impl Default for NetworkProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NetworkProvider {
    fn drop(&mut self) {
        if !self.agent_registered.load(Ordering::SeqCst) {
            return;
        }
        let Some(connection) = &self.connection else {
            return;
        };
        let Ok(manager) = zbus::blocking::Proxy::new(
            connection,
            IWD_SERVICE,
            IWD_ROOT,
            "net.connman.iwd.AgentManager",
        ) else {
            return;
        };
        if let Ok(path) = OwnedObjectPath::try_from(AGENT_PATH) {
            let _ = manager.call::<_, _, ()>("UnregisterAgent", &(path));
        }
    }
}

impl Provider for NetworkProvider {
    type Snapshot = Option<NetworkSnapshot>;

    fn poll(&mut self) -> Self::Snapshot {
        let connection = self.connection.as_ref()?;
        let mut snapshot = iwd_snapshot(connection).unwrap_or_default();
        let (available, enabled, strength) = modem_snapshot(connection);
        snapshot.cellular_available = available;
        snapshot.cellular_enabled = enabled;
        snapshot.cellular = strength;
        snapshot.wired = wired_connected();
        Some(snapshot)
    }
}

type Interfaces =
    HashMap<zbus::names::OwnedInterfaceName, HashMap<String, zbus::zvariant::OwnedValue>>;
type Objects = HashMap<OwnedObjectPath, Interfaces>;

fn iwd_objects(connection: &zbus::blocking::Connection) -> Result<Objects, NetworkError> {
    let manager = zbus::blocking::fdo::ObjectManagerProxy::builder(connection)
        .destination(IWD_SERVICE)
        .map_err(dbus_error("invalid iwd destination"))?
        .path("/")
        .map_err(dbus_error("invalid iwd object-manager path"))?
        .build()
        .map_err(dbus_error("could not create iwd object manager"))?;
    manager
        .get_managed_objects()
        .map_err(dbus_error("could not enumerate iwd objects"))
}

fn proxy<'a>(
    connection: &'a zbus::blocking::Connection,
    path: &'a OwnedObjectPath,
    interface: &'a str,
) -> Result<zbus::blocking::Proxy<'a>, NetworkError> {
    zbus::blocking::Proxy::new(connection, IWD_SERVICE, path.as_str(), interface)
        .map_err(dbus_error("could not create iwd proxy"))
}

fn any_iwd_device(
    connection: &zbus::blocking::Connection,
) -> Result<(OwnedObjectPath, Option<OwnedObjectPath>), NetworkError> {
    let objects = iwd_objects(connection)?;
    let mut fallback = None;
    for (path, interfaces) in objects {
        if interfaces.contains_key(DEVICE_INTERFACE) {
            let station = interfaces
                .contains_key(STATION_INTERFACE)
                .then(|| path.clone());
            if station.is_some() {
                return Ok((path, station));
            }
            fallback.get_or_insert(path);
        }
    }
    if let Some(path) = fallback {
        return Ok((path, None));
    }
    Err(NetworkError("iwd did not expose a Wi-Fi device".into()))
}

fn station_device(
    connection: &zbus::blocking::Connection,
) -> Result<(OwnedObjectPath, OwnedObjectPath), NetworkError> {
    let (device, station) = any_iwd_device(connection)?;
    station
        .map(|station| (device, station))
        .ok_or_else(|| NetworkError("the iwd device is not in station mode".into()))
}

fn find_network(
    connection: &zbus::blocking::Connection,
    ssid: &str,
) -> Result<OwnedObjectPath, NetworkError> {
    let (_, station_path) = station_device(connection)?;
    let station = proxy(connection, &station_path, STATION_INTERFACE)?;
    let ordered: Vec<(OwnedObjectPath, i16)> = station
        .call("GetOrderedNetworks", &())
        .map_err(dbus_error("could not read iwd scan results"))?;
    for (path, _) in ordered {
        let network = proxy(connection, &path, NETWORK_INTERFACE)?;
        let name: String = network.get_property("Name").unwrap_or_default();
        if name == ssid {
            return Ok(path.clone());
        }
    }
    Err(NetworkError(format!("iwd cannot currently see {ssid}")))
}

fn iwd_snapshot(connection: &zbus::blocking::Connection) -> Option<NetworkSnapshot> {
    let objects = iwd_objects(connection).ok()?;
    let mut snapshot = NetworkSnapshot::default();
    for (path, interfaces) in objects {
        if !interfaces.contains_key(DEVICE_INTERFACE) {
            continue;
        }
        snapshot.wifi_available = true;
        let device = proxy(connection, &path, DEVICE_INTERFACE).ok()?;
        snapshot.wifi_enabled |= device.get_property("Powered").unwrap_or(false);
        let mode: String = device.get_property("Mode").unwrap_or_default();
        if mode == "ap" && interfaces.contains_key(ACCESS_POINT_INTERFACE) {
            snapshot.hotspot_active = proxy(connection, &path, ACCESS_POINT_INTERFACE)
                .ok()
                .and_then(|ap| ap.get_property("Started").ok())
                .unwrap_or(false);
        } else if interfaces.contains_key(STATION_INTERFACE) {
            let station = proxy(connection, &path, STATION_INTERFACE).ok()?;
            let ordered: Vec<(OwnedObjectPath, i16)> =
                station.call("GetOrderedNetworks", &()).unwrap_or_default();
            for (network_path, signal) in ordered {
                let network = proxy(connection, &network_path, NETWORK_INTERFACE).ok()?;
                if network.get_property("Connected").unwrap_or(false) {
                    snapshot.wifi = Some(signal_percentage(signal));
                    break;
                }
            }
        }
    }
    Some(snapshot)
}

fn iwd_network_configuration_enabled(connection: &zbus::blocking::Connection) -> bool {
    let Ok(daemon) =
        zbus::blocking::Proxy::new(connection, IWD_SERVICE, IWD_ROOT, "net.connman.iwd.Daemon")
    else {
        return false;
    };
    let Ok(mut info) =
        daemon.call::<_, _, HashMap<String, zbus::zvariant::OwnedValue>>("GetInfo", &())
    else {
        return false;
    };
    info.remove("NetworkConfigurationEnabled")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false)
}

fn security_from_iwd(kind: &str) -> WifiSecurity {
    match kind {
        "open" => WifiSecurity::Open,
        "psk" => WifiSecurity::Personal,
        _ => WifiSecurity::Unsupported,
    }
}

fn signal_percentage(signal: i16) -> u8 {
    let dbm = f32::from(signal) / 100.0;
    (((dbm + 100.0) / 70.0) * 100.0).round().clamp(0.0, 100.0) as u8
}

fn modem_path(connection: &zbus::blocking::Connection) -> Option<OwnedObjectPath> {
    const MODEM_INTERFACE: &str = "org.freedesktop.ModemManager1.Modem";
    let manager = zbus::blocking::fdo::ObjectManagerProxy::builder(connection)
        .destination("org.freedesktop.ModemManager1")
        .ok()?
        .path("/org/freedesktop/ModemManager1")
        .ok()?
        .build()
        .ok()?;
    manager
        .get_managed_objects()
        .ok()?
        .into_iter()
        .find_map(|(path, interfaces)| interfaces.contains_key(MODEM_INTERFACE).then_some(path))
}

fn modem_snapshot(connection: &zbus::blocking::Connection) -> (bool, bool, Option<u8>) {
    const MODEM_INTERFACE: &str = "org.freedesktop.ModemManager1.Modem";
    const ENABLED: i32 = 6;
    const REGISTERED: i32 = 8;
    let Some(path) = modem_path(connection) else {
        return (false, false, None);
    };
    let Ok(modem) = zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.ModemManager1",
        path.as_str(),
        MODEM_INTERFACE,
    ) else {
        return (true, false, None);
    };
    let state: i32 = modem.get_property("State").unwrap_or(0);
    let signal = (state >= REGISTERED)
        .then(|| modem.get_property::<(u32, bool)>("SignalQuality").ok())
        .flatten()
        .map(|(percentage, _)| percentage.min(100) as u8);
    (true, state >= ENABLED, signal)
}

fn wired_connected() -> bool {
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        !path.join("wireless").exists()
            && entry.file_name() != "lo"
            && fs::read_to_string(path.join("carrier")).is_ok_and(|carrier| carrier.trim() == "1")
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredHotspot {
    ssid: String,
    password: String,
}

fn hotspot_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|root| root.join("patin/hotspot.conf"))
}

fn read_hotspot() -> Option<StoredHotspot> {
    let contents = fs::read_to_string(hotspot_path()?).ok()?;
    let mut lines = contents.lines();
    Some(StoredHotspot {
        ssid: decode_hex(lines.next()?)?,
        password: decode_hex(lines.next()?)?,
    })
}

fn write_hotspot(hotspot: &StoredHotspot) -> Result<(), NetworkError> {
    let path = hotspot_path()
        .ok_or_else(|| NetworkError("no user configuration directory is available".into()))?;
    let parent = path
        .parent()
        .ok_or_else(|| NetworkError("invalid hotspot configuration path".into()))?;
    fs::create_dir_all(parent).map_err(|error| {
        NetworkError(format!("could not create Patin config directory: {error}"))
    })?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| NetworkError(format!("could not save hotspot settings: {error}")))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| NetworkError(format!("could not protect hotspot settings: {error}")))?;
    writeln!(
        file,
        "{}\n{}",
        encode_hex(&hotspot.ssid),
        encode_hex(&hotspot.password)
    )
    .map_err(|error| NetworkError(format!("could not save hotspot settings: {error}")))
}

fn encode_hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

fn validate_hotspot(config: &HotspotConfig, password: Option<&str>) -> Result<(), NetworkError> {
    if config.ssid.is_empty() || config.ssid.len() > 32 {
        return Err(NetworkError(
            "hotspot SSID must contain 1 to 32 bytes".into(),
        ));
    }
    if config.security != HotspotSecurity::WpaPersonal {
        return Err(NetworkError(
            "iwd AP mode requires WPA-personal security".into(),
        ));
    }
    if config.band != HotspotBand::Automatic {
        return Err(NetworkError(
            "iwd's dynamic AP API chooses the band automatically".into(),
        ));
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

fn dbus_error<E: fmt::Display>(prefix: &'static str) -> impl FnOnce(E) -> NetworkError {
    move |error| NetworkError(format!("{prefix}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut provider = NetworkProvider {
            connection: None,
            passphrase: Arc::new(Mutex::new(None)),
            agent_registered: AtomicBool::new(false),
        };
        assert_eq!(provider.poll(), None);
    }

    #[test]
    fn maps_iwd_signal_strength_to_a_bounded_percentage() {
        assert_eq!(signal_percentage(0), 100);
        assert_eq!(signal_percentage(-10_000), 0);
        assert!(signal_percentage(-6_500) > signal_percentage(-8_000));
    }

    #[test]
    fn hotspot_storage_encoding_round_trips_delimiters() {
        let value = "Phone: café\nline";
        assert_eq!(decode_hex(&encode_hex(value)).as_deref(), Some(value));
    }

    #[test]
    fn validates_iwd_hotspot_constraints() {
        let config = HotspotConfig {
            ssid: "Patin".into(),
            password_configured: false,
            security: HotspotSecurity::WpaPersonal,
            band: HotspotBand::Automatic,
        };
        assert!(validate_hotspot(&config, Some("eight888")).is_ok());
        assert!(validate_hotspot(&config, Some("short")).is_err());
    }
}
