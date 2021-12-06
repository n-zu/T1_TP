#![allow(dead_code)]

use std::{
    convert::TryFrom,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use threadpool::ThreadPool;
use tracing::{debug, error, info};

use packets::{
    connack::Connack,
    connect::Connect,
    disconnect::Disconnect,
    traits::{MQTTDecoding, MQTTEncoding},
};
use packets::{
    helpers::PacketType, pingreq::PingReq, puback::Puback, subscribe::Subscribe,
    unsuback::Unsuback, unsubscribe::Unsubscribe,
};

mod packet_processing;
mod server_controller;
pub mod server_error;

pub use server_error::ServerError;

/// Maximum time between the client connection and the sending
/// of the [Connect] packet
const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
/// How often unacknowledged packets are sent
const UNACK_RESENDING_FREQ: Duration = Duration::from_millis(500);
/// How long the server sleeps between each failed TCP connection
/// atempt
const ACCEPT_SLEEP_DUR: Duration = Duration::from_millis(100);

const MIN_ELAPSED_TIME: Option<Duration> = Some(Duration::from_millis(2000));

const INFLIGHT_MESSAGES: Option<usize> = None;

use packets::publish::Publish;
use packets::qos::QoSLevel;

use crate::{
    clients_manager::{ClientsManager, ConnectInfo, DisconnectInfo},
    config::Config,
    logging::{self, LogKind},
    network_connection::NetworkConnection,
    server::server_error::ServerErrorKind,
    thread_joiner::ThreadJoiner,
    topic_handler::{Message, TopicHandler},
    traits::{Close, TryClone},
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
pub struct Server {
    /// Clients connected to the server.
    ///
    /// It handles the connection and disconnection of clients,
    /// as well as credential verification.
    ///
    /// If a client has *clean_session* set to False, it is
    /// responsible for saving their information even after
    /// disconnection. If it has *clean_session* set to True,
    /// their data is deleted
    clients_manager: RwLock<ClientsManager<TcpStream, SocketAddr>>,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree.
    ///
    /// When a customer subscribes to a topic or publish a message,
    /// all the information is stored in this handler. This includes
    /// Quality of Service and handling of retained messages.
    /// When it processes a Publish, the [TopicHandler] indicates to
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
    /// are the [Connect] and [Disconnect] packets.
    pool: Mutex<ThreadPool>,
}

impl Server {
    /// Creates and returns a server in a valid state
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        logging::log::<SocketAddr>(LogKind::StartingServer(config.ip(), config.port()));
        Arc::new(Self {
            clients_manager: RwLock::new(ClientsManager::new(Some(config.accounts_path()))),
            config,
            topic_handler: TopicHandler::new(),
            client_thread_joiner: Mutex::new(ThreadJoiner::new()),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
        })
    }

    /// Run the server in a new thread.
    ///
    /// Returns a ServerController that can be used to stop the server
    ///
    /// This method does not return until the server initializes everything
    /// necessary to start accepting connections
    pub fn run(self: Arc<Self>) -> io::Result<ServerController> {
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let (started_sender, started_receiver) = mpsc::channel();

        let server_handle = thread::spawn(move || {
            logging::log::<&str>(LogKind::ThreadStart(thread::current().id()));
            if let Err(err) = self.server_loop(shutdown_receiver, started_sender) {
                error!(
                    "Error inesperado del servidor: {} - Se recomienda apagarlo",
                    err.to_string()
                )
            }
        });
        if let Err(err) = started_receiver.recv() {
            println!(
                "Error leyendo de stdin: {}\nCerrando servidor...",
                err.to_string()
            );
        }

        let server_controller = ServerController::new(shutdown_sender, server_handle);
        Ok(server_controller)
    }

    /// Receives the [Connect] packet from a client, connects it to the
    /// server and sets its network_connection read Timeout with the Keep Alive Timeout
    /// provided by the client in the [Connect] packet.
    /// *network_connection* is the network_connection of the client from which the packet is received
    ///
    /// It does not send the [Connack] packet.
    ///
    /// If the connection was successful, it returns [Ok(ConnectionInfo)] with
    /// the necessary information to send the Connack, as well as the LastWill
    /// [Publish] packet from the previous network_connection, in case a Client Take-Over
    /// type reconnection has ocurred.
    ///
    /// In case an error has ocurred, but a Connack must be sent (for example,
    /// if the credentials are invalid), it returns an Error with kind
    /// [ServerErrorKind::ConnectionRefused], with the return code that the Connack
    /// must contain. If the error it returns is not of that kind, a Connack should
    /// not be send.
    fn connect_client(
        self: &Arc<Self>,
        network_connection: &mut NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<ConnectInfo> {
        logging::log(LogKind::Connecting(&network_connection.id()));
        match self.wait_for_connect(network_connection) {
            Ok(connect) => {
                network_connection
                    .stream_mut()
                    .set_read_timeout(Some(UNACK_RESENDING_FREQ))?;
                let connect_info = self
                    .clients_manager
                    .write()?
                    .new_session(network_connection.try_clone()?, connect)?;
                Ok(connect_info)
            }
            Err(err) => {
                return Err(ServerError::new_kind(
                    &format!(
                        "Error recibiendo CONNECT de cliente <{:?}>: {}",
                        network_connection.id(),
                        err.to_string()
                    ),
                    ServerErrorKind::ProtocolViolation,
                ));
            }
        }
    }

    /// Process a client until it disconnects. This includes receiving the
    /// packets that the client send, processing them, sending the corresponding
    /// acknowledgements, and finally disconnecting it. It does not publish the
    /// LastWill packet in case of an ungracefully disconnection. It also does
    /// not remove the data from the TopicHandler.
    ///
    /// Returns information related to the disconnection, as well as the
    /// Last will [Publish] that, if it is not None, must be published.
    fn client_loop(
        self: &Arc<Self>,
        id: &ClientIdArg,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<DisconnectInfo> {
        let mut timeout_counter_ms = 0;
        let keep_alive_ms = self
            .clients_manager
            .read()?
            .get_client_property(id, |client| Ok((client.keep_alive() * 1000f32) as u128))?;
        let mut gracefully = true;

        loop {
            match self.process_packet(&mut network_connection, id) {
                Ok(packet_type) => {
                    timeout_counter_ms = 0;
                    logging::log(LogKind::PacketProcessing(id, packet_type));
                    if packet_type == PacketType::Disconnect {
                        break;
                    }
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    timeout_counter_ms += UNACK_RESENDING_FREQ.as_millis();
                    self.clients_manager.read()?.client_do(id, |mut client| {
                        client.send_unacknowledged(INFLIGHT_MESSAGES, MIN_ELAPSED_TIME)
                    })?;
                }
                Err(err) => {
                    if err.kind() != ServerErrorKind::ClientDisconnected {
                        logging::log(LogKind::UnexpectedError(id, err.to_string()));
                    }
                    gracefully = false;
                    break;
                }
            }
            if keep_alive_ms != 0 && timeout_counter_ms > keep_alive_ms {
                logging::log(LogKind::KeepAliveTimeout(id));
                gracefully = false;
                break;
            }
        }
        self.clients_manager
            .write()?
            .disconnect(id, network_connection, gracefully)
    }

    /// Process a client after it sends the [Connect] packet. That is,
    /// it sends the corresponding [Connack], and processes all the packets
    /// sent by the client until it disconnects. When this happens, it also
    /// publishes the LastWill, if it was specified by the client.
    ///
    /// Clean the client session in case the client connects with
    /// clean_session set to true.
    ///
    /// In case a Client TakeOver occurs and the previous session had LastWill,
    /// it is also published.
    fn manage_succesfull_connection(
        self: &Arc<Self>,
        connect_info: ConnectInfo,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        logging::log(LogKind::Connected(&connect_info.id));
        network_connection.write_all(
            &Connack::new(connect_info.session_present, connect_info.return_code).encode()?,
        )?;
        if let Some(last_will) = connect_info.takeover_last_will {
            self.send_last_will(last_will, &connect_info.id)?;
        }

        // En caso de que haya ocurrido una reconexion y el cliente
        // tenia un last will, se publica
        let disconnect_info = self.client_loop(&connect_info.id, network_connection)?;
        if disconnect_info.clean_session {
            self.topic_handler.remove_client(&connect_info.id)?;
        }
        if let Some(last_will) = disconnect_info.publish_last_will {
            self.send_last_will(last_will, &connect_info.id)?;
        }

        logging::log(LogKind::SuccesfulClientEnd(&connect_info.id));
        Ok(())
    }

    /// Send a [Connack] to the client if the connection failed due to one
    /// of the errors listed in section *3.2.2.3* of the MQTT v3.1.1 protocol
    /// Otherwise, it returns a [ServerError]
    fn manage_failed_connection(
        &self,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
        error: ServerError,
    ) -> ServerResult<()> {
        match error.kind() {
            ServerErrorKind::ConnectionRefused(return_code) => {
                logging::log(LogKind::ConnectionRefusedError(
                    &network_connection.id(),
                    error.to_string(),
                ));
                network_connection.write_all(&Connack::new(false, return_code).encode()?)?;
                Ok(())
            }
            _ => {
                logging::log(LogKind::UnexpectedError(
                    &network_connection.id(),
                    error.to_string(),
                ));
                Err(error)
            }
        }
    }

    // Metodo usado para procesar los errores mas facilmente
    #[doc(hidden)]
    fn _run_client(
        self: Arc<Self>,
        mut network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        logging::log::<&str>(LogKind::ThreadStart(thread::current().id()));
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
            .unwrap_or_else(|err| panic!("Error irrecuperable: {}", err.to_string()));
        Ok(())
    }

    /// Creates a new thread in which the client will be handled. Adds that
    /// thread to the list of threads pending to be joined
    fn run_client(
        self: &Arc<Self>,
        network_connection: NetworkConnection<TcpStream, SocketAddr>,
    ) -> ServerResult<()> {
        let sv_copy = self.clone();
        let handle = thread::spawn(move || {
            if let Err(err) = sv_copy._run_client(network_connection) {
                // Si llega un error a este punto ya no se puede solucionar
                logging::log::<&str>(LogKind::UnhandledError(err))
            }
        });
        self.client_thread_joiner
            .lock()?
            .add_thread(handle.thread().id(), handle)?;
        Ok(())
    }

    /// Accepts clients and processes them as log as a shutdown signal is not
    /// received from the [ServerController] corresponding to this server
    fn server_loop(
        self: Arc<Self>,
        shutdown_receiver: Receiver<()>,
        started_sender: Sender<()>,
    ) -> ServerResult<()> {
        let mut recv_result = shutdown_receiver.try_recv();
        let listener = TcpListener::bind(format!("{}:{}", self.config.ip(), self.config.port()))?;
        started_sender.send(())?;

        listener.set_nonblocking(true)?;
        while recv_result.is_err() {
            match self.accept_client(&listener) {
                Ok(connection_stream) => {
                    // TODO: Agregar network_connection.addr al mensaje de error
                    self.run_client(connection_stream)
                        .unwrap_or_else(|err| error!("Error - {}", err.to_string()));
                }
                Err(err) if err.kind() == ServerErrorKind::Idle => {
                    thread::sleep(ACCEPT_SLEEP_DUR);
                }
                Err(err) => {
                    logging::log::<&str>(LogKind::IncomingConnectionError(&err.to_string()));
                    break;
                }
            }
            recv_result = shutdown_receiver.try_recv();
        }
        logging::log::<&str>(LogKind::ServerShutdown);
        self.clients_manager.write()?.finish_all_sessions(false)?;
        Ok(())
    }

    /// Accepts a TCP connection and returns the stream corresponding to that
    /// connection.
    ///
    /// If no connection has been received, it returns an error of kind
    /// [ServerErrorKind::Idle]
    fn accept_client(
        self: &Arc<Self>,
        listener: &TcpListener,
    ) -> ServerResult<NetworkConnection<TcpStream, SocketAddr>> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(ServerError::new_kind("Idle", ServerErrorKind::Idle))
            }
            Err(error) => {
                logging::log::<&str>(LogKind::IncomingConnectionError(&error.to_string()));
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                logging::log(LogKind::AcceptedIncoming(&addr));
                stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                Ok(NetworkConnection::new(addr, stream))
            }
        }
    }
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> Option<Self>
    where
        Self: Sized,
    {
        if let Ok(clone) = TcpStream::try_clone(self) {
            Some(clone)
        } else {
            None
        }
    }
}

impl Close for TcpStream {
    fn close(&mut self) -> io::Result<()> {
        self.shutdown(Shutdown::Both)
    }
}
