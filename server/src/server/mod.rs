#![allow(dead_code, unused_variables)]

use core::panic;
use std::{
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
    vec,
};

use threadpool::ThreadPool;
use tracing::{debug, error, info, warn};

use packets::packet_reader::{ErrorKind, PacketError};

pub mod server_error;
pub use server_error::ServerError;

const MPSC_BUF_SIZE: usize = 256;
const SLEEP_DUR: Duration = Duration::from_secs(2);

const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);

use packets::publish::Publish;

use crate::{
    client::Client,
    config::Config,
    server::server_error::ServerErrorKind,
    server_packets::{Connack, Connect, Disconnect, Subscribe},
    session::Session,
    topic_handler::{Publisher, TopicHandler},
};

pub type ServerResult<T> = Result<T, ServerError>;

type ClientId = String;

const RETURN_CODE_CONNECTION_ACCEPTED: u8 = 0;

pub enum Packet {
    ConnectType(Connect),
    ConnackType(Connack),
    PublishTypee(Publish),
    SubscribeType(Subscribe),
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
    session: Session,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree
    topic_handler: TopicHandler,
    /// Vector with the handlers of the clients running in parallel
    client_handlers: Mutex<Vec<JoinHandle<()>>>,
    pool: Mutex<ThreadPool>,
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

impl Publisher for Server {
    fn send_publish(&self, id: &str, publish: &Publish) {
        self.session
            .client_do(&id.to_owned(), |client| {
                client.send_publish(publish);
            })
            .unwrap();
    }
}

impl Server {
    /// Creates a new Server
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor");
        Arc::new(Self {
            session: Session::new(),
            config,
            topic_handler: TopicHandler::new(),
            client_handlers: Mutex::new(vec![]),
            pool: Mutex::new(ThreadPool::new(threadpool_size)),
        })
    }

    fn read_packet(
        &self,
        control_byte: u8,
        stream: &mut TcpStream,
        id: &ClientId,
    ) -> ServerResult<Packet> {
        let buf: [u8; 1] = [control_byte];

        let code = control_byte >> 4;
        match get_code_type(code)? {
            PacketType::Connect => {
                let packet = Connect::new(stream)?;
                Ok(Packet::ConnectType(packet))
            }
            PacketType::Publish => {
                let packet = Publish::read_from(stream, control_byte).unwrap();
                Ok(Packet::PublishTypee(packet))
            }
            PacketType::Puback => todo!(),
            PacketType::Subscribe => {
                let packet = Subscribe::new(stream, &buf).unwrap();
                Ok(Packet::SubscribeType(packet))
            }
            PacketType::Unsubscribe => todo!(),
            PacketType::Pingreq => {
                todo!()
            }
            PacketType::Disconnect => {
                let packet = Disconnect::read_from(buf[0], stream).unwrap();
                Ok(Packet::DisconnectType(packet))
            }
            _ => Err(ServerError::new_kind(
                "Codigo de paquete inesperado",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    fn receive_packet(&self, stream: &mut TcpStream, id: &ClientId) -> Result<Packet, ServerError> {
        let mut buf = [0u8; 1];
        match stream.read_exact(&mut buf) {
            Ok(_) => Ok(self.read_packet(buf[0], stream, id)?),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                Err(ServerError::new_kind(
                    "Cliente se desconecto sin avisar",
                    ServerErrorKind::ClientDisconnected,
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(ServerError::new_kind("Would block", ServerErrorKind::Idle))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    fn handle_connect(&self, connect: Connect, id: &ClientId) -> Result<(), ServerError> {
        error!(
            "El cliente con id <{}> envio un segundo CONNECT. Se procede a su desconexion",
            id
        );
        self.session.disconnect(id, false)?;
        Ok(())
    }

    fn handle_publish(&self, publish: Publish, id: &ClientId) -> Result<(), ServerError> {
        debug!("Recibido PUBLISH de <{}>", id);
        self.topic_handler.publish(&publish, self).unwrap();
        Ok(())
    }

    fn handle_subscribe(&self, subscribe: Subscribe, id: &ClientId) -> Result<(), ServerError> {
        debug!("Recibido SUBSCRIBE de <{}>", id);
        self.topic_handler.subscribe(&subscribe, id).unwrap();
        Ok(())
    }

    fn handle_disconnect(&self, disconnect: Disconnect, id: &ClientId) -> ServerResult<()> {
        debug!("Recibido DISCONNECT de <{}>", id);
        self.session.disconnect(id, true).unwrap();
        Ok(())
    }

    pub fn handle_packet(&self, packet: Packet, id: &ClientId) -> ServerResult<()> {
        match packet {
            Packet::ConnectType(packet) => self.handle_connect(packet, &id),
            Packet::PublishTypee(packet) => self.handle_publish(packet, &id),
            Packet::SubscribeType(packet) => self.handle_subscribe(packet, &id),
            Packet::DisconnectType(packet) => {
                self.handle_disconnect(packet, &id).unwrap();
                Ok(())
            }
            _ => Err(ServerError::new_kind(
                "Paquete invalido",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    fn wait_for_connect(&self, stream: &mut TcpStream) -> Result<Client, ServerError> {
        loop {
            match Connect::new_from_zero(stream) {
                Ok(packet) => {
                    info!(
                        "Recibido CONNECT de cliente <{}>: {:?}",
                        packet.client_id(),
                        packet
                    );
                    let stream_copy = stream.try_clone()?;
                    return Ok(Client::new(packet, stream_copy));
                }
                Err(err) if err.kind() == ErrorKind::InvalidFlags => continue,
                Err(err) => return Err(ServerError::from(err)),
            }
        }
    }

    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<String> {
        info!("Conectando <{}>", addr);
        loop {
            match self.wait_for_connect(stream) {
                Ok(client) => {
                    let id = client.id().to_owned();
                    self.session.connect(client)?;
                    return Ok(id);
                }
                Err(err) if err.kind() == ServerErrorKind::Idle => continue,
                Err(err) => {
                    error!(
                        "Error recibiendo Connect de cliente <{}>: {}",
                        addr,
                        err.to_string()
                    );
                    return Err(ServerError::new_kind(
                        "Error de conexion",
                        ServerErrorKind::ProtocolViolation,
                    ));
                }
            }
        }
    }

    fn to_threadpool(self: &Arc<Self>, id: &ClientId, packet: Packet) {
        let sv_copy = self.clone();
        let id_copy = id.to_owned();
        self.pool
            .lock()
            .expect("Lock envenenado")
            .spawn(move || sv_copy.handle_packet(packet, &id_copy).unwrap())
            .unwrap()
    }

    fn client_loop(self: Arc<Self>, id: ClientId, mut stream: TcpStream) {
        // TODO: TcpStream timeout
        debug!("Entrando al loop de {}", id);
        while self.session.connected(&id) {
            match self.receive_packet(&mut stream, &id) {
                Ok(packet) => {
                    self.session.refresh(&id);
                    self.to_threadpool(&id, packet);
                }
                Err(err) if err.kind() == ServerErrorKind::ClientDisconnected => {
                    warn!("Cliente <{}> se desconecto sin avisar", id);
                    self.session.disconnect(&id, false).unwrap();
                }
                Err(err) => {
                    error!("Error inesperado: {}", err.to_string());
                    self
                }
            }
        }
        debug!("Conexion finalizada con <{}>", id);
        self.session.finish_session(&id);
    }

    fn manage_client(self: Arc<Self>, mut stream: TcpStream, addr: SocketAddr) {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ProtocolViolation => {}
                ServerErrorKind::RepeatedId => {
                    error!("Conexion {} con id, rechazada", addr);
                }
                _ => panic!("Error inesperado"),
            },
            Ok(id) => self.client_loop(id, stream),
        }
    }

    fn accept_client(self: Arc<Self>, listener: &TcpListener) -> Result<Arc<Server>, ServerError> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(self),
            Err(error) => {
                error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                info!("Aceptada conexion TCP con {}", addr);
                stream.set_read_timeout(Some(CONNECTION_WAIT_TIMEOUT))?;
                let sv_copy = self.clone();
                // No funcionan los nombres en el trace
                let handle = thread::Builder::new()
                    .name(addr.to_string())
                    .spawn(move || sv_copy.manage_client(stream, addr))
                    .expect("Error creando el thread");
                self.client_handlers.lock().unwrap().push(handle);
                Ok(self)
            }
        }
    }

    pub fn run(self: Arc<Self>) -> Result<(), ServerError> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.config.port()))?;
        let mut server = self;
        loop {
            server = server.accept_client(&listener)?;
        }
    }
}
