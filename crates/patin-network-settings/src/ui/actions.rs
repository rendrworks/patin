//! What the controls do: editing and submitting a text field, and running
//! each [`Action`] against the network provider (including the optimistic
//! state updates that keep the view responsive while nmcli catches up).

use patin::platform::KeyInput;
use patin_service_network::{HotspotBand, HotspotSecurity, WifiSecurity};

use super::{
    Action, Editing, NetworkSettings, Page, WIFI_REFRESH_TICKS, WIFI_SCAN_TICKS,
};

impl NetworkSettings {
    fn result(&mut self, result: Result<(), impl std::fmt::Display>) {
        self.error = result.err().map(|error| error.to_string());
        self.redraw();
    }

    pub(super) fn edit_input(&mut self, input: KeyInput) {
        let Some(editing) = self.editing else { return };
        let target: &mut String = match editing {
            Editing::WifiPassword(_) => &mut self.wifi_password,
            Editing::HotspotSsid => &mut self.hotspot.ssid,
            Editing::HotspotPassword => &mut self.hotspot_password,
        };
        match input {
            KeyInput::Text(text) => {
                for character in text.chars() {
                    if target.len() + character.len_utf8() <= 63 {
                        target.push(character);
                    }
                }
            }
            KeyInput::Backspace => {
                target.pop();
            }
            KeyInput::Enter => self.submit_edit(),
            KeyInput::Escape => {
                self.editing = None;
            }
        }
        self.redraw();
    }

    fn submit_edit(&mut self) {
        if let Some(Editing::WifiPassword(index)) = self.editing
            && let Some(network) = self.networks.get(index)
        {
            let result = self
                .provider
                .connect_wifi(&network.ssid, Some(&self.wifi_password));
            if result.is_ok() {
                self.mark_connected(index);
            }
            self.wifi_password.clear();
            self.editing = None;
            self.result(result);
            return;
        }
        self.editing = None;
    }

    pub(super) fn mark_connected(&mut self, index: usize) {
        let strength = self.networks.get(index).map(|network| network.strength);
        for (candidate_index, network) in self.networks.iter_mut().enumerate() {
            network.active = candidate_index == index;
        }
        if let Some(network) = self.networks.get_mut(index) {
            network.available = true;
            network.known = true;
        }
        self.snapshot.wifi = strength;
    }

    pub(super) fn mark_disconnected(&mut self) {
        for network in &mut self.networks {
            network.active = false;
        }
        self.snapshot.wifi = None;
    }

    pub(super) fn act(&mut self, action: Action) {
        match action {
            Action::Close => {
                self.editing = None;
                self.close = true;
            }
            Action::WifiPage => {
                self.page = Page::Wifi;
                self.editing = None;
                self.wifi_refresh_ticks = WIFI_REFRESH_TICKS;
                self.wifi_scan_ticks = WIFI_SCAN_TICKS;
                self.redraw();
            }
            Action::CellularPage => {
                self.page = Page::Cellular;
                self.editing = None;
                self.redraw();
            }
            Action::HotspotPage => {
                self.page = Page::Hotspot;
                self.editing = None;
                self.redraw();
            }
            Action::ToggleWifi => {
                let result = self.provider.set_wifi_enabled(!self.snapshot.wifi_enabled);
                self.result(result);
            }
            Action::ScanWifi => {
                if !self.scan_pending {
                    self.scan_pending = true;
                    self.error = None;
                    self.redraw();
                }
            }
            Action::ToggleCellular => {
                let result = self
                    .provider
                    .set_cellular_enabled(!self.snapshot.cellular_enabled);
                self.result(result);
            }
            Action::Disconnect => {
                let result = self.provider.disconnect_wifi();
                if result.is_ok() {
                    self.mark_disconnected();
                }
                self.result(result);
            }
            Action::Forget(index) => {
                if let Some(network) = self.networks.get(index) {
                    let result = self.provider.forget_wifi(&network.ssid);
                    if result.is_ok() {
                        self.networks.remove(index);
                    }
                    self.result(result);
                }
            }
            Action::Connect(index) => {
                if let Some(network) = self.networks.get(index) {
                    match network.security {
                        WifiSecurity::Open => {
                            let result = self.provider.connect_wifi(&network.ssid, None);
                            if result.is_ok() {
                                self.mark_connected(index);
                            }
                            self.result(result);
                        }
                        WifiSecurity::Personal => {
                            let result = self.provider.connect_wifi(&network.ssid, None);
                            if result.is_ok() {
                                self.mark_connected(index);
                                self.result(result);
                            } else {
                                self.wifi_password.clear();
                                self.editing = Some(Editing::WifiPassword(index));
                                self.redraw();
                            }
                        }
                        WifiSecurity::Unsupported => {
                            self.error = Some(
                                "This network needs unsupported enterprise credentials".into(),
                            );
                            self.redraw();
                        }
                    }
                }
            }
            Action::EditHotspotSsid => {
                self.editing = Some(Editing::HotspotSsid);
                self.redraw();
            }
            Action::EditHotspotPassword => {
                self.hotspot_password.clear();
                self.editing = Some(Editing::HotspotPassword);
                self.redraw();
            }
            Action::ToggleHotspotSecurity => {
                self.hotspot.security = match self.hotspot.security {
                    HotspotSecurity::Open => HotspotSecurity::WpaPersonal,
                    HotspotSecurity::WpaPersonal => HotspotSecurity::Open,
                };
                self.redraw();
            }
            Action::ToggleHotspotBand => {
                self.hotspot.band = match self.hotspot.band {
                    HotspotBand::Automatic => HotspotBand::Ghz2_4,
                    HotspotBand::Ghz2_4 => HotspotBand::Ghz5,
                    HotspotBand::Ghz5 => HotspotBand::Automatic,
                };
                self.redraw();
            }
            Action::SaveHotspot => {
                let password =
                    (!self.hotspot_password.is_empty()).then_some(self.hotspot_password.as_str());
                let result = self.provider.save_hotspot(&self.hotspot, password);
                if result.is_ok() && password.is_some() {
                    self.hotspot.password_configured = true;
                    self.hotspot_password.clear();
                }
                self.result(result);
            }
            Action::ToggleHotspot => {
                let result = self
                    .provider
                    .set_hotspot_enabled(!self.snapshot.hotspot_active);
                self.result(result);
            }
        }
    }
}
