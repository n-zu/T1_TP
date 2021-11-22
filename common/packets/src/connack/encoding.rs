use crate::{
    helpers::{build_control_byte, PacketType},
    traits::MQTTEncoding,
};

use super::*;

#[doc(hidden)]
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 2;
#[doc(hidden)]
const CONNACK_SESSION_PRESENT_TRUE: u8 = 1;
#[doc(hidden)]
const CONNACK_SESSION_PRESENT_FALSE: u8 = 0;

impl MQTTEncoding for Connack {
    fn encode(&self) -> crate::packet_error::PacketResult<crate::traits::MQTTBytes> {
        let control_byte = build_control_byte(PacketType::Connack, RESERVED_BITS);
        let mut bytes = vec![control_byte, CONNACK_FIXED_REMAINING_LENGTH];

        if self.session_present {
            bytes.push(CONNACK_SESSION_PRESENT_TRUE)
        } else {
            bytes.push(CONNACK_SESSION_PRESENT_FALSE)
        }
        bytes.push(u8::from(self.return_code));
        Ok(bytes)
    }
}

impl Connack {
    pub fn new(session_present: bool, return_code: ConnackReturnCode) -> Connack {
        Connack {
            session_present,
            return_code,
        }
    }
}
