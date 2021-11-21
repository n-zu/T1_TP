#![allow(dead_code)]

use std::io::{self, Read};

use super::*;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
};

impl MQTTDecoding for Disconnect {
    /// Reads a Disconnect packet from a stream of bytes
    ///
    /// # Errors
    ///
    /// Returns error if the packet fields do not meet the
    /// requirements of the MQTT V3.1.1 standard
    /// (packet type should be Disconnect, reserved bytes should be 0,
    /// remaining length should be 0)
    fn read_from(stream: &mut impl Read, control_byte: u8) -> PacketResult<Disconnect> {
        check_packet_type(control_byte, PacketType::Disconnect)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut packet_bytes = packet_reader::read_remaining_bytes(stream)?;
        Disconnect::check_packet_end(&mut packet_bytes)?;
        Ok(Self {})
    }
}

impl Disconnect {
    #[doc(hidden)]
    fn check_packet_end(packet_bytes: &mut impl Read) -> PacketResult<()> {
        let mut buff = [0u8; 1];
        match packet_bytes.read_exact(&mut buff) {
            Ok(()) => Err(PacketError::new_msg(
                "El paquete contiene mas bytes de lo esperado",
            )),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
            Err(_) => Err(PacketError::new_msg("Error leyendo el paquete")),
        }
    }
}
