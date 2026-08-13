//! Behavioural tests for the settings composition: page selection, row
//! geometry, action side effects, and the polling cadence.

use super::*;
use patin::platform::{KeyInput, Shell, TextInputPurpose};
use patin::ui::Size;
use patin_icons::WifiSignal;
use patin_service_network::WifiSecurity;

    #[test]
    fn requested_page_is_selected() {
        assert_eq!(NetworkSettings::new(Some("cellular")).page, Page::Cellular);
        assert_eq!(NetworkSettings::new(Some("hotspot")).page, Page::Hotspot);
        assert_eq!(NetworkSettings::new(Some("unknown")).page, Page::Wifi);
    }

    #[test]
    fn construction_defers_network_discovery_until_after_window_creation() {
        let ui = NetworkSettings::new(Some("wifi"));
        assert!(ui.initial_refresh_pending);
        assert!(ui.networks.is_empty());
        assert_eq!(ui.hotspot, HotspotConfig::default());
        assert!(
            ui.buttons
                .iter()
                .any(|button| button.label == "Wi-Fi: loading…")
        );
    }

    #[test]
    fn scan_is_explicit_and_close_is_centered() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        assert!(!ui.scan_pending);
        assert!(ui.buttons.iter().any(|button| {
            matches!(button.action, Action::ScanWifi) && button.label == "Scan for new networks"
        }));
        assert!(ui.buttons.iter().any(|button| {
            matches!(button.action, Action::Close)
                && button.label == "×"
                && button.text_align == TextAlign::Center
        }));

        ui.act(Action::ScanWifi);
        assert!(ui.scan_pending);
        assert!(
            ui.buttons
                .iter()
                .any(|button| button.label == "Scanning for new networks…")
        );
    }

    #[test]
    fn scan_row_keeps_network_buttons_inside_minimum_height() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.snapshot.wifi = Some(80);
        ui.networks = (0..5)
            .map(|index| WifiNetwork {
                ssid: format!("Network {index}"),
                strength: 50,
                security: WifiSecurity::Personal,
                active: index == 0,
                available: true,
                known: true,
            })
            .collect();
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        assert!(ui.buttons.iter().all(|button| {
            button.bounds.origin.y + button.bounds.size.height <= ui.size.height
        }));
    }

    #[test]
    fn disconnect_sits_right_of_wifi_toggle_when_connected() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.snapshot.wifi = Some(69);
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        let wifi = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::ToggleWifi))
            .unwrap();
        let disconnect = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::Disconnect))
            .unwrap();
        assert_eq!(wifi.bounds.origin.y, disconnect.bounds.origin.y);
        assert!(disconnect.bounds.origin.x > wifi.bounds.origin.x);
        assert_eq!(disconnect.label, "Disconnect");
    }

    #[test]
    fn successful_connection_actions_update_visible_active_state_immediately() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.networks = vec![
            WifiNetwork {
                ssid: "DELTA".into(),
                strength: 69,
                security: WifiSecurity::Personal,
                active: true,
                available: true,
                known: true,
            },
            WifiNetwork {
                ssid: "Corner".into(),
                strength: 42,
                security: WifiSecurity::Personal,
                active: false,
                available: true,
                known: true,
            },
        ];

        ui.mark_disconnected();
        assert_eq!(ui.snapshot.wifi, None);
        assert!(ui.networks.iter().all(|network| !network.active));

        ui.mark_connected(1);
        assert_eq!(ui.snapshot.wifi, Some(42));
        assert!(!ui.networks[0].active);
        assert!(ui.networks[1].active);
    }

    #[test]
    fn unavailable_known_network_rows_stay_hidden() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.networks = vec![WifiNetwork {
            ssid: "Corner".into(),
            strength: 0,
            security: WifiSecurity::Unsupported,
            active: false,
            available: false,
            known: true,
        }];
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        assert!(ui.wifi_icons.is_empty());
        assert!(
            !ui.buttons
                .iter()
                .any(|button| matches!(button.action, Action::Connect(_) | Action::Forget(_)))
        );
    }

    #[test]
    fn available_network_rows_use_icons_without_numeric_percentages() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.networks = vec![WifiNetwork {
            ssid: "DELTA".into(),
            strength: 69,
            security: WifiSecurity::Personal,
            active: true,
            available: true,
            known: true,
        }];
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        let row = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::Connect(0)))
            .unwrap();
        assert_eq!(row.label, "DELTA • connected");
        assert!(!row.label.contains('%'));
        assert_eq!(ui.wifi_icons[0].1, WifiSignal::Good);
        let forget = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::Forget(0)))
            .unwrap();
        assert!(forget.bounds.size.width >= 82.0);
        assert!(
            forget.bounds.origin.x + forget.bounds.size.width
                <= ui.size.width - 16.0 + f32::EPSILON
        );
    }

    #[test]
    fn wifi_cache_refresh_is_due_every_two_update_ticks() {
        let mut ticks = 0;
        assert!(!wifi_refresh_due(Page::Wifi, &mut ticks));
        assert!(wifi_refresh_due(Page::Wifi, &mut ticks));
        assert!(!wifi_refresh_due(Page::Cellular, &mut ticks));
    }

    #[test]
    fn background_wifi_scan_is_due_every_ten_update_ticks() {
        let mut ticks = 0;
        for _ in 0..9 {
            assert!(!wifi_scan_due(Page::Wifi, &mut ticks));
        }
        assert!(wifi_scan_due(Page::Wifi, &mut ticks));
    }

    #[test]
    fn hotspot_controls_are_only_on_the_hotspot_page() {
        let wifi = NetworkSettings::new(Some("wifi"));
        assert!(
            !wifi
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleHotspot))
        );

        let hotspot = NetworkSettings::new(Some("hotspot"));
        assert!(
            hotspot
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleHotspot))
        );
        assert!(
            !hotspot
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleWifi))
        );
    }
    #[test]
    fn escape_closes_when_not_editing() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.key_input(KeyInput::Escape);
        assert!(ui.close_requested());
    }

    #[test]
    fn editing_exposes_system_text_input_purpose() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        assert_eq!(ui.text_input(), None);

        ui.editing = Some(Editing::HotspotSsid);
        assert_eq!(ui.text_input(), Some(TextInputPurpose::Normal));

        ui.editing = Some(Editing::HotspotPassword);
        assert_eq!(ui.text_input(), Some(TextInputPurpose::Password));

        ui.key_input(KeyInput::Escape);
        assert_eq!(ui.text_input(), None);
        assert!(!ui.close_requested());
    }
