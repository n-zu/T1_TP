#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::{self, Write},
    net::TcpStream,
};

type PacketId = u16;

use packets::{packet_reader::QoSLevel, puback::Puback, publish::Publish};
use tracing::{debug, error, info};

use crate::{
    server::{ServerError, ServerResult},
    server_packets::{Connack, Connect},
};

#[derive(PartialEq)]
pub enum ClientStatus {
    Connected,
    DisconnectedGracefully,
    DisconnectedUngracefully,
}

/// Represents the state of a client on the server
pub struct Client {
    /// Id of the client
    id: String,
    /// TCP connection to send packets to the client
    stream: TcpStream,
    /// Indicates if the client is currently connected
    status: ClientStatus,
    connect: Connect,
    /// Unacknowledge packets
    unacknowledged: HashMap<PacketId, Publish>,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream,
            status: ClientStatus::Connected,
            connect,
            unacknowledged: HashMap::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn connected(&self) -> bool {
        self.status == ClientStatus::Connected
    }

    fn send_last_will(&self) {
        todo!()
    }

    pub fn disconnect(&mut self, gracefully: bool) {
        if gracefully {
            self.status = ClientStatus::DisconnectedGracefully;
        } else {
            // En que casos se envia el last_will? gracefully o ungracefully?
            self.status = ClientStatus::DisconnectedUngracefully;
            self.send_last_will();
        }
    }

    // TODO: probablemente no sea buena idea que sea publico
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.connected() {
            self.stream.write_all(buf)
        } else {
            todo!()
        }
    }

    pub fn send_connack(&mut self, connack: Connack) -> io::Result<()> {
        self.stream.write_all(&connack.encode())?;
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
            self.status = ClientStatus::Connected;
            self.send_unacknowledged();
            Ok(())
        }
    }

    pub fn keep_alive(&self) -> u16 {
        *self.connect.keep_alive()
    }

    pub fn acknowledge(&mut self, puback: Puback) {
        debug!("<{}>: Acknowledge {}", self.id, puback.packet_id());
        self.unacknowledged.remove(&puback.packet_id()).unwrap();
    }

    pub fn send_unacknowledged(&mut self) {
        for (id, publish) in self.unacknowledged.iter() {
            debug!(
                "Reenviando paquete con id <{}> a cliente <{}>",
                id,
                publish.packet_id().unwrap()
            );
            self.stream.write_all(&publish.encode().unwrap()).unwrap();
        }
    }

    fn add_unacknowledged(&mut self, publish: Publish) {
        self.unacknowledged
            .insert(*publish.packet_id().unwrap(), publish);
    }

    pub fn send_publish(&mut self, publish: Publish) {
        if self.connected() {
            debug!("Publish bytes: {:?}", publish.encode().unwrap());
            debug!("Packet identifier: {}", publish.packet_id.unwrap());
            self.write_all(&publish.encode().unwrap()).unwrap();
            if publish.qos() == QoSLevel::QoSLevel1 {
                // TODO: que pasa si el paquete ya existe en el HashMap?
                debug!(
                    "{}: Agregando PUBLISH a lista de paquetes no confirmados",
                    self.id
                );
                self.add_unacknowledged(publish);
            }
        } else if publish.qos() == QoSLevel::QoSLevel1 {
            self.add_unacknowledged(publish);
        }
    }
}
