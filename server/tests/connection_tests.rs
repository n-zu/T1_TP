mod common;
use common::*;
use packets::connack::*;
use packets::connect::*;
use packets::traits::{MQTTDecoding, MQTTEncoding};
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[test]
fn test_connect_clean_session() {
    let (_s, port) = start_server();
    let mut stream = TcpStream::connect(format!("localhost:{}", port)).unwrap();
    let mut connect_builder = ConnectBuilder::new("id", 0, false).unwrap();
    connect_builder = connect_builder.user_name("user").unwrap();
    connect_builder = connect_builder.password("pass").unwrap();
    let connect = connect_builder.build().unwrap();

    stream.write(&connect.encode().unwrap()).unwrap();
    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 2);
    let connack = Connack::read_from(&mut stream, control[0]).unwrap();
    let connack_expected = Connack::new(false, ConnackReturnCode::Accepted);
    assert_eq!(connack, connack_expected);
}
