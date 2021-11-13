#![allow(dead_code)]

use core::panic;
use std::{
    collections::HashMap,
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use threadpool::ThreadPool;
use tracing::{debug, error, info, warn};

use packets::{
    packet_reader::{ErrorKind, PacketError, QoSLevel},
    puback::Puback,
};

pub mod server_error;
pub use server_error::ServerError;

const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(1);
const ACCEPT_SLEEP_DUR: Duration = Duration::from_millis(100);

use packets::publish::Publish;

use crate::{
    client::Client,
    clients_manager::ClientsManager,
    config::Config,
    server::server_error::ServerErrorKind,
    server_packets::{Connect, Disconnect, PingReq, Subscribe},
    topic_handler::{Message, TopicHandler},
};

pub type ServerResult<T> = Result<T, ServerError>;

pub enum Packet {
    PublishTypee(Publish),
    PubackType(Puback),
    SubscribeType(Subscribe),
    PingReqType(PingReq),
    DisconnectType(Disconnect),
}

pub enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Pingreq,
    Pingresp,
    Disconnect,
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
    client_join_handles: Mutex<HashMap<SocketAddr, JoinHandle<()>>>,
    pool: Mutex<ThreadPool>,
}

pub struct ServerController {
    shutdown_sender: Sender<()>,
    handle: JoinHandle<()>,
}

impl ServerController {
    fn new(shutdown_sender: Sender<()>, handle: JoinHandle<()>) -> ServerController {
        ServerController {
            shutdown_sender,
            handle,
        }
    }

    pub fn shutdown(self) -> ServerResult<()> {
        self.shutdown_sender.send(())?;
        match self.handle.join() {
            Ok(()) => debug!("El thread del servidor fue joineado normalmente (sin panic)"),
            Err(err) => debug!("El thread del servidor fue joineado con panic: {:?}", err),
        }
        Ok(())
    }
}

// Temporal
fn get_code_type(code: u8) -> Result<PacketType, PacketError> {
    match code {
        1 => Ok(PacketType::Connect),
        2 => Ok(PacketType::Connack),
        3 => Ok(PacketType::Publish),
        4 => Ok(PacketType::Puback),
        8 => Ok(PacketType::Subscribe),
        9 => Ok(PacketType::Suback),
        10 => Ok(PacketType::Unsubscribe),
        11 => Ok(PacketType::Unsuback),
        12 => Ok(PacketType::Pingreq),
        13 => Ok(PacketType::Pingresp),
        14 => Ok(PacketType::Disconnect),
        _ => Err(PacketError::new_kind(
            "Tipo de paquete invalido/no soportado",
            ErrorKind::InvalidControlPacketType,
        )),
    }
}

impl Server {
    /// Creates a new Server
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor");
        Arc::new(Self {
            clients_manager: RwLock::new(ClientsManager::new(config.accounts_path())),
            config,
            topic_handler: TopicHandler::new(),
            client_join_handles: Mutex::new(HashMap::new()),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
        })
    }

    fn read_packet(&self, control_byte: u8, stream: &mut TcpStream) -> ServerResult<Packet> {
        let buf: [u8; 1] = [control_byte];

        let code = control_byte >> 4;
        match get_code_type(code)? {
            PacketType::Publish => {
                let packet = Publish::read_from(stream, control_byte)?;
                Ok(Packet::PublishTypee(packet))
            }
            PacketType::Puback => {
                let packet = Puback::read_from(stream, control_byte)?;
                Ok(Packet::PubackType(packet))
            }
            PacketType::Subscribe => {
                let packet = Subscribe::new(stream, &buf)?;
                Ok(Packet::SubscribeType(packet))
            }
            PacketType::Unsubscribe => todo!(),
            PacketType::Pingreq => {
                let packet = PingReq::read_from(stream, control_byte)?;
                Ok(Packet::PingReqType(packet))
            }
            PacketType::Disconnect => {
                let packet = Disconnect::read_from(buf[0], stream)?;
                Ok(Packet::DisconnectType(packet))
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

    fn handle_publish(self: &Arc<Self>, mut publish: Publish, id: &str) -> ServerResult<()> {
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
                client.send_puback(Puback::new(*packet_id)?)?;
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

    fn handle_subscribe(&self, mut subscribe: Subscribe, id: &str) -> ServerResult<()> {
        debug!("<{}> Recibido SUBSCRIBE", id);
        subscribe.set_max_qos(QoSLevel::QoSLevel1);
        self.topic_handler.subscribe(&subscribe, id)?;
        self.clients_manager
            .read()?
            .client_do(id, |mut client| client.send_suback(subscribe.response()?))?;
        Ok(())
    }

    pub fn handle_packet(self: &Arc<Self>, packet: Packet, id: &str) -> ServerResult<()> {
        match packet {
            Packet::PublishTypee(packet) => self.handle_publish(packet, id),
            Packet::SubscribeType(subscribe) => self.handle_subscribe(subscribe, id),
            packet => self
                .clients_manager
                .read()?
                .client_do(id, |mut client| client.handle_packet(packet)),
        }
    }

    fn wait_for_connect(&self, stream: &mut TcpStream) -> ServerResult<Client> {
        match Connect::new_from_zero(stream) {
            Ok(packet) => {
                info!("<{}>: Recibido CONNECT - {:?}", packet.client_id(), packet);
                Ok(Client::new(packet, stream.try_clone()?))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<String> {
        info!("<{}>: Conectando", addr);
        match self.wait_for_connect(stream) {
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
                error!("<{}> timeout esperando CONNECT", addr);
                Err(err)
            }
            Err(err) => {
                error!(
                    "Error recibiendo Connect de cliente <{}>: {}",
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

    fn to_threadpool(self: &Arc<Self>, id: &str, packet: Packet) -> ServerResult<()> {
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

    fn client_loop(self: Arc<Self>, id: String, mut stream: TcpStream) -> ServerResult<()> {
        debug!("<{}> Entrando al loop", id);
        let mut timeout_counter = 0;
        let client_keep_alive = self.clients_manager.read()?.keep_alive(&id)?;

        while self.clients_manager.read()?.is_connected(&id)? {
            match self.receive_packet(&mut stream) {
                Ok(packet) => {
                    timeout_counter = 0;
                    if let Packet::DisconnectType(_disconnect) = packet {
                        self.clients_manager.write()?.finish_all_sessions(true)?;
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
        debug!("Conexion finalizada con <{}>", id);
        Ok(())
    }

    fn manage_client(
        self: Arc<Self>,
        mut stream: TcpStream,
        addr: SocketAddr,
        finished_thread_sender: Sender<SocketAddr>,
    ) -> ServerResult<()> {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ClientNotInWhitelist => {
                    warn!("Addr {} - Intento de conexion con usuario invalido", addr);
                    finished_thread_sender.send(addr)?;
                }
                ServerErrorKind::InvalidPassword => {
                    warn!(
                        "Addr {} - Intento de conexion con contraseña invalido",
                        addr
                    );
                    finished_thread_sender.send(addr)?;
                }
                ServerErrorKind::ProtocolViolation => {
                    error!(
                        "Addr {} - Violacion de protocolo: {}",
                        addr,
                        err.to_string()
                    );
                    finished_thread_sender.send(addr)?;
                    return Err(err);
                }
                _ => panic!("Error inesperado: {}", err.to_string()),
            },
            Ok(id) => {
                self.client_loop(id, stream)?;
                finished_thread_sender.send(addr)?;
            }
        }
        Ok(())
    }

    fn accept_client(
        self: &Arc<Self>,
        listener: &TcpListener,
        finished_thread_sender: Sender<SocketAddr>,
    ) -> ServerResult<()> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_SLEEP_DUR);
                Ok(())
            }
            Err(error) => {
                error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                info!("Aceptada conexion TCP con {}", addr);
                stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                let sv_copy = self.clone();
                let handle = thread::spawn(move || {
                    match sv_copy.manage_client(stream, addr, finished_thread_sender) {
                        Ok(()) => debug!("Cliente con Address {} procesado con exito", addr),
                        Err(err) => error!(
                            "Error procesando al cliente con Address {}: {}",
                            addr,
                            err.to_string()
                        ),
                    }
                });
                self.client_join_handles.lock()?.insert(addr, handle);
                Ok(())
            }
        }
    }

    fn join_finished_threads(
        &self,
        finished_thread_receiver: &Receiver<SocketAddr>,
    ) -> ServerResult<()> {
        while let Ok(finished_addr) = finished_thread_receiver.try_recv() {
            debug!("Iniciando join de thread con Addr {}", finished_addr);
            let handle = self
                .client_join_handles
                .lock()?
                .remove(&finished_addr)
                .unwrap_or_else(|| {
                    panic!(
                        "No se encontro el handle con address {} para joinear",
                        finished_addr
                    )
                });
            match handle.join() {
                Ok(()) => debug!(
                    "El thread con Addr {} finalizo normalmente (sin panic) y fue joineado",
                    finished_addr
                ),
                Err(_) => debug!(
                    "El thread con Addr {} finalizo con panic y fue joineado",
                    finished_addr
                ),
            }
        }
        Ok(())
    }

    fn server_loop(
        self: Arc<Self>,
        listener: TcpListener,
        shutdown_receiver: Receiver<()>,
    ) -> ServerResult<()> {
        let (finished_thread_sender, finished_thread_receiver) = channel();
        let mut recv_result = shutdown_receiver.try_recv();
        while recv_result.is_err() {
            self.accept_client(&listener, finished_thread_sender.clone())?;
            self.join_finished_threads(&finished_thread_receiver)?;
            recv_result = shutdown_receiver.try_recv();
        }
        debug!("Apagando Servidor");
        self.clients_manager.write()?.finish_all_sessions(false)
    }

    pub fn run(self: Arc<Self>) -> ServerResult<ServerController> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.config.port()))?;
        listener
            .set_nonblocking(true)
            .expect("No se pudo setear el Listener de forma no bloqueante");
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
