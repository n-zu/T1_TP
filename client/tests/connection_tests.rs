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
    disconnect::Disconnect,
    pingreq::PingReq,
    pingresp::PingResp,
    traits::{MQTTDecoding, MQTTEncoding},
};

#[test]
fn test_connection() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let connect_bytes = connect.encode().unwrap();
    let observer = ObserverMock::new();
    let _client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);
    let connack_bytes = connack.encode().unwrap();
    thread::sleep(Duration::from_millis(500));
    let (_stream, recv_connect) = server.accept_connection(connack);

    assert_eq!(recv_connect.encode().unwrap(), connect_bytes);

    // Le doy medio segundo para mandarle al observer
    thread::sleep(Duration::from_millis(500));
    let msgs = observer.messages.lock().unwrap();
    assert!(matches!(msgs[0], Message::Connected(Ok(_))));
    if let Message::Connected(Ok(recv_connack)) = msgs[0] {
        assert_eq!(recv_connack.encode().unwrap(), connack_bytes);
        return;
    }
}

#[test]
fn test_pings() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 1, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let _client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);
    let (mut stream, _) = server.accept_connection(connack);

    let mut buf = [0; 1];

    thread::sleep(Duration::from_millis(1000));
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(buf[0] >> 4, 12);
    let _ = PingReq::read_from(&mut stream, buf[0]).unwrap();
    stream
        .write_all(&PingResp::new().encode().unwrap())
        .unwrap();

    thread::sleep(Duration::from_millis(1000));
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(buf[0] >> 4, 12);
    let _ = PingReq::read_from(&mut stream, buf[0]).unwrap();
    stream
        .write_all(&PingResp::new().encode().unwrap())
        .unwrap();
}

#[test]
fn test_disconnect() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);

    thread::sleep(Duration::from_millis(500));
    let (mut stream, _) = server.accept_connection(connack);
    drop(client);

    let mut buf = [0; 1];
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(buf[0] >> 4, 14);
    let _ = Disconnect::read_from(&mut stream, buf[0]).unwrap();
}

#[test]
fn test_failed_connection() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let _client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();

    thread::sleep(Duration::from_millis(500));
    let connack = Connack::new(false, ConnackReturnCode::BadUserNameOrPassword);
    let (_s, _) = server.accept_connection(connack);

    thread::sleep(Duration::from_millis(500));
    let msgs = observer.messages.lock().unwrap();
    assert!(matches!(msgs[0], Message::Connected(Err(_))));
}

#[test]
fn test_server_closed() {
    let mut server = ServerMock::new();
    let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
    let observer = ObserverMock::new();
    let _client = Client::new(
        &format!("localhost:{}", server.port),
        observer.clone(),
        connect,
    )
    .unwrap();
    let connack = Connack::new(false, ConnackReturnCode::Accepted);
    let (s, _) = server.accept_connection(connack);
    drop(server);
    drop(s);

    thread::sleep(Duration::from_millis(500));
    let msgs = observer.messages.lock().unwrap();
    // Tiene que ser el segundo mensaje, el primero es el connected
    assert!(matches!(msgs[1], Message::InternalError(_)));
}
