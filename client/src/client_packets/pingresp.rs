#![allow(dead_code)]

use packets::packet_error::{ErrorKind, PacketError};
use std::io::{self, Read};

use packets::packet_reader::{self};

#[doc(hidden)]
const PINGREQ_PACKET_TYPE: u8 = 0b11000000;
#[doc(hidden)]
const PACKET_TYPE_MASK: u8 = 0b11110000;
#[doc(hidden)]
const RESERVED_BYTES_MASK: u8 = 0b00001111;
#[doc(hidden)]
const RESERVED_BYTES: u8 = 0b00000000;

/// A PingResp Packet is sent by the Server to the Client in response
/// to a PingReq Packet.
/// It indicates that the Server is alive.
pub struct PingResp;

#[doc(hidden)]
const PINGRESP_PACKET_TYPE: u8 = 0b11010000;
#[doc(hidden)]
const PINGRESP_RESERVED_BYTES: u8 = 0b00000000;
#[doc(hidden)]
const REMAINING_LENGTH: u8 = 0b00000000;

impl PingResp {
    /// Creates a PingResp packet from a stream of bytes
    /// It assumes the first byte (control byte) is read previously from the stream.
    /// Therefore, it is not present in the stream
    ///
    /// # Errors
    ///
    /// Returns error if the packet does not follow the MQTT V3.1.1 protocol
    pub fn read_from(stream: &mut impl Read, control_byte: u8) -> Result<PingResp, PacketError> {
        PingResp::check_packet_type(control_byte)?;
        PingResp::check_reserved_bytes(control_byte)?;
        let mut bytes = packet_reader::read_remaining_bytes(stream)?;
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

    #[doc(hidden)]
    fn check_packet_type(control_byte: u8) -> Result<(), PacketError> {
        if (control_byte & PACKET_TYPE_MASK) != PINGRESP_PACKET_TYPE {
            Err(PacketError::new_kind(
                "Tipo de paquete invalido",
                ErrorKind::InvalidControlPacketType,
            ))
        } else {
            Ok(())
        }
    }

    #[doc(hidden)]
    fn check_reserved_bytes(control_byte: u8) -> Result<(), PacketError> {
        if (control_byte & RESERVED_BYTES_MASK) != RESERVED_BYTES {
            Err(PacketError::new_msg(
                "Los bytes reservados no coinciden con los esperados",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use packets::packet_error::ErrorKind;

    use crate::client_packets::PingResp;

    #[test]
    fn test_valid() {
        let control_byte = 0b11010000;
        let remaining_bytes = vec![0b00000000];
        let mut stream = Cursor::new(remaining_bytes);
        let packet = PingResp::read_from(&mut stream, control_byte);
        assert!(packet.is_ok());
    }

    #[test]
    fn test_invalid_packet_type() {
        let control_byte = 0b11110000;
        let remaining_bytes = vec![0b00000000];
        let mut stream = Cursor::new(remaining_bytes);
        let packet = PingResp::read_from(&mut stream, control_byte);
        assert!(packet.is_err());
        assert_eq!(
            packet.err().unwrap().kind(),
            ErrorKind::InvalidControlPacketType
        );
    }

    #[test]
    fn test_invalid_reserved_bytes() {
        let control_byte = 0b11010010;
        let remaining_bytes = vec![0b00000000];
        let mut stream = Cursor::new(remaining_bytes);
        let packet = PingResp::read_from(&mut stream, control_byte);
        assert!(packet.is_err());
    }

    #[test]
    fn test_invalid_remaining_length() {
        let control_byte = 0b11010000;
        let remaining_bytes = vec![0b00000001, 0b00000000];
        let mut stream = Cursor::new(remaining_bytes);
        let packet = PingResp::read_from(&mut stream, control_byte);
        assert!(packet.is_err());
    }
}
