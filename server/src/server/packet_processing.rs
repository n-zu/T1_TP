use packets::pingresp::PingResp;

use super::*;

impl<C: Config> Server<C> {
    /// Submit a job to the ThreadPool
    fn to_threadpool<F>(self: &Arc<Self>, action: F, id: &ClientIdArg) -> ServerResult<()>
    where
        F: FnOnce(Arc<Self>, &ClientId) -> ServerResult<()> + Send + 'static,
    {
        let sv_copy = self.clone();
        let id_copy = id.to_owned();
        self.pool.lock()?.spawn(move || {
            action(sv_copy, &id_copy).unwrap_or_else(|e| {
                if e.kind() != ServerErrorKind::ClientNotFound {
                    error!("Error de ThreadPool: {}", e);
                }
            });
        })?;
        Ok(())
    }

    /// Reads a packet from the stream and processes it.
    ///
    /// The first byte of the packet must have already been read, and
    /// corresponds to the *control_byte* parameter.
    ///
    /// Returns the type of package that was read
    fn process_packet_given_control_byte<T: Read>(
        self: &Arc<Self>,
        control_byte: u8,
        stream: &mut T,
        id: &ClientIdArg,
    ) -> ServerResult<PacketType>
    {
        let packet_type = PacketType::try_from(control_byte)?;
        match packet_type {
            PacketType::Publish => {
                let publish = Publish::read_from(stream, control_byte)?;
                self.to_threadpool(|server, id| server.handle_publish(publish, id), id)?;
            }
            PacketType::Puback => {
                let packet = Puback::read_from(stream, control_byte)?;
                self.clients_manager
                    .read()?
                    .client_do(id, |mut client| client.acknowledge(packet))?;
            }
            PacketType::Subscribe => {
                let subscribe = Subscribe::read_from(stream, control_byte)?;
                self.to_threadpool(|server, id| server.handle_subscribe(subscribe, id), id)?;
            }
            PacketType::Unsubscribe => {
                let unsubscribe = Unsubscribe::read_from(stream, control_byte)?;
                self.to_threadpool(|server, id| server.handle_unsubscribe(unsubscribe, id), id)?;
            }
            PacketType::PingReq => {
                let _packet = PingReq::read_from(stream, control_byte)?;
                self.clients_manager
                    .read()?
                    .client_do(id, |mut client| client.send_packet(&PingResp::new()))?;
            }
            PacketType::Disconnect => {
                let _packet = Disconnect::read_from(stream, control_byte)?;
            }
            _ => {
                return Err(ServerError::new_kind(
                    "Codigo de paquete inesperado",
                    ServerErrorKind::ProtocolViolation,
                ))
            }
        }
        info!("Procesando {}", packet_type);
        Ok(packet_type)
    }

    /// Reads a packet from the stream and processes it.
    ///
    /// In case the client associated with the stream has disconnected,
    /// it returns an error of kin [`ServerErrorKind::ClientDisconnected`]
    #[instrument(skip(self, stream, id))]
    pub fn process_packet<T: Read>(
        self: &Arc<Self>,
        stream: &mut T,
        id: &ClientIdArg,
    ) -> ServerResult<PacketType> {
        let mut control_byte_buff = [0u8; 1];
        stream.read_exact(&mut control_byte_buff)?;
        self.process_packet_given_control_byte(control_byte_buff[0], stream, id)
    }

    #[inline]
    #[doc(hidden)]
    fn _send_publish(
        self: Arc<Self>,
        client_id_receiver: ClientId,
        publish: Publish,
    ) -> ServerResult<()> {
        self.clients_manager
            .read()?
            .client_do(&client_id_receiver, |mut client| {
                client.send_publish(publish)
            })
    }

    #[instrument(skip(self, threadpool_copy, message), fields(client_id_receiver = %message.client_id))]
    #[inline]
    fn publish_dispatch(
        self: &Arc<Self>,
        threadpool_copy: &ThreadPool,
        message: Message,
    ) -> ServerResult<()> {
        let client_id_receiver = message.client_id;
        let publish = message.packet;
        info!("Enviando PUBLISH");
        let sv_copy = self.clone();
        threadpool_copy
            .spawn(move || {
                sv_copy
                    ._send_publish(client_id_receiver, publish)
                    .unwrap_or_else(|e| {
                        if e.kind() != ServerErrorKind::ClientNotFound
                            && e.kind() != ServerErrorKind::ClientDisconnected
                        {
                            error!("Error enviando PUBLISH: {}", e);
                        }
                    });
            })
            .unwrap_or_else(|e| {
                error!("Eror de ThreadPool: {}", e);
            });

        Ok(())
    }

    /// Receives through the channel the packets to be published, and
    /// publishes them
    fn publish_dispatcher_loop(self: &Arc<Self>, receiver: Receiver<Message>) -> ServerResult<()> {
        let lock = self.pool.lock()?;
        let threadpool_copy = lock.clone();
        drop(lock);

        for message in receiver {
            self.publish_dispatch(&threadpool_copy, message)?;
        }
        Ok(())
    }

    /// Send [`Publish`] to all clients that are subscribed to the topic
    fn broadcast_publish(self: &Arc<Self>, publish: Publish) -> ServerResult<()> {
        let (sender, receiver) = mpsc::channel();
        let sv_copy = self.clone();
        self.pool.lock()?.spawn(move || {
            sv_copy
                .publish_dispatcher_loop(receiver)
                .unwrap_or_else(|e| error!("Error despachando el PUBLISH: {}", e));
        })?;

        self.topic_handler.publish(&publish, sender)?;
        Ok(())
    }

    /// Publish the packet so that all clients subscribed
    /// to the topics can receive them
    pub fn handle_publish(
        self: &Arc<Self>,
        mut publish: Publish,
        id: &ClientIdArg,
    ) -> ServerResult<()> {
        publish.set_max_qos(QoSLevel::QoSLevel1);
        let packet_id = publish.packet_id();
        self.broadcast_publish(publish)?;
        if let Some(packet_id) = packet_id {
            self.clients_manager.read()?.client_do(id, |mut client| {
                client.send_packet(&Puback::new(packet_id)?)
            })?;
        }
        Ok(())
    }

    /// Subscribes the client to all the topics specified in the
    /// [`Subscribe`] packet
    /// Send the corresponding Suback
    fn handle_subscribe(&self, mut subscribe: Subscribe, id: &ClientIdArg) -> ServerResult<()> {
        subscribe.set_max_qos(QoSLevel::QoSLevel1);
        let retained_messages = self.topic_handler.subscribe(&subscribe, id)?;
        self.clients_manager
            .read()?
            .client_do(id, |mut client| client.send_packet(&subscribe.response()?))?;
        if !retained_messages.is_empty() {
            self.clients_manager.read()?.client_do(id, |mut client| {
                for retained in retained_messages {
                    client.send_publish(retained)?;
                }
                Ok(())
            })?;
        }

        Ok(())
    }

    /// Unsubscribe the client from the topics specified in the
    /// [`Unsubscribe`] packet
    /// Send the corresponding [`Unsuback`]
    fn handle_unsubscribe(&self, unsubscribe: Unsubscribe, id: &ClientIdArg) -> ServerResult<()> {
        let packet_id = unsubscribe.packet_id();
        self.topic_handler.unsubscribe(unsubscribe, id)?;
        self.clients_manager.read()?.client_do(id, |mut client| {
            client.send_packet(&Unsuback::new(packet_id)?)?;
            Ok(())
        })?;
        Ok(())
    }

    /// Sends the LastWill packet, previously converted to the
    /// [`Publish`] format
    #[instrument(skip(self, last_will) fields(client_id = %id))]
    pub fn send_last_will(
        self: &Arc<Self>,
        mut last_will: Publish,
        id: &ClientIdArg,
    ) -> ServerResult<()> {
        info!("Enviando LAST WILL");
        last_will.set_max_qos(QoSLevel::QoSLevel1);

        self.broadcast_publish(last_will)
    }

    /// Waits until it receives the [`Connect`] packet. In case the
    /// read fails due to timeout, it returns an error of kind
    /// [`ServerErrorKind::Timeout`]
    #[instrument(skip(self, network_connection))]
    pub fn wait_for_connect(
        &self,
        network_connection: &mut NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<Connect> {
        match Connect::new_from_zero(network_connection) {
            Ok(connect) => {
                info!("Recibido CONNECT");
                Ok(connect)
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }
}
