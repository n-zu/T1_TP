mod common;
use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};

use packets::{
    connect::{ConnectBuilder, LastWill},
    disconnect::Disconnect,
    puback::Puback,
    publish::Publish,
    qos::QoSLevel::*,
    suback::Suback,
    subscribe::Subscribe,
    topic_filter::TopicFilter,
    traits::{MQTTDecoding, MQTTEncoding},
};

use crate::common::*;

#[test]
fn test_subscription_qos0() {
    let (_s, port) = start_server(None, None);
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
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
    let (_s, port) = start_server(None, None);
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel1)], 123);
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

    // Recibo puback
    stream.read_exact(&mut control).unwrap();
    let recv_puback = Puback::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(recv_puback.packet_id(), 10);

    // Recibo publish
    stream.read_exact(&mut control).unwrap();
    let recv_publish = Publish::read_from(&mut stream, control[0]).unwrap();
    assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
}

#[test]
fn test_subscription_lowers_qos() {
    let (_s, port) = start_server(None, None);
    let builder = ConnectBuilder::new("id", 0, true).unwrap();
    let mut stream = connect_client(builder, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
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
    let (_s, port) = start_server(None, None);
    let builder_1 = ConnectBuilder::new("id1", 0, true).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // Mando subscribe
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
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
    let (_s, port) = start_server(None, None);
    // Me conecto con clean session en false
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // Mando subscribe con QoS1
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel1)], 123);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    stream_1
        .write_all(&Disconnect::new().encode().unwrap())
        .unwrap();
    drop(stream_1); // Debería recordar al usuario

    // Mando publish con QoS1
    let publish = Publish::new(false, QoSLevel1, false, "topic", "message", Some(10)).unwrap();
    stream_2.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));
    // Ignoro el puback

    // Me reconecto
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);

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

#[test]
fn test_last_will() {
    let (_s, port) = start_server(None, None);
    // Me conecto con last will
    let mut builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    builder_1 = builder_1.last_will(LastWill::new(
        TopicFilter::new("topic", QoSLevel0).unwrap(),
        "message".to_string(),
        false,
    ));
    let stream_1 = connect_client(builder_1, port, true);

    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // Mando subscribe con QoS0
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
    stream_2.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_2.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_2, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Me desconecto sin mandar disconnect
    drop(stream_1);

    // El otro debería recibir publish
    stream_2.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_2, control[0]).unwrap();
    // ignoro el primer byte por si le ponen la dup flag
    assert_eq!(recv_publish.topic_name(), "topic");
    assert_eq!(recv_publish.payload(), "message");
}

#[test]
fn test_gracefully_disconnection_should_not_send_last_will() {
    let (_s, port) = start_server(None, None);
    // Me conecto con last will
    let mut builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    builder_1 = builder_1.last_will(LastWill::new(
        TopicFilter::new("topic", QoSLevel0).unwrap(),
        "message".to_string(),
        false,
    ));
    let mut stream_1 = connect_client(builder_1, port, true);

    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // Mando subscribe con QoS0
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
    stream_2.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_2.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_2, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Me desconecto mandando disconnect
    stream_1
        .write_all(&Disconnect::new().encode().unwrap())
        .unwrap();

    // El otro no debería recibir publish
    stream_2
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    assert_eq!(
        stream_2.read_exact(&mut control).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn test_takeover_should_change_clean_session() {
    let (_s, port) = start_server(None, None);
    // Me conecto con clean_session false y me suscribo a topic
    // Me reconecto con clean_session true y LastWill en topic con QoS 1
    // Me desconecto ungracefully para que se mande el LastWill
    // Me vuelvo a conectar
    // Si se cambio el clean_session, no deberia recibir el LastWill porque la
    // segunda conexion fue con clean_session en true
    let builder_1 = ConnectBuilder::new("id", 1, false).unwrap();
    let builder_2 = ConnectBuilder::new("id", 1, true)
        .unwrap()
        .last_will(LastWill::new(
            TopicFilter::new("topic", QoSLevel1).unwrap(),
            "no deberia llegar".to_owned(),
            false,
        ));
    let builder_3 = ConnectBuilder::new("id", 1, false).unwrap();

    let subscribe = Subscribe::new(tpc![("topic", QoSLevel1)], 123);

    let mut control = [0u8];
    let mut stream_1 = connect_client(builder_1, port, true);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    stream_1.read_exact(&mut control).unwrap();
    Suback::read_from(&mut stream_1, control[0]).unwrap();

    let stream_2 = connect_client(builder_2, port, true);
    drop(stream_2);

    let mut stream_3 = connect_client(builder_3, port, true);
    stream_3
        .set_read_timeout(Some(Duration::from_millis(1500)))
        .unwrap();
    assert_eq!(
        stream_3.read_exact(&mut control).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn test_retained_message() {
    let (_s, port) = start_server(None, None);
    let builder_1 = ConnectBuilder::new("id1", 0, true).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();
    let mut stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // Mando publish retained de cliente 2
    let publish = Publish::new(false, QoSLevel0, true, "topic", "message", None).unwrap();
    stream_2.write_all(&publish.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Mando subscribe de cliente 1
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback para cliente 1
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Recibo publish en cliente 1
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(recv_publish.encode().unwrap(), publish.encode().unwrap());
}

#[test]
fn test_retained_message_in_last_will() {
    let (_s, port) = start_server(None, None);
    let builder_1 = ConnectBuilder::new("id1", 0, true).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);

    // cliente 2 tiene last will con retained message
    let last_will = LastWill::new(
        TopicFilter::new("topic", QoSLevel0).unwrap(),
        "lw".to_string(),
        true,
    );
    let builder_2 = ConnectBuilder::new("id2", 0, true)
        .unwrap()
        .last_will(last_will);
    let stream_2 = connect_client(builder_2, port, true);
    let mut control = [0u8];

    // cliente 3 por ahora no se suscribe
    let builder_3 = ConnectBuilder::new("id3", 0, true).unwrap();
    let mut stream_3 = connect_client(builder_3, port, true);

    // Mando subscribe de cliente 1
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
    stream_1.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // cliente 2 se desconecta ungracefully
    drop(stream_2);

    // Recibo publish
    stream_1.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(recv_publish.payload(), "lw");
    assert_eq!(recv_publish.topic_name(), "topic");
    assert!(!recv_publish.retain_flag()); // no me deberia llegar al principio como retained

    // Me suscribo con cliente 3
    let subscribe = Subscribe::new(tpc![("topic", QoSLevel0)], 123);
    stream_3.write_all(&subscribe.encode().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(100));

    // Recibo suback en cliente 3
    stream_3.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 9);
    let suback = Suback::read_from(&mut stream_3, control[0]).unwrap();
    assert_eq!(suback.packet_id(), 123);

    // Ahora me tiene que llegar el publish retenido, con la flag en true
    stream_3.read_exact(&mut control).unwrap();
    assert_eq!(control[0] >> 4, 3);
    let recv_publish = Publish::read_from(&mut stream_3, control[0]).unwrap();
    assert_eq!(recv_publish.payload(), "lw");
    assert_eq!(recv_publish.topic_name(), "topic");
    assert!(recv_publish.retain_flag());
}

#[test]
fn test_subscription_dump() {
    let _ = fs::remove_file("tests/files/dumps/dump2.json");
    let (s, port) = start_server(
        Some(("tests/files/dumps/dump2.json", Duration::from_secs(10))),
        None,
    );
    let builder = ConnectBuilder::new("id", 0, false).unwrap();

    let mut stream = connect_client(builder, port, true);

    // Me suscribo
    stream
        .write_all(
            &Subscribe::new(tpc![("topic", QoSLevel0)], 123)
                .encode()
                .unwrap(),
        )
        .unwrap();

    let mut control = [0u8];
    stream.read_exact(&mut control).unwrap();
    Suback::read_from(&mut stream, control[0]).unwrap();

    // Me desconecto gracefully
    stream
        .write_all(&Disconnect::new().encode().unwrap())
        .unwrap();

    // Apago server: debería dumpear
    drop(s);

    let (_s, port) = start_server(
        Some(("tests/files/dumps/dump2.json", Duration::from_secs(10))),
        None,
    );
    let builder = ConnectBuilder::new("id", 0, false).unwrap();
    let mut stream = connect_client(builder, port, true);

    stream
        .write_all(
            &Publish::new(false, QoSLevel0, false, "topic", "msg", None)
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();

    // Debería recibir el publish
    stream.read_exact(&mut control).unwrap();
    let publish = Publish::read_from(&mut stream, control[0]).unwrap();
    assert!(publish.packet_id().is_none());
    assert_eq!(publish.payload(), "msg");
}

#[test]
fn test_subscription_dump_qos1() {
    let _ = fs::remove_file("tests/files/dumps/dump3.json");
    let (s, port) = start_server(
        Some(("tests/files/dumps/dump3.json", Duration::from_secs(10))),
        None,
    );
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let builder_2 = ConnectBuilder::new("id2", 0, true).unwrap();

    let mut stream_1 = connect_client(builder_1, port, true);
    let mut stream_2 = connect_client(builder_2, port, true);

    // Me suscribo con cliente 1 y QoS 1
    stream_1
        .write_all(
            &Subscribe::new(tpc![("topic", QoSLevel1)], 123)
                .encode()
                .unwrap(),
        )
        .unwrap();

    let mut control = [0u8];
    stream_1.read_exact(&mut control).unwrap();
    Suback::read_from(&mut stream_1, control[0]).unwrap();

    // Me desconecto gracefully con cliente 1
    stream_1
        .write_all(&Disconnect::new().encode().unwrap())
        .unwrap();

    // Envio publish QoS 1 con cliente 2
    stream_2
        .write_all(
            &Publish::new(false, QoSLevel1, false, "topic", "msg", Some(123))
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();

    stream_2.read_exact(&mut control).unwrap();
    Puback::read_from(&mut stream_2, control[0]).unwrap();

    // Revisar: por que falla si cliente 2 no se desconecta gracefully
    stream_2
        .write_all(&Disconnect::new().encode().unwrap())
        .unwrap();

    // Apago server: debería dumpear
    drop(s);

    // Vuelvo a cargar el dump y conecto cliente 1
    let (_s, port) = start_server(
        Some(("tests/files/dumps/dump3.json", Duration::from_secs(10))),
        None,
    );
    let builder_1 = ConnectBuilder::new("id1", 0, false).unwrap();
    let mut stream_1 = connect_client(builder_1, port, true);

    // Debería recibir el publish
    stream_1.read_exact(&mut control).unwrap();
    let publish = Publish::read_from(&mut stream_1, control[0]).unwrap();
    assert_eq!(publish.payload(), "msg");
    assert_eq!(publish.topic_name(), "topic");
    assert_eq!(publish.qos(), QoSLevel1);
}
