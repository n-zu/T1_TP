use std::{
    io::{Write},
    net::TcpStream,
};

use packets::packet_reader::PacketError;

use crate::connack::Connack;
use crate::connect::Connect;

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(address: &str) -> Result<Client, PacketError> {
        Ok(Client {
            stream: TcpStream::connect(address)?,
        })
    }

    pub fn connect(&mut self, connect: Connect) -> Result<(), PacketError> {
        self.stream
            .write_all(&connect.encode())?;
            
        match Connack::read_from(&mut self.stream) {
            Ok(connack_packet) => {
                println!("Llego bien el connack packet, {:?}", connack_packet);
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }

        Ok(())
    }
}
