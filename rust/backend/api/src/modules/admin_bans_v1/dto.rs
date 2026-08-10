use std::net::IpAddr;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use validator::Validate;

// One canonical ban unit: an IPv4 address collapses to a /32, an IPv6 address
// collapses to its /64 network (host bits truncated). Fixed limiting, abuse
// history, active bans, persistence, inspection, and export all key on this same
// string, so a /64 CGNAT range and every address inside it resolve to one unit.
// The Display form of IpNet is already canonical ("203.0.113.5/32",
// "2001:db8::/64") and carries no characters that URL-encode, so it round-trips
// through JSON verbatim. parse() accepts only a /32 or /64 width, so an
// unparseable or wrong-width store key (email, user-id, totp, fixed-rate) can
// never pose as an exportable ban unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BanUnit(IpNet);

impl BanUnit {
    pub fn from_ip(ip: IpAddr) -> Option<Self> {
        let net = match ip {
            IpAddr::V4(v4) => IpNet::V4(ipnet::Ipv4Net::new(v4, 32).ok()?),
            IpAddr::V6(v6) => {
                let n = ipnet::Ipv6Net::new(v6, 64).ok()?;
                IpNet::V6(n.trunc())
            }
        };
        Some(Self(net))
    }

    pub fn parse(s: &str) -> Option<Self> {
        let net: IpNet = s.parse().ok()?;
        match net {
            IpNet::V4(v4) if v4.prefix_len() == 32 => Some(Self(IpNet::V4(v4))),
            IpNet::V6(v6) if v6.prefix_len() == 64 => Some(Self(IpNet::V6(v6.trunc()))),
            _ => None,
        }
    }

    pub fn canonical(&self) -> String {
        self.0.to_string()
    }
}

impl Serialize for BanUnit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.canonical())
    }
}

impl<'de> Deserialize<'de> for BanUnit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("invalid banUnit: {raw}")))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BanRow {
    pub ban_unit: String,
    pub scope: String,
    pub block_until_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BanListResponse {
    pub data: Vec<BanRow>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBanPayload {
    #[validate(length(min = 1, max = 64, message = "banUnit is required"))]
    pub ban_unit: String,
}

// Export neutral JSON: the deployment-owned proxy adapter consumes canonical
// ban units + scope + absolute expiry. Field names are intentionally
// self-describing so an external Traefik/CF adapter never has to guess.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ExportRow {
    pub ban_unit: String,
    pub scope: String,
    pub expires_at: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ExportResponse {
    pub bans: Vec<ExportRow>,
}
