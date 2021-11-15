#![allow(unused)]

use packets::packet_error::{ErrorKind, PacketError};
use std::io::Read;
use std::result::Result;

#[derive(Debug, PartialEq)]
/// Client-side Connack packet structure
pub struct Connack {
    session_present: u8,
    return_code: u8,
}

#[doc(hidden)]
const CONNACK_CONTROL_TYPE: u8 = 0b10;
#[doc(hidden)]
const CONNACK_RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 0b10;
#[doc(hidden)]
const CONNACK_UNACCEPTABLE_PROTOCOL: u8 = 1;
#[doc(hidden)]
const CONNACK_IDENTIFIER_REJECTED: u8 = 2;
#[doc(hidden)]
const CONNACK_SERVER_UNAVAILABLE: u8 = 3;
#[doc(hidden)]
const CONNACK_BAD_USER_NAME_OR_PASSWORD: u8 = 4;
#[doc(hidden)]
const CONNACK_NOT_AUTHORIZED: u8 = 5;

#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 0";
#[doc(hidden)]
const MSG_PACKET_TYPE_CONNACK: &str = "Packet type must be 2 for a Connack packet";
#[doc(hidden)]
const MSG_INVALID_REMAINING_LENGTH: &str =
    "Remaining length does not follow MQTT v3.1.1 protocol for a Connack packet";
#[doc(hidden)]
const MSG_INVALID_SESSION_PRESENT_FLAG: &str =
    "Session flag does not follow MQTT v3.1.1 protocol for a Connack packet";
#[doc(hidden)]
const MSG_UNACCEPTABLE_PROTOCOL_VERSION: &str =
    "The server does not support the level of the MQTT protocol requested by the Client";
#[doc(hidden)]
const MSG_IDENTIFIER_REJECTED: &str =
    "The Client identifier is correct UTF-8 but not allowed by the Server";
#[doc(hidden)]
const MSG_SERVER_UNAVAILABLE: &str =
    "The Network connection has been made but the MQTT service is unavailable";
const MSG_BAD_USER_NAME_OR_PASSWORD: &str = "The data in the user name or password is malformed";
#[doc(hidden)]
const MSG_NOT_AUTHORIZED: &str = "The Client is not authorized to connect";

impl Connack {
    /// Return a Connack with valid state from a stream of bytes
    ///
    /// # Arguments
    ///
    /// * `stream`: &mut impl Read
    ///
    /// returns: Result<Connack, ConnackError>
    ///
    /// # Examples
    ///
    /// ```
    /// let v: Vec<u8> = vec![0b100000, 0b10, 0b1, 0b0];
    /// let mut stream = Cursor::new(v);
    /// let result = Connack::read_from(&mut stream)?;
    /// let connack_expected = Connack { session_present: 1, return_code: 0, };
    /// assert_eq!(result, connack_expected);
    /// ```
    /// # Errors
    /// This function returns a Packet error if:
    /// - the stream's bytes doesn't follow MQTT v3.1.1 protocol
    /// - return_code is not 0
    ///

    /// If return_code is not 0, this function returns a specific ConnackError
    pub fn read_from(stream: &mut impl Read, control_byte: u8) -> Result<Connack, PacketError> {
        let buffer = [0u8; 1];
        Connack::verify_first_byte_control_packet(control_byte)?;
        Connack::verify_remaining_length(buffer, stream)?;
        let session_present = Connack::verify_session_present_flag(buffer, stream)?;
        let return_code = Connack::verify_return_code(buffer, stream)?;
        Ok(Self {
            return_code,
            session_present,
        })
    }

    #[doc(hidden)]
    fn verify_first_byte_control_packet(first_byte: u8) -> Result<(), PacketError> {
        let reserved_bits = first_byte & 0b1111;
        if reserved_bits != CONNACK_RESERVED_BITS {
            return Err(PacketError::new_kind(
                MSG_INVALID_RESERVED_BITS,
                ErrorKind::InvalidReservedBits,
            ));
        }
        let control_type = (first_byte & 0b11110000) >> 4;
        if control_type != CONNACK_CONTROL_TYPE {
            return Err(PacketError::new_kind(
                MSG_PACKET_TYPE_CONNACK,
                ErrorKind::InvalidControlPacketType,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_remaining_length(
        mut buffer: [u8; 1],
        stream: &mut impl Read,
    ) -> Result<(), PacketError> {
        stream.read_exact(&mut buffer);
        let remaining_length = buffer[0];
        if remaining_length != CONNACK_FIXED_REMAINING_LENGTH {
            return Err(PacketError::new_kind(
                MSG_INVALID_REMAINING_LENGTH,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_session_present_flag(
        mut buffer: [u8; 1],
        stream: &mut impl Read,
    ) -> Result<u8, PacketError> {
        stream.read_exact(&mut buffer);
        let session_present = buffer[0];
        if session_present != 1 && session_present != 0 {
            return Err(PacketError::new_kind(
                MSG_INVALID_SESSION_PRESENT_FLAG,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(session_present)
    }

    #[doc(hidden)]
    fn verify_return_code(mut buffer: [u8; 1], stream: &mut impl Read) -> Result<u8, PacketError> {
        stream.read_exact(&mut buffer);
        let return_code = buffer[0];
        match return_code {
            CONNACK_UNACCEPTABLE_PROTOCOL => Err(PacketError::new_kind(
                MSG_UNACCEPTABLE_PROTOCOL_VERSION,
                ErrorKind::UnacceptableProtocolVersion,
            )),
            CONNACK_IDENTIFIER_REJECTED => Err(PacketError::new_kind(
                MSG_IDENTIFIER_REJECTED,
                ErrorKind::IdentifierRejected,
            )),
            CONNACK_SERVER_UNAVAILABLE => Err(PacketError::new_kind(
                MSG_SERVER_UNAVAILABLE,
                ErrorKind::ServerUnavailable,
            )),
            CONNACK_BAD_USER_NAME_OR_PASSWORD => Err(PacketError::new_kind(
                MSG_BAD_USER_NAME_OR_PASSWORD,
                ErrorKind::BadUserNameOrPassword,
            )),
            CONNACK_NOT_AUTHORIZED => Err(PacketError::new_kind(
                MSG_NOT_AUTHORIZED,
                ErrorKind::NotAuthorized,
            )),
            _ => Ok(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn test_reserved_bits_at_1_should_raise_invalid_reserved_bits_error() {
        let v: Vec<u8> = vec![2, 0, 0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100101).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RESERVED_BITS, ErrorKind::InvalidReservedBits);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_control_packet_type_other_than_2_should_raise_invalid_control_packet_type_error() {
        let v: Vec<u8> = vec![2, 0, 0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b110000).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_PACKET_TYPE_CONNACK, ErrorKind::InvalidControlPacketType);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_should_raise_wrong_encoding_error_at_remaining_length() {
        let v: Vec<u8> = vec![0b11];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 32).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_REMAINING_LENGTH, ErrorKind::InvalidProtocol);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_should_raise_wrong_encoding_error_at_unknown_connack_flag() {
        let v: Vec<u8> = vec![0b10, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_SESSION_PRESENT_FLAG, ErrorKind::InvalidProtocol);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_return_code_1_should_raise_unacceptable_protocol_version_error() {
        let v: Vec<u8> = vec![2, 0, 1];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let error_expected = PacketError::new_kind(
            MSG_UNACCEPTABLE_PROTOCOL_VERSION,
            ErrorKind::UnacceptableProtocolVersion,
        );
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_2_should_raise_identifier_rejected_error() {
        let v: Vec<u8> = vec![2, 1, 2];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let error_expected =
            PacketError::new_kind(MSG_IDENTIFIER_REJECTED, ErrorKind::IdentifierRejected);
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_3_should_raise_server_unavailable_error() {
        let v: Vec<u8> = vec![2, 1, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let error_expected =
            PacketError::new_kind(MSG_SERVER_UNAVAILABLE, ErrorKind::ServerUnavailable);
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_4_should_raise_bad_username_or_password_error() {
        let v: Vec<u8> = vec![2, 1, 4];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let error_expected = PacketError::new_kind(
            MSG_BAD_USER_NAME_OR_PASSWORD,
            ErrorKind::BadUserNameOrPassword,
        );
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_5_should_raise_not_authorized_error() {
        let v: Vec<u8> = vec![2, 1, 5];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000).unwrap_err();
        let error_expected = PacketError::new_kind(MSG_NOT_AUTHORIZED, ErrorKind::NotAuthorized);
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_should_return_valid_connack_packet_with_session_present_0_and_return_code_0(
    ) -> Result<(), PacketError> {
        let v: Vec<u8> = vec![0b10, 0b0, 0b0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 0b100000)?;
        let connack_expected = Connack {
            session_present: 0,
            return_code: 0,
        };
        assert_eq!(result, connack_expected);
        Ok(())
    }

    #[test]
    fn test_should_return_valid_connack_packet_with_session_present_1_and_return_code_0(
    ) -> Result<(), PacketError> {
        let v: Vec<u8> = vec![2, 1, 0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream, 32)?;
        let connack_expected = Connack {
            session_present: 1,
            return_code: 0,
        };
        assert_eq!(result, connack_expected);
        Ok(())
    }
}
