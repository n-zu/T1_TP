use std::io::Read;
use std::{io::Write, net::TcpStream};

use packets::packet_reader::PacketError;

use crate::client_error::ClientError;
use crate::connack::Connack;
use crate::connect::Connect;
use crate::publish::Publish;
use crate::subscribe::Subscribe;
use threadpool::ThreadPool;

pub struct Client {
    stream: TcpStream,
    thread_pool: ThreadPool,
}

impl Client {
    #![allow(dead_code)]
    pub fn new(address: &str) -> Result<Client, ClientError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
            thread_pool: ThreadPool::new(4),
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), ClientError> {
        let stream_listener = self.stream.try_clone()?;
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

        self.thread_pool.spawn(move || {
            Self::wait_for_packets(stream_listener);
        })?;

        Ok(())
    }

    fn wait_for_packets(mut stream: TcpStream) {
        let mut buf = [0u8; 1];
        while stream.read_exact(&mut buf).is_ok() {
            println!("Llego un paquete: {:?}", buf);
        }
    }

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), PacketError> {
        self.stream.write_all(&subscribe.encode()?)?;
        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), PacketError> {
        self.stream.write_all(&publish.encode()?)?;
        Ok(())
    }
}
