use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

use client::{Message, Observer};
use packets::{
    connack::Connack,
    connect::Connect,
    traits::{MQTTDecoding, MQTTEncoding},
};
use rand::Rng;

pub struct ServerMock {
    listener: TcpListener,
    pub port: u32,
}

impl ServerMock {
    pub fn new() -> Self {
        let mut port = random_port();
        let mut result = TcpListener::bind(format!("localhost:{}", port));
        for _ in 0..50 {
            // Intento crear el servidor bindeando a 50 puertos al azar
            if let Ok(listener) = result {
                listener.set_nonblocking(true).unwrap();
                return Self { listener, port };
            } else {
                port = random_port();
                result = TcpListener::bind(format!("localhost:{}", port));
            }
        }

        panic!("No se pudo encontrar puerto disponible");
    }

    pub fn accept_connection(&mut self, connack: Connack) -> (TcpStream, Connect) {
        let (mut stream, _) = self.listener.accept().unwrap();
        let mut buffer = [0; 1];
        stream.read_exact(&mut buffer).unwrap();
        assert_eq!(buffer[0] >> 4, 1);
        stream.write_all(&connack.encode().unwrap()).unwrap();
        let connect = Connect::read_from(&mut stream, buffer[0]).unwrap();
        (stream, connect)
    }
}

#[derive(Clone)]
pub struct ObserverMock {
    pub messages: Arc<Mutex<Vec<Message>>>,
}

impl Observer for ObserverMock {
    fn update(&self, message: Message) {
        self.messages.lock().unwrap().push(message);
    }
}

impl ObserverMock {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn random_port() -> u32 {
    // Esos números salen de esta información
    // https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers#Dynamic,_private_or_ephemeral_ports
    rand::thread_rng().gen_range(49152..65536)
}
