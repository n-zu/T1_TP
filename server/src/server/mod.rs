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

use crate::{
    client::Client,
    config::Config,
    packet_scheduler::PacketScheduler,
    server::server_error::ServerErrorKind,
    server_packets::{Connack, Connect, Subscribe},
    topic_handler::{Publisher, TopicHandler},
};

pub enum Packet {
    ConnectType(Connect),
    ConnackType(Connack),
    PublishTypee(Publish),
    SubscribeType(Subscribe),
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
        info!("Inicializando servidor");
        Arc::new(Self {
            clients: RwLock::new(HashMap::new()),
            config,
            topic_handler: TopicHandler::new(),
            client_handlers: Mutex::new(vec![]),
        })
    }

    /// Server start listening to connections
    pub fn run(self: Arc<Self>) -> Result<(), ServerError> {
        info!("Escuchando conexiones");
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.config.port()))?;
        loop {
            self.accept_client(&listener)?;
        }
    }

    /// Handles packets depending on packet type
    pub fn handle_packet(&self, packet: Packet, client_id: String) -> Result<(), ServerError> {
        match packet {
            Packet::ConnectType(packet) => self.handle_connect(packet, &client_id),
            Packet::PublishTypee(packet) => self.handle_publish(packet, &client_id),
            Packet::SubscribeType(packet) => self.handle_subscribe(packet, &client_id),
            _ => Err(ServerError::new_kind(
                "Invalid packet",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    #[doc(hidden)]
    fn read_packet(&self, control_byte: u8, stream: &mut TcpStream) -> Result<Packet, ServerError> {
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
            PacketType::Pingreq => todo!(),
            PacketType::Disconnect => todo!(),
            _ => Err(ServerError::new_kind(
                "Codigo de paquete inesperado",
                ServerErrorKind::ProtocolViolation,
            )),
        }
    }

    #[doc(hidden)]
    fn receive_packet(&self, stream: &mut TcpStream) -> Result<Packet, ServerError> {
        let mut buf = [0u8; 1];
        match stream.read_exact(&mut buf) {
            Ok(_) => Ok(self.read_packet(buf[0], stream)?),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                error!("Cliente se desconecto de forma inesperada");
                Err(ServerError::new_kind(
                    "Cliente se desconecto sin avisar",
                    ServerErrorKind::ClientDisconnected,
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                warn!("Error WouldBlock");
                Err(ServerError::new_msg(&error.to_string()))
            }
            Err(err) => Err(ServerError::from(err)),
        }
    }

    #[doc(hidden)]
    fn handle_connect(&self, connect: Connect, client_id: &str) -> Result<(), ServerError> {
        // Como la conexion se maneja antes de entrar al loop de paquetes
        // si se llega a este punto es porque se mando un segundo connect
        // Por lo tanto, se debe desconectar al cliente
        error!(
            "El cliente con id {} envio un segundo connect. Se procede a su desconexion",
            client_id
        );
        self.disconnect(client_id);
        Err(ServerError::new_kind(
            "Se desconecto al cliente porque envio un segundo Connect",
            ServerErrorKind::ProtocolViolation,
        ))
    }

    #[doc(hidden)]
    fn handle_publish(&self, publish: Publish, client_id: &str) -> Result<(), ServerError> {
        info!("Recibido Publish de <{}>", client_id);
        self.topic_handler.publish(&publish, self).unwrap();
        Ok(())
    }

    #[doc(hidden)]
    fn handle_subscribe(&self, subscribe: Subscribe, client_id: &str) -> Result<(), ServerError> {
        info!("Recibido subscribe de <{}>", client_id);
        self.topic_handler.subscribe(&subscribe, client_id).unwrap();
        Ok(())
    }

    #[doc(hidden)]
    fn connect_new_client(
        &self,
        connect: Connect,
        stream: &mut TcpStream,
    ) -> Result<Client, ServerError> {
        let stream_copy = stream.try_clone()?;
        Ok(Client::new(connect, stream_copy))
    }

    #[doc(hidden)]
    fn wait_for_connect(&self, stream: &mut TcpStream) -> Result<Client, ServerError> {
        match self.receive_packet(stream) {
            Ok(packet) => {
                if let Packet::ConnectType(connect) = packet {
                    info!("Recibido CONNECT de cliente {}", connect.client_id());
                    self.connect_new_client(connect, stream)
                } else {
                    error!("Primer paquete recibido en la conexion no es CONNECT");
                    Err(ServerError::new_kind(
                        "Primer paquete recibido no es CONNECT",
                        ServerErrorKind::ProtocolViolation,
                    ))
                }
            }
            Err(err) => Err(err),
        }
    }

    #[doc(hidden)]
    fn new_client(&self, client: Client) -> Result<(), ServerError> {
        match self
            .clients
            .write()
            .expect("Lock envenenado")
            .insert(client.id().to_owned(), Mutex::new(client))
        {
            None => Ok(()),
            Some(old_client) => {
                error!("Se encontro un cliente con la misma id");
                Err(ServerError::new_kind(
                    "Se encontro un cliente con la misma id",
                    ServerErrorKind::RepeatedId,
                ))
            }
        }
    }

    // Temporal
    #[doc(hidden)]
    fn send_connack(&self, client_id: &str) -> Result<(), ServerError> {
        let response = *self
            .clients
            .read()
            .expect("Lock envenenado")
            .get(client_id)
            .expect("No se encontro el client_id")
            .lock()
            .expect("Lock envenenado")
            .connect()
            .response();
        self.clients
            .read()
            .expect("Lock envenenado")
            .get(client_id)
            .expect("No se encontro el client_id")
            .lock()
            .expect("Lock envenenado")
            .write_all(&response.encode())?;
        Ok(())
    }

    #[doc(hidden)]
    fn is_alive(&self, client_id: &str) -> bool {
        self.clients
            .read()
            .expect("Lock envenenado")
            .get(client_id)
            .expect("No se encontro el id del cliente en la lista de clientes")
            .lock()
            .expect("Lock envenenado")
            .alive()
    }

    #[doc(hidden)]
    fn disconnect(&self, client_id: &str) {
        self.clients
            .read()
            .unwrap()
            .get(client_id)
            .unwrap()
            .lock()
            .unwrap()
            .disconnect();
        self.clients.write().unwrap().remove(client_id).unwrap();
    }

    #[doc(hidden)]
    fn remove_client(&self, client_id: &str) {}

    #[doc(hidden)]
    fn connect_client(
        &self,
        stream: &mut TcpStream,
        addr: SocketAddr,
    ) -> Result<String, ServerError> {
        match self.wait_for_connect(stream) {
            Ok(client) => {
                let client_id = client.id().to_owned();
                self.new_client(client)?;
                info!("Sending Connack packet to {}", addr.to_string());
                self.send_connack(&client_id)?;
                Ok(client_id)
            }
            Err(err) => {
                error!(
                    "Error recibiendo Connect de cliente <{}>: {}",
                    addr,
                    err.to_string()
                );
                Err(ServerError::new_kind(
                    "Error de conexion",
                    ServerErrorKind::ProtocolViolation,
                ))
            }
        }
    }

    #[doc(hidden)]
    fn client_loop(self: Arc<Self>, client_id: String, mut stream: TcpStream) {
        let mut packet_manager = PacketScheduler::new(self.clone(), &client_id);
        while self.is_alive(&client_id) {
            match self.receive_packet(&mut stream) {
                Ok(packet) => {
                    packet_manager.new_packet(packet);
                }
                Err(err) if err.kind() == ServerErrorKind::ClientDisconnected => {
                    info!("Desconectando <{}>", client_id);
                    self.disconnect(&client_id);
                    info!("Conexion finalizada con <{}>", client_id);
                    break;
                }
                Err(err) => {
                    error!("Error grave: {}", err.to_string());
                }
            }
        }

        // Implementar Drop para PacketManager
    }

    #[doc(hidden)]
    fn manage_client(self: Arc<Self>, mut stream: TcpStream, addr: SocketAddr) {
        match self.connect_client(&mut stream, addr) {
            Err(err) => match err.kind() {
                ServerErrorKind::ProtocolViolation => {}
                _ => panic!("Unexpected error"),
            },
            Ok(client_id) => self.client_loop(client_id, stream),
        }
    }

    #[doc(hidden)]
    fn accept_client(self: &Arc<Self>, listener: &TcpListener) -> Result<(), ServerError> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => {
                error!("Could not accept TCP connection: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                info!("TCP connection to {} accepted", addr);
                // En la implementacion original habia un set_nonblocking, para que lo precisamos?
                let sv_copy = self.clone();
                // No funcionan los nombres en el trace
                let handle = thread::Builder::new()
                    .name(addr.to_string())
                    .spawn(move || sv_copy.manage_client(stream, addr))
                    .expect("Error creando el thread");
                self.client_handlers.lock().unwrap().push(handle);
                Ok(())
            }
        }
    }
}
