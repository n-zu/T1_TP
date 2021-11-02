use crate::packet_reader;
use crate::packet_reader::{ErrorKind, PacketError, RemainingLength};
use std::io::Read;

const FIXED_RESERVED_BITS: u8 = 0;
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
const MSG_PACKET_TYPE_PUBACK: &str = "Packet type must be 4 for a Puback packet";

pub struct Puback {
    packet_id: u16,
}

impl Puback {
    pub fn read_from(bytes: &mut impl Read, control_byte: u8) -> Result<Self, PacketError> {
        Self::verify_reserved_bits(&control_byte)?;
        Self::verify_control_packet_type(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(bytes)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        Ok(Self { packet_id })
    }

    fn verify_reserved_bits(control_byte: &u8) -> Result<(), PacketError> {
        let reserved_bits = control_byte & 0b1111;
        if reserved_bits != FIXED_RESERVED_BITS {
            PacketError::new_kind(MSG_INVALID_RESERVED_BITS, ErrorKind::InvalidProtocol)
        }
        Ok(())
    }

    fn verify_control_packet_type(control_byte: &u8) -> Result<(), PacketError> {
        let control_packet_type = (control_byte & 0b11110000) >> 4;
        if control_packet_type != 4 {
            PacketError::new_kind(MSG_PACKET_TYPE_PUBACK, ErrorKind::InvalidControlPacketType)
        }
        Ok(())
    }

    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }
}

#[cfg(test)]
mod tests {}
