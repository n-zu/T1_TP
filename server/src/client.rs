use std::{
    io::{self, Write},
    net::TcpStream,
    vec,
};

use packets::{packet_reader::QoSLevel, puback::Puback, publish::Publish, suback::Suback};
use tracing::{debug, error, info};

use crate::{
    server::{ClientId, ServerError, ServerResult},
    server_packets::{unsuback::Unsuback, Connack, Connect, PingResp},
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
    id: ClientId,
    /// TCP connection to send packets to the client
    stream: TcpStream,
    /// Indicates if the client is currently connected
    status: ClientStatus,
    connect: Connect,
    /// Unacknowledge packets
    unacknowledged: Vec<Publish>,
}

impl Client {
    pub fn new(connect: Connect, stream: TcpStream) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream,
            status: ClientStatus::Connected,
            connect,
            unacknowledged: vec![],
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn connected(&self) -> bool {
        self.status == ClientStatus::Connected
    }

    fn send_last_will(&self) {
        debug!("<{}>: Enviando last will (TODO)", self.id);
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

    pub fn send_pingresp(&mut self) -> ServerResult<()> {
        debug!("<{}> Enviando PINGRESP", self.id);
        self.write_all(&PingResp::new().encode())?;
        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
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

    pub fn send_puback(&mut self, puback: Puback) -> ServerResult<()> {
        self.write_all(&puback.encode())?;
        Ok(())
    }

    pub fn send_unsuback(&mut self, unsuback: Unsuback) -> ServerResult<()> {
        self.write_all(&unsuback.encode())?;
        Ok(())
    }

    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    pub fn reconnect(&mut self, new_client: Client) -> ServerResult<()> {
        if self.connected() {
            error!("Se intento reconectar un usuario que ya esta conectado");
            Err(ServerError::new_msg("Usuario ya conectado"))
        } else {
            info!("Reconectado <{}>", self.id);
            self.stream = new_client.stream;
            self.status = ClientStatus::Connected;
            self.connect = new_client.connect;
            Ok(())
        }
    }

    pub fn keep_alive(&self) -> u16 {
        *self.connect.keep_alive()
    }

    pub fn user_name(&self) -> Option<&String> {
        self.connect.user_name()
    }

    pub fn password(&self) -> Option<&String> {
        self.connect.password()
    }

    pub fn acknowledge(&mut self, puback: Puback) -> ServerResult<()> {
        debug!("<{}>: Acknowledge {}", self.id, puback.packet_id());
        self.unacknowledged.retain(|publish| {
            puback.packet_id()
                != *publish
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
        });
        Ok(())
    }

    pub fn send_suback(&mut self, suback: Suback) -> ServerResult<()> {
        self.write_all(&suback.encode()?)?;
        Ok(())
    }

    pub fn send_suback(&mut self, suback: &mut Suback) -> ServerResult<()> {
        self.write_all(&suback.encode()?)?;
        Ok(())
    }

    pub fn send_unacknowledged(&mut self) {
        for publish in self.unacknowledged.iter() {
            debug!(
                "<{}>: Reenviando paquete con id <{}>",
                self.id,
                publish
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
            );
            self.stream.write_all(&publish.encode().unwrap()).unwrap();
        }
    }

    fn add_unacknowledged(&mut self, publish: Publish) {
        self.unacknowledged.push(publish);
    }

    pub fn send_publish(&mut self, publish: Publish) {
        if self.connected() {
            self.write_all(&publish.encode().unwrap()).unwrap();
            if publish.qos() == QoSLevel::QoSLevel1 {
                // TODO: que pasa si el paquete ya existe en el HashMap?
                debug!("{}: Agregando PUBLISH a UNACKNOWLEDGED", self.id);
                self.add_unacknowledged(publish);
            } else {
                debug!("<{}>: Conectado y con QoS == 0", self.id);
            }
        } else if publish.qos() == QoSLevel::QoSLevel1 {
            debug!(
                "<{}> Agregando Publish a UNACKNOWLEDGED desconectado",
                self.id
            );
            self.add_unacknowledged(publish);
        }
    }
}
