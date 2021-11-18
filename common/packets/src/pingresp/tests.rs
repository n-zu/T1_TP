use std::io::Cursor;

use crate::{
    packet_error::ErrorKind,
    pingresp::PingResp,
    traits::{MQTTDecoding, MQTTEncoding},
};

// client_side tests
#[test]
fn test_valid() {
    let control_byte = 0b11010000;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingResp::read_from(&mut stream, control_byte);
    assert!(packet.is_ok());
}

#[test]
fn test_invalid_packet_type() {
    let control_byte = 0b11110000;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingResp::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
    assert_eq!(
        packet.err().unwrap().kind(),
        ErrorKind::InvalidControlPacketType
    );
}

#[test]
fn test_invalid_reserved_bytes() {
    let control_byte = 0b11010010;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingResp::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

#[test]
fn test_invalid_remaining_length() {
    let control_byte = 0b11010000;
    let remaining_bytes = vec![0b00000001, 0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingResp::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

// server_side tests
#[test]
fn test_control_byte() {
    let encoded = PingResp::new().encode().unwrap();
    let control_byte = encoded[0];
    assert_eq!(control_byte, 0b11010000);
}

#[test]
fn test_remaining_length() {
    let encoded = PingResp::new().encode().unwrap();
    let remaining_length = encoded[1];
    assert_eq!(remaining_length, 0b00000000);
}

#[test]
fn test_packet_should_be_two_bytes_long() {
    let encoded = PingResp::new().encode().unwrap();
    assert_eq!(encoded.len(), 2);
}
