#![allow(unused)]

use super::*;
use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    traits::{MQTTBytes, MQTTEncoding},
};

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
const FIXED_REMAINING_LENGTH: u8 = 0b10;

impl MQTTEncoding for Unsuback {
    /// Encodes a Unsuback packet into its bytes representation following MQTT v3.1.1 protocol
    ///
    /// # Examples
    ///
    /// ```
    /// use packets::traits::MQTTEncoding;
    /// use packets::unsuback::Unsuback;
    ///
    /// const FIXED_REMAINING_LENGTH: u8 = 0b10;
    ///
    /// let unsuback = Unsuback::new(1).unwrap();
    /// let result = unsuback.encode().unwrap();
    /// let control_byte = 0b10110000;
    /// let expected: Vec<u8> = vec![control_byte, FIXED_REMAINING_LENGTH, 0, 1];
    /// assert_eq!(expected, result)
    /// ```
    ///
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        Ok(bytes)
    }
}

impl Unsuback {
    /// Returns a new Unsuback packet with given packet_id
    ///
    /// # Errors
    ///
    /// If packet_id is zero, this functions returns a [ErrorKind::InvalidProtocol]
    pub fn new(packet_id: u16) -> PacketResult<Unsuback> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self {
            packet_id,
            topics: Vec::new(),
        })
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> MQTTBytes {
        let control_byte = build_control_byte(PacketType::Unsuback, RESERVED_BITS);
        let fixed_header_buffer: Vec<u8> = vec![control_byte, FIXED_REMAINING_LENGTH];
        fixed_header_buffer
    }

    #[doc(hidden)]
    fn variable_header(&self) -> MQTTBytes {
        let mut variable_header_buffer: Vec<u8> = vec![];
        let packet_id_representation = self.packet_id.to_be_bytes();
        variable_header_buffer.push(packet_id_representation[0]);
        variable_header_buffer.push(packet_id_representation[1]);
        variable_header_buffer
    }
}
