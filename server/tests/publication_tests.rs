mod common;
use std::{
    io::{Read, Write},
    thread,
    time::Duration,
};

use packets::{
    connect::ConnectBuilder,
    puback::Puback,
    publish::Publish,
    qos::QoSLevel::*,
    suback::Suback,
    subscribe::Subscribe,
    topic::Topic,
    traits::{MQTTDecoding, MQTTEncoding},
};

use crate::common::*;

#[test]
fn test_subscription_qos0() {
    let (_s, port) = start_server();
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, true, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(vec![Topic::new("topic", QoSLevel0).unwrap()], 123);
    stream.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Mando publish
    let publish = Publish::new(false, QoSLevel0, false, "topic", "message", None).unwrap();
    stream.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo publish
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
}

#[test]
fn test_subscription_qos1() {
    let (_s, port) = start_server();
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, true, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(vec![Topic::new("topic", QoSLevel1).unwrap()], 123);
    stream.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Mando publish
    let publish = Publish::new(false, QoSLevel1, false, "topic", "message", Some(10)).unwrap();
    stream.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo publish o puback
    let mut puback_received = false;
    let mut publish_received = false;
    while !puback_received || !publish_received {
        stream.read_exact(&mut control).unwrap();
        match control[0] >> 4 {
            3 if !publish_received => {
                // Debería mandarle un puback pero ya me desconecto
                let recv_publish = Publish::read_from(&mut stream, control[0]).unwrap();
                assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
                publish_received = true;
            }
            4 if !puback_received => {
                let recv_puback = Puback::read_from(&mut stream, control[0]).unwrap();
                assert_eq!(recv_puback.packet_id(), 10);
                puback_received = true;
            }
            _ => panic!("Se recibió paquete inválido"),
        }
    }
}

#[test]
fn test_subscription_lowers_qos() {
    let (_s, port) = start_server();
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, true, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(vec![Topic::new("topic", QoSLevel0).unwrap()], 123);
    stream.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Mando publish
    let mut publish = Publish::new(false, QoSLevel1, false, "topic", "message", Some(10)).unwrap();
    stream.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Me suscribi con QoS 0, deberia bajar el QoS del publish
    publish.set_max_qos(QoSLevel0);
    let mut puback_received = false;
    let mut publish_received = false;
    while !puback_received || !publish_received {
        stream.read_exact(&mut control).unwrap();
        match control[0] >> 4 {
            3 if !publish_received => {
                let recv_publish = Publish::read_from(&mut stream, control[0]).unwrap();
                assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
                publish_received = true;
            }
            4 if !puback_received => {
                let recv_puback = Puback::read_from(&mut stream, control[0]).unwrap();
                assert_eq!(recv_puback.packet_id(), 10);
                puback_received = true;
            }
            _ => panic!("Se recibió paquete inválido"),
        }
    }
}

#[test]
fn test_subscription_different_clients() {
    let (_s, port) = start_server();
    let builder_1 = ConnectBuilder::new("id1", 0, true).unwrap();
    let mut stream_1 = connect_client(builder_1, true, port, true);
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, true, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(vec![Topic::new("topic", QoSLevel0).unwrap()], 123);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Mando publish
    let publish = Publish::new(false, QoSLevel0, false, "topic", "message", None).unwrap();
    stream_2.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo publish
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
}

#[test]
fn test_subscription_different_clients_persistent_session() {
    let (_s, port) = start_server();
    // Me conecto con clean session en false
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let mut stream_1 = connect_client(builder_1, true, port, true);
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, true, port, true);
    let mut control = [0u8];

    // Mando subscribe con QoS1
    let subscribe = Subscribe::new(vec![Topic::new("topic", QoSLevel1).unwrap()], 123);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    drop(stream_1); // Debería recordar al usuario

    // Mando publish con QoS1
    let publish = Publish::new(false, QoSLevel1, false, "topic", "message", Some(10)).unwrap();
    stream_2.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));
    // Ignoro el puback

    // Me reconecto
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let mut stream_1 = connect_client(builder_1, true, port, true);

    // Recibo publish
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_1, control[0]).unwrap();
    // ignoro el primer byte por si le ponen la dup flag
    assert_eq!(
        recv_publish.encode().unwrap()[1..],
        publish.encode().unwrap()[1..]
    );
}
