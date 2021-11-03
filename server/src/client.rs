#![allow(dead_code)]

use std::{
    io::{self, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::SystemTime,
};

type ClientId = String;

use packets::publish::Publish;
use tracing::{debug, info};

use crate::{
    server::ServerResult,
    server_packets::{Connack, Connect},
};

/// Represents the state of a client on the server
pub struct Client {
    /// Id of the client
    id: ClientId,
    /// TCP connection to send packets to the client
    stream: Mutex<TcpStream>,
    /// Indicates if the client is currently connected
    connected: AtomicBool,
    connect: Connect,
    /// Time in which the last package was received
    last_activity: SystemTime,
    /// Unacknowledge packets
    unacknowledged: Mutex<Vec<Publish>>,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: Mutex::new(stream),
            connected: AtomicBool::new(true),
            connect,
            last_activity: SystemTime::now(),
            unacknowledged: Mutex::new(vec![]),
        }
    }

    pub fn id(&self) -> &ClientId {
        &self.id
    }

    pub fn connected(&self) -> bool {
        if !self.connected.load(Ordering::Relaxed) {
            return false;
        }
        if SystemTime::now()
            .duration_since(self.last_activity)
            .unwrap()
            .as_secs()
            > *self.connect.keep_alive() as u64
        {
            debug!("TIMEOUT: <{}>", self.id);
            self.connected.store(false, Ordering::Relaxed)
        }

        self.connected.load(Ordering::Relaxed)
    }

    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Relaxed)
    }

    pub fn connect(&self) {
        self.connected.store(true, Ordering::Relaxed);
    }

    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.stream.lock().unwrap().write_all(buf)
    }

    pub fn send_connack(&mut self, connack: Connack) -> io::Result<()> {
        self.stream.lock().unwrap().write_all(&connack.encode())?;
        Ok(())
    }

    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    pub fn reconnect(&mut self, new_client: Client) -> ServerResult<()> {
        // TODO: chequeo de usuario y contraseña
        info!("Reconectado");
        self.stream = new_client.stream;
        Ok(())
    }

    pub fn refresh(&mut self) {
        self.last_activity = SystemTime::now();
    }

    pub fn send_unacknowledged(&mut self) {
        // Recorro los unacklowledged y los envio al cliente
        todo!()
    }

    pub fn send_publish(&mut self, _publish: &Publish) {
        // Lo que se haga aca depende del QoS del paquete y
        // el estado de conexion del cliente
        todo!()
    }
}
