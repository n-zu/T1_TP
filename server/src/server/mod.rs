#![allow(dead_code)]

use std::{convert::TryFrom, io::{self, Read, Write}, net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs}, sync::{
        mpsc::{self, Receiver},
        Arc, Mutex, RwLock,
    }, thread::{self, JoinHandle, ThreadId}, time::Duration};

use threadpool::ThreadPool;
use tracing::{debug, error, info};

use packets::{connack::Connack, connect::Connect, disconnect::Disconnect, traits::{MQTTDecoding, MQTTEncoding}};
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

use crate::{client_thread_joiner::ClientThreadJoiner, clients_manager::{ClientsManager, ConnectInfo, DisconnectInfo}, config::Config, connection_stream::ConnectionStream, logging::{self, LogKind}, server::server_error::ServerErrorKind, topic_handler::{Message, TopicHandler}, traits::{BidirectionalStream, ClientAccepter, Id, StreamError}};

pub use self::server_controller::ServerController;

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
pub struct Server<S, A>
where
    S: BidirectionalStream,
    A: ClientAccepter<S> + Sized + Send + Sync + 'static,
{
    /// Clients connected to the server.
    ///
    /// It handles the connection and disconnection of clients,
    /// as well as credential verification.
    ///
    /// If a client has a clean connection_stream set to False, it is
    /// responsible for saving their information even after
    /// disconnection. If it has clean connection_stream set to True,
    /// their data is deleted
    clients_manager: RwLock<ClientsManager<S, ThreadId>>,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree.
    ///
    /// When a customer subscribes to a topic or publish a message,
    /// all the information is stored in this handler. This includes
    /// Quality of Service and handling of retained messages.
    /// When it processes a Publish, the TopicHandler indicates to
    /// the Server the clients to whom it should send it through a
    /// MPSC channel. An independent thread receives the information
    /// and sends the packets.
    ///
    /// The TopicHandler is not responsible for keeping the subscriptions
    /// of users connected with clean connection_stream set to True. The
    /// server is responsible to invoke remove_client() when the client
    /// has clean connection_stream set to True.
    topic_handler: TopicHandler,
    /// Manages the join of the threads that are created for each of the
    /// clients. It does not interfere in the threads of the threadpool
    /// or the Server thread.
    ///
    /// When a new client thread is created, it saves its JoinHandle
    /// assosiationg it with the SockerAddr of the client that just
    /// connected.
    client_thread_joiner: Mutex<ClientThreadJoiner>,
    /// Threadpool used to process packets received from clients
    /// The only ones that are not processed in the Threadpool
    /// are the Connect and Disconnect packets.
    pool: Mutex<ThreadPool>,

    client_accepter: A,
}

pub trait ServerInterface {
    fn new(config: Config, threadpool_size: usize) -> Arc<Self>;
    fn run(self: Arc<Self>) -> io::Result<ServerController>;
}

impl<S, A> Server<S, A>
where
    S: BidirectionalStream,
    A: ClientAccepter<S> + Sized + Send + Sync + 'static,
{
    /// Creates and returns a server in a valid state
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor en {}:{}", config.ip(), config.port());
        let client_accepter = A::new(config.ip(), config.port()).unwrap();
        Arc::new(Self {
            clients_manager: RwLock::new(ClientsManager::new(config.accounts_path())),
            config,
            topic_handler: TopicHandler::new(),
            client_thread_joiner: Mutex::new(ClientThreadJoiner::new()),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
            client_accepter,
        })
    }

    /// Run the server in a new thread.
    ///
    /// Returns a ServerController that can be used to stop the server
    pub fn run(self: Arc<Self>) -> io::Result<ServerController> {
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let server_handle = thread::spawn(move || {
            if let Err(err) = self.server_loop(shutdown_receiver) {
                error!(
                    "Error inesperado del servidor: {} - Se recomienda apagarlo",
                    err.to_string()
                )
            }
        });
        let server_controller = ServerController::new(shutdown_sender, server_handle);
        Ok(server_controller)
    }

    /// Receives the [Connect] packet from a client, connects it to the
    /// server and sets its connection_stream read Timeout with the Keep Alive Timeout
    /// provided by the client in the [Connect] packet.
    /// *connection_stream* is the connection_stream of the client from which the packet is received
    ///
    /// It does not send the [Connack] packet.
    ///
    /// If the connection was successful, it returns [Ok(ConnectionInfo)] with
    /// the necessary information to send the Connack, as well as the LastWill
    /// [Publish] packet from the previous connection_stream, in case a Client Take-Over
    /// type reconnection has ocurred.
    ///
    /// In case an error has ocurred, but a Connack must be sent (for example,
    /// if the credentials are invalid), it returns an Error with kind
    /// [ServerErrorKind::ConnectionRefused], with the return code that the Connack
    /// must contain. If the error it returns is not of that kind, a Connack should
    /// not be send.
    fn connect_client(
        self: &Arc<Self>,
        connection_stream: &mut ConnectionStream<S, ThreadId>,
    ) -> ServerResult<ConnectInfo> {
        info!("<{:?}>: Conectando", connection_stream.id());
        match self.wait_for_connect(connection_stream) {
            Ok(connect) => {
                connection_stream
                    .stream_mut()
                    .change_read_timeout(Some(CLIENT_READ_TIMEOUT))?;
                let connect_info = self
                    .clients_manager
                    .write()?
                    .new_session(connection_stream, connect)?;
                Ok(connect_info)
            }
            Err(err) => {
                return Err(ServerError::new_kind(
                    &format!(
                        "Error recibiendo CONNECT de cliente <{:?}>: {}",
                        connection_stream.id(),
                        err.to_string()
                    ),
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
        mut connection_stream: ConnectionStream<S, ThreadId>,
    ) -> ServerResult<DisconnectInfo> {
        let mut timeout_counter = 0;
        let keep_alive = self
            .clients_manager
            .read()?
            .get_client_property(id, |client| Ok(client.keep_alive()))?;
        let keep_alive = (1.5 * keep_alive as f32) as u32;

        loop {
            match self.process_packet(&mut connection_stream, id) {
                Ok(packet_type) => {
                    timeout_counter = 0;
                    self.log_processed_packet(packet_type, id);
                    if packet_type == PacketType::Disconnect {
                        return self.clients_manager.write()?.finish_session(
                            id,
                            connection_stream,
                            true,
                        );
                    }
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    timeout_counter += 1;
                    self.clients_manager.read()?.send_unacknowledged(id)?;
                }
                Err(err) => {
                    error!("<{}>: {}", id, err.to_string());
                    return self.clients_manager.write()?.finish_session(
                        id,
                        connection_stream,
                        false,
                    );
                }
            }
            if keep_alive != 0 && timeout_counter > keep_alive {
                error!("<{}>: Keep Alive timeout", id);
                return self
                    .clients_manager
                    .write()?
                    .finish_session(id, connection_stream, false);
            }
        }
    }

    /// It processes a client from when it is accepted by the server
    /// (that is, it has not yet send the Connect packet) until it is
    /// disconnected. Cleans all the client information in case the
    /// client connects with *clean_session* set to True
    fn manage_client(
        self: &Arc<Self>,
        mut connection_stream: ConnectionStream<S, ThreadId>,
    ) -> ServerResult<ClientId> {
        match self.connect_client(&mut connection_stream) {
            Err(err) => match err.kind() {
                ServerErrorKind::ConnectionRefused(return_code) => {
                    error!(
                        "<{:?}>: Error de conexion: {}",
                        connection_stream.id(),
                        err.to_string()
                    );
                    connection_stream.write_all(&Connack::new(false, return_code).encode()?)?;
                    Err(err)
                }
                _ => {
                    error!("Error inesperado: {}", err.to_string());
                    Err(err)
                }
            },
            Ok(connect_info) => {
                info!(
                    "<{:?}> -> <{}>: Conectado",
                    connection_stream.id(),
                    connect_info.id
                );
                connection_stream.write_all(
                    &Connack::new(connect_info.session_present, connect_info.return_code)
                        .encode()?,
                )?;
                if let Some(last_will) = connect_info.takeover_last_will {
                    self.send_last_will(last_will, &connect_info.id)?;
                }

                // En caso de que haya ocurrido una reconexion y el cliente
                // tenia un last will, se publica
                let disconnect_info = self.client_loop(&connect_info.id, connection_stream)?;
                if let Some(last_will) = disconnect_info.publish_last_will {
                    self.send_last_will(last_will, &connect_info.id)?;
                }
                if disconnect_info.clean_session {
                    self.topic_handler.remove_client(&connect_info.id)?;
                }
                Ok(connect_info.id)
            }
        }
    }

    /// Creates a new thread in which the client will be handled. Adds that
    /// thread to the list of threads pending to be joined
    fn run_client(self: &Arc<Self>, stream: S) -> ServerResult<()> {
        let sv_copy = self.clone();
        let handle = thread::spawn(move || {
            let connection_stream = ConnectionStream::new(thread::current().id(), stream);
            let connection_stream_id = connection_stream.id();
            match sv_copy.manage_client(connection_stream) {
                Ok(id) => debug!(
                    "<{}>: Procesado con exito",
                    id
                ),
                Err(err) => error!(
                    "Error procesando al cliente en thread <{:?}>: {}",
                    connection_stream_id,
                    err.to_string()
                ),
            }
            sv_copy
                .client_thread_joiner
                .lock()
                .expect("Lock envenenado")
                .finished(thread::current().id())
                .unwrap_or_else(|err| panic!("Error irrecuperable: {}", err.to_string()));
        });
        self.client_thread_joiner
            .lock()?
            .add_thread(handle.thread().id(), handle)?;
        Ok(())
    }

    /// Accepts clients and processes them as log as a shutdown signal is not
    /// received from the [ServerController] corresponding to this server
    fn server_loop(self: Arc<Self>, shutdown_receiver: Receiver<()>) -> ServerResult<()> {
        let mut recv_result = shutdown_receiver.try_recv();
        while recv_result.is_err() {
            match self.accept_client() {
                Ok(stream) => {
                    // TODO: Agregar connection_stream.addr al mensaje de error
                    self.run_client(stream)
                        .unwrap_or_else(|err| error!("Error - {}", err.to_string()));
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

    /// Accepts a TCP connection and returns the stream corresponding to that
    /// connection.
    ///
    /// If no connection has been received, it returns an error of kind
    /// [ServerErrorKind::Idle]
    fn accept_client(self: &Arc<Self>) -> ServerResult<S> {
        match self.client_accepter.accept_client() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(ServerError::new_kind("Idle", ServerErrorKind::Idle))
            }
            Err(error) => {
                error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok(mut stream) => {
                info!("Aceptada conexion TCP");
                stream.change_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                Ok(stream)
            }
        }
    }
}

impl BidirectionalStream for TcpStream {
    fn try_clone(&self) -> Result<Self, StreamError>
    where
        Self: Sized,
    {
        if let Ok(copy) = self.try_clone() {
            Ok(copy)
        } else {
            Err(StreamError {})
        }
    }

    fn close(&mut self) -> io::Result<()> {
        self.shutdown(Shutdown::Both)
    }

    fn change_read_timeout(&mut self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }

    fn change_write_timeout(&mut self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }
}

impl ClientAccepter<TcpStream> for TcpListener {
    fn new(ip: &str, port: u16) -> io::Result<Self>
    where
        Self: Sized,
    {
        let listener = Self::bind(format!("{}:{}", ip, port))?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    fn accept_client(&self) -> io::Result<TcpStream> {
        let (stream, _addr) = self.accept()?;
        Ok(stream)
    }
}
