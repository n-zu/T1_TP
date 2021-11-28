#![allow(unused)]
use std::io::Read;

use super::*;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
};

#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
#[doc(hidden)]
const MSG_PACKET_TYPE_UNSUBACK: &str = "Packet type must be 11 for a Unsuback packet";
#[doc(hidden)]
const UNSUBACK_CONTROL_PACKET_TYPE: u8 = 11;
#[doc(hidden)]
const FIXED_REMAINING_LENGTH: usize = 2;

impl MQTTDecoding for Unsuback {
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self> {
        check_packet_type(control_byte, PacketType::Unsuback)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(bytes)?;
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
    fn verify_control_packet_type(control_byte: &u8) -> PacketResult<()> {
        let control_packet_type = (control_byte & 0b11110000) >> 4;
        if control_packet_type != UNSUBACK_CONTROL_PACKET_TYPE {
            return Err(PacketError::new_kind(
                MSG_PACKET_TYPE_UNSUBACK,
                ErrorKind::InvalidControlPacketType,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; FIXED_REMAINING_LENGTH];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }
}
