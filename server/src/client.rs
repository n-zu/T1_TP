use std::{io::Write, net::{Shutdown, TcpStream}, vec};

use packets::{connect::Connect, pingresp::PingResp, qos::QoSLevel, traits::MQTTEncoding};
use packets::{puback::Puback, publish::Publish, suback::Suback};
use tracing::{debug, info};

use crate::server::{
    server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult,
};

/// Represents the state of a client on the server
pub struct Client {
    /// Id of the client
    id: ClientId,
    /// TCP connection to send packets to the client
    stream: Option<TcpStream>,
    connect: Connect,
    /// Unacknowledge packets
    unacknowledged: Vec<Publish>,
}

impl Client {
    pub fn new(connect: Connect) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: None,
            connect,
            unacknowledged: vec![],
        }
    }

    pub fn connect(&mut self, stream: TcpStream) -> ServerResult<()> {
        if self.stream.is_none() {
            self.stream = Some(stream);
            Ok(())
        } else {
            Err(ServerError::new_msg("Cliente ya conectado"))
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn set_id(&mut self, new_id: &ClientIdArg) -> ServerResult<()> {
        if self.id().is_empty() {
            self.id = new_id.to_owned();
            Ok(())
        } else {
            Err(ServerError::new_msg(
                "Solo se puede setear la id de un cliente con id vacia",
            ))
        }
    }

    pub fn connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn disconnect(&mut self, gracefully: bool) -> ServerResult<Option<Publish>> {
        if let Some(stream) = self.stream.take() {
            stream.shutdown(Shutdown::Both)?;
            if gracefully {
                Ok(None)
            } else if let Some(last_will) = self.connect.last_will().take() {
                let packet_identifier: Option<u16>;
                if last_will.qos != QoSLevel::QoSLevel0 {
                    packet_identifier = Some(rand::random());
                } else {
                    packet_identifier = None;
                }
                let publish_last_will = Publish::new(
                        false,
                        last_will.qos,
                        last_will.retain_flag,
                        &last_will.topic_name,
                        &last_will.topic_message,
                        packet_identifier
                    ).expect("Se esperaba un formato de Publish valido al crearlo con los datos del LastWill");
                Ok(Some(publish_last_will))
            } else {
                Ok(None)
            }
        }
        // El cliente ya estaba desconectado
        else {
            Ok(None)
        }
    }

    pub fn send_pingresp(&mut self) -> ServerResult<()> {
        debug!("<{}> Enviando PINGRESP", self.id);
        Client::write_all(self.stream.as_mut(), &PingResp::new().encode()?)?;
        Ok(())
    }

    fn write_all(mut stream: Option<&mut TcpStream>, buf: &[u8]) -> ServerResult<()> {
        if let Some(stream) = stream.take() {
            stream.write_all(buf)?;
            Ok(())
        } else {
            Err(ServerError::new_kind(
                "Se intento escribir en un stream de un cliente desconectado",
                ServerErrorKind::ProtocolViolation,
            ))
        }
    }

    pub fn send_packet(&mut self, packet: impl MQTTEncoding) -> ServerResult<()> {
        Client::write_all(self.stream.as_mut(), &packet.encode()?)?;
        Ok(())
    }

    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    pub fn reconnect(&mut self, new_client: Client) -> ServerResult<Option<Publish>> {
        if self.id != new_client.id {
            // No deberia pasar
            return Err(ServerError::new_kind(
                &format!(
                    "<{}>: Intento de reconexion con un cliente con id diferente ({})",
                    self.id, new_client.id
                ),
                ServerErrorKind::Irrecoverable,
            ));
        }
        info!("<{}>: Reconectando", self.id);
        if *self.connect.clean_session() {
            self.unacknowledged = vec![];
        }

        let last_will = self.disconnect(false)?;
        self.stream = new_client.stream;
        self.connect = new_client.connect;
        Ok(last_will)
    }

    pub fn keep_alive(&self) -> u16 {
        self.connect.keep_alive()
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
                != publish
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
        });
        Ok(())
    }

    pub fn send_suback(&mut self, suback: Suback) -> ServerResult<()> {
        Client::write_all(self.stream.as_mut(), &suback.encode()?)?;
        Ok(())
    }

    pub fn send_unacknowledged(&mut self) -> ServerResult<()> {
        for publish in self.unacknowledged.iter() {
            debug!(
                "<{}>: Reenviando paquete con id <{}>",
                self.id,
                publish
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
            );
            Client::write_all(self.stream.as_mut(), &publish.encode()?)?;
        }
        Ok(())
    }

    fn add_unacknowledged(&mut self, mut publish: Publish) {
        publish.set_dup(true);
        self.unacknowledged.push(publish);
    }

    pub fn send_publish(&mut self, publish: Publish) {
        if self.connected() {
            Client::write_all(self.stream.as_mut(), &publish.encode().unwrap()).unwrap();
            if publish.qos() == QoSLevel::QoSLevel1 {
                // TODO: que pasa si el paquete ya existe en el HashMap?
                debug!("<{}>: Agregando PUBLISH a UNACKNOWLEDGED", self.id);
                self.add_unacknowledged(publish);
            } else {
                debug!("<{}>: Conectado y con QoS == 0", self.id);
            }
        } else if publish.qos() == QoSLevel::QoSLevel1 {
            debug!(
                "<{}>: Agregando Publish a UNACKNOWLEDGED desconectado",
                self.id
            );
            self.add_unacknowledged(publish);
        }
    }
}
