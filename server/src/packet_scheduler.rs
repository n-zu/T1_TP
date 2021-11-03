#![allow(dead_code)]

use std::{
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};

use crate::server::{Packet, Server};

type ClientId = String;

pub struct PacketScheduler {
    server: Arc<Server>,
    id: ClientId,
    handles: Vec<JoinHandle<()>>,
    sender: mpsc::Sender<Message>,
    handler: Option<JoinHandle<()>>,
}

enum Message {
    Job(Packet),
    Finish,
}

fn listener_loop(receiver: Receiver<Message>, server: Arc<Server>, id: ClientId) {
    loop {
        match receiver.recv() {
            Ok(message) => match message {
                Message::Job(packet) => server.handle_packet(packet, &id).unwrap(),
                Message::Finish => break,
            },
            Err(err) => panic!(
                "Error recibiendo un mensaje del listener_loop: {}",
                err.to_string()
            ),
        }
    }
}

impl PacketScheduler {
    pub fn new(server: Arc<Server>, id: &ClientId) -> Self {
        let (sender, receiver) = mpsc::channel();

        let sv_copy = server.clone();
        let id_copy = id.clone();
        let handler = thread::spawn(move || {
            listener_loop(receiver, sv_copy, id_copy);
        });

        Self {
            server,
            id: id.to_owned(),
            handles: vec![],
            sender,
            handler: Some(handler),
        }
    }

    pub fn new_packet(&mut self, packet: Packet) {
        self.sender.send(Message::Job(packet)).unwrap();
    }
}

impl Drop for PacketScheduler {
    fn drop(&mut self) {
        self.sender.send(Message::Finish).unwrap();
        self.handler.take().unwrap().join().unwrap();
    }
}
