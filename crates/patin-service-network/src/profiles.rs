//! Saved connection profiles and the merge rules that reconcile them with
//! what the radio can currently see — including the freshness window that
//! decides whether a remembered access point still counts as in range.

use std::fs;

use crate::nmcli::{nmcli, split_escaped};
use crate::{NetworkError, WIFI_FRESHNESS_SECONDS, WifiNetwork, WifiProfile, WifiSecurity};

pub(crate) fn merge_wifi_profiles(
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

pub(crate) fn merge_wifi_network(networks: &mut Vec<WifiNetwork>, candidate: WifiNetwork) {
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

pub(crate) fn wifi_profile_uuids(profiles: &str) -> Vec<String> {
    profiles
        .lines()
        .filter_map(|line| {
            let fields = split_escaped(line);
            (fields.len() == 2 && fields[1] == "802-11-wireless" && !fields[0].is_empty())
                .then(|| fields[0].clone())
        })
        .collect()
}

pub(crate) fn wifi_profiles() -> Result<Vec<WifiProfile>, NetworkError> {
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

pub(crate) fn parse_wifi_profile(uuid: &str, values: &str) -> Option<WifiProfile> {
    let mut lines = values.lines();
    let ssid = unescape_field(lines.next()?).trim().to_owned();
    let mode = lines.next().unwrap_or_default().trim();
    (!ssid.is_empty() && mode != "ap").then(|| WifiProfile {
        uuid: uuid.into(),
        ssid,
    })
}

pub(crate) fn unescape_field(value: &str) -> String {
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

pub(crate) fn system_uptime_seconds() -> Option<i32> {
    fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

pub(crate) fn wifi_last_seen_is_recent(last_seen: i32, uptime: i32) -> bool {
    last_seen >= 0 && uptime.saturating_sub(last_seen) <= WIFI_FRESHNESS_SECONDS
}

pub(crate) fn wifi_profile_uuid<'a>(profiles: &'a [WifiProfile], ssid: &str) -> Option<&'a str> {
    profiles
        .iter()
        .find(|profile| profile.ssid == ssid)
        .map(|profile| profile.uuid.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        merge_wifi_network, merge_wifi_profiles, parse_wifi_profile, wifi_last_seen_is_recent,
        wifi_profile_uuid, wifi_profile_uuids,
    };
    use crate::{WifiNetwork, WifiProfile, WifiSecurity};

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
}
