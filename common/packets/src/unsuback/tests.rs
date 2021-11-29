use super::*;
use crate::packet_error::ErrorKind;
use crate::traits::MQTTDecoding;
use crate::traits::MQTTEncoding;
use crate::unsuback::PacketError;
use crate::unsuback::Unsuback;
use std::io::Cursor;

#[doc(hidden)]
const CONTROL_BYTE: u8 = 0b10110000;
#[doc(hidden)]
const FIXED_REMAINING_LENGTH: u8 = 0b00000010;

#[test]
fn test_reserved_bits_other_than_0_should_raise_invalid_reserved_bits() {
    let control_byte = 0b10110001;
    let reserved_bits_buffer = [0b1111u8; 1];
    let mut stream = Cursor::new(reserved_bits_buffer);
    let result = Unsuback::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidReservedBits);
}

#[test]
fn test_control_packet_type_other_than_11_should_raise_invalid_control_packet_type_error() {
    let control_byte = 0b000;
    let control_packet_type_buffer = [0b1111u8; 1];
    let mut stream = Cursor::new(control_packet_type_buffer);
    let result = Unsuback::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType);
}

#[test]
fn test_valid_unsuback_packet_with_packet_id_1() {
    let control_byte = 0b10110000;
    let remaining_length = 2;
    let data_buffer: Vec<u8> = vec![remaining_length, 0, 1];
    let mut stream = Cursor::new(data_buffer);
    let expected_id = 1;
    let result = Unsuback::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(expected_id, result.packet_id());
}

#[test]
fn test_valid_unsuback_packet_with_packet_id_0_should_raise_invalid_protocol_error() {
    let control_byte = 0b10110000u8;
    let remaining_length = 2;
    let data_buffer: Vec<u8> = vec![remaining_length, 0, 0];
    let mut stream = Cursor::new(data_buffer);
    let result = Unsuback::read_from(&mut stream, control_byte).unwrap_err();
    let expected_error = PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
    assert_eq!(expected_error, result);
}

#[test]
fn test_returns_same_identifier() {
    let control_byte = 0b10110000;
    let remaining_length = 2;
    let data_buffer: Vec<u8> = vec![remaining_length, 0, 123];
    let mut stream = Cursor::new(data_buffer);
    let result = Unsuback::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(123, result.packet_id());
}

#[test]
fn test_unsuback_with_packet_id_0_should_raise_invalid_protocol_error() {
    let result = Unsuback::new(0).unwrap_err();
    let expected_error = PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
    assert_eq!(expected_error, result)
}

#[test]
fn test_encoding_unsuback_packet_with_packet_id_1() {
    let unsuback = Unsuback::new(1).unwrap();
    let result = unsuback.encode().unwrap();
    let expected: Vec<u8> = vec![CONTROL_BYTE, FIXED_REMAINING_LENGTH, 0, 1];
    assert_eq!(expected, result)
}
