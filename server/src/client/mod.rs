use core::fmt;
use std::time::{Duration, SystemTime};
use std::{io::Write, vec};

use packets::{connect::Connect, qos::QoSLevel, traits::MQTTEncoding};
use packets::{puback::Puback, publish::Publish};
use rand::{self};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::server::UNACK_RESENDING_FREQ;
use crate::traits::{Close, Interrupt};
use crate::{
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientId, ServerError, ServerResult},
};

#[cfg(test)]
mod tests;

/// Represents the state of a client on the server.
///
/// This structure only handles the state of the client
/// session. It does not handle things like retained messages,
/// since they do not correspond to the client session
/// (see [MQTT-4.1.0-1](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718095))
#[derive(Debug, Serialize, Deserialize)]
pub struct Client<S, I>
where
    S: Write + Interrupt + Send + Sync + 'static,
    I: fmt::Display,
{
    /// Id of the client.
    ///
    /// The server guarantees that two clients
    /// cannot have the same id at the same time.
    id: ClientId,
    /// Connection used to communicate with the client
    /// To allow simultaneous reading and writing with
    /// the client, the server has a copy of this
    /// connection. This end is for writing, while the
    /// copy is for reading.
    ///
    /// If the client is currently disconnected, it is
    /// None.
    #[serde(skip, default = "Default::default")]
    connection: Option<NetworkConnection<S, I>>,
    /// [Connect] packet received in the last client
    /// session.
    ///
    /// Note that a client can have a [`Connect`] packet
    /// and be disconnected. However, when a reconnection
    /// occurs, said packet is replaced by the packet
    /// received on the new connection.
    connect: Connect,
    /// Unacknowledged packets, along with the time they
    /// were last sent.
    unacknowledged: Vec<(SystemTime, Publish)>,
}

impl<S, I> Client<S, I>
where
    S: Write + Interrupt + Send + Sync + 'static,
    I: fmt::Display,
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

    /// Return the id of the client.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return true if the client is connected.
    /// Otherwise, return false.
    pub fn connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Send a packet to the client, through the current
    /// connection.
    ///
    /// This method should not be used to send a [`Publish`]
    /// packet, since it would not be saved in the unacknowledged
    /// list if the Quality of Service is greater than 0. Instead,
    /// the `send_publish()` method should be used.
    ///
    /// Returns error if the client is disconnected.
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
    /// true on its last connection.
    pub fn clean_session(&self) -> bool {
        *self.connect.clean_session()
    }

    /// Disconnects the client, closing the connection
    /// so that it cannot be read or written from any end
    /// (both the one that has this structure and the copy
    /// that the server owns).
    ///
    /// If `gracefully` is false and the client specified a
    /// LastWill on its last connection, the package to be
    /// published is returned. Otherwise, it returns None.
    ///
    /// If the client was already disconnected, it silently does
    /// nothing.
    pub fn disconnect(&mut self, gracefully: bool) -> ServerResult<Option<Publish>>
    where
        S: Close,
    {
        if let Some(mut connection) = self.connection.take() {
            connection.close()?;
        }

        if gracefully {
            self.connect.take_last_will();
            Ok(None)
        } else if let Some(last_will) = self.connect.take_last_will() {
            let packet_identifier: Option<u16>;
            if last_will.topic.qos() != QoSLevel::QoSLevel0 {
                packet_identifier = Some(rand::random());
            } else {
                packet_identifier = None;
            }
            let publish_last_will = Publish::new(
                false,
                last_will.topic.qos(),
                last_will.retain_flag,
                last_will.topic.name(),
                &last_will.topic_message,
                packet_identifier,
            )
            .expect(
                "Se esperaba un formato de Publish valido al crearlo con los datos del LastWill",
            );
            Ok(Some(publish_last_will))
        } else {
            Ok(None)
        }
    }

    /// Check that the id of the new connection
    /// matches the id of the client.
    ///
    /// If they do not match, it returns an error of kind
    /// [`ServerErrorKind::Irrecoverable`]. Therefore, the server
    /// should ensure that said ids are the same.
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

    /// Reconnects a client. Does not send the Connack packet.
    ///
    /// If the reconnection produces a Client TakeOver and LastWill
    /// was specified in the previous session, a [`Publish`] packet is
    /// returned. Otherwise, it returns None.
    pub fn reconnect(
        &mut self,
        new_connect: Connect,
        new_connection: NetworkConnection<S, I>,
    ) -> ServerResult<Option<Publish>>
    where
        S: Close,
    {
        self.check_reconnect_id(&new_connect)?;

        if *new_connect.clean_session() {
            self.unacknowledged = vec![];
        }

        let last_will = self.disconnect(false)?;
        self.connection = Some(new_connection);
        self.connect = new_connect;
        Ok(last_will)
    }

    /// Returns the maximum idle time between communication with
    /// the client before the server decides to disconnect it
    /// (see [MQTT-3.1.2-24])
    pub fn keep_alive(&self) -> Option<Duration> {
        if self.connect.keep_alive() == 0 {
            None
        } else {
            let keep_alive = self.connect.keep_alive();
            Some(Duration::from_millis(1500 * keep_alive as u64))
        }
    }

    /// Returns the username of the client, if specified.
    /// Otherwise, it returns None.
    pub fn user_name(&self) -> Option<&String> {
        self.connect.user_name()
    }

    /// Returns the connection id, if the client is connected.
    /// Otherwise, it returns None.
    pub fn connection_id(&self) -> Option<&I> {
        self.connection.as_ref().map(|connection| connection.id())
    }

    /// Removes from the unacknowledged list, the packet whose
    /// *packet_id* matches the *packet_id* of the received [`Puback`]
    /// packet. If no packet meets this condition, it returns an
    /// error of kind [`ServerErrorKind::Other`].
    #[instrument(skip(self, puback) fields(client_id = %self.id, packet_id = %puback.packet_id()))]
    pub fn acknowledge(&mut self, puback: Puback) -> ServerResult<()> {
        debug!("Acknowledge");
        let idx = self.unacknowledged.iter().position(|publish| {
            puback.packet_id()
                == publish
                    .1
                    .packet_id()
                    .expect("Se esperaba un paquete con identificador (QoS > 0)")
        });
        if let Some(idx) = idx {
            self.unacknowledged.remove(idx);
        }
        if self.unacknowledged.is_empty() {
            match self.keep_alive() {
                None => {
                    if let Some(connection) = &mut self.connection {
                        connection.sleep()?;
                    }
                }
                Some(keep_alive) => {
                    if let Some(connection) = &mut self.connection {
                        connection.alert(keep_alive)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Sends the packets that have not been acknowledged by
    /// the client.
    ///
    /// `min_elapsed_time` is the minimum time that must have elapsed
    /// between the last time the packet was sent and the moment the
    /// method is executed, for the packet to be sent. If it is None,
    /// 1 packet will be sent.
    pub fn send_unacknowledged(&mut self, min_elapsed_time: Option<Duration>) -> ServerResult<()> {
        let now = SystemTime::now();
        if self.unacknowledged.is_empty() {
            return Ok(());
        }

        let (last_time_published, publish) = self.unacknowledged.remove(0);
        if let Some(min_elapsed_time) = min_elapsed_time {
            if now.duration_since(last_time_published).unwrap() > min_elapsed_time {
                self.send_packet(&publish)?;
                self.unacknowledged.insert(0, (now, publish));
            } else {
                // No se envio, no actualizo la hora
                self.unacknowledged
                    .insert(0, (last_time_published, publish));
            }
        } else {
            self.send_packet(&publish)?;
            self.unacknowledged.insert(0, (now, publish));
        }

        Ok(())
    }

    /// Sends a [`Publish`] packet to the client and, if applicable,
    /// adds it to the unacknowledged packet list.
    pub fn send_publish(&mut self, mut publish: Publish) -> ServerResult<()> {
        if self.connected() {
            self.send_packet(&publish)?;
        }
        if publish.qos() == QoSLevel::QoSLevel1 {
            publish.set_dup(true);
            self.unacknowledged.push((SystemTime::now(), publish));
            if self.unacknowledged.len() > 1 {
                if let Some(connection) = &mut self.connection {
                    connection.alert(UNACK_RESENDING_FREQ)?;
                }
            }
        }
        Ok(())
    }
}
