mod common;
use std::{
    io::{Read, Write},
    thread,
    time::Duration,
};

use client::{Client, Message};
use common::*;
use packets::{
    connack::{Connack, ConnackReturnCode},
    connect::ConnectBuilder,
    puback::Puback,
    publish::Publish,
    qos::QoSLevel::*,
    traits::{MQTTDecoding, MQTTEncoding},
};

#[test]
fn test_send_publish_qos0() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let mut client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);
    thread::sleep(Duration::from_millis(500));
    let (mut stream, _) = server.accept_connection(connack);

    let publish = Publish::new(false, QoSLevel0, false, "topic", "msg", None).unwrap();
    let pub_bytes = publish.encode().unwrap();
    client.publish(publish).unwrap();

    thread::sleep(Duration::from_millis(1000));
    let msgs = observer.messages.lock().unwrap();
    assert!(matches!(msgs[1], Message::Published(Ok(None))));

    let mut buf = [0; 1];
    stream.read_exact(&mut buf).unwrap();
    let recv_publish = Publish::read_from(&mut stream, buf[0]).unwrap();
    assert_eq!(pub_bytes, recv_publish.encode().unwrap());
}

#[test]
fn test_send_publish_qos1() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let mut client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);
    thread::sleep(Duration::from_millis(500));
    let (mut stream, _) = server.accept_connection(connack);

    let publish = Publish::new(false, QoSLevel1, false, "topic", "msg", Some(123)).unwrap();
    let pub_bytes = publish.encode().unwrap();
    client.publish(publish).unwrap();

    // Este wait es para no mandar el puback antes de que el cliente llegue a mandar su publish en el otro thread
    thread::sleep(Duration::from_millis(1000));
    let puback = Puback::new(123).unwrap();
    stream.write_all(&puback.encode()).unwrap();

    thread::sleep(Duration::from_millis(1000));
    let msgs = observer.messages.lock().unwrap();
    assert!(matches!(msgs[1], Message::Published(Ok(Some(_)))));
    if let Message::Published(Ok(Some(recv_puback))) = &msgs[1] {
        assert_eq!(puback.encode(), recv_puback.encode());
    }

    let mut buf = [0; 1];
    stream.read_exact(&mut buf).unwrap();
    let recv_publish = Publish::read_from(&mut stream, buf[0]).unwrap();
    assert_eq!(pub_bytes, recv_publish.encode().unwrap());
}
