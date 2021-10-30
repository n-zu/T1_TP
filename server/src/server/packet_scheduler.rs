#![allow(dead_code)]

use std::{net::{SocketAddr, TcpStream}, sync::Arc, thread::{self, JoinHandle}};

use super::{Packet, Server};

pub struct PacketScheduler {
    server: Arc<Server>,
    client_id: String,
    handles: Vec<JoinHandle<()>>,
}

impl PacketScheduler {
    pub fn new(server: Arc<Server>, client_id: &str) -> Self {
        Self {
            server,
            client_id: client_id.to_owned(),
            handles: vec![]
        }
    }

    pub fn new_packet(&mut self, packet: Packet) {
        let sv_copy = self.server.clone();
        let client_id = self.client_id.clone();
        let handle = thread::spawn(move || {
            sv_copy.handle_packet(packet, client_id);
        });
        self.handles.push(handle);
    }
}

