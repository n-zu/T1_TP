#![allow(dead_code)]

use std::{
    convert::TryFrom,
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, channel, Receiver},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use threadpool::ThreadPool;
use tracing::{debug, error, info, warn};

use packets::{connect::Connect, disconnect::Disconnect, traits::MQTTDecoding};
use packets::{
    helpers::PacketType, pingreq::PingReq, puback::Puback, subscribe::Subscribe,
    unsuback::Unsuback, unsubscribe::Unsubscribe,
};

mod server_controller;
pub mod server_error;

pub use server_error::ServerError;

const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(1);
const ACCEPT_SLEEP_DUR: Duration = Duration::from_millis(100);

use packets::publish::Publish;
use packets::qos::QoSLevel;

use crate::{
    client::Client,
    client_thread_joiner::ClientThreadJoiner,
    clients_manager::ClientsManager,
    config::Config,
    server::server_error::ServerErrorKind,
    topic_handler::{Message, TopicHandler},
};

use self::server_controller::ServerController;

pub type ServerResult<T> = Result<T, ServerError>;
pub type ClientId = String;
// Se necesita para que Clippy lo detecte como slice
pub type ClientIdArg = str;

pub enum Packet {
    Publish(Publish),
    Puback(Puback),
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
    PingReq(PingReq),
    Disconnect(Disconnect),
}

/// Represents a Server that complies with the
/// MQTT V3.1.1 protocol
pub struct Server {
    /// Clients connected to the server
    clients_manager: RwLock<ClientsManager>,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree
    topic_handler: TopicHandler,
    /// Vector with the handlers of the clients running in parallel
    client_thread_joiner: Mutex<ClientThreadJoiner>,
    pool: Mutex<ThreadPool>,
}

impl Server {
    /// Creates a new Server
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor en localhost:{}", config.port());
        Arc::new(Self {
            clients_manager: RwLock::new(ClientsManager::new(config.accounts_path())),
            config,
            topic_handler: TopicHandler::new(),
            client_thread_joiner: Mutex::new(ClientThreadJoiner::new()),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
        })
    }

    fn read_packet(&self, control_byte: u8, stream: &mut TcpStream) -> ServerResult<Packet> {
        match PacketType::try_from(control_byte)? {
            PacketType::Publish => {
                let packet = Publish::read_from(stream, control_byte)?;
                Ok(Packet::Publish(packet))
            }
            PacketType::Puback => {
                let packet = Puback::read_from(stream, control_byte)?;
                Ok(Packet::Puback(packet))
            }
            PacketType::Subscribe => {
                let packet = Subscribe::read_from(stream, control_byte)?;
                Ok(Packet::Subscribe(packet))
            }
            PacketType::Unsubscribe => {
                let packet = Unsubscribe::read_from(stream, control_byte)?;
                Ok(Packet::Unsubscribe(packet))
            }
            PacketType::PingReq => {
                let packet = PingReq::read_from(stream, control_byte)?;
                Ok(Packet::PingReq(packet))
            }
            PacketType::Disconnect => {
                let packet = Disconnect::read_from(stream, control_byte)?;
                Ok(Packet::Disconnect(packet))
            }
            _ => Err(ServerError::new_kind(
                "Codigo de paquete inesperado",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    fn receive_packet(&self, stream: &mut TcpStream) -> ServerResult<Packet> {
        let mut control_byte_buff = [0u8; 1];
        match stream.read_exact(&mut control_byte_buff) {
            Ok(_) => Ok(self.read_packet(control_byte_buff[0], stream)?),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                Err(ServerError::new_kind(
                    "Cliente se desconecto sin avisar",
                    ServerErrorKind::ClientDisconnected,
                ))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    fn publish_dispatcher_loop(&self, receiver: Receiver<Message>) -> ServerResult<()> {
        for message in receiver {
            debug!("<{}>: enviando PUBLISH", message.client_id);
            let id = message.client_id.clone();
            self.clients_manager
                .read()?
                .send_publish(&id, message.packet)?;
        }
        Ok(())
    }

    fn handle_publish(
        self: &Arc<Self>,
        mut publish: Publish,
        id: &ClientIdArg,
    ) -> ServerResult<()> {
        info!("<{}> Recibido PUBLISH", id);
        publish.set_max_qos(QoSLevel::QoSLevel1);
        let (sender, receiver) = mpsc::channel();
        let sv_copy = self.clone();
        let handler: JoinHandle<ServerResult<()>> = thread::spawn(move || {
            sv_copy.publish_dispatcher_loop(receiver)?;
            Ok(())
        });
        self.topic_handler.publish(&publish, sender)?;
        // QoSLevel1
        if let Some(packet_id) = publish.packet_id() {
            self.clients_manager.read()?.client_do(id, |mut client| {
                client.send_packet(Puback::new(*packet_id)?)?;
                Ok(())
            })?;
        }
        if let Err(err) = handler.join() {
            Err(ServerError::new_msg(&format!(
                "Error en el thread de publish_dispatcher_loop: {:?}",
                err
            )))
        } else {
            Ok(())
        }
    }

    fn handle_subscribe(&self, mut subscribe: Subscribe, id: &ClientIdArg) -> ServerResult<()> {
        debug!("<{}>: Recibido SUBSCRIBE", id);
        subscribe.set_max_qos(QoSLevel::QoSLevel1);
        self.clients_manager
            .read()?
            .client_do(id, |mut client| client.send_suback(subscribe.response()?))?;

        if let Some(retained_messages) = self.topic_handler.subscribe(&subscribe, id)? {
            self.clients_manager.read()?.client_do(id, |mut client| {
                for retained in retained_messages {
                    client.send_publish(retained);
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    fn handle_unsubscribe(&self, unsubscribe: Unsubscribe, id: &ClientIdArg) -> ServerResult<()> {
        debug!("<{}>: Recibido UNSUBSCRIBE", id);
        let packet_id = unsubscribe.packet_id();
        self.topic_handler.unsubscribe(unsubscribe, id)?;
        self.clients_manager.read()?.client_do(id, |mut client| {
            client.send_packet(Unsuback::new(packet_id)?)?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn handle_packet(self: &Arc<Self>, packet: Packet, id: &ClientIdArg) -> ServerResult<()> {
        match packet {
            Packet::Publish(packet) => self.handle_publish(packet, id),
            Packet::Subscribe(subscribe) => self.handle_subscribe(subscribe, id),
            Packet::Unsubscribe(unsubscribe) => self.handle_unsubscribe(unsubscribe, id),
            Packet::PingReq(_pingreq) => self
                .clients_manager
                .read()?
                .client_do(id, |mut client| client.send_pingresp()),
            Packet::Puback(puback) => self
                .clients_manager
                .read()?
                .client_do(id, |mut client| client.acknowledge(puback)),
            Packet::Disconnect(_disconnect) => unreachable!(),
        }
    }

    fn wait_for_connect(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<Client> {
        match Connect::new_from_zero(stream) {
            Ok(packet) => {
                info!("<{}> -> <{}>: Recibido CONNECT", addr, packet.client_id());
                Ok(Client::new(packet, stream.try_clone()?))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<String> {
        info!("<{}>: Conectando", addr);
        match self.wait_for_connect(stream, addr) {
            Ok(client) => {
                let id = client.id().to_owned();
                let keep_alive = client.keep_alive();
                if keep_alive == 0 {
                    stream.set_read_timeout(None)?;
                } else {
                    stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT))?;
                }
                self.clients_manager.write()?.new_session(client)?;
                Ok(id)
            }
            Err(err) if err.kind() == ServerErrorKind::Timeout => {
                error!("<{}>: timeout esperando CONNECT", addr);
                Err(err)
            }
            Err(err) => {
                error!(
                    "Error recibiendo CONNECT de cliente <{}>: {}",
                    addr,
                    err.to_string()
                );
                return Err(ServerError::new_kind(
                    &format!("Error de conexion: {}", err.to_string()),
                    ServerErrorKind::ProtocolViolation,
                ));
            }
        }
    }

    fn to_threadpool(self: &Arc<Self>, id: &ClientIdArg, packet: Packet) -> ServerResult<()> {
        let sv_copy = self.clone();
        let id_copy = id.to_owned();
        self.pool
            .lock()?
            .spawn(move || match sv_copy.handle_packet(packet, &id_copy) {
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

    fn client_loop(self: &Arc<Self>, id: String, mut stream: TcpStream) -> ServerResult<()> {
        let mut timeout_counter = 0;
        let client_keep_alive = self
            .clients_manager
            .read()?
            .get_client_property(&id, |client| Ok(client.keep_alive()))?;

        while self.clients_manager.read()?.is_connected(&id)? {
            match self.receive_packet(&mut stream) {
                Ok(packet) => {
                    timeout_counter = 0;
                    if let Packet::Disconnect(_disconnect) = packet {
                        debug!("<{}>: Recibido DISCONNECT", id);
                        self.clients_manager.write()?.finish_session(&id, true)?;
                    } else {
                        self.to_threadpool(&id, packet)?;
                    }
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    timeout_counter += 1;
                    self.clients_manager.read()?.send_unacknowledged(&id)?;
                }
                Err(err) => {
                    error!("<{}>: {}", id, err.to_string());
                    self.clients_manager.write()?.finish_session(&id, false)?;
                    break;
                }
            }
            if timeout_counter > client_keep_alive {
                error!("<{}>: Keep Alive timeout", id);
                self.clients_manager.write()?.finish_session(&id, false)?;
                break;
            }
        }
        Ok(())
    }

    fn manage_client(
        self: &Arc<Self>,
        mut stream: TcpStream,
        addr: SocketAddr,
    ) -> ServerResult<()> {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ClientNotInWhitelist => {
                    warn!("Addr {} - Intento de conexion con usuario invalido", addr);
                }
                ServerErrorKind::InvalidPassword => {
                    warn!(
                        "Addr {} - Intento de conexion con contraseña invalido",
                        addr
                    );
                }
                ServerErrorKind::ProtocolViolation => {
                    error!(
                        "Addr {} - Violacion de protocolo: {}",
                        addr,
                        err.to_string()
                    );
                    return Err(err);
                }
                _ => {
                    error!("Error inesperado: {}", err.to_string());
                    return Err(err);
                }
            },
            Ok(id) => {
                self.client_loop(id, stream)?;
            }
        }
        Ok(())
    }

    fn accept_client(
        self: &Arc<Self>,
        listener: &TcpListener,
    ) -> ServerResult<(TcpStream, SocketAddr)> {
        loop {
            match listener.accept() {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(ACCEPT_SLEEP_DUR);
                }
                Err(error) => {
                    error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                    return Err(ServerError::from(error));
                }
                Ok((stream, addr)) => {
                    info!("Aceptada conexion TCP con {}", addr);
                    stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                    return Ok((stream, addr));
                }
            }
        }
    }

    fn run_client(self: &Arc<Self>, stream: TcpStream, addr: SocketAddr) -> ServerResult<()> {
        let sv_copy = self.clone();
        let handle = thread::spawn(move || {
            match sv_copy.manage_client(stream, addr) {
                Ok(()) => debug!("Cliente con Address {} procesado con exito", addr),
                Err(err) => error!(
                    "Error procesando al cliente con Address {}: {}",
                    addr,
                    err.to_string()
                ),
            }
            sv_copy
                .client_thread_joiner
                .lock()
                .expect("Lock envenenado")
                .finished(addr)
                .unwrap_or_else(|err| panic!("Error irrecuperable: {}", err.to_string()));
        });
        self.client_thread_joiner.lock()?.add(addr, handle);
        Ok(())
    }

    //
    fn server_loop(
        self: Arc<Self>,
        listener: TcpListener,
        shutdown_receiver: Receiver<()>,
    ) -> ServerResult<()> {
        let mut recv_result = shutdown_receiver.try_recv();
        while recv_result.is_err() {
            match self.accept_client(&listener) {
                Ok((stream, addr)) => {
                    self.run_client(stream, addr)
                        .unwrap_or_else(|err| error!("{}: Error - {}", addr, err.to_string()));
                }
                Err(err) => {
                    error!("Error aceptando una nueva conexion: {}", err.to_string());
                    break;
                }
            }
            self.client_thread_joiner.lock()?.join_finished();
            recv_result = shutdown_receiver.try_recv();
        }
        debug!("Apagando Servidor");
        self.clients_manager.write()?.finish_all_sessions(false)?;
        Ok(())
    }

    /// Run the server in a new thread
    /// Returns a ServerController that can be used to stop the server
    pub fn run(self: Arc<Self>) -> io::Result<ServerController> {
        let listener = TcpListener::bind(format!("localhost:{}", self.config.port()))?;
        listener.set_nonblocking(true)?;
        let (shutdown_sender, shutdown_receiver) = channel();
        let server = self;

        let server_handle = thread::spawn(move || {
            if let Err(err) = server.server_loop(listener, shutdown_receiver) {
                error!(
                    "Error inesperado del servidor: {} - Se recomienda apagarlo",
                    err.to_string()
                )
            }
        });
        let server_controller = ServerController::new(shutdown_sender, server_handle);
        Ok(server_controller)
    }
}
