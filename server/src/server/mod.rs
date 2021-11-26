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

use packets::{connect::{Connect}, disconnect::Disconnect, traits::MQTTDecoding};
use packets::{
    helpers::PacketType, pingreq::PingReq, puback::Puback, subscribe::Subscribe,
    unsuback::Unsuback, unsubscribe::Unsubscribe,
};

mod client_stream_reader;
mod packet_processing;
mod server_controller;
pub mod server_error;

pub use server_error::ServerError;

const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(1);
const ACCEPT_SLEEP_DUR: Duration = Duration::from_millis(100);

use packets::publish::Publish;
use packets::qos::QoSLevel;

use crate::{client::Client, client_thread_joiner::ClientThreadJoiner, clients_manager::{ClientsManager, ConnectInfo, DisconnectInfo}, config::Config, server::server_error::ServerErrorKind, topic_handler::{Message, TopicHandler}};

use self::server_controller::ServerController;

pub type ServerResult<T> = Result<T, ServerError>;
pub type ClientId = String;
// Se necesita para que Clippy lo detecte como slice
pub type ClientIdArg = str;

/// Represents a Server that complies with the
/// MQTT V3.1.1 protocol
///
/// A server runs on a separate thread from which it
/// is invoked
/// When a client connects, a new thread is created to
/// handle it
/// Every time a client sends a packet, it is read from
/// the client thread and sent to a threadpool that
/// processes it (exceptions to this rule are found in the
/// description of the process_packet method)
/// The shutdown of the server is controlled through a
/// ServerController that sends a message to the server thread
/// to stop it
pub struct Server {
    /// Clients connected to the server
    /// It handles the connection and disconnection of clients,
    /// as well as credential verification
    /// If a client has a clean session set to False, it is
    /// responsible for saving their information even after
    /// disconnection. If it has clean session set to True,
    /// their data is deleted
    clients_manager: RwLock<ClientsManager>,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree
    /// When a customer subscribes to a topic or publish a message,
    /// all the information is stored in this handler. This includes
    /// Quality of Service and handling of retained messages.
    /// When it processes a Publish, the TopicHandler indicates to
    /// the Server the clients to whom it should send it through a
    /// MPSC channel. An independent thread receives the information
    /// and sends the packets.
    /// The TopicHandler is not responsible for keeping the subscriptions
    /// of users connected with clean session set to True. The
    /// server is responsible to invoke remove_client() when the client
    /// has clean session set to True
    topic_handler: TopicHandler,
    /// Manages the join of the threads that are created for each of the
    /// clients. It does not interfere in the threads of the threadpool
    /// or the Server thread.
    /// When a new client thread is created, it saves its JoinHandle
    /// assosiationg it with the SockerAddr of the client that just
    /// connected
    client_thread_joiner: Mutex<ClientThreadJoiner>,
    pool: Mutex<ThreadPool>,
}

pub trait ServerInterface {
    fn new(config: Config, threadpool_size: usize) -> Arc<Self>;
    fn run(self: Arc<Self>) -> io::Result<ServerController>;
}

impl ServerInterface for Server {
    /// Creates and returns a server in a valid state
    fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor en localhost: {}", config.port());
        Arc::new(Self {
            clients_manager: RwLock::new(ClientsManager::new(config.accounts_path())),
            config,
            topic_handler: TopicHandler::new(),
            client_thread_joiner: Mutex::new(ClientThreadJoiner::new()),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
        })
    }

    /// Run the server in a new thread
    /// Returns a ServerController that can be used to stop the server
    fn run(self: Arc<Self>) -> io::Result<ServerController> {
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

impl Server {
    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<ConnectInfo> {
        info!("<{}>: Conectando", addr);
        match self.wait_for_connect(stream, addr) {
            Ok(client) => {
                let keep_alive = client.keep_alive();
                if keep_alive == 0 {
                    stream.set_read_timeout(None)?;
                } else {
                    stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT))?;
                }
                let connect_info = self.clients_manager.write()?.new_session(client)?;
                info!("<{}> -> <{}>: Conectado", addr, connect_info.id);
                Ok(connect_info)
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

    // TODO: convertirlo en macro
    fn log_processed_packet(&self, packet_type: PacketType, id: &ClientIdArg) {
        match packet_type {
            PacketType::Connect => debug!("<{}>: Procesado CONNECT", id),
            PacketType::Publish => debug!("<{}>: Procesado PUBLISH", id),
            PacketType::Puback => debug!("<{}>: Procesado PUBACK", id),
            PacketType::Subscribe => debug!("<{}>: Procesado SUBSCRIBE", id),
            PacketType::Unsubscribe => debug!("<{}>: Procesado UNSUBSCRIBE", id),
            PacketType::PingReq => debug!("<{}>: Procesado PINGREQ", id),
            PacketType::Disconnect => debug!("<{}>: Procesado DISCONNECT", id),
            _ => unreachable!(),
        }
    }

    fn client_loop(
        self: &Arc<Self>,
        id: String,
        mut stream: TcpStream,
    ) -> ServerResult<DisconnectInfo> {
        let mut timeout_counter = 0;
        let client_keep_alive = self
            .clients_manager
            .read()?
            .get_client_property(&id, |client| Ok(client.keep_alive()))?;

        loop {
            match self.process_packet(&mut stream, &id) {
                Ok(packet_type) => {
                    timeout_counter = 0;
                    self.log_processed_packet(packet_type, &id);
                    if packet_type == PacketType::Disconnect {
                        return self.clients_manager.write()?.finish_session(&id, true);
                    }
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    timeout_counter += 1;
                    self.clients_manager.read()?.send_unacknowledged(&id)?;
                }
                Err(err) if err.kind() == ServerErrorKind::ClientDisconnected => {
                    info!("<{}>: Desconectado - {}", id, err.to_string());
                    return self.clients_manager.write()?.finish_session(&id, false);
                }
                Err(err) => {
                    error!("<{}>: {}", id, err.to_string());
                    return self.clients_manager.write()?.finish_session(&id, false);
                }
            }
            if timeout_counter > client_keep_alive {
                error!("<{}>: Keep Alive timeout", id);
                return self.clients_manager.write()?.finish_session(&id, false);
            }
        }
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
            Ok(connect_info) => {
                // En caso de que haya ocurrido una reconexion y el cliente
                // tenia un last will, se publica
                if let Some(last_will) = connect_info.takeover_last_will {
                    self.send_last_will(last_will, &connect_info.id)?;
                }
                let disconnect_info = self.client_loop(connect_info.id.clone(), stream)?;
                if let Some(last_will) = disconnect_info.publish_last_will {
                    self.send_last_will(last_will, &connect_info.id)?;
                }
                if disconnect_info.clean_session {
                    self.topic_handler.remove_client(&connect_info.id)?;
                }
            }
        }
        Ok(())
    }

    fn accept_client(
        self: &Arc<Self>,
        listener: &TcpListener,
    ) -> ServerResult<(TcpStream, SocketAddr)> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(ServerError::new_kind("Idle", ServerErrorKind::Idle))
            }
            Err(error) => {
                error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                info!("Aceptada conexion TCP con {}", addr);
                stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                Ok((stream, addr))
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
        self.client_thread_joiner.lock()?.add_thread(addr, handle)?;
        Ok(())
    }

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
                Err(err) if err.kind() == ServerErrorKind::Idle => {
                    thread::sleep(ACCEPT_SLEEP_DUR);
                }
                Err(err) => {
                    error!("Error aceptando una nueva conexion: {}", err.to_string());
                    break;
                }
            }
            recv_result = shutdown_receiver.try_recv();
        }
        debug!("Apagando Servidor");
        self.clients_manager.write()?.finish_all_sessions(false)?;
        Ok(())
    }
}
