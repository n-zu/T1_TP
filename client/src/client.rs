use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::PacketError;

use crate::client_error::ClientError;
use crate::connack::Connack;
use crate::connect::Connect;
use crate::publish::Publish;
use crate::subscribe::Subscribe;
use threadpool::ThreadPool;
use packets::publish::Publish as PublishRecv;

const READ_TIMEOUT : Duration = Duration::from_millis(5000);

enum SentPacket {
    Subscribe(Subscribe),
    Publish(Publish),
    //Pingreq(Pingreq),
}

pub trait PublishObserver {
    fn got_publish(&self, publish: PublishRecv);
}

pub struct Client<T: PublishObserver> {
    stream: TcpStream,
    thread_pool: ThreadPool,
    pending_acks: Mutex<Vec<SentPacket>>,
    observer: T,
}

impl<T: PublishObserver> Client<T> {
    #![allow(dead_code)]
    pub fn new(address: &str, observer : T) -> Result<Client<T>, ClientError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
            thread_pool: ThreadPool::new(4),
            pending_acks: Mutex::new(Vec::new()),
            observer
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

        Ok(())
    }

    pub fn handle_publish(&self, stream: &mut TcpStream, header : u8) {
        let publish = PublishRecv::read_from(stream, header).unwrap();
        self.observer.got_publish(publish);
    }

    pub fn wait_for_packets(mut stream: TcpStream) {
        let mut buf = [0u8; 1];
        println!("Esperando paquetes...");
        while stream.read_exact(&mut buf).is_ok() {
            println!("Llego un paquete: {:?}", buf);
            let packet_type = buf[0] >> 4;
            match packet_type {
                3 => {
                    // let mut publish = Publish::read_from(&mut stream)?;
                    
                    println!("Llego un paquete de tipo 3");
                }
                _ => {
                    println!("Llego un paquete de tipo {}", packet_type);
                }
            }
        }
    }

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), PacketError> {
        self.stream.write_all(&subscribe.encode()?)?;
        self.pending_acks.lock().unwrap().push(SentPacket::Subscribe(subscribe));
        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), PacketError> {
        self.stream.write_all(&publish.encode()?)?;
        self.pending_acks.lock().unwrap().push(SentPacket::Publish(publish));
        Ok(())
    }

    pub fn start_listening(&mut self) {
        println!("Start Listening");
        let stream_listener = self.stream.try_clone().unwrap();
        self.thread_pool
            .spawn(move || {
                Self::wait_for_packets(stream_listener);
            })
            .expect("Error creating listener thread");
    }
}
