#![allow(dead_code)]

const KEEP_ALIVE_DEFAULT: u16 = 15000;

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
};

use packets::{puback::Puback, publish::Publish};

use super::client_packets as cl_packets;
use crate::server_packets as sv_packets;

pub struct ClientMock {
    stream: TcpStream,
    id: String,
}

pub struct ClientSetMock {
    clients: HashMap<String, ClientMock>,
}

impl ClientMock {
    pub fn new_connect_tcp(id: String) -> ClientMock {
        let stream = TcpStream::connect("localhost").expect("Fallo al crear cliente");
        ClientMock { stream, id }
    }

    pub fn send_connect(&mut self, connect: cl_packets::Connect) {
        self.stream.write_all(&connect.encode()).unwrap()
    }

    pub fn receive_connack(&mut self) -> cl_packets::Connack {
        let mut buf = [0u8; 1];
        self.stream.read_exact(&mut buf).unwrap();
        cl_packets::Connack::read_from(&mut self.stream).unwrap()
    }

    pub fn send_publish(&mut self, publish: Publish) {
        self.stream.write_all(&publish.encode().unwrap()).unwrap()
    }

    pub fn receive_publish(&mut self) -> Publish {
        let mut buf = [0u8; 1];
        self.stream.read_exact(&mut buf).unwrap();
        Publish::read_from(&mut self.stream, buf[0]).unwrap()
    }

    pub fn send_puback(&mut self, puback: Puback) {
        self.stream.write_all(&puback.encode()).unwrap();
    }

    pub fn receive_puback(&mut self) -> Puback {
        let mut buf = [0u8; 1];
        self.stream.read_exact(&mut buf).unwrap();
        Puback::read_from(&mut self.stream, buf[0]).unwrap()
    }

    pub fn disconnect(&mut self) {
        self.stream
            .write_all(&cl_packets::Disconnect::new().encode())
            .unwrap()
    }

    pub fn send_pingreq(&mut self) {
        self.stream
            .write_all(&cl_packets::PingReq::new().encode())
            .unwrap()
    }

    /*
    pub fn receive_pingresp(&mut self) {
        let mut buf = [0u8; 1];
        self.stream.read_exact(&mut buf).unwrap();
        cl_packets::PingResp::read_from(&mut self.stream, buf[0]).unwrap()
    }
    */
}

fn add_suffix(id: &str, num: u32) -> String {
    let mut id = id.to_owned();
    id.push('_');
    id.push(char::from_u32(num).unwrap());
    id
}

impl ClientSetMock {
    pub fn new() -> ClientSetMock {
        ClientSetMock {
            clients: HashMap::new(),
        }
    }

    pub fn add_client(&mut self, client: ClientMock) {
        self.clients.insert(client.id.clone(), client).unwrap();
    }

    pub fn add_full_connected(&mut self, id: String) {
        let mut client = ClientMock::new_connect_tcp(id.clone());
        client.send_connect(
            cl_packets::ConnectBuilder::new(&id, KEEP_ALIVE_DEFAULT, true)
                .unwrap()
                .build()
                .unwrap(),
        );
        client.receive_connack();
        self.add_client(client);
    }

    pub fn add_bulk(&mut self, size: u32, id_prefix: String) {
        for i in 0..size {
            let id = add_suffix(&id_prefix, i);
            self.add_full_connected(id);
        }
    }
}
