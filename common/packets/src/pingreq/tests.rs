use std::io::Cursor;

use crate::{
    packet_error::ErrorKind,
    pingreq::PingReq,
    traits::{MQTTDecoding, MQTTEncoding},
};

#[doc(hidden)]
const RESERVED_BYTES_MASK: u8 = 0b00001111;

#[test]
fn test_valid() {
    let control_byte = 0b11000000;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingReq::read_from(&mut stream, control_byte);
    assert!(packet.is_ok());
}

#[test]
fn test_invalid_packet_type() {
    let control_byte = 0b11100000;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingReq::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
    assert_eq!(
        packet.err().unwrap().kind(),
        ErrorKind::InvalidControlPacketType
    );
}

#[test]
fn test_invalid_reserved_bytes() {
    let control_byte = 0b11000010;
    let remaining_bytes = vec![0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingReq::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

#[test]
fn test_invalid_remaining_length() {
    let control_byte = 0b11000000;
    let remaining_bytes = vec![0b00000001, 0b00000000];
    let mut stream = Cursor::new(remaining_bytes);
    let packet = PingReq::read_from(&mut stream, control_byte);
    assert!(packet.is_err());
}

#[test]
fn test_packet_type() {
    let bytes = PingReq::new().encode().unwrap();
    let packet_type = bytes[0] >> 4;
    assert_eq!(packet_type, 12);
}

#[test]
fn test_reserved_bytes_are_zero() {
    let bytes = PingReq::new().encode().unwrap();
    let reserved_bytes = bytes[0] & RESERVED_BYTES_MASK;
    assert_eq!(reserved_bytes, 0);
}

#[test]
fn test_remaining_length_is_zero() {
    let bytes = PingReq::new().encode().unwrap();
    let remaining_length = bytes[1];
    assert_eq!(remaining_length, 0);
}
