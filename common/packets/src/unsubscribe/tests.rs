use crate::{
    packet_error::{ErrorKind, PacketError},
    traits::{MQTTDecoding, MQTTEncoding},
    utf8::Field,
};
use std::io::Cursor;

use super::*;

#[doc(hidden)]
const CONTROL_TYPE_UNSUBSCRIBE: u8 = 0b10100010;

#[test]
fn test_unsubscribe_packet_with_empty_topic_filter_should_raise_invalid_protocol_error() {
    let control_byte = 0b10100010u8;
    let v: Vec<u8> = vec![2, 0, 1]; // remaining length + packet id + no payload
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
    let expected_error =
        PacketError::new_kind(MSG_AT_LEAST_ONE_TOPIC_FILTER, ErrorKind::InvalidProtocol);
    assert_eq!(result, expected_error);
}

#[test]
fn test_unsubscribe_packet_with_empty_string_as_topic_filter_should_raise_invalid_protocol_error() {
    let control_byte = 0b10100010u8;
    let v: Vec<u8> = vec![4, 0, 1, 0, 0]; // remaining length + packet id + two bytes as 0 indicating empty string as topic filter
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
    let expected_error = PacketError::new_kind(
        MSG_AT_LEAST_ONE_CHAR_LONG_TOPIC_FILTER,
        ErrorKind::InvalidProtocol,
    );
    assert_eq!(result, expected_error);
}

#[test]
fn test_unsubscribe_packet_with_control_byte_other_than_10_should_raise_invalid_control_packet_type_error(
) {
    let control_byte = 0b10000010u8; // control byte 8 + 0010 reserved bits
    let mut topic = Field::new_from_string("temperatura/uruguay")
        .unwrap()
        .encode();
    let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
    v.append(&mut topic); // + payload
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType);
}

#[test]
fn test_unsubscribe_packet_with_reserved_bits_other_than_2_should_raise_error() {
    let control_byte = 0b10100000u8; // control byte 10 + reserved bits 0000
    let mut topic = Field::new_from_string("temperatura/uruguay")
        .unwrap()
        .encode();
    let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
    v.append(&mut topic); // + payload
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidReservedBits);
}

#[test]
fn test_valid_unsubscribe_packet_with_one_topic() {
    let control_byte = 0b10100010u8; // control byte 10 + reserved bits 0010
    let mut topic = Field::new_from_string("temperatura/uruguay")
        .unwrap()
        .encode();
    let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
    v.append(&mut topic); // + payload
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(result.packet_id(), 1u16);
    assert_eq!(
        *result.topic_filters(),
        vec!["temperatura/uruguay".to_string()]
    );
}

#[test]
fn test_valid_unsubscribe_packet_with_two_topics() {
    let control_byte = 0b10100010u8; // control byte 10 + reserved bits 0010
    let mut topic_uruguay = Field::new_from_string("temperatura/uruguay")
        .unwrap()
        .encode();
    let mut topic_argentina = Field::new_from_string("temperatura/argentina")
        .unwrap()
        .encode();
    let mut v: Vec<u8> = vec![46, 0, 1]; // remaining length + packet id
    v.append(&mut topic_uruguay); // + payload
    v.append(&mut topic_argentina); // + payload
    let mut stream = Cursor::new(v);
    let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap();
    let expected_id = 1u16;
    let expected_topic_filters = vec![
        "temperatura/uruguay".to_string(),
        "temperatura/argentina".to_string(),
    ];
    assert_eq!(result.packet_id(), expected_id);
    assert_eq!(*result.topic_filters(), expected_topic_filters);
}

#[test]
fn unsubscribe_packet_with_packet_id_0_should_raise_protocol_error() {
    let topics = vec!["temperatura/Argentina".to_string()];
    let result = Unsubscribe::new(0, topics).unwrap_err();
    let expected_error = PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
    assert_eq!(result, expected_error)
}

#[test]
fn valid_unsubscribe_packet_with_id_1_and_one_topic() {
    let topics = vec!["temperatura/Argentina".to_string()];
    let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
    let mut topic_encoded = Field::new_from_string("temperatura/Argentina")
        .unwrap()
        .encode();
    let mut expected: Vec<u8> = vec![
        CONTROL_TYPE_UNSUBSCRIBE,
        25, // remaining length
        0,
        1, // packet id
    ];
    expected.append(&mut topic_encoded);
    assert_eq!(result, expected);
}

#[test]
fn valid_unsubscribe_packet_with_id_5_and_two_topic() {
    let topics = vec![
        "temperatura/Argentina".to_string(),
        "temperatura/Uruguay".to_string(),
    ];
    let result = Unsubscribe::new(5, topics).unwrap().encode().unwrap();
    let mut topic_encoded_arg = Field::new_from_string("temperatura/Argentina")
        .unwrap()
        .encode();
    let mut topic_encoded_uru = Field::new_from_string("temperatura/Uruguay")
        .unwrap()
        .encode();
    let mut expected: Vec<u8> = vec![
        CONTROL_TYPE_UNSUBSCRIBE,
        46, // remaining length
        0,
        5, // packet id
    ];
    expected.append(&mut topic_encoded_arg);
    expected.append(&mut topic_encoded_uru);
    assert_eq!(result, expected);
}

#[test]
fn unsubscribe_packet_can_have_empty_string_as_topic_filter() {
    let topics = vec!["".to_string()];
    let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
    let mut topic_encoded = Field::new_from_string("").unwrap().encode();
    let mut expected: Vec<u8> = vec![
        CONTROL_TYPE_UNSUBSCRIBE,
        4, // remaining length
        0,
        1, // packet id
    ];
    expected.append(&mut topic_encoded);
    assert_eq!(result, expected);
}

#[test]
fn client_unsubscribe_packet_can_have_empty_topic_filter() {
    let topics = vec![];
    let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
    let expected: Vec<u8> = vec![
        CONTROL_TYPE_UNSUBSCRIBE,
        2, // remaining length
        0,
        1, // packet id
    ];
    assert_eq!(result, expected);
}

#[test]
fn test_returns_same_identifier() {
    let topics = vec!["temperatura/Argentina".to_string()];
    let packet = Unsubscribe::new(123, topics).unwrap();
    assert_eq!(packet.packet_id(), 123);
}
