use crate::{
    helpers::{build_control_byte, PacketType},
    traits::MQTTEncoding,
};

use super::*;

#[doc(hidden)]
const REMAINING_LENGTH: u8 = 0b00000000;

impl MQTTEncoding for PingReq {
    /// Encodes a PingReq packet according to
    /// MQTT V3.1.1 protocol
    fn encode(&self) -> crate::packet_error::PacketResult<crate::traits::MQTTBytes> {
        let control_byte = build_control_byte(PacketType::PingReq, RESERVED_BITS);

        Ok(vec![control_byte, REMAINING_LENGTH])
    }
}
impl PingReq {
    /// Creates a new PingReq packet
    pub fn new() -> PingReq {
        PingReq
    }
}

impl Default for PingReq {
    fn default() -> Self {
        Self::new()
    }
}
