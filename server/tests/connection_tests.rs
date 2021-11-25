mod common;
use crate::common::*;
use packets::connack::*;
use packets::connect::*;
use packets::disconnect::Disconnect;
use packets::packet_error::ErrorKind;
use packets::traits::{MQTTDecoding, MQTTEncoding};
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[test]
fn test_connect_clean_session() {
    let (_s, port) = start_server();
    let mut stream = connect_client(0, true, port, false);

    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let connack = Connack::read_from(&mut stream, control[0]).unwrap();
    let connack_expected = Connack::new(false, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);
}

#[test]
fn test_connect_incorrect_password() {
    let (_s, port) = start_server();
    let mut stream = TcpStream::connect(format!("localhost:{}", port)).unwrap();
    let mut connect_builder = ConnectBuilder::new("id", 0, false).unwrap();
    connect_builder = connect_builder.user_name("user").unwrap();
    connect_builder = connect_builder.password("contraseña totalmente incorrecta").unwrap();
    let connect = connect_builder.build().unwrap();

    stream.write_all(&connect.encode().unwrap()).unwrap();
    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let connack = Connack::read_from(&mut stream, control[0]);
    assert!(connack.is_err());
    assert_eq!(connack.unwrap_err().kind(), ErrorKind::BadUserNameOrPassword);
}

#[test]
fn test_connect_present_after_reconnection() {
    let (_s, port) = start_server();
    // Me conecto con clean_session = false
    let mut stream = connect_client(0, false, port, false);

    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let mut connack = Connack::read_from(&mut stream, control[0]).unwrap();

    // Primera conexion: session present debería ser false
    let mut connack_expected = Connack::new(false, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);

    // Me desconecto
    stream.write_all(&Disconnect::new().encode()).unwrap();    

    stream = connect_client(0, false, port, false);

    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    connack = Connack::read_from(&mut stream, control[0]).unwrap();

    // Segunda conexion: session present debería ser true
    connack_expected = Connack::new(true, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);
}