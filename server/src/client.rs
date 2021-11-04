#![allow(dead_code)]

use std::{
    collections::{HashMap},
    io::{self, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicI8, Ordering},
        Mutex,
    },
    time::SystemTime,
};

type ClientId = String;
type PacketId = u16;

use packets::publish::{Publish, QoSLevel};
use tracing::{debug, error, info};

use crate::{
    server::{ServerError, ServerResult},
    server_packets::{Connack, Connect},
};

const CONNECTED: i8 = 0;
const DISCONNECTED_GRACEFULLY: i8 = 1;
const DISCONNECTED_UNGRACEFULLY: i8 = 2;

/// Represents the state of a client on the server
pub struct Client {
    /// Id of the client
    id: ClientId,
    /// TCP connection to send packets to the client
    stream: Mutex<TcpStream>,
    /// Indicates if the client is currently connected
    status: AtomicI8,
    connect: Connect,
    /// Time in which the last package was received
    last_activity: SystemTime,
    /// Unacknowledge packets
    unacknowledged: Mutex<HashMap<PacketId, Publish>>,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: Mutex::new(stream),
            status: AtomicI8::new(CONNECTED),
            connect,
            last_activity: SystemTime::now(),
            unacknowledged: Mutex::new(HashMap::new()),
        }
    }

    pub fn id(&self) -> &ClientId {
        &self.id
    }

    fn check_keep_alive(&self) -> bool {
        SystemTime::now()
            .duration_since(self.last_activity)
            .expect("Error inesperado de keep_alive")
            .as_secs()
            > *self.connect.keep_alive() as u64
    }

    fn connected_internal(&self) -> bool {
        self.status.load(Ordering::Relaxed) == CONNECTED
    }

    pub fn connected(&self) -> bool {
        self.status.load(Ordering::Relaxed) == CONNECTED
    }

    fn send_last_will(&self) {
        todo!()
    }

    pub fn disconnect(&self, gracefully: bool) {
        if gracefully {
            self.status
                .store(DISCONNECTED_GRACEFULLY, Ordering::Relaxed);
            self.send_last_will();
        } else {
            self.status
                .store(DISCONNECTED_UNGRACEFULLY, Ordering::Relaxed)
        }
    }

    // TODO: probablemente no sea buena idea que sea publico
    pub fn write_all(&self, buf: &[u8]) -> io::Result<()> {
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
        if self.connected() {
            error!("Se intento reconectar un usuario que ya esta conectado");
            Err(ServerError::new_msg("Usuario ya conectado"))
        } else {
            info!("Reconectado <{}>", self.id);
            self.stream = new_client.stream;
            self.status.store(CONNECTED, Ordering::Relaxed);
            Ok(())
        }
    }

    pub fn refresh(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /*
    pub fn acknowledge(&self, puback: Puback) {
        // Elimina el paquete del hashmap
        todo!()
    }
    */

    // Esta en publico porque el server quiza quiera reenviar paquetes
    // antes de una reconexion
    pub fn send_unacknowledged(&self) {
        for (id, publish) in self.unacknowledged.lock().unwrap().iter() {
            debug!("Reenviando paquete con id <{}> a cliente <{}>", id, self.id);
            self.write_all(&publish.encode().unwrap()).unwrap();
        }
    }

    fn add_unacknowledged(&self, publish: &Publish) {
        self.unacknowledged
        .lock()
        .unwrap()
        .insert(*publish.packet_id().unwrap(), publish.clone());
    }

    pub fn send_publish(&self, publish: &Publish) {
        if self.connected_internal() {
            self.write_all(&mut publish.encode().unwrap()).unwrap();
            if publish.qos == QoSLevel::QoSLevel1 {
                // TODO: que pasa si el paquete ya existe en el HashMap?
                self.add_unacknowledged(publish);
            }
        } else {
            if publish.qos == QoSLevel::QoSLevel1 {
                self.add_unacknowledged(publish);
            }
        }

        match publish.qos() {
            QoSLevel::QoSLevel0 => self.write_all(&mut publish.encode().unwrap()).unwrap(),
            QoSLevel::QoSLevel1 => {}
        }
    }
}
