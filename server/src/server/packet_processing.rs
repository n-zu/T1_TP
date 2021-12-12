use packets::pingresp::PingResp;

use super::*;

impl Server {
    /// Submit a job to the ThreadPool
    fn to_threadpool<F>(self: &Arc<Self>, action: F, id: &ClientIdArg) -> ServerResult<()>
    where
        F: FnOnce(Arc<Self>, &ClientId) -> ServerResult<()> + Send + 'static,
    {
        let sv_copy = self.clone();
        let id_copy = id.to_owned();
        self.pool
            .lock()?
            .spawn(move || match action(sv_copy, &id_copy) {
                Ok(()) => debug!(
                    "ThreadPool: Paquete de cliente <{}> procesado con exito",
                    id_copy
                ),
                Err(err) => error!(
                    "ThreadPool: Error procesando paquete de cliente <{}>: {}",
                    id_copy,
                    err.to_string()
                ),
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
    ) -> ServerResult<PacketType> {
        let packet_type = PacketType::try_from(control_byte)?;
        logging::log(LogKind::PacketProcessing(id, packet_type));
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
        Ok(packet_type)
    }

    /// Reads a packet from the stream and processes it.
    ///
    /// In case the client associated with the stream has disconnected,
    /// it returns an error of kin [ServerErrorKind::ClientDisconnected]
    pub fn process_packet<T: Read>(
        self: &Arc<Self>,
        stream: &mut T,
        id: &ClientIdArg,
    ) -> ServerResult<PacketType> {
        let mut control_byte_buff = [0u8; 1];
        match stream.read_exact(&mut control_byte_buff) {
            Ok(_) => {
                Ok(self.process_packet_given_control_byte(control_byte_buff[0], stream, id)?)
            }
            Err(error)
                if error.kind() == io::ErrorKind::UnexpectedEof
                    || error.kind() == io::ErrorKind::ConnectionReset =>
            {
                Err(ServerError::new_kind(
                    "Cliente se desconecto sin avisar",
                    ServerErrorKind::ClientDisconnected,
                ))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    /// Receives through the channel the packets to be published, and
    /// publishes them
    fn publish_dispatcher_loop(self: &Arc<Self>, receiver: Receiver<Message>) -> ServerResult<()> {
        let lock = self.pool.lock()?;
        let threadpool_copy = lock.clone();
        drop(lock);

        for message in receiver {
            let id = message.client_id;
            let publish = message.packet;
            logging::log(LogKind::Publishing(&id));
            let sv_copy = self.clone();
            threadpool_copy
                .spawn(move || {
                    sv_copy
                        .clients_manager
                        .read()
                        .unwrap()
                        .client_do(&id, |mut client| client.send_publish(publish))
                        .unwrap();
                })
                .unwrap();
        }
        Ok(())
    }

    /// Send [Publish] to all clients that are subscribed to the topic
    fn broadcast_publish(self: &Arc<Self>, publish: Publish) -> ServerResult<()> {
        let (sender, receiver) = mpsc::channel();
        let sv_copy = self.clone();
        let handler: JoinHandle<ServerResult<()>> = thread::spawn(move || {
            logging::log::<&str>(LogKind::ThreadStart(thread::current().id()));
            sv_copy.publish_dispatcher_loop(receiver)?;
            Ok(())
        });
        self.topic_handler.publish(&publish, sender)?;

        if let Err(err) = handler.join() {
            Err(ServerError::new_msg(&format!(
                "Error en el thread de publish_dispatcher_loop: {:?}",
                err
            )))
        } else {
            Ok(())
        }
    }

    /// Publish the packet so that all clients subscribed
    /// to the topics can receive them
    pub fn handle_publish(
        self: &Arc<Self>,
        mut publish: Publish,
        id: &ClientIdArg,
    ) -> ServerResult<()> {
        debug!("<{}>: Procesando PUBLISH", id);
        publish.set_max_qos(QoSLevel::QoSLevel1);
        if let Some(packet_id) = publish.packet_id() {
            self.clients_manager.read()?.client_do(id, |mut client| {
                client.send_packet(&Puback::new(packet_id)?)
            })?;
        }
        self.broadcast_publish(publish)
    }

    /// Subscribes the client to all the topics specified in the
    /// [Subscribe] packet
    /// Send the corresponding Suback
    fn handle_subscribe(&self, mut subscribe: Subscribe, id: &ClientIdArg) -> ServerResult<()> {
        debug!("<{}>: Recibido SUBSCRIBE", id);
        subscribe.set_max_qos(QoSLevel::QoSLevel1);
        self.clients_manager
            .read()?
            .client_do(id, |mut client| client.send_packet(&subscribe.response()?))?;

        let retained_messages = self.topic_handler.subscribe(&subscribe, id)?;
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
    /// [Unsubscribe] packet
    /// Send the corresponding [Unsuback]
    fn handle_unsubscribe(&self, unsubscribe: Unsubscribe, id: &ClientIdArg) -> ServerResult<()> {
        debug!("<{}>: Recibido UNSUBSCRIBE", id);
        let packet_id = unsubscribe.packet_id();
        self.topic_handler.unsubscribe(unsubscribe, id)?;
        self.clients_manager.read()?.client_do(id, |mut client| {
            client.send_packet(&Unsuback::new(packet_id)?)?;
            Ok(())
        })?;
        Ok(())
    }

    /// Sends the LastWill packet, previously converted to the
    /// [Publish] format
    pub fn send_last_will(
        self: &Arc<Self>,
        mut last_will: Publish,
        id: &ClientIdArg,
    ) -> ServerResult<()> {
        debug!("<{}>: Enviando LAST WILL", id);
        last_will.set_max_qos(QoSLevel::QoSLevel1);

        self.broadcast_publish(last_will)
    }

    /// Waits until it receives the [Connect] packet. In case the
    /// read fails due to timeout, it returns an error of kind
    /// [ServerErrorKind::Timeout]
    pub fn wait_for_connect(
        &self,
        connection_stream: &mut NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<Connect> {
        match Connect::new_from_zero(connection_stream) {
            Ok(connect) => {
                info!("<{}>: Recibido CONNECT", connection_stream.id());
                Ok(connect)
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }
}
