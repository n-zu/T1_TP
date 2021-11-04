#![allow(dead_code)]

use std::{io::{self, Write}, net::TcpStream, sync::{Mutex, atomic::{AtomicI8, Ordering}}, time::SystemTime};

type ClientId = String;

use packets::publish::Publish;
use tracing::{debug, error, info};

use crate::{server::{ServerError, ServerResult}, server_packets::{Connack, Connect}};

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
    unacknowledged: Mutex<Vec<Publish>>,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: Mutex::new(stream),
            status: AtomicI8::new(CONNECTED),
            connect,
            last_activity: SystemTime::now(),
            unacknowledged: Mutex::new(vec![]),
        }
    }

    pub fn id(&self) -> &ClientId {
        &self.id
    }

    pub fn connected(&self) -> bool {
        if self.status.load(Ordering::Relaxed) != CONNECTED {
            return false;
        }
        if SystemTime::now()
            .duration_since(self.last_activity)
            .unwrap()
            .as_secs()
            > *self.connect.keep_alive() as u64
        {
            debug!("TIMEOUT: <{}>", self.id);
            self.status.store(DISCONNECTED_UNGRACEFULLY, Ordering::Relaxed);
            return false;
        }
        true
    }

    pub fn disconnect(&self, gracefully: bool) {
        if gracefully {
            self.status.store(DISCONNECTED_GRACEFULLY, Ordering::Relaxed)
        } else {
            self.status.store(DISCONNECTED_UNGRACEFULLY, Ordering::Relaxed)
        }
    }

    pub fn connect(&self) {
        self.status.store(CONNECTED, Ordering::Relaxed)
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
        if self.connected() {
            error!("Se intento reconectar un usuario que ya esta conectado");
            Err(ServerError::new_msg("Usuario ya conectado"))
        } else {
            info!("Reconectado");
            self.stream = new_client.stream;
            self.status.store(CONNECTED, Ordering::Relaxed);
            Ok(())
        }
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
