use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProtocolFamilyVersion {
    pub family: &'static str,
    pub major: u32,
    pub minor: u32,
}

impl ProtocolFamilyVersion {
    pub const fn new(family: &'static str, major: u32, minor: u32) -> Self {
        Self {
            family,
            major,
            minor,
        }
    }

    pub fn is_compatible_with(self, requested: &RequestedProtocolVersion) -> bool {
        self.family == requested.family
            && self.major == requested.major
            && requested.minor <= self.minor
    }
}

impl fmt::Display for ProtocolFamilyVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}.{}", self.family, self.major, self.minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequestedProtocolVersion {
    pub family: String,
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for RequestedProtocolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}.{}", self.family, self.major, self.minor)
    }
}

pub const RPC_PROTOCOL_VERSION: ProtocolFamilyVersion = ProtocolFamilyVersion::new("rpc", 1, 0);
pub const PRODUCT_EVENT_PROTOCOL_VERSION: ProtocolFamilyVersion =
    ProtocolFamilyVersion::new("product_event", 1, 0);
pub const UI_SNAPSHOT_PROTOCOL_VERSION: ProtocolFamilyVersion =
    ProtocolFamilyVersion::new("ui_snapshot", 1, 0);
