use std::io::Cursor;

use super::*;
use crate::{
    packet_error::ErrorKind,
    traits::{MQTTDecoding, MQTTEncoding},
    utf8::Field,
};

#[doc(hidden)]
const CONNECT_CONTROL_BYTE: u8 = 0b00010000;
#[doc(hidden)]
const INVALID_RESERVED: u8 = 0b00000001;

// server_side tests
#[test]
fn test_keep_alive() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(0u8); //Flags
    v.append(&mut vec![16u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert_eq!(
        Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
            .unwrap()
            .keep_alive(),
        ((16 << 8) + 60) as u16
    );
}

#[test]
fn test_invalid_protocol() {
    let mut v = Field::new_from_string("Not MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(0u8); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert_eq!(
        Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidProtocol
    );
}

#[test]
fn test_invalid_protocol_level() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(7u8); // Nivel
    v.push(0u8); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert_eq!(
        Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidProtocolLevel
    );
}

#[test]
fn test_invalid_reserved_flag() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(INVALID_RESERVED); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert_eq!(
        Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidFlags
    );
}

#[test]
fn test_will_flag_0_topic_message_1() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(((QoSLevel::QoSLevel1 as u8) << WILL_QOS_SHIFT) | WILL_RETAIN); //Flags
    v.append(&mut Field::new_from_string("id").unwrap().encode());
    v.append(&mut vec![0u8, 60u8]); //Keep alive

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert_eq!(
        Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidFlags
    );
}

#[test]
fn test_username_missing_but_needed() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(USER_NAME_PRESENT); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert!(Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE).is_err());
}

#[test]
fn test_username_present_but_shouldnt() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(0); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());
    v.append(&mut Field::new_from_string("unNombre").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert!(Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE).is_err());
}

#[test]
fn test_connect_clean_session() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(CLEAN_SESSION); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    assert!(Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE)
        .unwrap()
        .clean_session());
}

#[test]
fn test_will_flag() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(LAST_WILL_PRESENT | WILL_RETAIN); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());
    v.append(&mut Field::new_from_string("soyUnTopic").unwrap().encode());
    v.append(&mut Field::new_from_string("soyUnMensaje").unwrap().encode());

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    let packet = Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE).unwrap();
    let will = packet.last_will().unwrap();

    assert!(will.retain_flag);
    assert_eq!(will.topic_name, "soyUnTopic");
    assert_eq!(will.topic_message, "soyUnMensaje");
}

#[test]
fn test_will_flag_username_password() {
    let mut v = Field::new_from_string("MQTT").unwrap().encode();
    v.push(4u8); // Nivel
    v.push(LAST_WILL_PRESENT | WILL_RETAIN | USER_NAME_PRESENT | PASSWORD_PRESENT); //Flags
    v.append(&mut vec![0u8, 60u8]); //Keep alive
    v.append(&mut Field::new_from_string("id").unwrap().encode());
    v.append(&mut Field::new_from_string("soyUnTopic").unwrap().encode());
    v.append(&mut Field::new_from_string("soyUnMensaje").unwrap().encode());
    v.append(
        &mut Field::new_from_string("siAlguienLeeEstoFelicitaciones")
            .unwrap()
            .encode(),
    );
    v.append(
        &mut Field::new_from_string("contraseñaSuperSecreta")
            .unwrap()
            .encode(),
    );

    let mut bytes = vec![v.len() as u8];
    bytes.append(&mut v);
    let mut stream = Cursor::new(bytes);

    let packet = Connect::read_from(&mut stream, CONNECT_CONTROL_BYTE).unwrap();
    let will = packet.last_will().unwrap();

    assert!(will.retain_flag);
    assert_eq!(will.topic_name, "soyUnTopic");
    assert_eq!(will.topic_message, "soyUnMensaje");
    assert_eq!(
        packet.user_name().unwrap(),
        "siAlguienLeeEstoFelicitaciones"
    );
    assert_eq!(packet.password().unwrap(), "contraseñaSuperSecreta");
}

// client_side tests
#[test]
fn test_basics() {
    let packet = ConnectBuilder::new("rust", 13, true)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            16, // fixed_header
            16, // remaining_length
            0, 4, 77, 81, 84, 84, // (0) (4) MQTT
            4,  // protocol_level
            2,  // flags: CLEAN_SESSION
            0, 13, // keep_alive
            0, 4, 114, 117, 115, 116 // (0) (4) rust
        ]
    );
}

#[test]
fn test_username_password() {
    let packet = ConnectBuilder::new("rust", 25, true)
        .unwrap()
        .user_name("yo")
        .unwrap()
        .password("pass")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            16, // fixed_header
            26, // remaining_length
            0, 4, 77, 81, 84, 84,  // (0) (4) MQTT
            4,   // protocol_level
            194, // flags: CLEAN_SESSION | USER_NAME_PRESENT | PASSWORD_PRESENT
            0, 25, // keep_alive
            0, 4, 114, 117, 115, 116, // (0) (4) rust
            0, 2, 121, 111, // user_name: (0) (2) yo
            0, 4, 112, 97, 115, 115 // password: (0) (4) pass
        ]
    );
}

#[test]
fn test_password_without_username() {
    let builder = ConnectBuilder::new("rust", 25, true)
        .unwrap()
        .password("pass")
        .unwrap();

    assert!(builder.build().is_err());
}

#[test]
fn test_clean_session_false() {
    let packet = ConnectBuilder::new("rust", 25, false)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        packet.encode().unwrap(),
        [
            16, // fixed_header
            16, // remaining_length
            0, 4, 77, 81, 84, 84, // (0) (4) MQTT
            4,  // protocol_level
            0,  // flags
            0, 25, // keep_alive
            0, 4, 114, 117, 115, 116 // password: (0) (4) rust
        ]
    )
}
