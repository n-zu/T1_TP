#![allow(dead_code)]

use crate::{
    helpers::{build_control_byte, PacketType},
    traits::{MQTTBytes, MQTTEncoding},
};

use super::*;

#[doc(hidden)]
const DISCONNECT_REMAINING_LENGTH: u8 = 0;
const PACKET_TYPE_SHIFT: u8 = 4;

impl MQTTEncoding for Disconnect {
    /// Encodes the Disconnect packet according to the
    /// MQTT V3.1.1 protocol
    fn encode(&self) -> crate::packet_error::PacketResult<MQTTBytes> {
        let control_byte = (PacketType::Disconnect as u8) << PACKET_TYPE_SHIFT | RESERVED_BITS;
        Ok(vec![control_byte, DISCONNECT_REMAINING_LENGTH])
    }
}

impl Disconnect {
    /// Creates a new Disconnect packet
    pub fn new() -> Disconnect {
        Disconnect()
    }

    pub fn encode(&self) -> MQTTBytes {
        let control_byte = build_control_byte(PacketType::Disconnect, RESERVED_BITS);
        vec![control_byte, DISCONNECT_REMAINING_LENGTH]
    }
}

impl Default for Disconnect {
    fn default() -> Self {
        Self::new()
    }
}
