use std::{io::Cursor, vec};

use crate::{disconnect::Disconnect, traits::MQTTDecoding, traits::MQTTEncoding};

// server_side tests
#[test]
fn test_valid_disconnect() {
    let mut bytes = vec![];
    let control_byte = 0b11100000;
    bytes.push(0b00000000); // remaining_length
    let mut stream = Cursor::new(bytes);
    let packet = Disconnect::read_from(&mut stream, control_byte);
    assert!(packet.is_ok());
}

#[test]
fn test_remaining_length_not_zero_should_fail() {
    let mut bytes = vec![];
    let control_byte = 0b11100000;
    bytes.push(0b00000001); // remaining_length
    let mut stream = Cursor::new(bytes);
    let packet = Disconnect::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

#[test]
fn test_invalid_packet_type() {
    let mut bytes = vec![];
    let control_byte = 0b11110000;
    bytes.push(0b00000000); // remaining_length
    let mut stream = Cursor::new(bytes);
    let packet = Disconnect::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

#[test]
fn test_invalid_reserved_bytes() {
    let mut bytes = vec![];
    let control_byte = 0b11100100;
    bytes.push(0b00000000); // remaining_length
    let mut stream = Cursor::new(bytes);
    let packet = Disconnect::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

// client_side tests
#[test]
fn test_control_byte() {
    let packet = Disconnect::new();
    let control_byte = packet.encode().unwrap()[0];
    let expected_control_byte = 0b11100000;
    assert_eq!(control_byte, expected_control_byte);
}

#[test]
fn test_remaining_length_should_be_zero() {
    let packet = Disconnect::new();
    let remaining_length = packet.encode().unwrap()[1];
    let expected_control_byte = 0;
    assert_eq!(remaining_length, expected_control_byte);
}

#[test]
fn test_packet_should_be_two_bytes_long() {
    let packet = Disconnect::new();
    let encoded = packet.encode().unwrap();
    assert_eq!(encoded.len(), 2);
}
