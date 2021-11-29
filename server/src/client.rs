use std::{
    io::{self, Write},
    vec,
};

use packets::{connect::Connect, qos::QoSLevel, traits::MQTTEncoding};
use packets::{puback::Puback, publish::Publish};
use tracing::{debug, info};

use crate::{
    server::{server_error::ServerErrorKind, ClientId, ServerError, ServerResult},
    traits::BidirectionalStream,
};

/// Represents the state of a client on the server
pub struct Client<S> {
    /// Id of the client
    id: ClientId,
    stream: Option<S>,
    connect: Connect,
    /// Unacknowledge packets
    unacknowledged: Vec<Publish>,
}

impl<S: io::Write> io::Write for Client<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(stream) = &mut self.stream {
            stream.write(buf)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("Cliente <{}> desconectado", self.id),
            ))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.flush()
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("Cliente <{}> desconectado", self.id),
            ))
        }
    }
}

impl<S> Client<S>
where
    S: BidirectionalStream,
{
    pub fn new(connect: Connect) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            stream: None,
            connect,
            unacknowledged: vec![],
        }
    }

    pub fn connect(&mut self, stream: S) -> ServerResult<()> {
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

    pub fn connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn send_packet<T: MQTTEncoding>(&mut self, packet: &T) -> ServerResult<()> {
        self.write_all(&packet.encode()?)?;
        Ok(())
    }

    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    pub fn disconnect(&mut self, gracefully: bool) -> ServerResult<Option<Publish>> {
        if let Some(mut stream) = self.stream.take() {
            stream.close()?;
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

    pub fn reconnect(
        &mut self,
        new_connect: Connect,
        new_stream: S,
    ) -> ServerResult<Option<Publish>> {
        if self.id != new_connect.client_id() {
            return Err(ServerError::new_kind(
                &format!(
                    "<{}>: Intento de reconexion con un cliente con id diferente ({})",
                    self.id,
                    new_connect.client_id()
                ),
                ServerErrorKind::Irrecoverable,
            ));
        }
        info!("<{}>: Reconectando", self.id);
        if *self.connect.clean_session() {
            self.unacknowledged = vec![];
        }

        let last_will = self.disconnect(false)?;
        self.stream = Some(new_stream);
        self.connect = new_connect;
        Ok(last_will)
    }

    pub fn keep_alive(&self) -> u16 {
        self.connect.keep_alive()
    }

    pub fn user_name(&self) -> Option<&String> {
        self.connect.user_name()
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

    pub fn send_unacknowledged(&mut self) -> ServerResult<()>
    where
        S: Write,
    {
        for publish in self.unacknowledged.iter() {
            debug!(
                "<{}>: Reenviando paquete con id <{}>",
                self.id,
                publish
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
            );
            self.stream
                .as_mut()
                .unwrap()
                .write_all(&publish.encode()?)?;
        }
        Ok(())
    }

    fn add_unacknowledged(&mut self, mut publish: Publish) {
        publish.set_dup(true);
        self.unacknowledged.push(publish);
    }

    pub fn send_publish(&mut self, publish: Publish) -> ServerResult<()>
    where
        S: Write,
    {
        if self.connected() {
            self.send_packet(&publish)?;
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
        Ok(())
    }
}
