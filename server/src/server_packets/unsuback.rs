#![allow(unused)]

use packets::packet_error::{ErrorKind, PacketError};

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
const CONTROL_BYTE: u8 = 0b10110000;
const FIXED_REMAINING_LENGTH: u8 = 0b10;

#[derive(Debug, PartialEq)]
/// The UNSUBACK Packet is sent by the Server to the Client to confirm receipt of an UNSUBSCRIBE Packet.
pub struct Unsuback {
    packet_id: u16,
}

impl Unsuback {
    /// Returns a new Unsuback packet with given packet_id
    ///
    /// # Errors
    ///
    /// If packet_id is zero, this functions returns a [ErrorKind::InvalidProtocol]
    pub fn new(packet_id: u16) -> Result<Unsuback, PacketError> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self { packet_id })
    }

    /// Encodes a Unsuback packet into its bytes representation following MQTT v3.1.1 protocol
    ///
    /// # Examples
    ///
    /// ```
    /// let unsuback = Unsuback::new(1).unwrap();
    /// let result = unsuback.encode();
    /// let expected: Vec<u8> = vec![CONTROL_BYTE, FIXED_REMAINING_LENGTH, 0, 1];
    /// assert_eq!(expected, result)
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        bytes
    }

    #[doc(hidden)]
    fn verify_packet_id(packet_id: &u16) -> Result<(), PacketError> {
        if *packet_id == 0 {
            return Err(PacketError::new_kind(
                MSG_INVALID_PACKET_ID,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Vec<u8> {
        let fixed_header_buffer: Vec<u8> = vec![CONTROL_BYTE, FIXED_REMAINING_LENGTH];
        fixed_header_buffer
    }

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header_buffer: Vec<u8> = vec![];
        let packet_id_representation = self.packet_id.to_be_bytes();
        variable_header_buffer.push(packet_id_representation[0]);
        variable_header_buffer.push(packet_id_representation[1]);
        variable_header_buffer
    }
}

#[cfg(test)]
mod tests {
    use crate::server_packets::unsuback::{
        Unsuback, CONTROL_BYTE, FIXED_REMAINING_LENGTH, MSG_INVALID_PACKET_ID,
    };
    use packets::packet_error::{ErrorKind, PacketError};

    #[test]
    fn test_unsuback_with_packet_id_0_should_raise_invalid_protocol_error() {
        let result = Unsuback::new(0).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
        assert_eq!(expected_error, result)
    }

    #[test]
    fn test_encoding_unsuback_packet_with_packet_id_1() {
        let unsuback = Unsuback::new(1).unwrap();
        let result = unsuback.encode();
        let expected: Vec<u8> = vec![CONTROL_BYTE, FIXED_REMAINING_LENGTH, 0, 1];
        assert_eq!(expected, result)
    }
}
