#![allow(dead_code)]

use std::io::{self, Read};

use packets::packet_reader::{self, ErrorKind, PacketError};

#[doc(hidden)]
const DISCONNECT_PACKET_TYPE: u8 = 0b11100000;
#[doc(hidden)]
const PACKET_TYPE_MASK: u8 = 0b11110000;
#[doc(hidden)]
const RESERVED_MASK: u8 = 0b00001111;

/// The Disconnect packet is the final packet sent from the Client to the Server.
/// It indicates that the Client is disconnecting cleanly.
pub struct Disconnect {}

impl Disconnect {
    #[doc(hidden)]
    fn check_packet_type_is_disconnect(control_byte: u8) -> Result<(), PacketError> {
        if control_byte & PACKET_TYPE_MASK != DISCONNECT_PACKET_TYPE {
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
        if control_byte & RESERVED_MASK != 0 {
            println!("FAIL {}", control_byte & RESERVED_MASK);
            Err(PacketError::new_kind(
                "Los bytes reservados del byte de control no son 0",
                ErrorKind::InvalidReservedBits,
            ))
        } else {
            Ok(())
        }
    }

    #[doc(hidden)]
    fn check_packet_end(packet_bytes: &mut impl Read) -> Result<(), PacketError> {
        let mut buff = [0u8; 1];
        match packet_bytes.read_exact(&mut buff) {
            Ok(()) => Err(PacketError::new_msg(
                "El paquete contiene mas bytes de lo esperado",
            )),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
            Err(_) => Err(PacketError::new_msg("Error leyendo el paquete")),
        }
    }

    /// Reads a Disconnect packet from a stream of bytes
    ///
    /// # Errors
    ///
    /// Returns error if the packet fields do not meet the
    /// requirements of the MQTT V3.1.1 standard
    /// (packet type should be Disconnect, reserved bytes should be 0,
    /// remaining length should be 0)
    pub fn read_from(control_byte: u8, stream: &mut impl Read) -> Result<Disconnect, PacketError> {
        Disconnect::check_packet_type_is_disconnect(control_byte)?;
        Disconnect::check_reserved_bytes(control_byte)?;
        let mut packet_bytes = packet_reader::read_packet_bytes(stream)?;
        Disconnect::check_packet_end(&mut packet_bytes)?;
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, vec};

    use crate::server_packets::Disconnect;

    #[test]
    fn test_valid_disconnect() {
        let mut bytes = vec![];
        let control_byte = 0b11100000;
        bytes.push(0b00000000); // remaining_length
        let mut stream = Cursor::new(bytes);
        let packet = Disconnect::read_from(control_byte, &mut stream);
        assert!(packet.is_ok());
    }

    #[test]
    fn test_remaining_length_not_zero_should_fail() {
        let mut bytes = vec![];
        let control_byte = 0b11100000;
        bytes.push(0b00000001); // remaining_length
        let mut stream = Cursor::new(bytes);
        let packet = Disconnect::read_from(control_byte, &mut stream);
        assert!(packet.is_err());
    }

    #[test]
    fn test_invalid_packet_type() {
        let mut bytes = vec![];
        let control_byte = 0b11110000;
        bytes.push(0b00000000); // remaining_length
        let mut stream = Cursor::new(bytes);
        let packet = Disconnect::read_from(control_byte, &mut stream);
        assert!(packet.is_err());
    }

    #[test]
    fn test_invalid_reserved_bytes() {
        let mut bytes = vec![];
        let control_byte = 0b11100100;
        bytes.push(0b00000000); // remaining_length
        let mut stream = Cursor::new(bytes);
        let packet = Disconnect::read_from(control_byte, &mut stream);
        assert!(packet.is_err());
    }
}
