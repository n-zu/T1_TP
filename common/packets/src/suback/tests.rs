use super::*;

use crate::{
    packet_error::{ErrorKind, PacketError},
    traits::{MQTTDecoding, MQTTEncoding},
};
use std::io::Cursor;

#[doc(hidden)]
const CONTROL_BYTE_SUBACK: u8 = 0b10010000;

#[test]
fn test_valid_suback_with_return_codes_0_1_1_1() {
    let return_codes: Vec<u8> = vec![0, 1, 1, 1];
    let suback = Suback::new_from_vec(return_codes, 1).unwrap();
    let encoded_suback = suback.encode().unwrap();
    let expected: Vec<u8> = vec![CONTROL_BYTE_SUBACK, 6, 0, 1, 0, 1, 1, 1];
    assert_eq!(encoded_suback, expected)
}

#[test]
fn test_valid_suback_with_return_codes_0_0_0_0() {
    let return_codes: Vec<u8> = vec![0, 0, 0, 0];
    let suback = Suback::new_from_vec(return_codes, 2).unwrap();
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
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType)
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
