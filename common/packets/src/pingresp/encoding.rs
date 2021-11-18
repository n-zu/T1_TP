#![allow(dead_code)]

use super::*;
use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::PacketResult,
    traits::{MQTTBytes, MQTTEncoding},
};

#[doc(hidden)]
const PINGRESP_PACKET_TYPE: u8 = 0b11010000;
#[doc(hidden)]
const RESERVED_BITS: u8 = 0b00000000;
#[doc(hidden)]
const REMAINING_LENGTH: u8 = 0b00000000;

impl MQTTEncoding for PingResp {
    /// Encodes a PingResp packet
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let control_byte = build_control_byte(PacketType::PingResp, RESERVED_BITS);
        let bytes = vec![control_byte, REMAINING_LENGTH];
        Ok(bytes)
    }
}

impl PingResp {
    /// Creates a new PingResp packet
    pub fn new() -> PingResp {
        PingResp
    }
}

impl Default for PingResp {
    fn default() -> Self {
        Self::new()
    }
}
