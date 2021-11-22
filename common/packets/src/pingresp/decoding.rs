#![allow(dead_code)]

use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
};
use std::io::{self, Read};

use super::*;

impl MQTTDecoding for PingResp {
    /// Creates a PingResp packet from a stream of bytes
    /// It assumes the first byte (control byte) is read previously from the stream.
    /// Therefore, it is not present in the stream
    ///
    /// # Errors
    ///
    /// Returns error if the packet does not follow the MQTT V3.1.1 protocol
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self>
    where
        Self: Sized,
    {
        check_packet_type(control_byte, PacketType::PingResp)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut bytes = packet_reader::read_remaining_bytes(bytes)?;
        let mut buff = [0];
        match bytes.read_exact(&mut buff) {
            Ok(_) => Err(PacketError::new_msg(
                "Se recibio PingResp con remaining_length != 0",
            )),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(PingResp {}),
            Err(err) => Err(PacketError::new_msg(&format!(
                "Error inesperado: {}",
                err.to_string()
            ))),
        }
    }
}
