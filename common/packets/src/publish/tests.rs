use super::*;
use crate::packet_error::{ErrorKind, PacketError};
use crate::publish::Publish;
use crate::qos::QoSLevel;
use crate::traits::{MQTTDecoding, MQTTEncoding};
use crate::utf8::Field;
use std::io::Cursor;

#[test]
fn test_dup_flag_0_with_qos_level_different_from_0_should_raise_invalid_dup_flag() {
    let control_byte = 0b111000u8;
    let dummy: Vec<u8> = vec![0b111000];
    let mut stream = Cursor::new(dummy);
    let expected_error =
        PacketError::new_kind(MSG_DUP_FLAG_1_WITH_QOS_LEVEL_0, ErrorKind::InvalidDupFlag);
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result, expected_error);
}

#[test]
fn test_qos_level_3_should_raise_invalid_qos_level_error() {
    let control_byte = 0b110110u8;
    let dummy: Vec<u8> = vec![0b110110];
    let mut stream = Cursor::new(dummy);
    let expected_error = PacketError::new_kind("Invalid QoS level", ErrorKind::InvalidQoSLevel);
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result, expected_error);
}

#[test]
fn test_packet_control_type_5_should_raise_invalid_control_packet_type_error() {
    let control_byte = 0b100000u8;
    let dummy: Vec<u8> = vec![0b100000];
    let mut stream = Cursor::new(dummy);
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(result.kind(), ErrorKind::InvalidControlPacketType);
}

#[test]
fn test_publish_packet_with_qos_level_0_must_not_have_a_packet_id() {
    let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/b").unwrap().encode();
    let mut payload = "mensaje".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);
    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected = Publish {
        packet_id: None,
        topic_name: "a/b".to_string(),
        qos: QoSLevel::QoSLevel0,
        retain_flag: false,
        dup_flag: false,
        payload: "mensaje".to_string(),
    };
    let result = Publish::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(expected, result);
}

#[test]
fn test_publish_packet_with_qos_level_1_must_have_a_packet_id() {
    let control_byte = 0b110010u8; // primer byte con los flags con QoS level 1;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/b").unwrap().encode();
    let mut payload = "mensaje".as_bytes().to_vec();
    let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
    remaining_data.append(&mut topic);
    remaining_data.append(&mut packet_id_buf);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected = Publish {
        packet_id: Option::from(10_u16),
        topic_name: "a/b".to_string(),
        qos: QoSLevel::QoSLevel1,
        retain_flag: false,
        dup_flag: false,
        payload: "mensaje".to_string(),
    };
    let result = Publish::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(expected, result);
}

#[test]
fn test_publish_packet_might_have_zero_length_payload() {
    let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/b").unwrap().encode();
    let mut payload = "".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected = Publish {
        packet_id: None,
        topic_name: "a/b".to_string(),
        qos: QoSLevel::QoSLevel0,
        retain_flag: false,
        dup_flag: false,
        payload: "".to_string(),
    };
    let result = Publish::read_from(&mut stream, control_byte).unwrap();
    assert_eq!(expected, result);
    assert_eq!(expected.payload(), "");
}

#[test]
fn test_publish_packet_topics_should_be_case_sensitive() {
    let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/B").unwrap().encode();
    let mut payload = "aa".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected = Publish {
        packet_id: None,
        topic_name: "a/b".to_string(),
        qos: QoSLevel::QoSLevel0,
        retain_flag: false,
        dup_flag: false,
        payload: "".to_string(),
    };
    let result = Publish::read_from(&mut stream, control_byte).unwrap();
    assert_ne!(expected, result);
}

#[test]
fn test_publish_packet_topic_name_must_be_at_least_one_character_long() {
    let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("").unwrap().encode();
    let mut payload = "aa".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected_error = PacketError::new_kind(
        MSG_TOPIC_NAME_ONE_CHAR,
        ErrorKind::TopicNameMustBeAtLeastOneCharacterLong,
    );
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(expected_error, result);
}
#[test]
fn test_topic_name_can_not_have_wildcards() {
    let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("asd/#").unwrap().encode();
    let mut payload = "mensaje".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected_error = PacketError::new_kind(
        MSG_TOPIC_WILDCARDS,
        ErrorKind::TopicNameMustNotHaveWildcards,
    );
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(expected_error, result);
}

#[test]
fn test_publish_packet_can_not_have_packet_id_0() {
    let control_byte = 0b110010u8; // primer byte con los flags con QoS level 1;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/b").unwrap().encode();
    let mut payload = "los pollos hermanos".as_bytes().to_vec();
    let mut packet_id_buf: Vec<u8> = vec![0b0, 0b0]; // Packet identifier 0
    remaining_data.append(&mut topic);
    remaining_data.append(&mut packet_id_buf);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let expected_error = PacketError::new_msg(MSG_INVALID_PACKET_ID);
    let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
    assert_eq!(expected_error, result);
}

#[test]
fn test_max_qos() {
    let control_byte = 0b110100u8; // primer byte con los flags con QoS level 2;
    let mut remaining_data: Vec<u8> = vec![];
    let mut topic = Field::new_from_string("a/b").unwrap().encode();
    let mut payload = "aa".as_bytes().to_vec();
    remaining_data.append(&mut topic);
    remaining_data.append(&mut payload);

    let mut bytes = vec![remaining_data.len() as u8];
    bytes.append(&mut remaining_data);
    let mut stream = Cursor::new(bytes);
    let mut result = Publish::read_from(&mut stream, control_byte).unwrap();

    assert_eq!(result.qos(), QoSLevel::QoSLevel2);
    result.set_max_qos(QoSLevel::QoSLevel1);
    assert_eq!(result.qos(), QoSLevel::QoSLevel1);
    result.set_max_qos(QoSLevel::QoSLevel0);
    assert_eq!(result.qos(), QoSLevel::QoSLevel0);
}

#[test]
fn basic_test() {
    let packet = Publish::new(false, QoSLevel::QoSLevel0, false, "topic", "message", None).unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            0b00110000, // control_byte
            14,         // remaining_length
            0, 5, // largo topic_name
            116, 111, 112, 105, 99, // topic
            109, 101, 115, 115, 97, 103, 101 // message
        ]
    );
}

#[test]
fn test_retain_flag() {
    let packet = Publish::new(false, QoSLevel::QoSLevel0, true, "topic", "message", None).unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            0b00110001, // control_byte
            14,         // remaining_length
            0, 5, // largo topic_name
            116, 111, 112, 105, 99, // topic
            109, 101, 115, 115, 97, 103, 101 // message
        ]
    );
}

#[test]
fn test_qos_level_1() {
    let packet = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "topic",
        "message",
        Some(153),
    )
    .unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            0b00110010, // control_byte
            16,         // remaining_length
            0, 5, // largo topic_name
            116, 111, 112, 105, 99, // topic
            0, 153, // packet_identifier
            109, 101, 115, 115, 97, 103, 101 // message
        ]
    );
}

#[test]
fn test_packet_identifier() {
    let packet = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "topic",
        "message",
        Some(350),
    )
    .unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            0b00110010, // control_byte
            16,         // remaining_length
            0, 5, // largo topic_name
            116, 111, 112, 105, 99, // topic
            1, 94, // packet_identifier
            109, 101, 115, 115, 97, 103, 101 // message
        ]
    );
}

#[test]
fn test_publish_cannot_have_packet_identifier_with_qos_0() {
    let packet = Publish::new(
        false,
        QoSLevel::QoSLevel0,
        false,
        "topic",
        "message",
        Some(350),
    );
    assert!(packet.is_err());
}

#[test]
fn test_publish_must_have_packet_identifier_with_qos_1() {
    let packet = Publish::new(false, QoSLevel::QoSLevel1, false, "topic", "message", None);
    assert!(packet.is_err());
}

#[test]
fn test_topic_name_cannot_contain_single_level_wildcard() {
    let packet = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "topic+",
        "message",
        Some(350),
    );
    assert!(packet.is_err());
}

#[test]
fn test_topic_name_cannot_contain_multi_level_wildcard() {
    let packet = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "topic#",
        "message",
        Some(350),
    );
    assert!(packet.is_err());
}

#[test]
fn test_zero_length_payload() {
    let packet = Publish::new(false, QoSLevel::QoSLevel1, false, "topic", "", Some(350)).unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            0b00110010, // control_byte
            9,          // remaining_length
            0, 5, // largo topic_name
            116, 111, 112, 105, 99, // topic
            1, 94, // packet_identifier + zero length payload
        ]
    );
}

#[test]
fn test_set_retain() {
    let mut packet =
        Publish::new(false, QoSLevel::QoSLevel1, true, "topic", "", Some(350)).unwrap();
    packet.set_retain_flag(false);
    assert!(!packet.retain_flag());
}
