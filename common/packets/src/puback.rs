use crate::packet_reader;
use crate::packet_reader::{ErrorKind, PacketError};
use std::io::Read;

#[doc(hidden)]
const FIXED_RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
#[doc(hidden)]
const MSG_PACKET_TYPE_PUBACK: &str = "Packet type must be 4 for a Puback packet";
#[doc(hidden)]
const PUBACK_CONTROL_PACKET_TYPE: u8 = 4;

#[derive(Debug, PartialEq)]
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
            return Err(PacketError::new_kind(
                MSG_INVALID_RESERVED_BITS,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    fn verify_control_packet_type(control_byte: &u8) -> Result<(), PacketError> {
        let control_packet_type = (control_byte & 0b11110000) >> 4;
        if control_packet_type != PUBACK_CONTROL_PACKET_TYPE {
            return Err(PacketError::new_kind(
                MSG_PACKET_TYPE_PUBACK,
                ErrorKind::InvalidControlPacketType,
            ));
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
mod tests {
    use crate::packet_reader::{ErrorKind, PacketError};
    use crate::puback::{Puback, MSG_INVALID_RESERVED_BITS, MSG_PACKET_TYPE_PUBACK};
    use std::io::Cursor;

    #[test]
    fn test_reserved_bits_other_than_0_should_raise_invalid_protocol_error() {
        let control_byte = 0b1111;
        let reserved_bits_buffer = [0b1111u8; 1];
        let mut stream = Cursor::new(reserved_bits_buffer);
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RESERVED_BITS, ErrorKind::InvalidProtocol);
        let result = Puback::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }

    #[test]
    fn test_control_packet_type_other_than_4_should_raise_invalid_control_packet_type_error() {
        let control_byte = 0b000;
        let control_packet_type_buffer = [0b1111u8; 1];
        let mut stream = Cursor::new(control_packet_type_buffer);
        let expected_error =
            PacketError::new_kind(MSG_PACKET_TYPE_PUBACK, ErrorKind::InvalidControlPacketType);
        let result = Puback::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }

    #[test]
    fn test_valid_puback_packet_with_packet_id_1() {
        let control_byte = 0b01000000u8;
        let remaining_length = 2u8;
        let data_buffer: Vec<u8> = vec![remaining_length, 0, 1];
        let mut stream = Cursor::new(data_buffer);
        let expected = Puback { packet_id: 1 };
        let result = Puback::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_valid_puback_packet_with_packet_id_0() {
        let control_byte = 0b01000000u8;
        let remaining_length = 2u8;
        let data_buffer: Vec<u8> = vec![remaining_length, 0, 0];
        let mut stream = Cursor::new(data_buffer);
        let expected = Puback { packet_id: 0 };
        let result = Puback::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(expected, result);
    }
}
