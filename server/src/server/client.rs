use std::{
    io::{self, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
};

use crate::connect::Connect;

pub struct Client {
    id: String,
    stream: Mutex<TcpStream>,
    alive: AtomicBool,
    connect: Connect,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: Mutex::new(stream),
            alive: AtomicBool::new(true),
            connect,
        }
    }

    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    pub fn disconnect(&self) {
        self.alive.store(false, Ordering::Relaxed)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn connect(&self) -> &Connect {
        &self.connect
    }

    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.stream.lock().unwrap().write_all(buf)
    }
}
