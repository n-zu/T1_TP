use std::io::{ErrorKind, Read};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::{PacketError, QoSLevel};

use crate::client_packets::{Connack, Connect, PingReq, Subscribe};

use crate::client_error::ClientError;
use packets::publish::Publish;
use threadpool::ThreadPool;

// Cuanto esperar recibir un paquete antes de checkear si hay que parar
const READ_TIMEOUT: Duration = Duration::from_millis(200);

// Cuanto esperar antes de checkear que
// haya que reenviar un paquete con QoS 1
const RESEND_TIMEOUT: Duration = Duration::from_millis(500);

// Cuantas veces reintentar reenviar un paquete
const MAX_RETRIES: u16 = 3;

#[allow(dead_code)]
pub enum PendingAck {
    Subscribe(Subscribe),
    PingReq(PingReq),
    Publish(Publish),
}

pub trait PublishObserver: Clone + Send {
    fn got_publish(&self, publish: Publish);
}

pub struct Client<T: PublishObserver> {
    stream: TcpStream,
    thread_pool: ThreadPool,
    pending_ack: Mutex<Option<PendingAck>>,
    observer: T,
}

impl<T: 'static + PublishObserver> Client<T> {
    #![allow(dead_code)]
    pub fn new(address: &str, observer: T) -> Result<Client<T>, ClientError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
            thread_pool: ThreadPool::new(4),
            pending_ack: Mutex::new(None),
            observer,
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), ClientError> {
        let stream_listener = self.stream.try_clone()?;
        stream_listener.set_read_timeout(Some(READ_TIMEOUT))?;

        self.stream.write_all(&connect.encode())?;
        println!("Enviando connect...");

        match Connack::read_from(&mut self.stream) {
            Ok(connack_packet) => {
                println!("Llego bien el Connack: {:?}", connack_packet);
            }
            Err(err) => {
                ClientError::new(&format!(
                    "Error: se recibió paquete de conexión inválido ({:?})",
                    err
                ));
            }
        }
        let observer_clone = self.observer.clone();
        self.thread_pool.spawn(move || {
            wait_for_packets(stream_listener, observer_clone);
        })?;

        Ok(())
    }

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), ClientError> {
        self.stream.write_all(&subscribe.encode()?)?;
        if subscribe.max_qos() != QoSLevel::QoSLevel0 {
            *self.pending_ack.lock()? = Some(PendingAck::Subscribe(subscribe));
            self.wait_for_ack()?;
        }
        // TODO: mandar por canal suscripcióñ exitosa
        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), PacketError> {
        self.stream.write_all(&publish.encode()?)?;
        /*
        self.pending_acks
            .lock()
            .unwrap()
            .push(SentPacket::Publish(publish));
        */
        Ok(())
    }

    fn resend_pending(stream: &mut TcpStream, pending_ack: &PendingAck) -> Result<(), ClientError> {
        // TODO: realmente deberiamos hacer de una vez el trait de encode asi evitamos esto
        match pending_ack {
            PendingAck::Subscribe(ref subscribe) => {
                stream.write_all(&subscribe.encode()?)?;
            }
            PendingAck::PingReq(ref ping_req) => {
                stream.write_all(&ping_req.encode())?;
            }
            PendingAck::Publish(ref publish) => {
                stream.write_all(&publish.encode()?)?;
            }
        }
        Ok(())
    }

    // Devuelve verdadero si se pudo mandar, falso si no se recibió el ack
    fn wait_for_ack(&mut self) -> Result<bool, ClientError> {
        let mut retries = 0;
        while retries < MAX_RETRIES {
            thread::sleep(RESEND_TIMEOUT);
            let pending = self.pending_ack.lock()?;
            match pending.as_ref() {
                None => {
                    return Ok(true);
                }
                Some(pending_ack) => {
                    Self::resend_pending(&mut self.stream, pending_ack)?;
                    retries += 1;
                }
            }
            drop(pending);
        }

        Ok(false)
    }
}

fn handle_publish(stream: &mut TcpStream, header: u8, observer: &impl PublishObserver) {
    let publish = Publish::read_from(stream, header).unwrap();
    observer.got_publish(publish);
}

fn wait_for_packets(mut stream: TcpStream, observer: impl PublishObserver) {
    let mut buf = [0u8; 1];
    println!("Esperando paquetes...");
    loop {
        match stream.read_exact(&mut buf) {
            Ok(_) => {
                println!("Llego un paquete: {:?}", buf);
                let packet_type = buf[0] >> 4;
                match packet_type {
                    3 => {
                        handle_publish(&mut stream, buf[0], &observer);
                    }
                    _ => {
                        println!("Llego un paquete de tipo {}", packet_type);
                    } // TODO: verificar los ACK
                }
            }
            Err(err)
                if (err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock) =>
            {
                continue;
            }
            Err(err) => {
                println!("Error: {}", err);
                break;
            }
        }
    }
}
