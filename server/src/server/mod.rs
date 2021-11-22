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

mod packet_processing;
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
    clients_manager::{ClientsManager, DisconnectInfo},
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

pub trait ServerInterface {
    fn new(config: Config, threadpool_size: usize) -> Arc<Self>;
    fn run(self: Arc<Self>) -> io::Result<ServerController>;
}

impl ServerInterface for Server {
    /// Creates a new Server
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

    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<String> {
        info!("<{}>: Conectando", addr);
        match self.wait_for_connect(stream, addr) {
            Ok(client) => {
                let keep_alive = client.keep_alive();
                if keep_alive == 0 {
                    stream.set_read_timeout(None)?;
                } else {
                    stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT))?;
                }
                let id = self.clients_manager.write()?.new_session(client)?;
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

        while self.clients_manager.read()?.is_connected(&id)? {
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
        // TODO: no deberia pasar
        Ok(DisconnectInfo {
            publish_last_will: None,
            clean_session: true,
        })
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
                let disconnect_info = self.client_loop(id.clone(), stream)?;
                if let Some(last_will) = disconnect_info.publish_last_will {
                    self.send_last_will(last_will, &id)?;
                }
                if disconnect_info.clean_session {
                    self.topic_handler.remove_client(&id)?;
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
        self.client_thread_joiner.lock()?.add(addr, handle);
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
            self.client_thread_joiner.lock()?.join_finished();
            recv_result = shutdown_receiver.try_recv();
        }
        debug!("Apagando Servidor");
        self.clients_manager.write()?.finish_all_sessions(false)?;
        Ok(())
    }
}
