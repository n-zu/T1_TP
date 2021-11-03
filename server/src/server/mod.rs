#![allow(dead_code, unused_variables)]

use core::panic;
use std::{
    collections::HashMap,
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
    vec,
};

use tracing::{error, info, warn};

use packets::packet_reader::{ErrorKind, PacketError};

mod server_error;
use server_error::ServerError;

const MPSC_BUF_SIZE: usize = 256;
const SLEEP_DUR: Duration = Duration::from_secs(2);

use packets::publish::Publish;

use crate::{client::Client, config::Config, packet_scheduler::PacketScheduler, server::server_error::ServerErrorKind, server_packets::{Connack, Connect, Disconnect, Subscribe, connack::CONNACK_CONNECTION_ACCEPTED}, topic_handler::{Publisher, TopicHandler}};

pub type ServerResult<T> = Result<T, ServerError>;

const RETURN_CODE_CONNECTION_ACCEPTED: u8 = 0;

pub enum Packet {
    ConnectType(Connect),
    ConnackType(Connack),
    PublishTypee(Publish),
    SubscribeType(Subscribe),
    DisconnectType(Disconnect)
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
    clients: RwLock<HashMap<String, Mutex<Client>>>,
    /// Initial Server setup
    config: Config,
    /// Manages the Publish / Subscribe tree
    topic_handler: TopicHandler,
    /// Vector with the handlers of the clients running in parallel
    client_handlers: Mutex<Vec<JoinHandle<()>>>,
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
    fn send_publish(&self, user_id: &str, publish: &Publish) {
        self.clients
            .read()
            .unwrap()
            .get(user_id)
            .unwrap()
            .lock()
            .unwrap()
            .write_all(&publish.encode().unwrap())
            .unwrap();
    }
}

impl Server {
    /// Creates a new Server
    pub fn new(config: Config) -> Arc<Self> {
        info!("Iniciando servidor");
        Arc::new(Self {
            clients: RwLock::new(HashMap::new()),
            config,
            topic_handler: TopicHandler::new(),
            client_handlers: Mutex::new(vec![]),
        })
    }

    fn read_packet(&self, control_byte: u8, stream: &mut TcpStream, client_id: &str) -> Result<Packet, ServerError> {
        let buf: [u8; 1] = [control_byte];

        let code = control_byte >> 4;
        match get_code_type(code)? {
            PacketType::Connect => {
                let packet = Connect::new(stream)?;
                Ok(Packet::ConnectType(packet))
            }
            PacketType::Publish => {
                let packet = Publish::read_from(stream, &buf).unwrap();
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
            },
            PacketType::Disconnect => {
                let packet = Disconnect::read_from(buf[0], stream).unwrap();
                Ok(Packet::DisconnectType(packet))
            },
            _ => Err(ServerError::new_kind(
                "Codigo de paquete inesperado",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    fn receive_packet(&self, stream: &mut TcpStream, client_id: &str) -> Result<Packet, ServerError> {
        let mut buf = [0u8; 1];
        match stream.read_exact(&mut buf) {
            Ok(_) => Ok(self.read_packet(buf[0], stream, client_id)?),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                error!("Cliente se desconecto de forma inesperada");
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

    fn handle_connect(&self, connect: Connect, client_id: &str) -> Result<(), ServerError> {
        // Como la conexion se maneja antes de entrar al loop de paquetes
        // si se llega a este punto es porque se mando un segundo connect
        // Por lo tanto, se debe desconectar al cliente
        error!(
            "El cliente con id {} envio un segundo connect. Se procede a su desconexion",
            client_id
        );
        self.disconnect(client_id)?;
        Ok(())
    }

    fn handle_publish(&self, publish: Publish, client_id: &str) -> Result<(), ServerError> {
        //self.get_client(client_id)?.unacknowledged(publish);

        self.topic_handler.publish(&publish, self).unwrap();
        Ok(())
    }

    fn handle_subscribe(&self, subscribe: Subscribe, client_id: &str) -> Result<(), ServerError> {
        info!("Recibido subscribe de <{}>", client_id);
        self.topic_handler.subscribe(&subscribe, client_id).unwrap();
        Ok(())
    }

    fn handle_disconnect(&self, disconnect: Disconnect, client_id: &str) -> ServerResult<()> {
        info!("DISCONNECT");
        self.client_do(client_id, |client| {
            client.disconnect();
            info!("desconectado");
        }).unwrap();
        info!("sali del client_do");
        Ok(())
    }

    pub fn handle_packet(&self, packet: Packet, client_id: String) -> ServerResult<()> {
        match packet {
            Packet::ConnectType(packet) => self.handle_connect(packet, &client_id),
            Packet::PublishTypee(packet) => self.handle_publish(packet, &client_id),
            Packet::SubscribeType(packet) => self.handle_subscribe(packet, &client_id),
            Packet::DisconnectType(packet) => {
                self.handle_disconnect(packet, &client_id).unwrap();
                info!("sali de handle_disconnect");
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
                    info!("Recibido CONNECT de cliente {}", packet.client_id());
                    let stream_copy = stream.try_clone()?;
                    return Ok(Client::new(packet, stream_copy))
                }
                Err(err) if err.kind() == ErrorKind::InvalidFlags => continue,
                Err(err) => {
                    return Err(ServerError::from(err))
                }
            }
        }
    }

    fn is_present(&self, client_id: &str) -> bool {
        self.clients.read().unwrap().get(client_id).is_some()
    }

    fn add_client(&self, client: Client) {
        self.clients.write().unwrap().insert(client.id().to_owned(), Mutex::new(client));
    }

    fn new_client(&self, client: Client) -> ServerResult<()> {
        info!("new_client: {}", client.id());
        if self.is_present(client.id()) {
            info!("Cliente <{}> se encuentra en el servidor", client.id());
            if self.is_alive(client.id()).unwrap()  {
                //error!("Se intento conectar <{}> pero ya habia una conexion con la misma id", client.id());
                return Err(ServerError::new_kind(
                    "Cliente con la misma id se encuentra conectado", 
                    ServerErrorKind::RepeatedId));
            } else {
                if client.clean_session() {
                    info!("eliminando");

                    self.remove_client(client.id());
                    let client_id = client.id().to_owned();
                    self.remove_client(&client_id);
                    self.add_client(client);
                    self.send_connack(&client_id, 0, CONNACK_CONNECTION_ACCEPTED)?;
                    return Ok(())
                } else {
                    info!("Se reconecto <{}> (clean_session = false)", client.id());
                    let mut result: ServerResult<()> = Ok(());
                    let client_id = client.id().to_owned();
                    self.client_do(&client_id, |mut old_client| {
                        result = old_client.reconnect(client);
                    }).unwrap();
                    self.send_connack(&client_id, 1, CONNACK_CONNECTION_ACCEPTED)?;
                    result
                }
            }
        } else {
            info!("Primera aparicion en el SV");
            let client_id = client.id().to_owned();
            self.add_client(client);
            self.send_connack(&client_id, 0, CONNACK_CONNECTION_ACCEPTED).unwrap();

            Ok(())
        }
    }

    fn send_connack(&self, client_id: &str, session_present: u8, return_code: u8) -> Result<(), ServerError> {
        let connack = Connack::new(session_present, return_code);
        self.client_do(client_id, |mut client| {
            client.write_all(&connack.encode()).unwrap();
        })
    }

    fn is_alive(&self, client_id: &str) -> ServerResult<bool> {
        let mut alive = false;
        self.client_do(client_id, |client| {
            alive = client.alive()
        }).unwrap();
        Ok(alive)
    }

    fn disconnect(&self, client_id: &str) -> Result<(), ServerError> {
        self.client_do(client_id, |client| {
            client.disconnect()
        })?;
        Ok(())
    }

    fn client_do<F>(&self, client_id: &str, action: F) -> Result<(), ServerError>
    where F: FnOnce(std::sync::MutexGuard<'_, Client, >) -> () {
        action(self.clients
            .read()
            .expect("Lock envenenado")
            .get(client_id)
            .unwrap()
            .lock()
            .expect("Lock envenenado"));
        Ok(())
    }

    fn remove_client(&self, client_id: &str) {
        info!("CLIENTE A PUNTO DE SER ELIMINADO");
        self.clients.write().unwrap().remove(client_id).unwrap();
        info!("CLIENTE ELIMINADO");
    }

    fn connect_client(
        &self,
        stream: &mut TcpStream,
        addr: SocketAddr,
    ) -> ServerResult<String> {
        loop {
            match self.wait_for_connect(stream) {
                Ok(client) => {
                    let client_id = client.id().to_owned();
                    self.new_client(client)?;
                    return Ok(client_id)
                }
                Err(err) if err.kind() == ServerErrorKind::Idle => {
                    continue
                },
                Err(err) => {
                    error!(
                        "Error recibiendo Connect de cliente <{}>: {}",
                        addr,
                        err.to_string()
                    );
                    return Err(ServerError::new_kind(
                        "Error de conexion",
                        ServerErrorKind::ProtocolViolation,
                    ))
                }
            }
        }
    }

    fn client_loop(self: Arc<Self>, client_id: String, mut stream: TcpStream) {
        info!("Entrando al loop de {}", client_id);
        let mut packet_scheduler = PacketScheduler::new(self.clone(), &client_id);
        while self.is_alive(&client_id).unwrap() {
            match self.receive_packet(&mut stream, &client_id) {
                Ok(packet) => {
                    packet_scheduler.new_packet(packet);
                }
                Err(err) if err.kind() == ServerErrorKind::Idle => {
                    continue;
                }
                Err(err) => {
                    error!("Error grave: {}", err.to_string());
                }
            }
        }
        info!("llegue hasta aca");
        self.client_do(&client_id, |client| {
            warn!("sali del loop");
            if client.clean_session() {
                info!("a removerlo");
                self.remove_client(&client_id);
            }
        }).unwrap();
        info!("Conexion finalizada con <{}>", client_id);

        // Implementar Drop para PacketScheduler
    }

    fn manage_client(self: Arc<Self>, mut stream: TcpStream, addr: SocketAddr) {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ProtocolViolation => {}
                ServerErrorKind::RepeatedId => {
                    //info!("Id repetida, conexion rechazada");
                },
                _ => panic!("Error inesperado")
            },
            Ok(client_id) => self.client_loop(client_id, stream),
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
                // En la implementacion original habia un set_nonblocking, para que lo precisamos?
                stream.set_nonblocking(true).unwrap();
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
