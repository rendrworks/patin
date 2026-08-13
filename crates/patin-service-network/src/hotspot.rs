//! The hotspot (Wi-Fi access point) profile: reading its current
//! configuration, validating and saving changes, and turning it on or off.

use crate::nmcli::nmcli;
use crate::{
    HOTSPOT_PROFILE, HotspotBand, HotspotConfig, HotspotSecurity, NetworkError, NetworkProvider,
};

impl NetworkProvider {
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
    use super::validate_hotspot;

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
