use crate::packet_reader;
use crate::packet_reader::{ErrorKind, PacketError};
use std::io;
use std::io::Read;

#[doc(hidden)]
const FIXED_RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
#[doc(hidden)]
const MSG_PACKET_TYPE_PUBACK: &str = "Packet type must be 4 for a Puback packet";
#[doc(hidden)]
const MSG_PACKET_MORE_BYTES_THAN_EXPECTED: &str = "Puback packet contains more bytes than expected";
#[doc(hidden)]
const PUBACK_CONTROL_PACKET_TYPE: u8 = 4;
#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";

#[derive(Debug, PartialEq)]
pub struct Puback {
    packet_id: u16,
}

impl Puback {
    /// Returns a new Puback packet with given packet_id
    pub fn new(packet_id: u16) -> Result<Self, PacketError> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self { packet_id })
    }
    /// Creates a new Puback packet from a given stream of bytes.
    ///
    /// It is assumed the first byte from the stream was read by the client/server
    ///
    /// # Errors
    ///
    /// If reserved bits from the bytes stream doesn't follow MQTT 3.1.1 (this is 0b0), this function returns an invalid protocol error
    ///
    /// If control packet type bits from the bytes stream doesn't follow MQTT 3.1.1 (this is 0b01000000), this function returns an invalid control packet type error
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::puback::Puback;
    /// let control_byte = 0b01000000u8;
    /// let remaining_length = 2u8;
    /// let data_buffer: Vec<u8> = vec![remaining_length, 0, 0];
    /// let mut stream = Cursor::new(data_buffer);
    /// let expected = Puback::new(0);
    /// let result = Puback::read_from(&mut stream, control_byte).unwrap();
    /// assert_eq!(expected, result);
    /// ```
    pub fn read_from(bytes: &mut impl Read, control_byte: u8) -> Result<Self, PacketError> {
        Self::verify_reserved_bits(&control_byte)?;
        Self::verify_control_packet_type(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(bytes)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        Self::verify_packet_end(&mut remaining_bytes)?;
        Ok(Self { packet_id })
    }

    /// Encodes a Puback packet into its bytes representation following MQTT 3.1.1 protocol
    ///
    /// # Examples
    /// ```
    /// use packets::puback::Puback;
    ///
    /// let puback = Puback::new(1).unwrap();
    /// let result = puback.encode();
    /// let expected: Vec<u8> = vec![0b01000000, 0b10, 0b0, 0b0];
    /// assert_eq!(expected, result)
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        bytes
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Vec<u8> {
        let fixed_header_buffer: Vec<u8> = vec![0b01000000, 0b10];
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

    #[doc(hidden)]
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

    #[doc(hidden)]
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

    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }

    #[doc(hidden)]
    fn verify_packet_end(bytes: &mut impl Read) -> Result<(), PacketError> {
        let mut buff = [0u8; 1];
        match bytes.read_exact(&mut buff) {
            Ok(()) => Err(PacketError::new_msg(
                "Puback packet contains more bytes than expected",
            )),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
            Err(_) => Err(PacketError::new_msg("Error at reading packet")),
        }
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
}

#[cfg(test)]
mod tests {
    use crate::packet_reader::{ErrorKind, PacketError};
    use crate::puback::{
        Puback, MSG_INVALID_PACKET_ID, MSG_INVALID_RESERVED_BITS,
        MSG_PACKET_MORE_BYTES_THAN_EXPECTED, MSG_PACKET_TYPE_PUBACK,
    };
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

    #[test]
    fn test_puback_packet_can_not_have_more_bytes_than_expected() {
        let control_byte = 0b01000000u8;
        let remaining_length = 3u8;
        let data_buffer: Vec<u8> = vec![remaining_length, 0, 0, 1];
        let mut stream = Cursor::new(data_buffer);
        let expected_error = PacketError::new_msg(MSG_PACKET_MORE_BYTES_THAN_EXPECTED);
        let result = Puback::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }

    #[test]
    fn test_puback_with_packet_id_0_should_raise_invalid_protocol_error() {
        let result = Puback::new(0).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
        assert_eq!(expected_error, result)
    }

    #[test]
    fn test_encoding_puback_packet_with_packet_id_1() {
        let puback = Puback::new(1).unwrap();
        let result = puback.encode();
        let expected: Vec<u8> = vec![0b01000000, 0b10, 0b0, 1];
        assert_eq!(expected, result)
    }
}
