use std::{
    io::{self, Read},
    thread,
    time::Duration,
};

use packets::{
    connect::{Connect, ConnectBuilder, LastWill},
    puback::Puback,
    publish::Publish,
    qos::QoSLevel,
    traits::MQTTDecoding,
};

use crate::{
    iomock::IOMock, network_connection::NetworkConnection, server::server_error::ServerErrorKind,
};

use super::Client;

fn make_publish(qos: QoSLevel) -> Publish {
    if qos == QoSLevel::QoSLevel0 {
        Publish::new(false, QoSLevel::QoSLevel0, false, "top", "message", None).unwrap()
    } else {
        Publish::new(false, QoSLevel::QoSLevel1, false, "top", "message", Some(1)).unwrap()
    }
}

fn make_connect(keep_alive: u16, clean_session: bool, last_will_qos: Option<QoSLevel>) -> Connect {
    let connect_builder = ConnectBuilder::new("client_id", keep_alive, clean_session).unwrap();
    if let Some(qos) = last_will_qos {
        if qos == QoSLevel::QoSLevel0 {
            connect_builder
                .last_will(LastWill::new(
                    String::from("top"),
                    String::from("message"),
                    QoSLevel::QoSLevel0,
                    false,
                ))
                .build()
                .unwrap()
        } else {
            connect_builder
                .last_will(LastWill::new(
                    String::from("top"),
                    String::from("message"),
                    QoSLevel::QoSLevel0,
                    false,
                ))
                .build()
                .unwrap()
        }
    } else {
        connect_builder.build().unwrap()
    }
}

#[test]
fn test_creation() {
    let connect = ConnectBuilder::new("client_id", 0, true)
        .unwrap()
        .build()
        .unwrap();

    let connect_copy = connect.clone();

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let client = Client::new(connect, network_connection);

    assert_eq!(client.connect, connect_copy);
    assert!(client.connection.is_some());
    assert!(client.unacknowledged.is_empty());
    assert_eq!(client.id, String::from("client_id"));
}

#[test]
fn test_keep_alive_returns_correct_value() {
    let connect = ConnectBuilder::new("client_id", 1, true)
        .unwrap()
        .build()
        .unwrap();

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let client = Client::new(connect, network_connection);
    assert_eq!(client.keep_alive(), Some(Duration::from_millis(1500)));
}

#[test]
fn test_publish_send_packet_through_network_connection() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel0);

    let publish_copy = publish.clone();

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();

    let mut control = [0u8];
    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    network_connection_copy.read_exact(&mut control).unwrap();

    let publish_received = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(publish_copy, publish_received);
}

#[test]
fn test_publish_does_not_save_packet_in_unacknowledged_if_qos_is_0() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel0);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    assert!(client.unacknowledged.is_empty());
}

#[test]
fn test_publish_saves_packet_in_unacknowledged_if_qos_is_1() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let mut publish_copy = publish.clone();
    publish_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    assert_eq!(client.unacknowledged[0].1, publish_copy);
}

#[test]
fn test_send_unacknowledged() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let mut publish_copy = publish.clone();
    publish_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    client.send_unacknowledged(None, None).unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Primer publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Segundo publish
    network_connection_copy.read_exact(&mut control).unwrap();
    let unacknowledged = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(unacknowledged, publish_copy);
}

#[test]
fn test_send_unacknowledged_multiple_times() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let mut publish_copy = publish.clone();
    publish_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    client.send_unacknowledged(None, None).unwrap();
    client.send_unacknowledged(None, None).unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Primer publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Segundo publish
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Tercer publish
    network_connection_copy.read_exact(&mut control).unwrap();
    let unacknowledged = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    assert_eq!(unacknowledged, publish_copy);
}

#[test]
fn test_acknowledge_remove_packet_from_list() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let puback = Puback::new(1).unwrap();

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    client.acknowledge(puback).unwrap();

    assert!(client.unacknowledged.is_empty());
}

#[test]
fn test_send_unacknowledged_inflight_messages() {
    let connect = make_connect(0, true, None);

    let publish_1 = make_publish(QoSLevel::QoSLevel1);

    let publish_2 = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "top",
        "message2",
        Some(2),
    )
    .unwrap();

    let mut publish_1_copy = publish_1.clone();
    let mut publish_2_copy = publish_2.clone();

    publish_1_copy.set_dup(true);
    publish_2_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish_1).unwrap();
    client.send_publish(publish_2).unwrap();

    client.send_unacknowledged(Some(1), None).unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Los primeros 2 paquetes son los originales (dup_flag false)
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Primer publish (con dup_flag true)
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    let received = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(received, publish_1_copy);

    // El segundo publish no se mando
    network_connection_copy.close().unwrap();
    let result = network_connection_copy.read_exact(&mut control);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn test_send_unacknowledged_inflight_messages_bigger_than_unacknowledged_should_work() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let mut publish_copy = publish.clone();
    publish_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    client.send_unacknowledged(None, None).unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Primer publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Segundo publish
    network_connection_copy.read_exact(&mut control).unwrap();
    let unacknowledged = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(unacknowledged, publish_copy);
}

#[test]
fn test_send_unacknowledged_min_elapsed_time_should_not_send_recent_packets() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let publish_copy = publish.clone();

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    client
        .send_unacknowledged(None, Some(Duration::from_secs(5)))
        .unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Primer publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    let received = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(received, publish_copy);

    // El segundo publish no se mando
    network_connection_copy.close().unwrap();
    let result = network_connection_copy.read_exact(&mut control);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn test_send_unacknowledged_min_elapsed_time_should_send_old_packets() {
    let connect = make_connect(0, true, None);

    let publish = make_publish(QoSLevel::QoSLevel1);

    let mut publish_copy = publish.clone();
    publish_copy.set_dup(true);

    let network_connection = NetworkConnection::new(0, IOMock::new());

    let mut client = Client::new(connect, network_connection);
    client.send_publish(publish).unwrap();
    thread::sleep(Duration::from_millis(150));

    client
        .send_unacknowledged(None, Some(Duration::from_millis(100)))
        .unwrap();

    let mut network_connection_copy = client.connection.unwrap().try_clone().unwrap();

    // Primer publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    Publish::read_from(&mut network_connection_copy, control[0]).unwrap();

    // Segundo publish
    let mut control = [0u8];
    network_connection_copy.read_exact(&mut control).unwrap();
    let received = Publish::read_from(&mut network_connection_copy, control[0]).unwrap();
    assert_eq!(received, publish_copy);
}

#[test]
fn test_disconnect_gracefully_should_remove_last_will() {
    let connect = make_connect(0, true, Some(QoSLevel::QoSLevel0));

    let network_connection = NetworkConnection::new(0, IOMock::new());
    let mut client = Client::new(connect, network_connection);
    client.disconnect(true).unwrap();
    assert!(client.connect.last_will().is_none());
}

#[test]
fn test_disconnect_ungracefully_should_remove_last_will() {
    let connect = make_connect(0, true, Some(QoSLevel::QoSLevel0));

    let network_connection = NetworkConnection::new(0, IOMock::new());
    let mut client = Client::new(connect, network_connection);
    client.disconnect(false).unwrap();
    assert!(client.connect.last_will().is_none());
}

#[test]
fn test_reconnect_updates_connection_info() {
    let connect_1 = make_connect(0, true, None);

    let connect_2 = make_connect(15, false, None);

    let connect_2_copy = connect_2.clone();

    let network_connection_1 = NetworkConnection::new(0, IOMock::new());
    let network_connection_2 = NetworkConnection::new(1, IOMock::new());
    let mut client = Client::new(connect_1, network_connection_1);
    client.reconnect(connect_2, network_connection_2).unwrap();

    assert_eq!(*client.connection_id().unwrap(), 1);
    assert_eq!(client.connect, connect_2_copy);
}

#[test]
fn test_reconnect_with_clean_session_should_clean_unacknowledged() {
    let connect_1 = make_connect(0, false, None);

    let connect_2 = make_connect(15, true, None);

    let publish =
        Publish::new(false, QoSLevel::QoSLevel1, false, "top", "message", Some(1)).unwrap();

    let network_connection_1 = NetworkConnection::new(0, IOMock::new());
    let network_connection_2 = NetworkConnection::new(1, IOMock::new());
    let mut client = Client::new(connect_1, network_connection_1);
    client.send_publish(publish).unwrap();

    client.reconnect(connect_2, network_connection_2).unwrap();

    assert!(client.unacknowledged.is_empty());
}

#[test]
fn test_reconnect_does_not_work_with_different_client_id() {
    let connect_1 = make_connect(0, true, None);

    let connect_2 = ConnectBuilder::new("otro_id", 0, true)
        .unwrap()
        .build()
        .unwrap();

    let network_connection_1 = NetworkConnection::new(0, IOMock::new());
    let network_connection_2 = NetworkConnection::new(1, IOMock::new());
    let mut client = Client::new(connect_1, network_connection_1);

    let result = client.reconnect(connect_2, network_connection_2);
    assert_eq!(result.unwrap_err().kind(), ServerErrorKind::Irrecoverable);
}
