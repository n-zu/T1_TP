use std::io::{ErrorKind, Read};
use std::sync::Mutex;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::PacketError;

use crate::client_packets::{Connack, Connect, Subscribe};

use crate::client_error::ClientError;
use packets::publish::Publish;
use threadpool::ThreadPool;

const READ_TIMEOUT: Duration = Duration::from_millis(5000);

enum SentPacket {
    Subscribe(Subscribe),
    Publish(Publish),
    //Pingreq(Pingreq),
}

pub trait PublishObserver: Clone + Send {
    fn got_publish(&self, publish: Publish);
}

pub struct Client<T: PublishObserver> {
    stream: TcpStream,
    thread_pool: ThreadPool,
    pending_acks: Mutex<Vec<SentPacket>>,
    observer: T,
}

impl<T: 'static + PublishObserver> Client<T> {
    #![allow(dead_code)]
    pub fn new(address: &str, observer: T) -> Result<Client<T>, ClientError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
            thread_pool: ThreadPool::new(4),
            pending_acks: Mutex::new(Vec::new()),
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

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), PacketError> {
        self.stream.write_all(&subscribe.encode()?)?;
        self.pending_acks
            .lock()
            .unwrap()
            .push(SentPacket::Subscribe(subscribe));
        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), PacketError> {
        self.stream.write_all(&publish.encode()?)?;
        self.pending_acks
            .lock()
            .unwrap()
            .push(SentPacket::Publish(publish));
        Ok(())
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
                    }
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
