#![allow(dead_code)]

use std::io::Read;

use super::*;
use crate::{
    helpers::check_reserved_bits,
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
    utf8::Field,
};

impl MQTTDecoding for Subscribe {
    /// Creates a new Subscribe packet from the given stream.
    /// Returns a PacketError in case the packet is malformed.
    /// It is assumed that the first identifier byte has already been read.
    fn read_from(stream: &mut impl Read, control_byte: u8) -> PacketResult<Subscribe> {
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut bytes = packet_reader::read_remaining_bytes(stream)?;

        let packet_identifier = Self::get_identifier(&mut bytes)?;
        let mut topics = Vec::new();

        while let Some(field) = Field::new_from_stream(&mut bytes) {
            let mut qos_buf = [0; 1];
            bytes.read_exact(&mut qos_buf)?;

            topics.push(Topic::new(&field.value, Self::get_qos(qos_buf[0])?)?);
        }

        if topics.is_empty() {
            return Err(PacketError::new_kind(
                "No topic filters found",
                ErrorKind::InvalidProtocol,
            ));
        }

        Ok(Subscribe {
            packet_identifier,
            topics,
        })
    }
}

impl Subscribe {
    /// Gets the next two bytes of the stream as an unsigned 16-bit integer.
    /// Returns a PacketError in case they can't be read.
    fn get_identifier(stream: &mut impl Read) -> PacketResult<u16> {
        let mut buf = [0; 2];
        stream.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
}
