use core::panic;
use std::{
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
    vec,
};

use threadpool::ThreadPool;
use tracing::{debug, error, info};

use packets::{
    packet_reader::{ErrorKind, PacketError, QoSLevel},
    puback::Puback,
};

pub mod server_error;
pub use server_error::ServerError;

const CONNECTION_WAIT_TIMEOUT: Duration = Duration::from_secs(180);

use packets::publish::Publish;

use crate::{
    client::Client,
    config::Config,
    server::server_error::ServerErrorKind,
    server_packets::{Connect, Disconnect, PingReq, PingResp, Subscribe},
    session::Session,
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
    session: RwLock<Session>,
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

impl Server {
    /// Creates a new Server
    pub fn new(config: Config, threadpool_size: usize) -> Arc<Self> {
        info!("Iniciando servidor");
        Arc::new(Self {
            session: RwLock::new(Session::new()),
            config,
            topic_handler: TopicHandler::new(),
            client_handlers: Mutex::new(vec![]),
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
            self.session.read()?.send_publish(&id, message.packet)?;
        }
        Ok(())
    }

    fn handle_publish(self: &Arc<Self>, mut publish: Publish, id: &str) -> ServerResult<()> {
        info!("Recibido PUBLISH de <{}>", id);
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
            self.session
                .read()?
                .send_puback(id, &Puback::new(*packet_id)?)?;
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

    fn handle_subscribe(&self, subscribe: Subscribe, id: &str) -> ServerResult<()> {
        debug!("Recibido SUBSCRIBE de <{}>", id);
        self.topic_handler.subscribe(&subscribe, id)?;
        Ok(())
    }

    fn handle_disconnect(&self, _disconnect: Disconnect, id: &str) -> ServerResult<()> {
        debug!("Recibido DISCONNECT de <{}>", id);
        self.session.write()?.disconnect(id, true)?;
        Ok(())
    }

    fn handle_pingreq(&self, _pingreq: PingReq, id: &str) -> ServerResult<()> {
        debug!("Recibido PINGREQ de <{}>", id);
        self.session.read()?.client_do(id, |mut client| {
            client.write_all(&PingResp::new().encode())?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn handle_packet(self: &Arc<Self>, packet: Packet, id: &str) -> ServerResult<()> {
        match packet {
            Packet::PublishTypee(packet) => self.handle_publish(packet, id),
            Packet::SubscribeType(packet) => self.handle_subscribe(packet, id),
            Packet::PingReqType(packet) => self.handle_pingreq(packet, id),
            Packet::DisconnectType(packet) => self.handle_disconnect(packet, id),
            _ => Err(ServerError::new_kind(
                "Paquete invalido",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    fn wait_for_connect(&self, stream: &mut TcpStream) -> ServerResult<Client> {
        match Connect::new_from_zero(stream) {
            Ok(packet) => {
                info!(
                    "Recibido CONNECT de cliente <{}>: {:?}",
                    packet.client_id(),
                    packet
                );
                Ok(Client::new(packet, stream.try_clone()?))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    fn connect_client(&self, stream: &mut TcpStream, addr: SocketAddr) -> ServerResult<String> {
        info!("Conectando <{}>", addr);
        match self.wait_for_connect(stream) {
            Ok(client) => {
                let id = client.id().to_owned();
                let keep_alive = client.keep_alive();
                if keep_alive == 0 {
                    stream.set_read_timeout(None)?;
                } else {
                    stream.set_read_timeout(Some(Duration::from_secs(keep_alive as u64))).unwrap();
                }
                self.session.write()?.connect(client)?;
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
            .spawn(move || sv_copy.handle_packet(packet, &id_copy).unwrap())?;
        Ok(())
    }

    fn client_loop(self: Arc<Self>, id: String, mut stream: TcpStream) -> ServerResult<()> {
        debug!("Entrando al loop de {}", id);
        let mut timeout_counter = 0;
        let client_keep_alive = self.session.read()?.keep_alive(&id)?;

        loop {
            match self.receive_packet(&mut stream) {
                Ok(packet) => {
                    timeout_counter = 0;
                    self.to_threadpool(&id, packet)?;
                }
                Err(err) if err.kind() == ServerErrorKind::Timeout => {
                    timeout_counter += 1;
                    self.session.read()?.send_unacknowledged(&id)?;
                }
                Err(err) => {
                    error!("<{}>: {}", id, err.to_string());
                    self.session.write()?.disconnect(&id, false)?;
                    break;
                }
            }
            if timeout_counter > client_keep_alive {
                error!("<{}>: Keep Alive timeout", id);
                break;
            }
        }
        debug!("Conexion finalizada con <{}>", id);
        self.session.write()?.finish_session(&id)
    }

    fn manage_client(self: Arc<Self>, mut stream: TcpStream, addr: SocketAddr) -> ServerResult<()> {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ProtocolViolation => Err(err),
                _ => panic!("Error inesperado: {}", err.to_string()),
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
                    .spawn(move || sv_copy.manage_client(stream, addr).unwrap())
                    .expect("Error creando el thread");
                self.client_handlers.lock().unwrap().push(handle);
                Ok(self)
            }
        }
    }

    pub fn run(self: Arc<Self>) -> ServerResult<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.config.port()))?;
        let mut server = self;
        loop {
            server = server.accept_client(&listener)?;
        }
    }
}
