use crate::packet_error::{ErrorKind, PacketError};
use crate::puback::{Puback, MSG_INVALID_PACKET_ID, MSG_PACKET_MORE_BYTES_THAN_EXPECTED};
use crate::traits::{MQTTDecoding, MQTTEncoding};
use std::io::Cursor;

#[test]
fn test_reserved_bits_other_than_0_should_raise_invalid_reserved_bits() {
    let control_byte = 0b01000001;
    let reserved_bits_buffer = [0b1111u8; 1];
    let mut stream = Cursor::new(reserved_bits_buffer);
    let result = Puback::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidReservedBits);
}

#[test]
fn test_control_packet_type_other_than_4_should_raise_invalid_control_packet_type_error() {
    let control_byte = 0b000;
    let control_packet_type_buffer = [0b1111u8; 1];
    let mut stream = Cursor::new(control_packet_type_buffer);
    let result = Puback::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType);
}

#[test]
fn test_valid_puback_packet_with_packet_id_1() {
    let control_byte = 0b01000000u8;
    let remaining_length = 2u8;
    let data_buffer: Vec<u8> = vec![remaining_length, 0, 1];
    let mut stream = Cursor::new(data_buffer);
    let expected = Puback::new(1).unwrap();
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
    let expected_error = PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
    assert_eq!(expected_error, result)
}

#[test]
fn test_encoding_puback_packet_with_packet_id_1() {
    let puback = Puback::new(1).unwrap();
    let result = puback.encode().unwrap();
    let expected: Vec<u8> = vec![0b01000000, 0b10, 0b0, 1];
    assert_eq!(expected, result)
}
