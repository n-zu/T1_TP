mod common;
use crate::common::*;
use packets::connack::*;
use packets::connect::*;
use packets::disconnect::Disconnect;
use packets::packet_error::ErrorKind;
use packets::pingreq::PingReq;
use packets::pingresp::PingResp;
use packets::traits::{MQTTDecoding, MQTTEncoding};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

#[test]
fn test_connect_clean_session_true() {
    let (_s, port) = start_server();
    // Me conecto con clean session en true
    let connection = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(connection, true, port, false);

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
    let mut connect_builder = ConnectBuilder::new("id", 0, true).unwrap();
    connect_builder = connect_builder.user_name("user").unwrap();
    connect_builder = connect_builder
        .password("contraseña totalmente incorrecta")
        .unwrap();
    let mut stream = connect_client(connect_builder, false, port, false);

    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let connack = Connack::read_from(&mut stream, control[0]);
    assert!(connack.is_err());
    assert_eq!(
        connack.unwrap_err().kind(),
        ErrorKind::BadUserNameOrPassword
    );
}

#[test]
fn test_connect_present_after_reconnection() {
    let (_s, port) = start_server();
    // Me conecto con clean_session = false
    let mut connect_builder = ConnectBuilder::new("id", 0, false).unwrap();

    let mut stream = connect_client(connect_builder, true, port, false);

    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let mut connack = Connack::read_from(&mut stream, control[0]).unwrap();

    // Primera conexion: session present debería ser false
    let mut connack_expected = Connack::new(false, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);

    // Me desconecto
    stream.write_all(&Disconnect::new().encode()).unwrap();

    connect_builder = ConnectBuilder::new("id", 0, false).unwrap();
    stream = connect_client(connect_builder, true, port, false);

    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    connack = Connack::read_from(&mut stream, control[0]).unwrap();

    // Segunda conexion: session present debería ser true
    connack_expected = Connack::new(true, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);
}

#[test]
fn test_pings() {
    let (_s, port) = start_server();
    let connect_builder = ConnectBuilder::new("id", 1, true).unwrap();
    let mut stream = connect_client(connect_builder, true, port, true);

    let mut control = [0u8];

    thread::sleep(Duration::from_millis(800));
    stream.write_all(&PingReq::new().encode().unwrap()).unwrap();
    stream.read_exact(&mut control).unwrap();
    PingResp::read_from(&mut stream, control[0]).unwrap();

    thread::sleep(Duration::from_millis(800));
    stream.write_all(&PingReq::new().encode().unwrap()).unwrap();
    stream.read_exact(&mut control).unwrap();
    PingResp::read_from(&mut stream, control[0]).unwrap();
}

#[test]
fn test_pings_should_disconnect() {
    let (_s, port) = start_server();
    let connect_builder = ConnectBuilder::new("id", 1, true).unwrap();
    let mut stream = connect_client(connect_builder, true, port, true);

    let mut control = [0u8];

    thread::sleep(Duration::from_millis(800));
    stream.write_all(&PingReq::new().encode().unwrap()).unwrap();
    stream.read_exact(&mut control).unwrap();
    PingResp::read_from(&mut stream, control[0]).unwrap();

    // Protocolo dice que si no mando ping luego de 1,5 veces el tiempo de keep_alive,
    // el servidor debería desconectarme. Le doy 100ms de márgen.
    thread::sleep(Duration::from_millis(1600));
    assert_eq!(stream.read(&mut control).unwrap(), 0);
}

#[test]
fn test_takeover_should_close_previous_connection() {
    let (_s, port) = start_server();
    let builder_1 = ConnectBuilder::new("id", 1, true).unwrap();
    let builder_2 = ConnectBuilder::new("id", 1, true).unwrap();

    let mut control = [0u8];
    let mut stream_1 = connect_client(builder_1, true, port, true);
    connect_client(builder_2, true, port, true);

    assert_eq!(stream_1.read(&mut control).unwrap(), 0);
}

#[test]
fn test_takeover_should_change_keep_alive() {
    let (_s, port) = start_server();
    let builder_1 = ConnectBuilder::new("id", 60, true).unwrap();
    let builder_2 = ConnectBuilder::new("id", 1, true).unwrap();

    let mut control = [0u8];
    let mut _stream_2 = connect_client(builder_1, true, port, true);
    let mut stream_2 = connect_client(builder_2, true, port, true);

    thread::sleep(Duration::from_millis(1600));
    assert_eq!(stream_2.read(&mut control).unwrap(), 0);
}

#[test]
fn test_takeover_only_works_with_same_username() {
    let (_s, port) = start_server();
    let builder_1 = ConnectBuilder::new("id", 60, true).unwrap();
    let builder_2 = ConnectBuilder::new("id", 1, true)
        .unwrap()
        .user_name("foo")
        .unwrap()
        .password("bar")
        .unwrap();

    let mut control = [0u8];
    let mut _stream_1 = connect_client(builder_1, true, port, true);
    let mut stream_2 = connect_client(builder_2, false, port, false);

    stream_2.read_exact(&mut control).unwrap();
    let err = Connack::read_from(&mut stream_2, control[0]).unwrap_err();

    assert_eq!(err.kind(), ErrorKind::IdentifierRejected);
}
