use std::{io::Write, vec};

use packets::{connect::Connect, qos::QoSLevel, traits::MQTTEncoding};
use packets::{puback::Puback, publish::Publish};

use crate::{
    logging::{self, LogKind},
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientId, ServerError, ServerResult},
    traits::{BidirectionalStream, Id},
};

/// Represents the state of a client on the server
///
/// This structure only handles the state of the client
/// session. It does not handle things like retained messages,
/// since they do not correspond to the client session
/// (see [MQTT-4.1.0-1])
pub struct Client<S, I>
where
    S: BidirectionalStream,
    I: Id,
{
    /// Id of the client.
    ///
    /// The server guarantees that two clients
    /// cannot have the same id at the same time
    id: ClientId,
    /// Connection used to communicate with the client
    /// To allow simultaneous reading and writing with
    /// the client, the server has a copy of this
    /// connection. This end is for writing, while the
    /// copy is for reading.
    ///
    /// If the client is currently disconnected, it is
    /// None
    connection: Option<NetworkConnection<S, I>>,
    /// [Connect] packet received in the last client
    /// session.
    ///
    /// Note that a client can have a [Connect] packet
    /// and be disconnected. However, when a reconnection
    /// occurs, said packet is replaced by the packet
    /// received on the new connection
    connect: Connect,
    /// Unacknowledged packets
    unacknowledged: Vec<Publish>,
}

impl<S, I> Client<S, I>
where
    S: BidirectionalStream,
    I: Id,
{
    /// Create a new connected client
    pub fn new(connect: Connect, network_connection: NetworkConnection<S, I>) -> Self {
        Self {
            id: connect.client_id().to_owned(),
            connect,
            unacknowledged: vec![],
            connection: Some(network_connection),
        }
    }

    /// Return the id of the client
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return true if the client is connected.
    /// Othrewise, return false
    pub fn connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Send a packet to the client, through the current
    /// connection.
    ///
    /// This method should not be used to send a [Publish]
    /// packet, since it would not be saved in the unacknowledged
    /// list if the Quality of Service is greater than 0. Instead,
    /// the *send_publish()* method should be used.
    ///
    /// Returns error if the client is disconnected
    pub fn send_packet<T: MQTTEncoding>(&mut self, packet: &T) -> ServerResult<()> {
        if let Some(connection) = &mut self.connection {
            connection.write_all(&packet.encode()?)?;
            Ok(())
        } else {
            Err(ServerError::new_kind(
                &format!(
                    "Intento de envio de paquete a cliente <{}> desconectado",
                    self.id
                ),
                ServerErrorKind::ClientDisconnected,
            ))
        }
    }

    /// Returns true if the client specified clean_session to
    /// true on its last connection
    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    /// Disconnects the client, closing the connection
    /// so that it cannot be read or written from any end
    /// (both the one that has this structure and the copy
    /// that the server owns).
    ///
    /// If *gracefully* is false and the client specified a
    /// LastWill on its last connection, the package to be
    /// published is returned. Otherwise, it returns None.
    ///
    /// If the client was already disconnected, it silently does
    /// nothing
    pub fn disconnect(&mut self, gracefully: bool) -> ServerResult<Option<Publish>> {
        if let Some(mut connection) = self.connection.take() {
            connection.close()?;
            if gracefully {
                self.connect.last_will().take();
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

    /// Check that the id of the new connection
    /// matches the id of the client.
    ///
    /// If they do not match, it returns an error of kind
    /// [ServerErrorKind::Irrecoverable]. Therefore, the server
    /// should ensure that said ids are the same
    fn check_reconnect_id(&self, new_connect: &Connect) -> ServerResult<()> {
        if self.id != new_connect.client_id() {
            return Err(ServerError::new_kind(
                &format!(
                    "<{}>: Intento de reconexion con un cliente con id diferente ({})",
                    self.id,
                    new_connect.client_id()
                ),
                ServerErrorKind::Irrecoverable,
            ));
        } else {
            Ok(())
        }
    }

    /// Reconnects a client. Does not send the [Connack] packet.
    ///
    /// If the reconnection produces a Client TakeOver and LastWill
    /// was specified in the previous session, a [Publish] packet is
    /// returned. Otherwise, it returns None
    pub fn reconnect(
        &mut self,
        new_connect: Connect,
        new_stream: NetworkConnection<S, I>,
    ) -> ServerResult<Option<Publish>> {
        self.check_reconnect_id(&new_connect)?;

        logging::log(LogKind::Reconnecting(&self.id));
        if *self.connect.clean_session() {
            self.unacknowledged = vec![];
        }

        let last_will = self.disconnect(false)?;
        self.connection = Some(new_stream);
        self.connect = new_connect;
        Ok(last_will)
    }

    /// Returns the maximum idle time between communication with
    /// the client before the server decides to disconnect it
    /// (see [MQTT-3.1.2-24])
    pub fn keep_alive(&self) -> u32 {
        let keep_alive = self.connect.keep_alive();
        (keep_alive as f32 * 1.5) as u32
    }

    /// Returns the username of the client, if specified.
    /// Otherwise, it returns None
    pub fn user_name(&self) -> Option<&String> {
        self.connect.user_name()
    }

    /// Returns the connection id, if the client is connected.
    /// Otherwise, it returns None
    pub fn connection_id(&self) -> Option<I>
    where
        I: Clone + Copy,
    {
        self.connection.as_ref().map(|connection| connection.id())
    }

    pub fn acknowledge(&mut self, puback: Puback) -> ServerResult<()> {
        logging::log(LogKind::Acknowledge(&self.id, puback.packet_id()));
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
            logging::log(LogKind::SendingUnacknowledged(
                &self.id,
                publish.packet_id().unwrap(),
            ));
            self.connection
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
                self.add_unacknowledged(publish);
            }
        } else if publish.qos() == QoSLevel::QoSLevel1 {
            self.add_unacknowledged(publish);
        }
        Ok(())
    }
}
