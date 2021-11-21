use crate::packet_error::ErrorKind;
use crate::traits::{MQTTDecoding, MQTTEncoding};
use crate::utf8::Field;

use super::Subscribe;
use super::*;
use std::io::Cursor;

const CONTROL_BYTE: u8 = 0b10000010;

#[test]
fn test_identifier() {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[123, 5]); // identifier
    v.extend(Field::new_from_string("unTopic").unwrap().encode());
    v.push(1); // QoS level 1

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), CONTROL_BYTE).unwrap();
    assert_eq!(packet.packet_identifier(), (123 << 8) + 5);
}

#[test]
fn test_no_topics_should_fail() {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[123, 5]); // identifier

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), CONTROL_BYTE);
    assert!(packet.is_err());
}

#[test]
fn test_one_topic() {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[123, 5]); // identifier
    v.extend(
        Field::new_from_string("unTopic/*/+/asd//x")
            .unwrap()
            .encode(),
    );
    v.push(1); // QoS level 1

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), CONTROL_BYTE).unwrap();
    assert_eq!(packet.topics().len(), 1);
    assert_eq!(
        packet.topics().first().unwrap().name(),
        "unTopic/*/+/asd//x"
    );
    assert_eq!(packet.topics().first().unwrap().qos(), QoSLevel::QoSLevel1);
}

#[test]
fn test_two_topics() {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[123, 5]); // identifier
    v.extend(Field::new_from_string("first").unwrap().encode());
    v.push(2); // QoS level 2
    v.extend(Field::new_from_string("second").unwrap().encode());
    v.push(0); // QoS level 0

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), CONTROL_BYTE).unwrap();
    assert_eq!(packet.topics().len(), 2);
    assert_eq!(packet.topics().first().unwrap().name(), "first");
    assert_eq!(packet.topics().first().unwrap().qos(), QoSLevel::QoSLevel2);
    assert_eq!(packet.topics()[1].name(), "second");
    assert_eq!(packet.topics()[1].qos(), QoSLevel::QoSLevel0);
}

#[test]
fn test_invalid_reserved_flags() {
    let invalid_first = 0b01000011;
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[0, 5]); // identifier
    v.extend(Field::new_from_string("unTopic").unwrap().encode());
    v.push(1); // QoS level 1

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), invalid_first).unwrap_err();
    assert_eq!(packet.kind(), ErrorKind::InvalidReservedBits);
}

#[test]
fn test_invalid_qos() {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(&[0, 5]); // identifier
    v.extend(Field::new_from_string("unTopic").unwrap().encode());
    v.push(3); // QoS level 3

    v.insert(0, v.len() as u8);
    let packet = Subscribe::read_from(&mut Cursor::new(v), CONTROL_BYTE);
    assert!(packet.is_err());
}

#[test]
fn test_subscribe_encode_1_topic() {
    let topic = Topic::new("topic", QoSLevel::QoSLevel1).unwrap();
    let topics = vec![topic];
    let subscribe = Subscribe::new(topics, 1);
    let packet = subscribe.encode().unwrap();
    assert_eq!(
        packet,
        [
            0b10000010, // Packet Type and Flags
            10,         // Remaining Length 10 = +2 +2 +5 +1
            0, 1, // Packet Identifier
            0, 5, // Topic Length
            116, 111, 112, 105, 99, // Topic Name
            1,  // Topic QoS
        ]
    );
}

#[test]
fn test_subscribe_encode_2_topics() {
    let topic1 = Topic::new("topic1", QoSLevel::QoSLevel0).unwrap();
    let topic2 = Topic::new("topic2", QoSLevel::QoSLevel1).unwrap();
    let topics = vec![topic1, topic2];
    let subscribe = Subscribe::new(topics, 2);
    let packet = subscribe.encode().unwrap();
    assert_eq!(
        packet,
        [
            0x82, // Packet Type and Flags
            20,   // Remaining Length 10 = +2 +9 +9
            0, 2, // Packet Identifier
            0, 6, // Topic 1 Length
            116, 111, 112, 105, 99, 49, // Topic 1 Name
            0,  // Topic 1 QoS
            0, 6, // Topic 2 Length
            116, 111, 112, 105, 99, 50, // Topic 2 Name
            1,  // Topic 2 QoS
        ]
    );
}
