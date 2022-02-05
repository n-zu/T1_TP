use std::io::Read;

use super::*;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::PacketResult,
    packet_reader,
    traits::MQTTDecoding,
};

#[doc(hidden)]
const FIXED_REMAINING_LENGTH: usize = 2;

impl MQTTDecoding for Unsuback {
    fn read_from<T: Read>(stream: &mut T, control_byte: u8) -> PacketResult<Self> {
        check_packet_type(control_byte, PacketType::Unsuback)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(stream)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        Self::verify_packet_id(&packet_id)?;
        Ok(Self {
            packet_id,
            topics: Vec::new(),
        })
    }
}

impl Unsuback {
    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; FIXED_REMAINING_LENGTH];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }
}
