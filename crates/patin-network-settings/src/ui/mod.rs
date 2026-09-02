//! The network settings composition: Wi-Fi, mobile data, and hotspot
//! controls on three switchable pages.
//!
//! This root owns the state and the shared vocabulary; the work is split
//! into [`layout`] (where the controls go), [`actions`] (what they do),
//! and [`shell`] (the Patin lifecycle and drawing).

mod actions;
mod layout;
mod shell;
#[cfg(test)]
mod tests;

use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, Size, TextAlign};
use patin_icons::WifiSignal;
use patin_service_network::{HotspotConfig, NetworkProvider, NetworkSnapshot, WifiNetwork};
use zeroize::Zeroizing;

const ROW: f32 = 52.0;
const WIFI_REFRESH_TICKS: u8 = 2;
const WIFI_SCAN_TICKS: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Wifi,
    Cellular,
    Hotspot,
}

#[derive(Clone, Debug)]
enum Action {
    Close,
    WifiPage,
    CellularPage,
    HotspotPage,
    ToggleWifi,
    ScanWifi,
    ToggleCellular,
    Connect(usize),
    Disconnect,
    Forget(usize),
    EditHotspotSsid,
    EditHotspotPassword,
    ToggleHotspotSecurity,
    ToggleHotspotBand,
    SaveHotspot,
    ToggleHotspot,
}

#[derive(Clone, Debug)]
struct Button {
    bounds: Rect,
    label: String,
    action: Action,
    text_align: TextAlign,
    text_inset: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Editing {
    WifiPassword(usize),
    HotspotSsid,
    HotspotPassword,
}

pub struct NetworkSettings {
    size: Size,
    page: Page,
    provider: NetworkProvider,
    snapshot: NetworkSnapshot,
    networks: Vec<WifiNetwork>,
    hotspot: HotspotConfig,
    hotspot_password: Zeroizing<String>,
    wifi_password: Zeroizing<String>,
    editing: Option<Editing>,
    buttons: Vec<Button>,
    wifi_icons: Vec<(Rect, WifiSignal)>,
    error: Option<String>,
    initial_refresh_pending: bool,
    scan_pending: bool,
    show_discovered: bool,
    wifi_refresh_ticks: u8,
    wifi_scan_ticks: u8,
    close: bool,
    damage: Vec<Rect>,
}

impl NetworkSettings {
    pub fn new(page: Option<&str>) -> Self {
        let provider = NetworkProvider::new();
        let mut settings = Self {
            size: Size::default(),
            page: match page {
                Some("cellular") => Page::Cellular,
                Some("hotspot") => Page::Hotspot,
                _ => Page::Wifi,
            },
            provider,
            snapshot: NetworkSnapshot::default(),
            networks: Vec::new(),
            hotspot: HotspotConfig::default(),
            hotspot_password: Zeroizing::new(String::new()),
            wifi_password: Zeroizing::new(String::new()),
            editing: None,
            buttons: Vec::new(),
            wifi_icons: Vec::new(),
            error: None,
            initial_refresh_pending: true,
            scan_pending: false,
            show_discovered: false,
            wifi_refresh_ticks: 0,
            wifi_scan_ticks: 0,
            close: false,
            damage: Vec::new(),
        };
        settings.layout();
        settings
    }

    fn redraw(&mut self) {
        self.layout();
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

fn text(bounds: Rect, value: &str, size: f32) -> DrawCommand {
    aligned_text(bounds, value, size, TextAlign::Start)
}

fn wifi_refresh_due(page: Page, ticks: &mut u8) -> bool {
    if page != Page::Wifi {
        return false;
    }
    *ticks = ticks.saturating_add(1);
    if *ticks < WIFI_REFRESH_TICKS {
        return false;
    }
    *ticks = 0;
    true
}

fn wifi_scan_due(page: Page, ticks: &mut u8) -> bool {
    if page != Page::Wifi {
        return false;
    }
    *ticks = ticks.saturating_add(1);
    if *ticks < WIFI_SCAN_TICKS {
        return false;
    }
    *ticks = 0;
    true
}

fn aligned_text(bounds: Rect, value: &str, size: f32, align: TextAlign) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color: Color(245, 243, 255, 255),
        font_size: size,
        line_height: size * 1.25,
        family: FontFamily::SansSerif,
        weight: FontWeight::Normal,
        align,
    }
}
