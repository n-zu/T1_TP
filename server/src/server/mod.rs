#![allow(dead_code, unused_variables)]

use core::panic;
use std::{
    collections::HashMap,
    fmt,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, Receiver, Sender, SyncSender},
        Arc, Mutex, MutexGuard, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
    vec,
};

use tracing::{debug, error, event, info, instrument, span, warn, Level};
use tracing_subscriber::{field::debug, FmtSubscriber};

use packets::packet_reader::{ErrorKind, PacketError};

use crate::{
    config::Config,
    connack::Connack,
    connect::{self, Connect},
    server::{packet_scheduler::PacketScheduler, server_error::ServerErrorKind},
};

mod server_error;
use server_error::ServerError;

mod client;
use client::Client;

mod client_list;

mod topic_handler;
use topic_handler::TopicHandler;

mod packet_scheduler;

use self::client_list::Clients;

const MPSC_BUF_SIZE: usize = 256;
const SLEEP_DUR: Duration = Duration::from_secs(2);

pub enum Packet {
    ConnectType(Connect),
    ConnackType(Connack),
    PublishTypee(Publish),
    SubscribeType(Subscribe),
}

// Temporal
pub struct Publish {}

pub struct Subscribe {}

impl Publish {
    fn encode(self) -> Vec<u8> {
        todo!()
    }
}

pub struct Server {
    clients: Clients,
    config: Config,
    topic_handler: TopicHandler,
    handlers: Mutex<Vec<JoinHandle<()>>>,
}

impl Server {
    fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            clients: Clients::new(),
            config,
            topic_handler: TopicHandler::new(),
            handlers: Mutex::new(vec![]),
        })
    }

    fn read_packet(&self, control_byte: u8, stream: &mut TcpStream) -> Packet {
        let code = control_byte >> 4;
        match code {
            1 => match connect::Connect::new(stream) {
                Ok(packet) => Packet::ConnectType(packet),
                Err(err) => {
                    todo!("{:?} {:?}", err.kind(), err.to_string());
                }
            },
            2 => todo!("Pendiente implementación"),
            3 => todo!("Pendiente implementación"),
            4 => todo!("Pendiente implementación"),
            5 => todo!("Pendiente implementación"),
            6 => todo!("Pendiente implementación"),
            7 => todo!("Pendiente implementación"),
            8 => todo!("Pendiente implementación"),
            9 => todo!("Pendiente implementación"),
            10 => todo!("Pendiente implementación"),
            11 => todo!("Pendiente implementación"),
            12 => todo!("Pendiente implementación"),
            13 => todo!("Pendiente implementación"),
            14 => todo!("Pendiente implementación"),
            _ => todo!("Error"),
        }
    }

    fn receive_packet(&self, stream: &mut TcpStream) -> Result<Packet, ServerError> {
        let mut buf = [0u8; 1];
        match stream.read_exact(&mut buf) {
            Ok(_) => Ok(self.read_packet(buf[0], stream)),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                error!("Cliente se desconecto sin avisar");
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

    fn handle_connect(&self, connect: Connect, client_id: &str) {
        // Como la conexion se maneja antes de entrar al loop de paquetes
        // si se llega a este punto es porque se mando un segundo connect
        // Por lo tanto, se debe desconectar al cliente
        error!(
            "El cliente con id {} envio un segundo connect. Se procede a su desconexion",
            client_id
        );
        self.clients.disconnect(client_id);
    }

    fn handle_publish(&self, publish: Publish, client_id: &str) {
        todo!()
    }

    fn handle_subscribe(&self, subscribe: Subscribe, client_id: &str) {}

    fn handle_packet(&self, packet: Packet, client_id: String) {
        match packet {
            Packet::ConnectType(packet) => self.handle_connect(packet, &client_id),
            Packet::ConnackType(_) => todo!(),
            Packet::PublishTypee(packet) => self.handle_publish(packet, &client_id),
            Packet::SubscribeType(packet) => self.handle_subscribe(packet, &client_id),
        }
    }

    fn connect_new_client(
        &self,
        connect: Connect,
        stream: &mut TcpStream,
    ) -> Result<Client, ServerError> {
        let stream_copy = stream.try_clone()?;
        Ok(Client::new(connect, stream_copy))
    }

    fn wait_for_connect(&self, stream: &mut TcpStream) -> Result<Client, ServerError> {
        match self.receive_packet(stream) {
            Ok(packet) => {
                if let Packet::ConnectType(connect) = packet {
                    info!("Recibido CONNECT de cliente {}", connect.client_id());
                    return self.connect_new_client(connect, stream);
                } else {
                    error!("Primer paquete recibido en la conexion no es CONNECT");
                    return Err(ServerError::new_kind(
                        "Primer paquete recibido no es CONNECT",
                        ServerErrorKind::ProtocolViolation,
                    ));
                }
            }
            Err(err) => Err(err),
        }
    }

    fn manage_client(
        self: Arc<Self>,
        mut stream: TcpStream,
        _addr: SocketAddr,
    ) -> Result<(), ServerError> {
        let client = self.wait_for_connect(&mut stream)?;
        let client_id = client.id().to_owned();
        self.clients.new_client(client)?;
        self.clients.send_connack(&client_id);
        let mut packet_manager = PacketScheduler::new(self.clone(), &client_id);
        while self.clients.is_alive(&client_id).unwrap() {
            match self.receive_packet(&mut stream) {
                Ok(packet) => {
                    packet_manager.new_packet(packet);
                }
                Err(err) if err.kind() == ServerErrorKind::ClientDisconnected => {
                    self.clients.disconnect(&client_id);
                    break;
                }
                Err(err) => {
                    error!("Error grave: {}", err.to_string());
                }
            }
        }
        info!("Desconectando {}", client_id);
        self.clients.remove(&client_id);
        info!("Conexion finalizada con {}", client_id);
        // Implementar Drop para PacketManager
        Ok(())
    }

    fn connect(self: &Arc<Self>, stream: TcpStream, addr: SocketAddr) -> Result<(), ServerError> {
        /*
        if let Err(err) = stream.set_nonblocking(true) {
            error!(
                "No se pudo establecer socket como no bloqueante: {}",
                err.to_string()
            );
            Err(ServerError::new_msg(
                "Error estableciendo conexion no bloqueante",
            ))

        } else {
            */
            let sv_copy = self.clone();
            let handle = thread::spawn(move || match sv_copy.manage_client(stream, addr) {
                Err(err) if err.kind() == ServerErrorKind::RepeatedId => {}
                Err(_) => todo!(),
                Ok(()) => {}
            });
            self.handlers.lock().unwrap().push(handle);
            Ok(())
    }

    fn accept_client(self: &Arc<Self>, listener: &TcpListener) -> Result<(), ServerError> {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => {
                error!("No se pudo aceptar conexion TCP: {}", error.to_string());
                Err(ServerError::from(error))
            }
            Ok((stream, addr)) => {
                info!("Aceptada conexion TCP con {}", addr);
                self.connect(stream, addr)?;
                Ok(())
            }
        }
    }

    pub fn send_publish_to(&self, publish: Publish, client_id: &str) {
        self.clients.send_publish(client_id, publish);
    }

    fn run(self: Arc<Self>) -> Result<(), ServerError> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.config.port())).unwrap();
        loop {
            self.accept_client(&listener)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, io,
        net::{TcpListener, TcpStream},
        sync::{mpsc::sync_channel, Arc},
        thread,
        time::Duration,
    };

    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::{
        config::{self, Config},
        server::{Client, MPSC_BUF_SIZE},
    };

    use super::{Packet, Server};

    #[test]
    fn test() {
        let config = Config::new("config.txt").expect("Error cargando la configuracion");

        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let server = Server::new(config);
        server.run().unwrap()
    }
}
