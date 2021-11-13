#![allow(unused)]
use crate::packet_reader;
use crate::packet_reader::{ErrorKind, PacketError, RemainingLength};
use std::io::Read;

#[derive(Debug, Eq, PartialEq)]
/// Client/Server side structure for Suback packet
pub struct Suback {
    return_codes: Vec<u8>,
    subscribe_packet_id: u16,
}

#[doc(hidden)]
const CONTROL_BYTE_SUBACK: u8 = 0b10010000;
#[doc(hidden)]
const MSG_INVALID_RETURN_CODE: &str = "Allowed return codes are 0x00, 0x01, 0x80";
#[doc(hidden)]
const SUCCESS_MAXIMUM_QOS_0: u8 = 0;
#[doc(hidden)]
const SUCCESS_MAXIMUM_QOS_1: u8 = 1;
#[doc(hidden)]
const FAILURE: u8 = 0x80;
#[doc(hidden)]
const FIXED_RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
#[doc(hidden)]
const SUBACK_CONTROL_PACKET_TYPE: u8 = 9;
#[doc(hidden)]
const MSG_PACKET_TYPE_SUBACK: &str = "Packet type must be 9 for a Suback packet";

impl Suback {
    /// Returns a new Suback packet struct from a given subscribe packet id and given return codes
    /// The subscribe_packet_id should be the same from the subscribe packet this Suback is acknowledging
    /// The order of return codes in the Suback packet must match the order of Topic Filters in the Subscribe Packet
    ///
    /// # Errors
    ///
    /// Allowed return codes are 0x00, 0x01, 0x80. If a return code doesn't match any of those, this function returns a [ErrorKind::InvalidReturnCode]
    pub fn new_from_vec(
        return_codes: Vec<u8>,
        subscribe_packet_id: u16,
    ) -> Result<Self, PacketError> {
        Self::verify_return_codes_from_vec(&return_codes)?;
        Ok(Self {
            return_codes,
            subscribe_packet_id,
        })
    }

    /// Returns this Suback packet representation following MQTT v3.1.1 protocol
    /// # Errors
    ///
    /// If remaining length of this packet is greater than 256 MB, this function returns a PacketError
    ///
    /// # Examples
    ///
    /// ```
    ///  use packets::suback::Suback;
    /// let return_codes: Vec<u8> = vec![0, 1, 1, 1];
    ///  let mut suback = Suback::new_from_vec(return_codes, 1).unwrap();
    ///  let encoded_suback = suback.encode().unwrap();
    ///  let expected: Vec<u8> = vec![0b10010000, 6, 0, 1, 0, 1, 1, 1];
    ///  assert_eq!(encoded_suback, expected)
    /// ```
    pub fn encode(mut self) -> Result<Vec<u8>, PacketError> {
        let mut bytes = vec![];
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.return_codes);
        Ok(bytes)
    }

    /// Reads from a stream of bytes and returns a valid Suback packet
    /// It is assumed that the first byte was read into control_byte parameter
    ///
    /// # Errors
    ///
    /// This function returns a PacketError if:
    /// - Control packet type is different from 9
    /// - Reserved bits are not 0b0000
    /// - Remaining length is greater than 256 MB
    /// - Any return code does not match any of these 0x00, 0x01, 0x80
    ///
    /// # Examples
    /// ```
    /// use std::io::Cursor;
    /// use packets::suback::Suback;
    ///
    /// let stream_aux = vec![6, 0, 1, 1, 0, 1, 0];
    /// let control_byte = 0b10010000;
    /// let mut stream = Cursor::new(stream_aux);
    /// let result = Suback::read_from(&mut stream, control_byte).unwrap().encode().unwrap();
    /// let expected = vec![control_byte, 6, 0, 1, 1, 0, 1, 0];
    /// assert_eq!(result, expected);
    /// ```
    pub fn read_from(bytes: &mut impl Read, control_byte: u8) -> Result<Suback, PacketError> {
        Self::verify_control_packet_type(&control_byte)?;
        Self::verify_reserved_bits(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(bytes)?;
        let subscribe_packet_id = Self::read_packet_id(&mut remaining_bytes);
        let return_codes = Self::read_return_codes(&mut remaining_bytes)?;
        Self::verify_return_codes_from_vec(&return_codes)?;
        Ok(Self {
            return_codes,
            subscribe_packet_id,
        })
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
        if control_packet_type != SUBACK_CONTROL_PACKET_TYPE {
            return Err(PacketError::new_kind(
                MSG_PACKET_TYPE_SUBACK,
                ErrorKind::InvalidControlPacketType,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut subscribe_packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut subscribe_packet_id_buffer);
        u16::from_be_bytes(subscribe_packet_id_buffer)
    }

    #[doc(hidden)]
    fn read_return_codes(bytes: &mut impl Read) -> Result<Vec<u8>, PacketError> {
        let mut return_codes = Vec::new();
        bytes.read_to_end(&mut return_codes)?;
        Ok(return_codes)
    }

    #[doc(hidden)]
    fn verify_return_codes_from_vec(return_codes: &[u8]) -> Result<(), PacketError> {
        for code in return_codes {
            Self::verify_return_code(code)?;
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_return_code(return_code: &u8) -> Result<(), PacketError> {
        if !Self::is_return_code_valid(return_code) {
            return Err(PacketError::new_kind(
                MSG_INVALID_RETURN_CODE,
                ErrorKind::InvalidReturnCode,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn is_return_code_valid(return_code: &u8) -> bool {
        *return_code == SUCCESS_MAXIMUM_QOS_0
            || *return_code == SUCCESS_MAXIMUM_QOS_1
            || *return_code == FAILURE
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Result<Vec<u8>, PacketError> {
        let mut fixed_header: Vec<u8> = vec![CONTROL_BYTE_SUBACK];
        let remaining_length =
            RemainingLength::from_uncoded(self.return_codes.len() + self.variable_header().len())?;
        let mut remaining_length_buff = remaining_length.encode();
        fixed_header.append(&mut remaining_length_buff);
        Ok(fixed_header)
    }

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header: Vec<u8> = vec![];
        variable_header.append(&mut self.subscribe_packet_id.to_be_bytes().to_vec());
        variable_header
    }

    /// Get the suback's subscribe packet id.
    pub fn packet_id(&self) -> u16 {
        self.subscribe_packet_id
    }
}

#[cfg(test)]
mod tests {
    use crate::packet_reader::{ErrorKind, PacketError};
    use crate::suback::{
        Suback, CONTROL_BYTE_SUBACK, MSG_INVALID_RETURN_CODE, MSG_PACKET_TYPE_SUBACK,
    };
    use std::io::Cursor;

    #[test]
    fn test_valid_suback_with_return_codes_0_1_1_1() {
        let return_codes: Vec<u8> = vec![0, 1, 1, 1];
        let mut suback = Suback::new_from_vec(return_codes, 1).unwrap();
        let encoded_suback = suback.encode().unwrap();
        let expected: Vec<u8> = vec![CONTROL_BYTE_SUBACK, 6, 0, 1, 0, 1, 1, 1];
        assert_eq!(encoded_suback, expected)
    }

    #[test]
    fn test_valid_suback_with_return_codes_0_0_0_0() {
        let return_codes: Vec<u8> = vec![0, 0, 0, 0];
        let mut suback = Suback::new_from_vec(return_codes, 2).unwrap();
        let expected_suback = suback.encode().unwrap();
        let expected: Vec<u8> = vec![CONTROL_BYTE_SUBACK, 6, 0, 2, 0, 0, 0, 0];
        assert_eq!(expected_suback, expected)
    }
    #[test]
    fn test_suback_with_return_code_65_should_raise_invalid_return_code_error() {
        let return_codes: Vec<u8> = vec![0, 65, 0, 0];
        let result = Suback::new_from_vec(return_codes, 3).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RETURN_CODE, ErrorKind::InvalidReturnCode);
        assert_eq!(result, expected_error)
    }

    #[test]
    fn test_control_byte_from_stream_other_than_9_should_raise_invalid_control_packet_type_error() {
        let return_codes: Vec<u8> = vec![0, 0, 0, 0];
        let control_byte = 1;
        let mut stream = Cursor::new(return_codes);
        let result = Suback::read_from(&mut stream, control_byte).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_PACKET_TYPE_SUBACK, ErrorKind::InvalidControlPacketType);
        assert_eq!(result, expected_error)
    }

    #[test]
    fn test_correct_suback_with_packet_id_1_and_return_codes_0_from_stream() {
        let stream_aux = vec![6, 0, 1, 0, 0, 0, 0];
        let control_byte = CONTROL_BYTE_SUBACK;
        let mut stream = Cursor::new(stream_aux);
        let result = Suback::read_from(&mut stream, control_byte)
            .unwrap()
            .encode()
            .unwrap();
        let expected = vec![CONTROL_BYTE_SUBACK, 6, 0, 1, 0, 0, 0, 0];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_stream_with_return_code_3_should_raise_invalid_return_code() {
        let stream_aux = vec![6, 0, 1, 3, 0, 1, 0];
        let control_byte = CONTROL_BYTE_SUBACK;
        let mut stream = Cursor::new(stream_aux);
        let result = Suback::read_from(&mut stream, control_byte).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RETURN_CODE, ErrorKind::InvalidReturnCode);
        assert_eq!(result, expected_error);
    }
}
