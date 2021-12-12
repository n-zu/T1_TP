use crate::{
    packet_error::{ErrorKind, PacketResult},
    traits::MQTTDecoding,
};

use super::*;
use std::io::Cursor;
#[test]
fn test_reserved_bits_at_1_should_raise_invalid_reserved_bits_error() {
    let v: Vec<u8> = vec![2, 0, 0];
    let mut stream = Cursor::new(v);
    let result = Connack::read_from(&mut stream, 0b100101).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidReservedBits);
}

#[test]
fn test_control_packet_type_other_than_2_should_raise_invalid_control_packet_type_error() {
    let v: Vec<u8> = vec![2, 0, 0];
    let mut stream = Cursor::new(v);
    let result = Connack::read_from(&mut stream, 0b110000).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType);
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
) -> PacketResult<()> {
    let v: Vec<u8> = vec![0b10, 0b0, 0b0];
    let mut stream = Cursor::new(v);
    let result = Connack::read_from(&mut stream, 0b100000)?;
    let connack_expected = Connack::new(false, ConnackReturnCode::Accepted);
    assert_eq!(result, connack_expected);
    Ok(())
}

#[test]
fn test_should_return_valid_connack_packet_with_session_present_1_and_return_code_0(
) -> PacketResult<()> {
    let v: Vec<u8> = vec![2, 1, 0];
    let mut stream = Cursor::new(v);
    let result = Connack::read_from(&mut stream, 32)?;
    let connack_expected = Connack::new(true, ConnackReturnCode::Accepted);
    assert_eq!(result, connack_expected);
    Ok(())
}
