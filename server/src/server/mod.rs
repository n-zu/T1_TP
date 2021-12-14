#![allow(dead_code)]

use std::{
    convert::TryFrom,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self},
    time::{Duration, SystemTime},
};

use threadpool::ThreadPool;
use tracing::{error, info, instrument, warn};

use packets::{
    connack::{Connack, ConnackReturnCode},
    connect::Connect,
    disconnect::Disconnect,
    traits::{MQTTDecoding, MQTTEncoding},
};
use packets::{
    helpers::PacketType, pingreq::PingReq, puback::Puback, subscribe::Subscribe,
    unsuback::Unsuback, unsubscribe::Unsubscribe,
};

mod dump;
mod packet_processing;
mod server_controller;
pub mod server_error;

pub use server_error::ServerError;

/// Maximum time between the client connection and the sending
/// of the [`Connect`] packet
const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
/// How often unacknowledged packets are sent
pub const UNACK_RESENDING_FREQ: Duration = Duration::from_millis(500);
/// How long the server sleeps between each failed TCP connection
/// atempt
const ACCEPT_SLEEP_DUR: Duration = Duration::from_millis(100);
/// Minimum time since the last sending of a packet for
/// it to be resent. This prevents very recent packets
/// from being resent
const MIN_ELAPSED_TIME: Option<Duration> = Some(Duration::from_millis(100));
/// Number of packets that are resent each time an
/// [`UNACK_RESENDING_FREQ`] elapses. If it is `Some(1)`, the server
/// guarantees that no QoS 1 message will be received after any later
/// one. For example a subscriber might receive them in the order
/// 1,2,3,3,4 but not 1,2,3,2,3,4
/// (See [4.6 Message ordering](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718105))
const INFLIGHT_MESSAGES: Option<usize> = Some(1);

use packets::publish::Publish;
use packets::qos::QoSLevel;

use crate::{
    clients_manager::{ClientsManager, ConnectInfo},
    network_connection::NetworkConnection,
    server::server_error::ServerErrorKind,
    thread_joiner::ThreadJoiner,
    topic_handler::{Message, TopicHandler},
    traits::*,
};

pub use self::server_controller::ServerController;

pub type ServerResult<T> = Result<T, ServerError>;
#[doc(hidden)]
pub type ClientId = String;
#[doc(hidden)]
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
/// [ServerController] that sends a message to the server thread
/// to stop it
pub struct Server<C: Config> {
    /// Clients connected to the server.
    ///
    /// It handles the connection and disconnection of clients,
    /// as well as credential verification.
    ///
    /// If a client has `clean_session` set to false, it is
    /// responsible for saving their information even after
    /// disconnection. If it has `clean_session` set to true,
    /// their data is deleted
    clients_manager: RwLock<ClientsManager<TcpStream, SocketAddr>>,
    /// Initial Server setup
    config: C,
    /// Manages the Publish / Subscribe tree.
    ///
    /// When a customer subscribes to a topic or publish a message,
    /// all the information is stored in this handler. This includes
    /// Quality of Service and handling of retained messages.
    /// When it processes a Publish, the [`TopicHandler`] indicates to
    /// the Server the clients to whom it should send it through a
    /// MPSC channel. An independent thread receives the information
    /// and sends the packets.
    ///
    /// The TopicHandler is not responsible for keeping the subscriptions
    /// of users connected with clean session set to True. The
    /// server is responsible to invoke remove_client() when the client
    /// has clean session set to True.
    topic_handler: TopicHandler,
    /// Manages the join of the threads that are created for each of the
    /// clients. It does not interfere in the threads of the threadpool
    /// or the Server thread.
    ///
    /// When a new client thread is created, it saves its JoinHandle
    /// assosiating it with the ThreadId of the client that just
    /// connected.
    client_thread_joiner: Mutex<ThreadJoiner>,
    /// Threadpool used to process packets received from clients
    /// The only ones that are not processed in the Threadpool
    /// are the [`Connect`] and [`Disconnect`] packets.
    pool: Mutex<ThreadPool>,
}

impl<C: Config> Server<C> {
    /// Creates and returns a server in a valid state
    pub fn new(config: C, threadpool_size: usize) -> Option<Arc<Self>> {
        info!("Creando servidor");
        match Server::try_restore(&config, threadpool_size) {
            Ok(server) => {
                if let Some(server) = server {
                    info!("Se encontro un archivo de DUMP - Creando servidor con su informacion");
                    Some(server)
                } else {
                    info!("No se encontro un archivo de DUMP - Creando servidor en blanco");

                    let server = Arc::new(Self {
                        clients_manager: RwLock::new(ClientsManager::new(config.authenticator())),
                        config,
                        topic_handler: TopicHandler::new(),
                        client_thread_joiner: Mutex::new(ThreadJoiner::new()),
                        pool: Mutex::new(ThreadPool::new(threadpool_size)),
                    });
                    Some(server)
                }
            }
            Err(_err) => None,
        }
    }

    /// Run the server in a new thread.
    ///
    /// Returns a ServerController that can be used to stop the server
    ///
    /// This method does not return until the server initializes everything
    /// necessary to start accepting connections
    #[instrument(skip(self) fields(ip = %self.config.ip(), port = %self.config.port()))]
    pub fn run(self: Arc<Self>) -> io::Result<ServerController> {
        let shutdown_bool = Arc::new(AtomicBool::new(false));
        let shutdown_bool_copy = shutdown_bool.clone();
        let (started_sender, started_receiver) = mpsc::channel();

        let server_handle = thread::Builder::new()
            .name("server_loop".to_owned())
            .spawn(move || {
                if let Err(err) = self.server_loop(shutdown_bool, started_sender) {
                    error!(
                        "Error inesperado del servidor: {} - Se recomienda apagarlo",
                        err.to_string()
                    )
                }
            })?;
        started_receiver.recv().unwrap_or_else(|e| {
            error!("Error iniciando el servidor: {}", e);
        });
        let server_controller = ServerController::new(shutdown_bool_copy, server_handle);
        Ok(server_controller)
    }

    /// Receives the [`Connect`] packet from a client, connects it to the
    /// server and sets its network_connection read Timeout with the Keep Alive Timeout
    /// provided by the client in the [`Connect`] packet.
    /// `network_connection` is the network_connection of the client from which the packet is received
    ///
    /// It does not send the [`Connack`] packet.
    ///
    /// If the connection was successful, it returns [`Ok(ConnectionInfo)`] with
    /// the necessary information to send the Connack, as well as the LastWill
    /// [`Publish`] packet from the previous network_connection, in case a Client Take-Over
    /// type reconnection has ocurred.
    ///
    /// In case an error has ocurred, but a Connack must be sent (for example,
    /// if the credentials are invalid), it returns an Error with kind
    /// [`ServerErrorKind::ConnectionRefused`], with the return code that the Connack
    /// must contain. If the error it returns is not of that kind, a Connack should
    /// not be send.
    #[instrument(skip(self, network_connection))]
    fn connect_client(
        self: &Arc<Self>,
        network_connection: &mut NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<ConnectInfo> {
        info!("Conectando cliente");
        let connect = self.wait_for_connect(network_connection)?;
        network_connection.stream_mut().set_read_timeout(Some(UNACK_RESENDING_FREQ))?;
        let connect_info = self
            .clients_manager
            .write()?
            .new_session(network_connection.try_clone()?, connect)?;
        Ok(connect_info)
    }

    /// Process a client until it disconnects. This includes receiving the
    /// packets that the client send, processing them, and sending the corresponding
    /// acknowledgements. It does not disconnect the client.
    ///
    /// Returns true if the client should be disconnected gracefully.
    /// If it returns false or error, it should be disconnected
    /// ungracefully
    #[instrument(skip(self, id, network_connection))]
    fn client_loop(
        self: &Arc<Self>,
        id: &ClientIdArg,
        network_connection: &mut NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<bool> {
        let mut last_activity = SystemTime::now();
        let keep_alive_opt = self
            .clients_manager
            .read()?
            .client_do(id, |client| Ok(client.keep_alive()))?;

        loop {
            match self.process_packet(network_connection, id) {
                Ok(packet_type) => {
                    last_activity = SystemTime::now();
                    if packet_type == PacketType::Disconnect {
                        return Ok(true);
                    }
                    continue;
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    self.clients_manager.read()?.client_do(id, |mut client| {
                        client.send_unacknowledged(INFLIGHT_MESSAGES, MIN_ELAPSED_TIME)
                    })?;
                }
                Err(err) => {
                    if err.kind() != ServerErrorKind::ClientDisconnected {
                        error!("Error inesperado: {}", err);
                    }
                    return Ok(false);
                }
            }
            if let Some(keep_alive) = keep_alive_opt {
                if SystemTime::now().duration_since(last_activity).unwrap() > keep_alive {
                    warn!("KeepAlive Timeout");
                    return Ok(false);
                }
            }
        }
    }

    /// Process a client after it sends the [`Connect`] packet. That is,
    /// it sends the corresponding [`Connack`], and processes all the packets
    /// sent by the client until it disconnects. When this happens, it also
    /// publishes the LastWill, if it was specified by the client.
    ///
    /// Clean the client session in case the client connects with
    /// clean_session set to true.
    ///
    /// In case a Client TakeOver occurs and the previous session had LastWill,
    /// it is also published.
    #[instrument(skip(self, connect_info, network_connection) fields(client_id = %connect_info.id))]
    fn manage_succesfull_connection(
        self: &Arc<Self>,
        connect_info: ConnectInfo,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        info!("Cliente aceptado");
        network_connection.write_all(
            &Connack::new(connect_info.session_present, ConnackReturnCode::Accepted).encode()?,
        )?;
        if let Some(last_will) = connect_info.takeover_last_will {
            self.send_last_will(last_will, &connect_info.id)?;
        }
        // En caso de que haya ocurrido una reconexion y el cliente
        // tenia un last will, se publica
        let disconnect_info;
        if let Ok(gracefully) = self.client_loop(&connect_info.id, &mut network_connection) {
            disconnect_info = self.clients_manager.write()?.disconnect(
                &connect_info.id,
                network_connection,
                gracefully,
            )?;
        } else {
            disconnect_info = self.clients_manager.write()?.disconnect(
                &connect_info.id,
                network_connection,
                false,
            )?;
        }
        if disconnect_info.clean_session {
            self.topic_handler.remove_client(&connect_info.id)?;
        }
        if let Some(last_will) = disconnect_info.publish_last_will {
            self.send_last_will(last_will, &connect_info.id)?;
        }
        Ok(())
    }

    /// Send a [`Connack`] to the client if the connection failed due to one
    /// of the errors listed in section `3.2.2.3` of the MQTT v3.1.1 protocol
    /// Otherwise, it returns a [`ServerError`]
    #[instrument(skip(self, network_connection, error))]
    fn manage_failed_connection(
        &self,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
        error: ServerError,
    ) -> ServerResult<()> {
        match error.kind() {
            ServerErrorKind::ConnectionRefused(return_code) => {
                warn!("Conexion rechazada: {} - {}", return_code, error);
                network_connection.write_all(&Connack::new(false, return_code).encode()?)?;
                Ok(())
            }
            _ => Err(error),
        }
    }

    // Metodo usado para procesar los errores mas facilmente
    #[doc(hidden)]
    #[instrument(skip(self, network_connection) name="run_client", fields(socket_addr = %network_connection.id()))]
    fn _run_client(
        self: Arc<Self>,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        match self.connect_client(&mut network_connection) {
            Ok(connect_info) => {
                self.manage_succesfull_connection(connect_info, network_connection)?
            }
            Err(err) => self.manage_failed_connection(network_connection, err)?,
        };
        self.client_thread_joiner
            .lock()
            .expect("Lock envenenado")
            .finished(thread::current().id())
            .unwrap_or_else(|err| error!("Error irrecuperable: {}", err.to_string()));
        Ok(())
    }

    /// Creates a new thread in which the client will be handled. Adds that
    /// thread to the list of threads pending to be joined
    #[instrument(skip(self, network_connection), fields(socket_addr = %network_connection.id()))]
    fn run_client(
        self: &Arc<Self>,
        network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        let sv_copy = self.clone();
        let handle = thread::spawn(move || {
            sv_copy._run_client(network_connection).unwrap_or_else(|e| {
                // Si llega un error a este punto ya no se puede solucionar
                if e.kind() != ServerErrorKind::ClientDisconnected {
                    error!("Error no manejado: {}", e);
                }
            });
        });
        self.client_thread_joiner
            .lock()?
            .add_thread(handle.thread().id(), handle)?;
        Ok(())
    }

    /// Accepts clients and processes them as log as a shutdown signal is not
    /// received from the [ServerController] corresponding to this server
    #[instrument(skip(self, shutdown_bool, started_sender) fields(ip = %self.config.ip(), port = %self.config.port()))]
    fn server_loop(
        self: Arc<Self>,
        shutdown_bool: Arc<AtomicBool>,
        started_sender: Sender<()>,
    ) -> ServerResult<()> {
        let listener = TcpListener::bind(format!("{}:{}", self.config.ip(), self.config.port()))?;
        let mut time_last_dump = SystemTime::now();
        let dump_info_opt = self.config.dump_info();
        started_sender.send(())?;

        listener.set_nonblocking(true)?;
        while !shutdown_bool.load(Ordering::Relaxed) {
            match self.accept_client(&listener) {
                Ok(connection_stream) => {
                    let socket_addr = *connection_stream.id();
                    self.run_client(connection_stream)
                        .unwrap_or_else(|e| error!("{}: Error - {}", socket_addr, e));
                }
                Err(err) if err.kind() == ServerErrorKind::Idle => {
                    thread::sleep(ACCEPT_SLEEP_DUR);
                }
                Err(e) => {
                    error!("Error de nueva conexion: {}", e);
                    break;
                }
            }
            if let Some(dump_info) = dump_info_opt {
                if SystemTime::now().duration_since(time_last_dump).unwrap() >= dump_info.1 {
                    self.dump()?;
                    time_last_dump = SystemTime::now();
                }
            }
        }
        info!("Apagando servidor");
        let shutdown_info = self.clients_manager.write()?.shutdown(false)?;
        for client_id in shutdown_info.clean_session_ids {
            self.topic_handler.remove_client(&client_id)?;
        }
        for (id, last_will) in shutdown_info.last_will_packets {
            self.send_last_will(last_will, &id)?;
        }
        println!("{}", Arc::try_unwrap(self).is_err());
        Ok(())
    }

    /// Accepts a TCP connection and returns the stream corresponding to that
    /// connection.
    ///
    /// If no connection has been received, it returns an error of kind
    /// [`ServerErrorKind::Idle`]
    #[instrument(skip(self, listener) fields(socket_addr))]
    fn accept_client(
        self: &Arc<Self>,
        listener: &TcpListener,
    ) -> ServerResult<NetworkConnection<TcpStream, SocketAddr>> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(ServerError::new_kind("Idle", ServerErrorKind::Idle))
            }
            Err(error) => {
                error!("Error aceptando conexion TCP: {}", error);
                Err(ServerError::from(error))
            }
            Ok((stream, socket_addr)) => {
                stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                Ok(NetworkConnection::new(socket_addr, stream))
            }
        }
    }
}

impl<C: Config> Drop for Server<C> {
    fn drop(&mut self) {
        self.dump().unwrap_or_else(|e| {
            println!(
                "Error realizando el Dump durante el apagado del servidor: {}",
                &e
            );
            error!(
                "Error realizando el Dump durante el apagado del servidor: {}",
                e
            );
        });
    }
}
